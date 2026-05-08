use std::sync::Arc;
use std::time::Duration;

use zbus::{interface, ConnectionBuilder};

use crate::app::{App, RefreshCompletion, RefreshStart};
use crate::error::AppError;
use crate::model::{
    BusyBehavior, RefreshOptions, RefreshReason, RefreshResult, RefreshStatus, SourceAdapterPolicy,
};
use crate::{DBUS_NAME, DBUS_OBJECT_PATH};

const REFRESH_STARTED_SIGNAL_DELAY: Duration = Duration::from_millis(25);
const REFRESH_FINISH_WORK_DELAY: Duration = Duration::from_millis(120);
const SETTINGS_INTERVAL_FALLBACK_SECONDS: u64 = 300;
const SCHEDULER_BACKOFF_MAX_EXPONENT: u32 = 5;
const SCHEDULER_BACKOFF_MAX_DELAY_SECONDS: u64 = 15 * 60;

#[derive(Clone, Debug, Default)]
struct RefreshCycleOutcome {
    refresh_id: Option<String>,
    backoff_failure: bool,
}

pub struct CodexbarInterface {
    app: Arc<App>,
}

impl CodexbarInterface {
    pub fn new(app: Arc<App>) -> Self {
        Self { app }
    }
}

#[interface(name = "org.codexbar.Linux1")]
impl CodexbarInterface {
    async fn get_snapshot(&self) -> Result<String, AppError> {
        self.app.get_snapshot_json()
    }

    async fn refresh(
        &self,
        options_json: &str,
        #[zbus(signal_context)] ctxt: zbus::object_server::SignalContext<'_>,
    ) -> Result<String, AppError> {
        let start = self.app.start_refresh(options_json)?;
        match start {
            RefreshStart::Started { refresh_id } => {
                let app = Arc::clone(&self.app);
                let ctxt = ctxt.to_owned();
                let spawned_refresh_id = refresh_id.clone();
                tokio::spawn(async move {
                    let _ = emit_refresh_lifecycle(app, ctxt, spawned_refresh_id).await;
                });
                Ok(refresh_id)
            }
            RefreshStart::Existing { refresh_id } => Ok(refresh_id),
        }
    }

    async fn get_diagnostics(&self, provider_id: &str) -> Result<String, AppError> {
        self.app.get_diagnostics_json(provider_id)
    }

    async fn get_daemon_info(&self) -> Result<String, AppError> {
        self.app.get_daemon_info_json()
    }

    async fn get_settings(&self) -> Result<String, AppError> {
        self.app.get_settings_json()
    }

    async fn set_settings_patch(
        &self,
        patch_json: &str,
        #[zbus(signal_context)] ctxt: zbus::object_server::SignalContext<'_>,
    ) -> Result<String, AppError> {
        let settings_json = self.app.set_settings_patch_json(patch_json)?;
        let _ = Self::settings_changed(&ctxt, &settings_json).await;
        Ok(settings_json)
    }

    async fn test_browser_import(&self, options_json: &str) -> Result<String, AppError> {
        self.app.test_browser_import_json(options_json)
    }

    #[zbus(signal)]
    pub async fn snapshot_changed(
        ctxt: &zbus::object_server::SignalContext<'_>,
        snapshot_json: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn refresh_started(
        ctxt: &zbus::object_server::SignalContext<'_>,
        refresh_id: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn refresh_finished(
        ctxt: &zbus::object_server::SignalContext<'_>,
        refresh_id: &str,
        result_json: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn provider_changed(
        ctxt: &zbus::object_server::SignalContext<'_>,
        provider_id: &str,
        provider_event_json: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn settings_changed(
        ctxt: &zbus::object_server::SignalContext<'_>,
        settings_json: &str,
    ) -> zbus::Result<()>;
}

pub async fn serve_until_shutdown() -> Result<(), Box<dyn std::error::Error>> {
    let app = Arc::new(App::from_env()?);
    let connection = ConnectionBuilder::session()?
        .name(DBUS_NAME)?
        .serve_at(DBUS_OBJECT_PATH, CodexbarInterface::new(Arc::clone(&app)))?
        .build()
        .await?;
    let signal_context =
        zbus::object_server::SignalContext::new(&connection, DBUS_OBJECT_PATH)?.to_owned();
    let scheduler = tokio::spawn(run_auto_refresh_scheduler(app, signal_context));

    wait_for_shutdown().await;
    scheduler.abort();
    Ok(())
}

async fn run_auto_refresh_scheduler(
    app: Arc<App>,
    ctxt: zbus::object_server::SignalContext<'static>,
) {
    let mut consecutive_backoff_failures = 0u32;
    if app
        .settings_snapshot()
        .map(|settings| settings.refresh.startup_refresh)
        .unwrap_or(false)
    {
        if let Ok(outcome) =
            run_refresh_cycle(Arc::clone(&app), ctxt.clone(), RefreshReason::Startup).await
        {
            consecutive_backoff_failures = if outcome.backoff_failure { 1 } else { 0 };
        }
    }

    let mut observed_revision = app.settings_revision();
    loop {
        let Some(base_interval) = scheduler_interval(&app) else {
            observed_revision = app.wait_for_settings_change(observed_revision).await;
            consecutive_backoff_failures = 0;
            continue;
        };
        let interval = scheduler_backoff_interval(base_interval, consecutive_backoff_failures);
        tokio::select! {
            _ = tokio::time::sleep(interval) => {
                if let Ok(outcome) = run_refresh_cycle(Arc::clone(&app), ctxt.clone(), RefreshReason::Scheduled).await {
                    let _ = outcome.refresh_id.as_deref();
                    if outcome.backoff_failure {
                        consecutive_backoff_failures = consecutive_backoff_failures.saturating_add(1);
                    } else {
                        consecutive_backoff_failures = 0;
                    }
                }
                observed_revision = app.settings_revision();
            }
            revision = app.wait_for_settings_change(observed_revision) => {
                observed_revision = revision;
                consecutive_backoff_failures = 0;
            }
        }
    }
}

async fn run_refresh_cycle(
    app: Arc<App>,
    ctxt: zbus::object_server::SignalContext<'static>,
    reason: RefreshReason,
) -> Result<RefreshCycleOutcome, AppError> {
    let options_json = scheduler_refresh_options_json(reason)?;
    match app.start_refresh(&options_json)? {
        RefreshStart::Started { refresh_id } => {
            let completion = emit_refresh_lifecycle(app, ctxt, refresh_id.clone()).await;
            Ok(RefreshCycleOutcome {
                refresh_id: Some(refresh_id),
                backoff_failure: completion
                    .as_ref()
                    .map(scheduler_should_backoff)
                    .unwrap_or(true),
            })
        }
        RefreshStart::Existing { refresh_id } => Ok(RefreshCycleOutcome {
            refresh_id: Some(refresh_id),
            backoff_failure: false,
        }),
    }
}

async fn emit_refresh_lifecycle(
    app: Arc<App>,
    ctxt: zbus::object_server::SignalContext<'static>,
    refresh_id: String,
) -> Option<RefreshCompletion> {
    tokio::time::sleep(REFRESH_STARTED_SIGNAL_DELAY).await;
    let _ = CodexbarInterface::refresh_started(&ctxt, &refresh_id).await;
    tokio::time::sleep(refresh_finish_work_delay()).await;
    let completion = match app.finish_refresh(&refresh_id).await {
        Ok(completion) => Some(completion),
        Err(_) => app
            .fail_refresh(
                &refresh_id,
                "refresh_failed",
                "Refresh failed; details were redacted.",
            )
            .ok(),
    };
    let completion = completion?;
    for (provider_id, provider_event_json) in &completion.provider_events {
        let _ = CodexbarInterface::provider_changed(&ctxt, provider_id, provider_event_json).await;
    }
    let _ = CodexbarInterface::snapshot_changed(&ctxt, &completion.snapshot_json).await;
    let _ =
        CodexbarInterface::refresh_finished(&ctxt, &completion.refresh_id, &completion.result_json)
            .await;
    Some(completion)
}

fn scheduler_refresh_options_json(reason: RefreshReason) -> Result<String, AppError> {
    serde_json::to_string(&RefreshOptions {
        schema_version: 1,
        reason,
        force: false,
        providers: Vec::new(),
        busy_behavior: BusyBehavior::ReturnExisting,
        source_adapter_policy: SourceAdapterPolicy::default(),
    })
    .map_err(|_| AppError::internal_redacted())
}

fn scheduler_interval(app: &App) -> Option<Duration> {
    let configured_seconds = app
        .settings_snapshot()
        .map(|settings| settings.refresh.interval_seconds)
        .unwrap_or(SETTINGS_INTERVAL_FALLBACK_SECONDS);
    if configured_seconds == 0 {
        return None;
    }

    #[cfg(debug_assertions)]
    if let Ok(value) = std::env::var("CODEXBAR_LINUX_TEST_SCHEDULER_INTERVAL_MS") {
        if let Ok(milliseconds) = value.parse::<u64>() {
            return Some(Duration::from_millis(milliseconds.max(1)));
        }
    }

    Some(Duration::from_secs(configured_seconds))
}

fn scheduler_backoff_interval(base: Duration, consecutive_failures: u32) -> Duration {
    let exponent = consecutive_failures.min(SCHEDULER_BACKOFF_MAX_EXPONENT);
    base.saturating_mul(1u32 << exponent)
        .min(Duration::from_secs(SCHEDULER_BACKOFF_MAX_DELAY_SECONDS))
}

fn scheduler_should_backoff(completion: &RefreshCompletion) -> bool {
    let Ok(result) = serde_json::from_str::<RefreshResult>(&completion.result_json) else {
        return true;
    };
    if result.status == RefreshStatus::Noop {
        return false;
    }
    if result.status == RefreshStatus::Error {
        return true;
    }
    result
        .diagnostic_codes
        .iter()
        .any(|code| scheduler_backoff_code(code))
        || result.providers.iter().any(|provider| {
            provider
                .diagnostic_codes
                .iter()
                .any(|code| scheduler_backoff_code(code))
        })
}

fn scheduler_backoff_code(code: &str) -> bool {
    matches!(
        code,
        "refresh_failed"
            | "upstream_cli_missing"
            | "upstream_cli_not_executable"
            | "upstream_cli_timeout"
            | "upstream_cli_parse_error"
            | "upstream_cli_output_truncated"
            | "upstream_cli_empty_stdout"
            | "upstream_cli_nonzero_exit"
            | "upstream_cli_spawn_failed"
            | "upstream_cli_io_error"
            | "upstream_cli_provider_cli_missing"
    )
}

fn refresh_finish_work_delay() -> Duration {
    #[cfg(debug_assertions)]
    if let Ok(value) = std::env::var("CODEXBAR_LINUX_TEST_REFRESH_FINISH_DELAY_MS") {
        if let Ok(milliseconds) = value.parse::<u64>() {
            return Duration::from_millis(milliseconds.max(1));
        }
    }

    REFRESH_FINISH_WORK_DELAY
}

async fn wait_for_shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigint = signal(SignalKind::interrupt()).expect("SIGINT handler");
        let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = sigint.recv() => {},
            _ = sigterm.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completion_with_result(result_json: &str) -> RefreshCompletion {
        RefreshCompletion {
            refresh_id: "refresh-test".to_string(),
            snapshot_json: "{}".to_string(),
            result_json: result_json.to_string(),
            provider_events: Vec::new(),
        }
    }

    #[test]
    fn scheduler_backoff_is_capped_for_desktop_use() {
        let base = Duration::from_secs(300);
        assert_eq!(
            scheduler_backoff_interval(base, 8),
            Duration::from_secs(SCHEDULER_BACKOFF_MAX_DELAY_SECONDS)
        );
    }

    #[test]
    fn scheduler_backoff_treats_generic_error_status_as_failure() {
        let completion = completion_with_result(
            r#"{"schemaVersion":1,"refreshId":"refresh-test","status":"error","startedAt":"2026-05-08T12:00:00Z","finishedAt":"2026-05-08T12:00:01Z","durationMs":1000,"reason":"scheduled","providers":[],"cacheWritten":false,"snapshotGeneratedAt":null,"diagnosticCodes":[]}"#,
        );
        assert!(scheduler_should_backoff(&completion));
    }

    #[test]
    fn scheduler_backoff_treats_refresh_failed_code_as_failure() {
        let completion = completion_with_result(
            r#"{"schemaVersion":1,"refreshId":"refresh-test","status":"partial","startedAt":"2026-05-08T12:00:00Z","finishedAt":"2026-05-08T12:00:01Z","durationMs":1000,"reason":"scheduled","providers":[],"cacheWritten":false,"snapshotGeneratedAt":null,"diagnosticCodes":["refresh_failed"]}"#,
        );
        assert!(scheduler_should_backoff(&completion));
    }

    #[test]
    fn scheduler_backoff_does_not_treat_noop_as_failure() {
        let completion = completion_with_result(
            r#"{"schemaVersion":1,"refreshId":"refresh-test","status":"noop","startedAt":"2026-05-08T12:00:00Z","finishedAt":"2026-05-08T12:00:01Z","durationMs":1000,"reason":"scheduled","providers":[],"cacheWritten":false,"snapshotGeneratedAt":null,"diagnosticCodes":["refresh_failed"]}"#,
        );
        assert!(!scheduler_should_backoff(&completion));
    }
}
