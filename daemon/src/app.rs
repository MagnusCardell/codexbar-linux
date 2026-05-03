use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::browser::keyring::{BrowserDecryptorMode, FakeDecryptorMode};
use crate::browser::profile::BrowserDiscoveryRoots;
use crate::browser::{self, BrowserImportRequest, BrowserSessionRequest};
use crate::cache::{stale_mutated, CacheLoad, SnapshotCache};
use crate::cli::{self, CliRefreshRequest, UpstreamCliAdapter};
use crate::clock::{duration_ms, now_rfc3339, Clock};
use crate::config::{
    apply_settings_patch, is_safe_id, parse_settings_patch, SettingsLoad, SettingsStore,
};
use crate::error::{AppError, AppResult};
use crate::fixtures;
use crate::model::{
    BrowserImportOptions, BusyBehavior, Capabilities, DaemonInfo, DaemonPathsInfo, DaemonState,
    DbusInfo, DiagnosticEvent, DiagnosticScope, DiagnosticSeverity, EventRedaction, ProviderEvent,
    ProviderEventReason, RedactionSummary, RefreshOptions, RefreshReason, RefreshResult,
    RefreshSourceAdapter, RefreshStatus, Settings, Snapshot, SourceAdapter,
};
use crate::paths::AppPaths;
use crate::redact;
use crate::web::client::{CodexWebFixture, FakeWebClient, ReqwestStaticGetClient};
use crate::{DBUS_INTERFACE, DBUS_NAME, DBUS_OBJECT_PATH};

pub struct App {
    paths: AppPaths,
    runtime: AppRuntime,
    clock: Clock,
    cache: SnapshotCache,
    settings_store: SettingsStore,
    refresh_counter: AtomicU64,
    state: Mutex<AppState>,
}

impl fmt::Debug for App {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("App")
            .field("paths", &"[redacted]")
            .field("runtime", &self.runtime)
            .field("clock", &self.clock)
            .field("cache", &"[redacted]")
            .field("settings_store", &"[redacted]")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct AppRuntime {
    allow_fixture_source: bool,
    browser_roots: Option<BrowserDiscoveryRoots>,
    browser_decryptor_mode: BrowserDecryptorMode,
    codex_web_fixture: Option<CodexWebFixture>,
    fake_web_session_available: bool,
    codex_web_live_transport: bool,
}

impl fmt::Debug for AppRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppRuntime")
            .field("allow_fixture_source", &self.allow_fixture_source)
            .field(
                "browser_roots",
                &self.browser_roots.as_ref().map(|_| "[redacted]"),
            )
            .field("browser_decryptor_mode", &self.browser_decryptor_mode)
            .field("codex_web_fixture", &self.codex_web_fixture)
            .field(
                "fake_web_session_available",
                &self.fake_web_session_available,
            )
            .field("codex_web_live_transport", &self.codex_web_live_transport)
            .finish()
    }
}

impl AppRuntime {
    pub fn production() -> Self {
        Self {
            allow_fixture_source: false,
            browser_roots: None,
            browser_decryptor_mode: BrowserDecryptorMode::Plain,
            codex_web_fixture: None,
            fake_web_session_available: false,
            codex_web_live_transport: false,
        }
    }

    pub fn from_env() -> AppResult<Self> {
        let browser_roots = env_browser_roots()?;
        let codex_web_live_transport = env_codex_web_live_transport();
        if codex_web_live_transport && browser_roots.is_none() {
            return Err(unsafe_browser_fake_home());
        }
        Ok(Self {
            allow_fixture_source: env_allows_fixture_source(),
            browser_roots,
            browser_decryptor_mode: BrowserDecryptorMode::Plain,
            codex_web_fixture: None,
            fake_web_session_available: false,
            codex_web_live_transport,
        })
    }

    pub fn with_fixture_source_for_tests() -> Self {
        Self {
            allow_fixture_source: true,
            browser_roots: None,
            browser_decryptor_mode: BrowserDecryptorMode::Plain,
            codex_web_fixture: None,
            fake_web_session_available: false,
            codex_web_live_transport: false,
        }
    }

    pub fn fixture_source_allowed(&self) -> bool {
        self.allow_fixture_source
    }

    pub fn with_browser_roots_for_tests(roots: BrowserDiscoveryRoots) -> Self {
        Self::production().with_browser_roots(roots)
    }

    pub fn with_browser_roots(mut self, roots: BrowserDiscoveryRoots) -> Self {
        self.browser_roots = roots.canonicalized();
        self
    }

    pub fn with_browser_decryptor_mode(mut self, mode: FakeDecryptorMode) -> Self {
        self.browser_decryptor_mode = BrowserDecryptorMode::fake(mode);
        self
    }

    pub fn with_browser_decryptor_backend(mut self, mode: BrowserDecryptorMode) -> Self {
        self.browser_decryptor_mode = mode;
        self
    }

    pub fn with_codex_web_fixture_for_tests(fixture: CodexWebFixture) -> Self {
        Self::production()
            .with_codex_web_fixture(fixture)
            .with_fake_web_session_available(true)
    }

    pub fn with_codex_web_fixture(mut self, fixture: CodexWebFixture) -> Self {
        self.codex_web_fixture = Some(fixture);
        self
    }

    pub fn with_fake_web_session_available(mut self, available: bool) -> Self {
        self.fake_web_session_available = available;
        self
    }

    pub fn with_codex_web_live_transport_for_tests(mut self, enabled: bool) -> Self {
        self.codex_web_live_transport = enabled;
        self
    }

    fn browser_roots(&self) -> Option<BrowserDiscoveryRoots> {
        self.browser_roots.clone()
    }

    fn browser_decryptor_mode(&self) -> BrowserDecryptorMode {
        self.browser_decryptor_mode
    }

    fn codex_web_fixture(&self) -> Option<CodexWebFixture> {
        self.codex_web_fixture
    }

    fn codex_web_live_transport(&self) -> bool {
        self.codex_web_live_transport
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
        Self::new_with_runtime(AppPaths::from_env(), AppRuntime::from_env()?)
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
        if !browser::validate_profile_ids(&options.profile_ids) {
            return Err(AppError::invalid_json());
        }
        let settings = {
            let state = self.lock_state()?;
            state.settings.clone()
        };
        let result = browser::test_import(BrowserImportRequest {
            options,
            settings,
            roots: self.runtime.browser_roots(),
            decryptor_mode: self.runtime.browser_decryptor_mode(),
            tested_at: now_rfc3339(),
        });
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
        let fixture_requested = fixture_requested(&active.options);
        let fixture_only = active.options.source_adapter_policy.allows_fixture()
            && !active.options.source_adapter_policy.allows_upstream_cli();
        let linux_web_requested = active.options.source_adapter_policy.mode
            == crate::model::SourceAdapterPolicyMode::Only
            && active
                .options
                .source_adapter_policy
                .adapters
                .contains(&RefreshSourceAdapter::LinuxWeb);

        let (mut snapshot, mut diagnostics, mut diagnostic_codes) =
            if fixture_requested && !self.runtime.fixture_source_allowed() {
                (
                    fixture_not_allowed_snapshot(refresh_id, &active.started_at, &finished_at)?,
                    vec![fixture_not_allowed_diagnostic(&finished_at)],
                    vec![
                        "capability_unimplemented".to_string(),
                        "fixture_not_allowed".to_string(),
                    ],
                )
            } else if fixture_only {
                (
                    fixtures::refreshed_snapshot(refresh_id, &active.started_at, &finished_at)?,
                    Vec::new(),
                    Vec::new(),
                )
            } else if linux_web_requested {
                let provider_targets =
                    crate::web::target_providers(&settings, &active.options.providers);
                let live_transport_allowed = self.runtime.codex_web_live_transport()
                    && codex_web_live_request_allowed(&active.options, &provider_targets);
                let mut sessions = std::collections::BTreeMap::new();
                let mut session_diagnostic_codes = std::collections::BTreeMap::new();
                if self.runtime.fake_web_session_available {
                    sessions.insert(
                        "codex".to_string(),
                        crate::web::fake_codex_session_for_tests(),
                    );
                } else if live_transport_allowed {
                    let collection = browser::collect_session_material(BrowserSessionRequest {
                        providers: provider_targets.clone(),
                        settings: settings.clone(),
                        roots: self.runtime.browser_roots(),
                        decryptor_mode: self.runtime.browser_decryptor_mode(),
                    });
                    sessions = collection.sessions;
                    session_diagnostic_codes = collection.provider_diagnostic_codes;
                }
                let request = crate::web::WebRefreshRequest {
                    refresh_id: refresh_id.to_string(),
                    started_at: active.started_at.clone(),
                    finished_at: finished_at.clone(),
                    providers: provider_targets,
                    selected_provider: previous_snapshot.selected_provider.clone(),
                    upstream_cli: cli::resolve_info(&self.paths),
                    sessions,
                    session_diagnostic_codes,
                };
                let refresh = if let Some(fixture) = self.runtime.codex_web_fixture() {
                    let client = FakeWebClient::codex_fixture(fixture);
                    crate::web::refresh_with_client(request, &client).await?
                } else if live_transport_allowed {
                    let client = ReqwestStaticGetClient::new();
                    crate::web::refresh_with_client(request, &client).await?
                } else {
                    crate::web::disabled_refresh(request)?
                };
                let diagnostic_codes = refresh
                    .diagnostics
                    .iter()
                    .map(|event| event.code.clone())
                    .collect::<Vec<_>>();
                (refresh.snapshot, refresh.diagnostics, diagnostic_codes)
            } else if active.options.source_adapter_policy.allows_upstream_cli() {
                let provider_targets = cli::target_providers(&settings, &active.options.providers);
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
                let diagnostic_codes = refresh
                    .diagnostics
                    .iter()
                    .map(|event| event.code.clone())
                    .collect::<Vec<_>>();
                (refresh.snapshot, refresh.diagnostics, diagnostic_codes)
            } else {
                (
                    fixtures::unsupported_adapter_snapshot(&finished_at)?,
                    Vec::new(),
                    vec!["upstream_cli_excluded".to_string()],
                )
            };

        let live_had_success = snapshot
            .providers
            .iter()
            .any(|provider| provider.state == crate::model::ProviderState::Ok);
        if !live_had_success
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
            snapshot = stale_mutated(previous_snapshot, &finished_at);
            snapshot.daemon.last_refresh_id = Some(refresh_id.to_string());
            snapshot.daemon.last_refresh_started_at = Some(active.started_at.clone());
            snapshot.daemon.last_refresh_finished_at = Some(finished_at.clone());
            diagnostic_codes.push("stale_cache_fallback".to_string());
            diagnostics.push(diagnostic_event(
                "stale_cache_fallback",
                "Live refresh failed; serving stale normalized cache",
                &finished_at,
                None,
            ));
        }

        let cache_written = live_had_success && self.cache.store(&snapshot).is_ok();
        if live_had_success && !cache_written {
            diagnostic_codes.push("internal_error_redacted".to_string());
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
        let upstream_cli = if resolved_upstream_cli.available {
            let mut info = resolved_upstream_cli;
            if info.version.is_none() {
                info.version = state
                    .snapshot
                    .daemon
                    .upstream_cli
                    .as_ref()
                    .and_then(|cli| cli.version.clone());
            }
            info
        } else {
            state
                .snapshot
                .daemon
                .upstream_cli
                .clone()
                .filter(|cli| cli.available)
                .unwrap_or(resolved_upstream_cli)
        };
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
                browser_import: true,
                linux_web_adapters: self.runtime.codex_web_live_transport()
                    || self.runtime.codex_web_fixture().is_some(),
                cost: upstream_cli_available,
                settings_patch: true,
            },
            paths: DaemonPathsInfo {
                config_file: Some(cli::resolver::display_safe_path(&self.paths.config_file)),
                cache_file: Some(cli::resolver::display_safe_path(&self.paths.cache_file)),
                upstream_config_file: self.paths.upstream_config_file_hint.clone(),
            },
            upstream_cli,
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

fn refresh_status(
    snapshot: &Snapshot,
    cache_written: bool,
    diagnostic_codes: &[String],
) -> RefreshStatus {
    let ok_count = snapshot
        .providers
        .iter()
        .filter(|provider| provider.state == crate::model::ProviderState::Ok)
        .count();
    if ok_count == snapshot.providers.len() && diagnostic_codes.is_empty() && cache_written {
        RefreshStatus::Ok
    } else if ok_count > 0
        || (diagnostic_codes
            .iter()
            .any(|code| code == "stale_cache_fallback")
            && !diagnostic_codes
                .iter()
                .any(|code| code == "fixture_not_allowed"))
    {
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

fn fixture_requested(options: &RefreshOptions) -> bool {
    options
        .source_adapter_policy
        .adapters
        .contains(&RefreshSourceAdapter::Fixture)
        || (options.source_adapter_policy.allows_fixture()
            && !options.source_adapter_policy.allows_upstream_cli())
}

fn codex_web_live_request_allowed(options: &RefreshOptions, provider_targets: &[String]) -> bool {
    options.providers.len() == 1
        && options.providers[0] == "codex"
        && provider_targets.iter().any(|provider| provider == "codex")
        && options.source_adapter_policy.mode == crate::model::SourceAdapterPolicyMode::Only
        && options.source_adapter_policy.adapters.as_slice() == [RefreshSourceAdapter::LinuxWeb]
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

fn env_codex_web_live_transport() -> bool {
    matches!(
        std::env::var("CODEXBAR_CODEX_WEB_LIVE").ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

fn env_browser_roots() -> AppResult<Option<BrowserDiscoveryRoots>> {
    let Some(value) = std::env::var_os("CODEXBAR_BROWSER_IMPORT_FAKE_HOME") else {
        return Ok(None);
    };
    if value.is_empty() {
        return Err(unsafe_browser_fake_home());
    }
    safe_browser_roots_from_fake_home(
        PathBuf::from(value),
        std::env::var_os("HOME").map(PathBuf::from),
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
    )
}

fn safe_browser_roots_from_fake_home(
    fake_home: PathBuf,
    real_home: Option<PathBuf>,
    real_xdg_config_home: Option<PathBuf>,
) -> AppResult<Option<BrowserDiscoveryRoots>> {
    if fake_home.as_os_str().is_empty() || fake_home == Path::new("/") || !fake_home.is_absolute() {
        return Err(unsafe_browser_fake_home());
    }
    let fake_home = std::fs::canonicalize(fake_home).map_err(|_| unsafe_browser_fake_home())?;
    if fake_home == Path::new("/") || !fake_home.is_dir() {
        return Err(unsafe_browser_fake_home());
    }

    if let Some(real_home) = real_home.and_then(|path| std::fs::canonicalize(path).ok()) {
        if fake_home == real_home {
            return Err(unsafe_browser_fake_home());
        }
        let real_config = real_home.join(".config");
        if fake_home.starts_with(&real_config) {
            return Err(unsafe_browser_fake_home());
        }
    }
    if let Some(real_xdg_config_home) =
        real_xdg_config_home.and_then(|path| std::fs::canonicalize(path).ok())
    {
        if fake_home == real_xdg_config_home || fake_home.starts_with(&real_xdg_config_home) {
            return Err(unsafe_browser_fake_home());
        }
    }
    if !fake_home.join(".codexbar-throwaway-browser-root").is_file() {
        return Err(unsafe_browser_fake_home());
    }

    let Some(roots) = BrowserDiscoveryRoots::synthetic_home(fake_home).canonicalized() else {
        return Err(unsafe_browser_fake_home());
    };
    Ok(Some(roots))
}

fn unsafe_browser_fake_home() -> AppError {
    AppError::DependencyUnavailable("unsafe browser fake home; details redacted".to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_browser_home_guard_rejects_real_home_and_unsafe_roots() {
        let real_home = tempfile::tempdir().expect("real home");
        let fake_home = tempfile::tempdir().expect("fake home");
        std::fs::create_dir_all(real_home.path().join(".config")).expect("real config");
        std::fs::create_dir_all(fake_home.path().join(".config")).expect("fake config");
        std::fs::write(
            fake_home.path().join(".codexbar-throwaway-browser-root"),
            b"throwaway",
        )
        .expect("fake marker");

        assert!(safe_browser_roots_from_fake_home(
            fake_home.path().to_path_buf(),
            Some(real_home.path().to_path_buf()),
            Some(real_home.path().join(".config"))
        )
        .expect("safe fake home")
        .is_some());
        assert!(safe_browser_roots_from_fake_home(
            real_home.path().to_path_buf(),
            Some(real_home.path().to_path_buf()),
            Some(real_home.path().join(".config"))
        )
        .is_err());
        assert!(safe_browser_roots_from_fake_home(PathBuf::from("/"), None, None).is_err());
        assert!(safe_browser_roots_from_fake_home(PathBuf::new(), None, None).is_err());
        assert!(safe_browser_roots_from_fake_home(PathBuf::from("relative"), None, None).is_err());
    }

    #[test]
    fn fake_browser_home_guard_rejects_real_config_descendants() {
        let real_home = tempfile::tempdir().expect("real home");
        let real_config_child = real_home.path().join(".config").join("throwaway");
        std::fs::create_dir_all(real_config_child.join(".config")).expect("config child");

        assert!(safe_browser_roots_from_fake_home(
            real_config_child,
            Some(real_home.path().to_path_buf()),
            Some(real_home.path().join(".config"))
        )
        .is_err());
    }

    #[test]
    fn fake_browser_home_guard_rejects_missing_throwaway_marker() {
        let real_home = tempfile::tempdir().expect("real home");
        let fake_home = tempfile::tempdir().expect("fake home");
        std::fs::create_dir_all(real_home.path().join(".config")).expect("real config");
        std::fs::create_dir_all(fake_home.path().join(".config")).expect("fake config");

        assert!(safe_browser_roots_from_fake_home(
            fake_home.path().to_path_buf(),
            Some(real_home.path().to_path_buf()),
            Some(real_home.path().join(".config"))
        )
        .is_err());
    }

    #[test]
    #[cfg(unix)]
    fn fake_browser_home_guard_rejects_home_and_config_symlink_escapes() {
        use std::os::unix::fs::symlink;

        let real_home = tempfile::tempdir().expect("real home");
        let symlink_home = tempfile::tempdir().expect("link holder");
        let home_link = symlink_home.path().join("home-link");
        symlink(real_home.path(), &home_link).expect("home symlink");
        assert!(safe_browser_roots_from_fake_home(
            home_link,
            Some(real_home.path().to_path_buf()),
            Some(real_home.path().join(".config"))
        )
        .is_err());

        let fake_home = tempfile::tempdir().expect("fake home");
        std::fs::create_dir_all(real_home.path().join(".config")).expect("real config");
        symlink(
            real_home.path().join(".config"),
            fake_home.path().join(".config"),
        )
        .expect("config symlink");
        std::fs::write(
            fake_home.path().join(".codexbar-throwaway-browser-root"),
            b"throwaway",
        )
        .expect("fake marker");
        assert!(safe_browser_roots_from_fake_home(
            fake_home.path().to_path_buf(),
            Some(real_home.path().to_path_buf()),
            Some(real_home.path().join(".config"))
        )
        .is_err());
    }
}
