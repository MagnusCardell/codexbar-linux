use std::collections::BTreeSet;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use tokio::sync::Notify;

use crate::cache::{stale_mutated, CacheLoad, SnapshotCache};
use crate::cli::{self, CliRefreshRequest, UpstreamCliAdapter};
use crate::clock::{duration_ms, now_rfc3339, Clock};
use crate::config::{
    apply_settings_patch, is_safe_id, parse_settings_patch, SettingsLoad, SettingsStore,
};
use crate::error::{AppError, AppResult};
use crate::fixtures;
use crate::model::{
    BrowserImportOptions, BrowserImportResult, BrowserImportStatus, BrowserProviderResult,
    BrowserProviderStatus, BrowserSourceAdapter, BusyBehavior, Capabilities, DaemonInfo,
    DaemonPathsInfo, DaemonState, DbusInfo, DiagnosticEvent, DiagnosticScope, DiagnosticSeverity,
    EventRedaction, ProviderEvent, ProviderEventReason, RedactionSummary, RefreshOptions,
    RefreshReason, RefreshResult, RefreshSourceAdapter, RefreshStatus, Settings, Snapshot,
    SourceAdapter,
};
use crate::paths::AppPaths;
use crate::redact;
use crate::{DBUS_INTERFACE, DBUS_NAME, DBUS_OBJECT_PATH};

const STALE_CACHE_USED_MESSAGE: &str = "Showing cached usage data.";
const NO_ENABLED_PROVIDERS_CODE: &str = "refresh_no_enabled_providers";
const NO_ENABLED_PROVIDERS_MESSAGE: &str =
    "No providers are enabled for automatic refresh; refresh was skipped.";

pub struct App {
    paths: AppPaths,
    runtime: AppRuntime,
    clock: Clock,
    cache: SnapshotCache,
    settings_store: SettingsStore,
    refresh_counter: AtomicU64,
    settings_revision: AtomicU64,
    settings_notify: Notify,
    state: Mutex<AppState>,
}

impl fmt::Debug for App {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("App")
            .field("paths", &"[redacted]")
            .field("runtime", &self.runtime)
            .field("clock", &self.clock)
            .field("cache", &"[redacted]")
            .field("settings_store", &"[redacted]")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppRuntime {
    allow_fixture_source: bool,
}

impl AppRuntime {
    pub fn production() -> Self {
        Self {
            allow_fixture_source: false,
        }
    }

    pub fn from_env() -> Self {
        Self {
            allow_fixture_source: env_allows_fixture_source(),
        }
    }

    pub fn with_fixture_source_for_tests() -> Self {
        Self {
            allow_fixture_source: true,
        }
    }

    pub fn fixture_source_allowed(&self) -> bool {
        self.allow_fixture_source
    }
}

#[derive(Clone, Debug)]
struct AppState {
    snapshot: Snapshot,
    settings: Settings,
    active_refresh: Option<ActiveRefresh>,
    diagnostics: Vec<DiagnosticEvent>,
}

#[derive(Clone, Debug)]
struct ActiveRefresh {
    id: String,
    started_at: String,
    started_instant: Instant,
    reason: RefreshReason,
    options: RefreshOptions,
}

#[derive(Clone, Debug)]
pub enum RefreshStart {
    Started { refresh_id: String },
    Existing { refresh_id: String },
}

#[derive(Clone, Debug)]
pub struct RefreshCompletion {
    pub refresh_id: String,
    pub snapshot_json: String,
    pub result_json: String,
    pub provider_events: Vec<(String, String)>,
}

impl App {
    pub fn from_env() -> AppResult<Self> {
        Self::new_with_runtime(AppPaths::from_env(), AppRuntime::from_env())
    }

    pub fn new(paths: AppPaths) -> AppResult<Self> {
        Self::new_with_runtime(paths, AppRuntime::production())
    }

    pub fn new_with_runtime(paths: AppPaths, runtime: AppRuntime) -> AppResult<Self> {
        let clock = Clock::started_now();
        let cache = SnapshotCache::new(paths.cache_dir.clone(), paths.cache_file.clone());
        let settings_store =
            SettingsStore::new(paths.config_dir.clone(), paths.config_file.clone());
        let mut diagnostics = Vec::new();
        let mut snapshot = match cache.load() {
            CacheLoad::Loaded(snapshot) => {
                diagnostics.push(diagnostic_event(
                    "cache_loaded",
                    "Loaded normalized snapshot cache and marked it stale",
                    clock.started_at(),
                    None,
                ));
                stale_mutated(*snapshot, clock.started_at())
            }
            CacheLoad::Missing => {
                diagnostics.push(diagnostic_event(
                    "cache_missing",
                    "Snapshot cache missing; loaded synthetic loading snapshot",
                    clock.started_at(),
                    None,
                ));
                fixtures::synthetic_loading(clock.started_at())?
            }
            CacheLoad::Invalid => {
                diagnostics.push(diagnostic_event(
                    "cache_invalid",
                    "Snapshot cache was invalid and ignored",
                    clock.started_at(),
                    None,
                ));
                fixtures::synthetic_loading(clock.started_at())?
            }
        };
        snapshot.daemon.version = env!("CARGO_PKG_VERSION").to_string();
        snapshot.daemon.upstream_cli = Some(cli::resolve_info(&paths));
        let settings = match settings_store.load() {
            SettingsLoad::Loaded(settings) => {
                diagnostics.push(diagnostic_event(
                    "settings_loaded",
                    "Daemon settings loaded",
                    clock.started_at(),
                    None,
                ));
                settings
            }
            SettingsLoad::Defaulted(settings) => {
                diagnostics.push(diagnostic_event(
                    "settings_loaded",
                    "Default daemon settings loaded",
                    clock.started_at(),
                    None,
                ));
                settings
            }
            SettingsLoad::Invalid(settings) => {
                diagnostics.push(diagnostic_event(
                    "invalid_settings_patch",
                    "Daemon settings were invalid and defaults were used",
                    clock.started_at(),
                    None,
                ));
                settings
            }
        };

        Ok(Self {
            paths,
            runtime,
            clock,
            cache,
            settings_store,
            refresh_counter: AtomicU64::new(1),
            settings_revision: AtomicU64::new(1),
            settings_notify: Notify::new(),
            state: Mutex::new(AppState {
                snapshot,
                settings,
                active_refresh: None,
                diagnostics,
            }),
        })
    }

    pub fn check_startup() -> AppResult<()> {
        validate_version_metadata()?;
        let app = Self::from_env()?;
        let _ = app.get_daemon_info_json()?;
        Ok(())
    }

    pub fn get_snapshot_json(&self) -> AppResult<String> {
        let state = self.lock_state()?;
        to_public_json(&state.snapshot)
    }

    pub fn get_daemon_info_json(&self) -> AppResult<String> {
        let state = self.lock_state()?;
        to_public_json(&self.daemon_info(&state))
    }

    pub fn get_settings_json(&self) -> AppResult<String> {
        let state = self.lock_state()?;
        to_public_json(&state.settings)
    }

    pub fn get_diagnostics_json(&self, provider_id: &str) -> AppResult<String> {
        let state = self.lock_state()?;
        let provider = if provider_id.is_empty() || provider_id == "global" {
            None
        } else {
            if !is_safe_id(provider_id) {
                return Err(AppError::invalid_json());
            }
            Some(provider_id.to_string())
        };
        let events = state
            .diagnostics
            .iter()
            .filter(|event| match &provider {
                Some(provider) => event.provider.as_deref() == Some(provider.as_str()),
                None => event.provider.is_none(),
            })
            .cloned()
            .collect();
        let diagnostics = crate::model::Diagnostics {
            schema_version: 1,
            generated_at: now_rfc3339(),
            scope: if provider.is_some() {
                DiagnosticScope::Provider
            } else {
                DiagnosticScope::Global
            },
            provider,
            events,
            redaction: redaction_summary(),
        };
        to_public_json(&diagnostics)
    }

    pub fn set_settings_patch_json(&self, patch_json: &str) -> AppResult<String> {
        let patch = parse_settings_patch(patch_json)?;
        let settings_json = {
            let mut state = self.lock_state()?;
            let next = apply_settings_patch(state.settings.clone(), patch)?;
            self.settings_store.store(&next)?;
            state.settings = next;
            let now = now_rfc3339();
            state.diagnostics.push(diagnostic_event(
                "settings_written",
                "Daemon settings patch applied",
                &now,
                None,
            ));
            to_public_json(&state.settings)?
        };
        self.settings_revision.fetch_add(1, Ordering::AcqRel);
        self.settings_notify.notify_waiters();
        Ok(settings_json)
    }

    pub fn settings_snapshot(&self) -> AppResult<Settings> {
        let state = self.lock_state()?;
        Ok(state.settings.clone())
    }

    pub fn settings_revision(&self) -> u64 {
        self.settings_revision.load(Ordering::Acquire)
    }

    pub async fn wait_for_settings_change(&self, observed_revision: u64) -> u64 {
        loop {
            let notified = self.settings_notify.notified();
            let current = self.settings_revision();
            if current != observed_revision {
                return current;
            }
            notified.await;
        }
    }

    pub fn test_browser_import_json(&self, options_json: &str) -> AppResult<String> {
        let options: BrowserImportOptions = parse_input_json(options_json)?;
        if options.schema_version != 1
            || has_duplicates(&options.providers)
            || has_duplicates(&options.profile_ids)
        {
            return Err(AppError::invalid_json());
        }
        for provider in &options.providers {
            if !is_safe_id(provider) {
                return Err(AppError::invalid_json());
            }
        }
        for profile_id in &options.profile_ids {
            if !is_safe_id(profile_id) {
                return Err(AppError::invalid_json());
            }
        }
        let result = BrowserImportResult {
            schema_version: 1,
            tested_at: now_rfc3339(),
            status: BrowserImportStatus::NotImplemented,
            policy: options.policy,
            profiles: Vec::new(),
            providers: options
                .providers
                .into_iter()
                .map(|provider| BrowserProviderResult {
                    provider,
                    status: BrowserProviderStatus::NotImplemented,
                    source_adapter: BrowserSourceAdapter::None,
                    diagnostic_codes: vec!["browser_import_not_implemented".to_string()],
                })
                .collect(),
            diagnostic_codes: vec!["browser_import_not_implemented".to_string()],
        };
        to_public_json(&result)
    }

    pub fn start_refresh(&self, options_json: &str) -> AppResult<RefreshStart> {
        let options: RefreshOptions = parse_input_json(options_json)?;
        validate_refresh_options(&options)?;
        let mut state = self.lock_state()?;
        if let Some(active) = &state.active_refresh {
            return match options.busy_behavior {
                BusyBehavior::ReturnExisting => Ok(RefreshStart::Existing {
                    refresh_id: active.id.clone(),
                }),
                BusyBehavior::Reject => Err(AppError::refresh_busy(&active.id)),
            };
        }

        let started_at = now_rfc3339();
        let refresh_id = self.next_refresh_id();
        state.active_refresh = Some(ActiveRefresh {
            id: refresh_id.clone(),
            started_at: started_at.clone(),
            started_instant: Instant::now(),
            reason: options.reason,
            options,
        });
        state.snapshot.daemon.state = DaemonState::Refreshing;
        state.snapshot.daemon.last_refresh_id = Some(refresh_id.clone());
        state.snapshot.daemon.last_refresh_started_at = Some(started_at);
        state.snapshot.daemon.last_refresh_finished_at = None;
        Ok(RefreshStart::Started { refresh_id })
    }

    pub async fn finish_refresh(&self, refresh_id: &str) -> AppResult<RefreshCompletion> {
        let (active, settings, previous_snapshot) = {
            let state = self.lock_state()?;
            let active = state
                .active_refresh
                .clone()
                .ok_or_else(AppError::internal_redacted)?;
            if active.id != refresh_id {
                return Err(AppError::internal_redacted());
            }
            (active, state.settings.clone(), state.snapshot.clone())
        };

        let finished_at = now_rfc3339();
        let provider_targets = cli::target_providers(&settings, &active.options.providers);
        let fixture_requested = fixture_requested(&active.options);
        let fixture_only = active.options.source_adapter_policy.allows_fixture()
            && !active.options.source_adapter_policy.allows_upstream_cli();

        let (mut snapshot, mut diagnostics, mut diagnostic_codes, allow_stale_fallback) =
            if fixture_requested && !self.runtime.fixture_source_allowed() {
                (
                    fixture_not_allowed_snapshot(refresh_id, &active.started_at, &finished_at)?,
                    vec![fixture_not_allowed_diagnostic(&finished_at)],
                    vec![
                        "capability_unimplemented".to_string(),
                        "fixture_not_allowed".to_string(),
                    ],
                    true,
                )
            } else if fixture_only {
                (
                    fixtures::refreshed_snapshot(refresh_id, &active.started_at, &finished_at)?,
                    Vec::new(),
                    Vec::new(),
                    true,
                )
            } else if provider_targets.is_empty() {
                (
                    no_enabled_providers_snapshot(
                        refresh_id,
                        &active.started_at,
                        &finished_at,
                        cli::resolve_info(&self.paths),
                    ),
                    vec![no_enabled_providers_diagnostic(&finished_at)],
                    vec![NO_ENABLED_PROVIDERS_CODE.to_string()],
                    false,
                )
            } else if active.options.source_adapter_policy.allows_upstream_cli() {
                let adapter = UpstreamCliAdapter::from_paths(&self.paths);
                let refresh = adapter
                    .refresh(CliRefreshRequest {
                        refresh_id: refresh_id.to_string(),
                        started_at: active.started_at.clone(),
                        finished_at: finished_at.clone(),
                        providers: provider_targets,
                        selected_provider: previous_snapshot.selected_provider.clone(),
                    })
                    .await?;
                let diagnostic_codes = result_diagnostic_codes(&refresh.diagnostics);
                (
                    refresh.snapshot,
                    refresh.diagnostics,
                    diagnostic_codes,
                    true,
                )
            } else {
                (
                    fixtures::unsupported_adapter_snapshot(&finished_at)?,
                    Vec::new(),
                    vec!["upstream_cli_excluded".to_string()],
                    true,
                )
            };

        let live_had_success = snapshot
            .providers
            .iter()
            .any(|provider| provider.state == crate::model::ProviderState::Ok);
        if allow_stale_fallback
            && !live_had_success
            && active
                .options
                .source_adapter_policy
                .allow_stale_cache_fallback
            && settings.refresh.allow_stale_cache_fallback
            && previous_snapshot
                .providers
                .iter()
                .any(|provider| provider.state.is_usable_for_stale_cache())
        {
            let current_daemon = snapshot.daemon.clone();
            snapshot = stale_mutated(previous_snapshot, &finished_at);
            snapshot.daemon.last_refresh_id = Some(refresh_id.to_string());
            snapshot.daemon.last_refresh_started_at = Some(active.started_at.clone());
            snapshot.daemon.last_refresh_finished_at = Some(finished_at.clone());
            snapshot.daemon.upstream_cli = current_daemon.upstream_cli;
            diagnostic_codes.push("stale_cache_used".to_string());
            diagnostics.push(diagnostic_event(
                "stale_cache_used",
                STALE_CACHE_USED_MESSAGE,
                &finished_at,
                None,
            ));
        }

        let cache_written = live_had_success && self.cache.store(&snapshot).is_ok();
        if live_had_success && !cache_written {
            diagnostic_codes.push("cache_write_failed".to_string());
        }

        let status = refresh_status(&snapshot, cache_written, &diagnostic_codes);
        let result = refresh_result(
            &snapshot,
            &active,
            &finished_at,
            cache_written,
            status,
            diagnostic_codes,
        );
        let provider_events = provider_events(&snapshot, refresh_id, &finished_at)?;

        let mut state = self.lock_state()?;
        match &state.active_refresh {
            Some(current) if current.id == refresh_id => {}
            _ => return Err(AppError::internal_redacted()),
        }
        state.snapshot = snapshot;
        state.active_refresh = None;
        state.diagnostics.append(&mut diagnostics);
        state.diagnostics.push(diagnostic_event(
            "refresh_finished",
            "Daemon refresh finished",
            &finished_at,
            None,
        ));
        let snapshot_json = to_public_json(&state.snapshot)?;
        let result_json = to_public_json(&result)?;
        Ok(RefreshCompletion {
            refresh_id: refresh_id.to_string(),
            snapshot_json,
            result_json,
            provider_events,
        })
    }

    pub fn fail_refresh(
        &self,
        refresh_id: &str,
        diagnostic_code: &str,
        safe_message: &str,
    ) -> AppResult<RefreshCompletion> {
        let finished_at = now_rfc3339();
        let mut state = self.lock_state()?;
        let active = state
            .active_refresh
            .clone()
            .ok_or_else(AppError::internal_redacted)?;
        if active.id != refresh_id {
            return Err(AppError::internal_redacted());
        }

        let has_usable_snapshot = state
            .snapshot
            .providers
            .iter()
            .any(|provider| provider.state.is_usable_for_stale_cache());
        let mut snapshot = if has_usable_snapshot {
            stale_mutated(state.snapshot.clone(), &finished_at)
        } else {
            let mut snapshot = state.snapshot.clone();
            snapshot.daemon.state = DaemonState::Error;
            for provider in &mut snapshot.providers {
                if provider.state == crate::model::ProviderState::Loading {
                    provider.state = crate::model::ProviderState::Error;
                    provider.source_adapter = SourceAdapter::None;
                    provider.diagnostics_summary = Some(safe_message.to_string());
                    if !provider
                        .diagnostic_codes
                        .iter()
                        .any(|code| code == diagnostic_code)
                    {
                        provider.diagnostic_codes.push(diagnostic_code.to_string());
                    }
                    if let Some(status) = provider.status.as_mut() {
                        status.indicator = Some("error".to_string());
                        status.description = Some(safe_message.to_string());
                        status.updated_at = Some(finished_at.clone());
                    }
                }
            }
            snapshot
        };
        snapshot.daemon.last_refresh_id = Some(refresh_id.to_string());
        snapshot.daemon.last_refresh_started_at = Some(active.started_at.clone());
        snapshot.daemon.last_refresh_finished_at = Some(finished_at.clone());

        state.snapshot = snapshot;
        state.active_refresh = None;
        state.diagnostics.push(DiagnosticEvent {
            code: diagnostic_code.to_string(),
            severity: DiagnosticSeverity::Error,
            safe_message: safe_message.to_string(),
            timestamp: finished_at.clone(),
            provider: None,
            source_adapter: None,
            recoverable: true,
            details: Default::default(),
            redacted: EventRedaction {
                applied: true,
                classes: vec!["secrets".to_string(), "identity".to_string()],
            },
        });
        let result = refresh_result(
            &state.snapshot,
            &active,
            &finished_at,
            false,
            RefreshStatus::Error,
            vec![diagnostic_code.to_string()],
        );
        let provider_events = provider_events(&state.snapshot, refresh_id, &finished_at)?;
        let snapshot_json = to_public_json(&state.snapshot)?;
        let result_json = to_public_json(&result)?;
        Ok(RefreshCompletion {
            refresh_id: refresh_id.to_string(),
            snapshot_json,
            result_json,
            provider_events,
        })
    }

    pub fn cache_file_path(&self) -> &std::path::Path {
        self.cache.file_path()
    }

    pub fn settings_file_path(&self) -> &std::path::Path {
        self.settings_store.file_path()
    }

    fn lock_state(&self) -> AppResult<std::sync::MutexGuard<'_, AppState>> {
        self.state.lock().map_err(|_| AppError::internal_redacted())
    }

    fn next_refresh_id(&self) -> String {
        let counter = self.refresh_counter.fetch_add(1, Ordering::Relaxed);
        format!(
            "refresh-{}-{counter}",
            time::OffsetDateTime::now_utc().unix_timestamp()
        )
    }

    fn daemon_info(&self, state: &AppState) -> DaemonInfo {
        let resolved_upstream_cli = cli::resolve_info(&self.paths);
        let mut upstream_cli = resolved_upstream_cli;
        if upstream_cli.available && upstream_cli.version.is_none() {
            upstream_cli.version = state
                .snapshot
                .daemon
                .upstream_cli
                .as_ref()
                .and_then(|cli| cli.version.clone());
        }
        if upstream_cli.available && upstream_cli.provider_inventory.is_empty() {
            upstream_cli.provider_inventory = state
                .snapshot
                .daemon
                .upstream_cli
                .as_ref()
                .map(|cli| cli.provider_inventory.clone())
                .unwrap_or_default();
        }
        let upstream_cli_available = upstream_cli.available;
        DaemonInfo {
            schema_version: 1,
            version: env!("CARGO_PKG_VERSION").to_string(),
            pid: std::process::id(),
            started_at: self.clock.started_at().to_string(),
            uptime_seconds: self.clock.uptime_seconds(),
            state: state.snapshot.daemon.state,
            dbus: DbusInfo {
                bus_name: DBUS_NAME.to_string(),
                object_path: DBUS_OBJECT_PATH.to_string(),
                interface: DBUS_INTERFACE.to_string(),
            },
            capabilities: Capabilities {
                upstream_cli: upstream_cli_available,
                browser_import: false,
                linux_web_adapters: false,
                cost: upstream_cli_available,
                settings_patch: true,
            },
            paths: DaemonPathsInfo {
                config_file: Some(cli::resolver::display_safe_owned_file(
                    &self.paths.config_file,
                )),
                cache_file: Some(cli::resolver::display_safe_owned_file(
                    &self.paths.cache_file,
                )),
                upstream_config_file: self
                    .paths
                    .upstream_config_file_hint
                    .as_ref()
                    .map(|_| "[redacted-config]/config.json".to_string()),
            },
            upstream_cli,
            build: Some(crate::model::BuildInfo {
                git_sha: option_env!("CODEXBAR_BUILD_GIT_SHA").map(str::to_string),
                target: option_env!("CODEXBAR_BUILD_TARGET").map(str::to_string),
                profile: option_env!("CODEXBAR_BUILD_PROFILE").map(str::to_string),
            }),
        }
    }
}

fn validate_version_metadata() -> AppResult<()> {
    let version = env!("CARGO_PKG_VERSION");
    if version.is_empty() || version == "0.0.0" {
        return Err(AppError::internal_redacted());
    }
    Ok(())
}

fn result_diagnostic_codes(events: &[DiagnosticEvent]) -> Vec<String> {
    events
        .iter()
        .filter(|event| event.severity != DiagnosticSeverity::Info)
        .map(|event| event.code.clone())
        .collect()
}

fn refresh_result(
    snapshot: &Snapshot,
    active: &ActiveRefresh,
    finished_at: &str,
    cache_written: bool,
    status: RefreshStatus,
    diagnostic_codes: Vec<String>,
) -> RefreshResult {
    RefreshResult {
        schema_version: 1,
        refresh_id: active.id.clone(),
        status,
        started_at: active.started_at.clone(),
        finished_at: finished_at.to_string(),
        duration_ms: duration_ms(active.started_instant.elapsed()),
        reason: active.reason,
        providers: fixtures::provider_results(snapshot),
        cache_written,
        snapshot_generated_at: Some(snapshot.generated_at.clone()),
        diagnostic_codes,
    }
}

fn refresh_status(
    snapshot: &Snapshot,
    cache_written: bool,
    diagnostic_codes: &[String],
) -> RefreshStatus {
    if diagnostic_codes
        .iter()
        .any(|code| code == "stale_cache_used")
        && !diagnostic_codes
            .iter()
            .any(|code| code == "fixture_not_allowed")
    {
        return RefreshStatus::Partial;
    }
    if diagnostic_codes
        .iter()
        .any(|code| code == NO_ENABLED_PROVIDERS_CODE)
    {
        return RefreshStatus::Noop;
    }
    let ok_count = snapshot
        .providers
        .iter()
        .filter(|provider| provider.state == crate::model::ProviderState::Ok)
        .count();
    if ok_count == snapshot.providers.len() && diagnostic_codes.is_empty() && cache_written {
        RefreshStatus::Ok
    } else if ok_count > 0 {
        RefreshStatus::Partial
    } else {
        RefreshStatus::Error
    }
}

fn provider_events(
    snapshot: &Snapshot,
    refresh_id: &str,
    emitted_at: &str,
) -> AppResult<Vec<(String, String)>> {
    let mut events = Vec::with_capacity(snapshot.providers.len());
    for provider in &snapshot.providers {
        let event = ProviderEvent {
            schema_version: 1,
            event_id: format!("provider-event-{refresh_id}-{}", provider.provider),
            emitted_at: emitted_at.to_string(),
            reason: ProviderEventReason::RefreshFinished,
            provider_id: provider.provider.clone(),
            provider: provider.clone(),
            diagnostic_codes: provider.diagnostic_codes.clone(),
        };
        events.push((provider.provider.clone(), to_public_json(&event)?));
    }
    Ok(events)
}

fn no_enabled_providers_snapshot(
    refresh_id: &str,
    started_at: &str,
    finished_at: &str,
    upstream_cli: crate::model::UpstreamCliInfo,
) -> Snapshot {
    Snapshot {
        schema_version: 1,
        generated_at: finished_at.to_string(),
        stale: false,
        selected_provider: None,
        daemon: crate::model::SnapshotDaemon {
            version: env!("CARGO_PKG_VERSION").to_string(),
            state: DaemonState::Ok,
            last_refresh_id: Some(refresh_id.to_string()),
            last_refresh_started_at: Some(started_at.to_string()),
            last_refresh_finished_at: Some(finished_at.to_string()),
            upstream_cli: Some(upstream_cli),
        },
        providers: Vec::new(),
    }
}

fn fixture_requested(options: &RefreshOptions) -> bool {
    let policy = &options.source_adapter_policy;
    let fixture_allowed = policy.allows_fixture();
    let fixture_explicitly_allowed =
        fixture_allowed && policy.adapters.contains(&RefreshSourceAdapter::Fixture);
    let fixture_is_only_usable_adapter = fixture_allowed && !policy.allows_upstream_cli();

    fixture_explicitly_allowed || fixture_is_only_usable_adapter
}

fn fixture_not_allowed_snapshot(
    refresh_id: &str,
    started_at: &str,
    finished_at: &str,
) -> AppResult<Snapshot> {
    let mut snapshot = fixtures::unsupported_adapter_snapshot(finished_at)?;
    snapshot.daemon.last_refresh_id = Some(refresh_id.to_string());
    snapshot.daemon.last_refresh_started_at = Some(started_at.to_string());
    snapshot.daemon.last_refresh_finished_at = Some(finished_at.to_string());
    for provider in &mut snapshot.providers {
        provider.source_adapter = SourceAdapter::None;
        provider.diagnostics_summary =
            Some("Fixture refresh source is disabled outside development mode".to_string());
        provider.diagnostic_codes = vec![
            "capability_unimplemented".to_string(),
            "fixture_not_allowed".to_string(),
        ];
        if let Some(status) = provider.status.as_mut() {
            status.indicator = Some("missing_dependency".to_string());
            status.description =
                Some("Fixture refresh source is disabled outside development mode".to_string());
            status.updated_at = Some(finished_at.to_string());
        }
    }
    Ok(snapshot)
}

fn validate_refresh_options(options: &RefreshOptions) -> AppResult<()> {
    if options.schema_version != 1
        || has_duplicates(&options.providers)
        || has_duplicates(&options.source_adapter_policy.adapters)
    {
        return Err(AppError::invalid_json());
    }
    for provider in &options.providers {
        if !is_safe_id(provider) {
            return Err(AppError::invalid_json());
        }
    }
    Ok(())
}

fn env_allows_fixture_source() -> bool {
    matches!(
        std::env::var("CODEXBAR_LINUX_ALLOW_FIXTURE")
            .ok()
            .as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

fn parse_input_json<T>(json: &str) -> AppResult<T>
where
    T: serde::de::DeserializeOwned,
{
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| AppError::invalid_json())?;
    if redact::contains_null(&value) {
        return Err(AppError::invalid_json());
    }
    serde_json::from_value(value).map_err(|_| AppError::invalid_json())
}

fn to_public_json<T>(value: &T) -> AppResult<String>
where
    T: serde::Serialize,
{
    let text = serde_json::to_string(value).map_err(|_| AppError::internal_redacted())?;
    redact::validate_public_json_text(&text).map_err(|_| AppError::internal_redacted())?;
    Ok(text)
}

fn has_duplicates<T>(values: &[T]) -> bool
where
    T: serde::Serialize,
{
    let mut seen = BTreeSet::new();
    values
        .iter()
        .map(|value| serde_json::to_string(value).unwrap_or_default())
        .any(|key| !seen.insert(key))
}

fn diagnostic_event(
    code: &str,
    safe_message: &str,
    timestamp: &str,
    provider: Option<String>,
) -> DiagnosticEvent {
    DiagnosticEvent {
        code: code.to_string(),
        severity: DiagnosticSeverity::Info,
        safe_message: safe_message.to_string(),
        timestamp: timestamp.to_string(),
        provider,
        source_adapter: None,
        recoverable: true,
        details: Default::default(),
        redacted: EventRedaction {
            applied: true,
            classes: vec!["secrets".to_string(), "identity".to_string()],
        },
    }
}

fn no_enabled_providers_diagnostic(timestamp: &str) -> DiagnosticEvent {
    DiagnosticEvent {
        code: NO_ENABLED_PROVIDERS_CODE.to_string(),
        severity: DiagnosticSeverity::Info,
        safe_message: NO_ENABLED_PROVIDERS_MESSAGE.to_string(),
        timestamp: timestamp.to_string(),
        provider: None,
        source_adapter: None,
        recoverable: true,
        details: Default::default(),
        redacted: EventRedaction {
            applied: true,
            classes: vec!["secrets".to_string(), "identity".to_string()],
        },
    }
}

fn fixture_not_allowed_diagnostic(timestamp: &str) -> DiagnosticEvent {
    DiagnosticEvent {
        code: "fixture_not_allowed".to_string(),
        severity: DiagnosticSeverity::Warning,
        safe_message: "Fixture refresh source is disabled outside development mode".to_string(),
        timestamp: timestamp.to_string(),
        provider: None,
        source_adapter: Some(SourceAdapter::Fixture),
        recoverable: true,
        details: Default::default(),
        redacted: EventRedaction {
            applied: true,
            classes: vec!["secrets".to_string(), "identity".to_string()],
        },
    }
}

fn redaction_summary() -> RedactionSummary {
    RedactionSummary {
        applied: true,
        policy_version: 1,
        notes: vec!["Task 01 diagnostics are redacted before D-Bus output".to_string()],
    }
}
