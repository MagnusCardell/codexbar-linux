use std::sync::Arc;
use std::time::Duration;

use zbus::{interface, ConnectionBuilder};

use crate::app::{App, RefreshStart};
use crate::error::AppError;
use crate::model::{BusyBehavior, RefreshOptions, RefreshReason, SourceAdapterPolicy};
use crate::{DBUS_NAME, DBUS_OBJECT_PATH};

const REFRESH_STARTED_SIGNAL_DELAY: Duration = Duration::from_millis(25);
const REFRESH_FINISH_WORK_DELAY: Duration = Duration::from_millis(120);
const SETTINGS_INTERVAL_FALLBACK: Duration = Duration::from_secs(120);

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
                tokio::spawn(emit_refresh_lifecycle(app, ctxt, spawned_refresh_id));
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

    async fn set_settings_patch(&self, patch_json: &str) -> Result<String, AppError> {
        self.app.set_settings_patch_json(patch_json)
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
    if app
        .settings_snapshot()
        .map(|settings| settings.refresh.startup_refresh)
        .unwrap_or(false)
    {
        let _ = run_refresh_cycle(Arc::clone(&app), ctxt.clone(), RefreshReason::Startup).await;
    }

    let mut observed_revision = app.settings_revision();
    loop {
        let interval = scheduler_interval(&app);
        tokio::select! {
            _ = tokio::time::sleep(interval) => {
                let _ = run_refresh_cycle(Arc::clone(&app), ctxt.clone(), RefreshReason::Scheduled).await;
                observed_revision = app.settings_revision();
            }
            revision = app.wait_for_settings_change(observed_revision) => {
                observed_revision = revision;
            }
        }
    }
}

async fn run_refresh_cycle(
    app: Arc<App>,
    ctxt: zbus::object_server::SignalContext<'static>,
    reason: RefreshReason,
) -> Result<Option<String>, AppError> {
    let options_json = scheduler_refresh_options_json(reason)?;
    match app.start_refresh(&options_json)? {
        RefreshStart::Started { refresh_id } => {
            emit_refresh_lifecycle(app, ctxt, refresh_id.clone()).await;
            Ok(Some(refresh_id))
        }
        RefreshStart::Existing { refresh_id } => Ok(Some(refresh_id)),
    }
}

async fn emit_refresh_lifecycle(
    app: Arc<App>,
    ctxt: zbus::object_server::SignalContext<'static>,
    refresh_id: String,
) {
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
    let Some(completion) = completion else {
        return;
    };
    for (provider_id, provider_event_json) in completion.provider_events {
        let _ =
            CodexbarInterface::provider_changed(&ctxt, &provider_id, &provider_event_json).await;
    }
    let _ = CodexbarInterface::snapshot_changed(&ctxt, &completion.snapshot_json).await;
    let _ =
        CodexbarInterface::refresh_finished(&ctxt, &completion.refresh_id, &completion.result_json)
            .await;
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

fn scheduler_interval(app: &App) -> Duration {
    let configured = app
        .settings_snapshot()
        .map(|settings| Duration::from_secs(settings.refresh.interval_seconds))
        .unwrap_or(SETTINGS_INTERVAL_FALLBACK);

    #[cfg(debug_assertions)]
    if let Ok(value) = std::env::var("CODEXBAR_LINUX_TEST_SCHEDULER_INTERVAL_MS") {
        if let Ok(milliseconds) = value.parse::<u64>() {
            return Duration::from_millis(milliseconds.max(1));
        }
    }

    configured
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
