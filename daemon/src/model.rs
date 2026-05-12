use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub schema_version: u8,
    pub generated_at: String,
    pub stale: bool,
    pub selected_provider: Option<String>,
    pub daemon: SnapshotDaemon,
    pub providers: Vec<Provider>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotDaemon {
    pub version: String,
    pub state: DaemonState,
    pub last_refresh_id: Option<String>,
    pub last_refresh_started_at: Option<String>,
    pub last_refresh_finished_at: Option<String>,
    pub upstream_cli: Option<UpstreamCliInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpstreamCliInfo {
    pub path: Option<String>,
    pub version: Option<String>,
    pub available: bool,
    pub diagnostic_code: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provider_inventory: Vec<ProviderInventoryItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderInventoryItem {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonState {
    Starting,
    Ok,
    Refreshing,
    Degraded,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provider {
    pub provider: String,
    pub display_name: String,
    pub version: Option<String>,
    pub source: SemanticSource,
    pub source_adapter: SourceAdapter,
    pub state: ProviderState,
    pub updated_at: Option<String>,
    pub stale_since: Option<String>,
    pub usage: Usage,
    pub credits: Option<Credits>,
    pub identity: Option<Identity>,
    pub status: Option<ProviderStatus>,
    pub cost: Option<CostSummary>,
    pub dashboard_url: Option<String>,
    pub diagnostics_summary: Option<String>,
    #[serde(default)]
    pub diagnostic_codes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderState {
    Loading,
    Ok,
    Stale,
    Unauthenticated,
    CookieRejected,
    MissingDependency,
    ProviderUnavailable,
    ParseError,
    Timeout,
    Error,
}

impl ProviderState {
    pub fn is_usable_for_stale_cache(self) -> bool {
        matches!(self, Self::Ok | Self::Stale)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticSource {
    Api,
    Local,
    Web,
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceAdapter {
    UpstreamCli,
    LinuxWeb,
    Cache,
    Fixture,
    Synthetic,
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Usage {
    pub primary: Option<Meter>,
    pub secondary: Option<Meter>,
    pub tertiary: Option<Meter>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Meter {
    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub window_minutes: Option<u64>,
    pub resets_at: Option<String>,
    pub label: Option<String>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Credits {
    pub remaining: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub updated_at: Option<String>,
    pub unit: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Identity {
    pub provider_account_id_hash: Option<String>,
    pub account_email_display: Option<String>,
    pub account_email_hash: Option<String>,
    pub account_organization_display: Option<String>,
    pub account_organization_hash: Option<String>,
    pub login_method: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderStatus {
    pub indicator: Option<String>,
    pub description: Option<String>,
    pub updated_at: Option<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CostSummary {
    pub updated_at: Option<String>,
    pub currency: Option<String>,
    pub total: Option<f64>,
    pub period_start_at: Option<String>,
    pub period_end_at: Option<String>,
    #[serde(default)]
    pub items: Vec<CostItem>,
    #[serde(default)]
    pub diagnostic_codes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CostItem {
    pub label: String,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefreshOptions {
    pub schema_version: u8,
    #[serde(default = "default_refresh_reason")]
    pub reason: RefreshReason,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub providers: Vec<String>,
    #[serde(default = "default_busy_behavior")]
    pub busy_behavior: BusyBehavior,
    #[serde(default)]
    pub source_adapter_policy: SourceAdapterPolicy,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RefreshReason {
    Manual,
    Scheduled,
    Startup,
    SettingsChanged,
    Retry,
    Test,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BusyBehavior {
    ReturnExisting,
    Reject,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceAdapterPolicy {
    #[serde(default = "default_source_policy_mode")]
    pub mode: SourceAdapterPolicyMode,
    #[serde(default)]
    pub adapters: Vec<RefreshSourceAdapter>,
    #[serde(default = "default_true")]
    pub allow_stale_cache_fallback: bool,
}

impl Default for SourceAdapterPolicy {
    fn default() -> Self {
        Self {
            mode: SourceAdapterPolicyMode::Auto,
            adapters: Vec::new(),
            allow_stale_cache_fallback: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceAdapterPolicyMode {
    Auto,
    Prefer,
    Only,
    Exclude,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum RefreshSourceAdapter {
    UpstreamCli,
    LinuxWeb,
    Fixture,
}

impl SourceAdapterPolicy {
    pub fn allows_upstream_cli(&self) -> bool {
        match self.mode {
            SourceAdapterPolicyMode::Auto => true,
            SourceAdapterPolicyMode::Prefer => {
                self.adapters.is_empty()
                    || self.adapters.contains(&RefreshSourceAdapter::UpstreamCli)
            }
            SourceAdapterPolicyMode::Only => {
                self.adapters.contains(&RefreshSourceAdapter::UpstreamCli)
            }
            SourceAdapterPolicyMode::Exclude => {
                !self.adapters.contains(&RefreshSourceAdapter::UpstreamCli)
            }
        }
    }

    pub fn allows_fixture(&self) -> bool {
        match self.mode {
            SourceAdapterPolicyMode::Auto => false,
            SourceAdapterPolicyMode::Prefer => {
                self.adapters.contains(&RefreshSourceAdapter::Fixture)
            }
            SourceAdapterPolicyMode::Only => self.adapters.contains(&RefreshSourceAdapter::Fixture),
            SourceAdapterPolicyMode::Exclude => {
                !self.adapters.contains(&RefreshSourceAdapter::Fixture)
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefreshResult {
    pub schema_version: u8,
    pub refresh_id: String,
    pub status: RefreshStatus,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: u64,
    pub reason: RefreshReason,
    pub providers: Vec<RefreshProviderResult>,
    pub cache_written: bool,
    pub snapshot_generated_at: Option<String>,
    #[serde(default)]
    pub diagnostic_codes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RefreshStatus {
    Ok,
    Partial,
    Error,
    Busy,
    Noop,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefreshProviderResult {
    pub provider: String,
    pub status: RefreshProviderStatus,
    pub source_adapter: Option<SourceAdapter>,
    #[serde(default)]
    pub diagnostic_codes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RefreshProviderStatus {
    Ok,
    Stale,
    Skipped,
    Unauthenticated,
    CookieRejected,
    MissingDependency,
    ProviderUnavailable,
    ParseError,
    Timeout,
    Error,
}

impl From<ProviderState> for RefreshProviderStatus {
    fn from(value: ProviderState) -> Self {
        match value {
            ProviderState::Loading => Self::Skipped,
            ProviderState::Ok => Self::Ok,
            ProviderState::Stale => Self::Stale,
            ProviderState::Unauthenticated => Self::Unauthenticated,
            ProviderState::CookieRejected => Self::CookieRejected,
            ProviderState::MissingDependency => Self::MissingDependency,
            ProviderState::ProviderUnavailable => Self::ProviderUnavailable,
            ProviderState::ParseError => Self::ParseError,
            ProviderState::Timeout => Self::Timeout,
            ProviderState::Error => Self::Error,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderEvent {
    pub schema_version: u8,
    pub event_id: String,
    pub emitted_at: String,
    pub reason: ProviderEventReason,
    pub provider_id: String,
    pub provider: Provider,
    #[serde(default)]
    pub diagnostic_codes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderEventReason {
    RefreshStarted,
    RefreshProgress,
    RefreshFinished,
    SettingsChanged,
    CacheLoaded,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Diagnostics {
    pub schema_version: u8,
    pub generated_at: String,
    pub scope: DiagnosticScope,
    pub provider: Option<String>,
    pub events: Vec<DiagnosticEvent>,
    pub redaction: RedactionSummary,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticScope {
    Global,
    Provider,
    BrowserImport,
    UpstreamCli,
    Settings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticEvent {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub safe_message: String,
    pub timestamp: String,
    pub provider: Option<String>,
    pub source_adapter: Option<SourceAdapter>,
    pub recoverable: bool,
    #[serde(default)]
    pub details: BTreeMap<String, serde_json::Value>,
    pub redacted: EventRedaction,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventRedaction {
    pub applied: bool,
    #[serde(default)]
    pub classes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RedactionSummary {
    pub applied: bool,
    pub policy_version: u8,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DaemonInfo {
    pub schema_version: u8,
    pub version: String,
    pub pid: u32,
    pub started_at: String,
    pub uptime_seconds: u64,
    pub state: DaemonState,
    pub dbus: DbusInfo,
    pub capabilities: Capabilities,
    pub paths: DaemonPathsInfo,
    pub upstream_cli: UpstreamCliInfo,
    pub build: Option<BuildInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DbusInfo {
    pub bus_name: String,
    pub object_path: String,
    pub interface: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capabilities {
    pub upstream_cli: bool,
    pub browser_import: bool,
    pub linux_web_adapters: bool,
    pub cost: bool,
    pub settings_patch: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DaemonPathsInfo {
    pub config_file: Option<String>,
    pub cache_file: Option<String>,
    pub upstream_config_file: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildInfo {
    pub git_sha: Option<String>,
    pub target: Option<String>,
    pub profile: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub schema_version: u8,
    pub refresh: RefreshSettings,
    pub providers: BTreeMap<String, ProviderSettings>,
    pub browser_import: BrowserImportSettings,
    pub diagnostics: DiagnosticsSettings,
}

pub const DEFAULT_PROVIDER_IDS: [&str; 2] = ["codex", "claude"];

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            refresh: RefreshSettings {
                interval_seconds: 300,
                startup_refresh: true,
                allow_stale_cache_fallback: true,
            },
            providers: default_provider_settings(),
            browser_import: BrowserImportSettings {
                enabled: false,
                policy: BrowserImportPolicy::Off,
                profile_id_allowlist: Vec::new(),
                domain_allowlist_mode: DomainAllowlistMode::ProviderRequiredOnly,
            },
            diagnostics: DiagnosticsSettings {
                verbosity: DiagnosticsVerbosity::Normal,
                keep_redacted_artifacts: false,
            },
        }
    }
}

pub fn default_provider_settings() -> BTreeMap<String, ProviderSettings> {
    DEFAULT_PROVIDER_IDS
        .into_iter()
        .map(|provider| (provider.to_string(), ProviderSettings::default()))
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefreshSettings {
    pub interval_seconds: u64,
    pub startup_refresh: bool,
    pub allow_stale_cache_fallback: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSettings {
    pub enabled: bool,
    pub preferred_source_adapter: PreferredSourceAdapter,
    pub allow_browser_import: bool,
    pub allow_cli_fallback: bool,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            preferred_source_adapter: PreferredSourceAdapter::Auto,
            allow_browser_import: false,
            allow_cli_fallback: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PreferredSourceAdapter {
    Auto,
    UpstreamCli,
    LinuxWeb,
    Off,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserImportSettings {
    pub enabled: bool,
    pub policy: BrowserImportPolicy,
    pub profile_id_allowlist: Vec<String>,
    pub domain_allowlist_mode: DomainAllowlistMode,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserImportPolicy {
    Auto,
    ChromiumFamily,
    Firefox,
    Off,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DomainAllowlistMode {
    ProviderRequiredOnly,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticsSettings {
    pub verbosity: DiagnosticsVerbosity,
    pub keep_redacted_artifacts: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticsVerbosity {
    Normal,
    Verbose,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatch {
    pub schema_version: u8,
    pub refresh: Option<RefreshSettingsPatch>,
    pub providers: Option<BTreeMap<String, ProviderSettingsPatch>>,
    pub browser_import: Option<BrowserImportSettingsPatch>,
    pub diagnostics: Option<DiagnosticsSettingsPatch>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefreshSettingsPatch {
    pub interval_seconds: Option<u64>,
    pub startup_refresh: Option<bool>,
    pub allow_stale_cache_fallback: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSettingsPatch {
    pub enabled: Option<bool>,
    pub preferred_source_adapter: Option<PreferredSourceAdapter>,
    pub allow_browser_import: Option<bool>,
    pub allow_cli_fallback: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserImportSettingsPatch {
    pub enabled: Option<bool>,
    pub policy: Option<BrowserImportPolicy>,
    pub profile_id_allowlist: Option<Vec<String>>,
    pub domain_allowlist_mode: Option<DomainAllowlistMode>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticsSettingsPatch {
    pub verbosity: Option<DiagnosticsVerbosity>,
    pub keep_redacted_artifacts: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserImportOptions {
    pub schema_version: u8,
    #[serde(default = "default_browser_import_policy")]
    pub policy: BrowserImportPolicy,
    #[serde(default)]
    pub providers: Vec<String>,
    #[serde(default)]
    pub profile_ids: Vec<String>,
    #[serde(default = "default_true")]
    pub include_diagnostics: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserImportResult {
    pub schema_version: u8,
    pub tested_at: String,
    pub status: BrowserImportStatus,
    pub policy: BrowserImportPolicy,
    pub profiles: Vec<BrowserProfileResult>,
    pub providers: Vec<BrowserProviderResult>,
    #[serde(default)]
    pub diagnostic_codes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserImportStatus {
    Success,
    Partial,
    Failure,
    Unavailable,
    NotImplemented,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserProfileResult {
    pub browser_family: BrowserFamily,
    pub profile_id: String,
    pub profile_display_name: String,
    pub available: bool,
    pub keyring_state: KeyringState,
    pub cookies_found: Option<u64>,
    #[serde(default)]
    pub diagnostic_codes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserFamily {
    Chrome,
    Chromium,
    Brave,
    Firefox,
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum KeyringState {
    Unlocked,
    Locked,
    Unavailable,
    NotRequired,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserProviderResult {
    pub provider: String,
    pub status: BrowserProviderStatus,
    pub source_adapter: BrowserSourceAdapter,
    #[serde(default)]
    pub diagnostic_codes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserProviderStatus {
    Success,
    Unauthenticated,
    CookieRejected,
    MissingDependency,
    ProviderUnavailable,
    Timeout,
    ParseError,
    Error,
    NotImplemented,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserSourceAdapter {
    LinuxWeb,
    None,
}

fn default_refresh_reason() -> RefreshReason {
    RefreshReason::Manual
}

fn default_busy_behavior() -> BusyBehavior {
    BusyBehavior::ReturnExisting
}

fn default_source_policy_mode() -> SourceAdapterPolicyMode {
    SourceAdapterPolicyMode::Auto
}

fn default_true() -> bool {
    true
}

fn default_browser_import_policy() -> BrowserImportPolicy {
    BrowserImportPolicy::Auto
}
