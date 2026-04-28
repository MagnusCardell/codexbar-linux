use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::cache::{stale_mutated, CacheLoad, SnapshotCache};
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
    RefreshReason, RefreshResult, RefreshStatus, Settings, Snapshot, UpstreamCliInfo,
};
use crate::paths::AppPaths;
use crate::redact;
use crate::{DBUS_INTERFACE, DBUS_NAME, DBUS_OBJECT_PATH};

#[derive(Debug)]
pub struct App {
    paths: AppPaths,
    clock: Clock,
    cache: SnapshotCache,
    settings_store: SettingsStore,
    refresh_counter: AtomicU64,
    state: Mutex<AppState>,
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
    fixture_allowed: bool,
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
        Self::new(AppPaths::from_env())
    }

    pub fn new(paths: AppPaths) -> AppResult<Self> {
        let clock = Clock::started_now();
        let cache = SnapshotCache::new(paths.cache_dir.clone(), paths.cache_file.clone());
        let settings_store =
            SettingsStore::new(paths.config_dir.clone(), paths.config_file.clone());
        let mut diagnostics = Vec::new();
        let snapshot = match cache.load() {
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
            clock,
            cache,
            settings_store,
            refresh_counter: AtomicU64::new(1),
            state: Mutex::new(AppState {
                snapshot,
                settings,
                active_refresh: None,
                diagnostics,
            }),
        })
    }

    pub fn check_startup() -> AppResult<()> {
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

    pub fn get_diagnostics_json(&self, provider_id: &str) -> AppResult<String> {
        let state = self.lock_state()?;
        let provider = if provider_id.is_empty() || provider_id == "global" {
            None
        } else {
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
        to_public_json(&state.settings)
    }

    pub fn test_browser_import_json(&self, options_json: &str) -> AppResult<String> {
        let options: BrowserImportOptions = parse_input_json(options_json)?;
        if options.schema_version != 1
            || has_duplicates(&options.providers)
            || has_duplicates(&options.profile_ids)
        {
            return Err(AppError::invalid_json());
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
            fixture_allowed: options.source_adapter_policy.allows_fixture(),
        });
        state.snapshot.daemon.state = DaemonState::Refreshing;
        state.snapshot.daemon.last_refresh_id = Some(refresh_id.clone());
        state.snapshot.daemon.last_refresh_started_at = Some(started_at);
        state.snapshot.daemon.last_refresh_finished_at = None;
        Ok(RefreshStart::Started { refresh_id })
    }

    pub fn finish_refresh(&self, refresh_id: &str) -> AppResult<RefreshCompletion> {
        let mut state = self.lock_state()?;
        let active = state
            .active_refresh
            .clone()
            .ok_or_else(AppError::internal_redacted)?;
        if active.id != refresh_id {
            return Err(AppError::internal_redacted());
        }
        let finished_at = now_rfc3339();
        let (snapshot, cache_written, status, diagnostic_codes) = if active.fixture_allowed {
            let snapshot =
                fixtures::refreshed_snapshot(refresh_id, &active.started_at, &finished_at)?;
            let cache_written = self.cache.store(&snapshot).is_ok();
            let status = if cache_written {
                RefreshStatus::Ok
            } else {
                RefreshStatus::Partial
            };
            let diagnostic_codes = if cache_written {
                Vec::new()
            } else {
                vec!["internal_error_redacted".to_string()]
            };
            (snapshot, cache_written, status, diagnostic_codes)
        } else {
            let snapshot = fixtures::unsupported_adapter_snapshot(&finished_at)?;
            (
                snapshot,
                false,
                RefreshStatus::Error,
                vec![
                    "upstream_cli_not_implemented".to_string(),
                    "browser_import_not_implemented".to_string(),
                ],
            )
        };
        let result = refresh_result(
            &snapshot,
            &active,
            &finished_at,
            cache_written,
            status,
            diagnostic_codes,
        );
        let provider_events = provider_events(&snapshot, refresh_id, &finished_at)?;
        state.snapshot = snapshot;
        state.active_refresh = None;
        state.diagnostics.push(diagnostic_event(
            "refresh_finished",
            "Task 01 fixture refresh completed and cache was written",
            &finished_at,
            None,
        ));
        Ok(RefreshCompletion {
            refresh_id: refresh_id.to_string(),
            snapshot_json: to_public_json(&state.snapshot)?,
            result_json: to_public_json(&result)?,
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
                upstream_cli: false,
                browser_import: false,
                linux_web_adapters: false,
                cost: false,
                settings_patch: true,
            },
            paths: DaemonPathsInfo {
                config_file: Some(self.paths.config_file.display().to_string()),
                cache_file: Some(self.paths.cache_file.display().to_string()),
                upstream_config_file: self.paths.upstream_config_file_hint.clone(),
            },
            upstream_cli: UpstreamCliInfo {
                available: false,
                path: None,
                version: None,
                diagnostic_code: Some("upstream_cli_not_implemented".to_string()),
            },
            build: Some(crate::model::BuildInfo {
                git_sha: option_env!("GIT_SHA").map(str::to_string),
                target: option_env!("TARGET").map(str::to_string),
                profile: option_env!("PROFILE").map(str::to_string),
            }),
        }
    }
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

fn validate_refresh_options(options: &RefreshOptions) -> AppResult<()> {
    if options.schema_version != 1
        || has_duplicates(&options.providers)
        || has_duplicates(&options.source_adapter_policy.adapters)
    {
        return Err(AppError::invalid_json());
    }
    Ok(())
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

fn redaction_summary() -> RedactionSummary {
    RedactionSummary {
        applied: true,
        policy_version: 1,
        notes: vec!["Task 01 diagnostics are redacted before D-Bus output".to_string()],
    }
}
