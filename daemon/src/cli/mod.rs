pub mod command;
pub mod normalize;
pub mod output;
pub mod resolver;
pub mod runner;
pub mod source;
pub mod types;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Map, Value};

use crate::cache::validate_snapshot;
use crate::config::is_safe_id;
use crate::error::AppResult;
use crate::model::{
    CostItem, CostSummary, Credits, DaemonState, DiagnosticEvent, DiagnosticSeverity,
    EventRedaction, Identity, Meter, Provider, ProviderState, ProviderStatus, SemanticSource,
    Settings, Snapshot, SnapshotDaemon, SourceAdapter, UpstreamCliInfo, Usage,
};
use crate::paths::AppPaths;
use crate::redact;

use output::{
    classify_stderr, classify_stdout, diagnostic_code_for_stdout, OutputClassification,
    StderrClassification,
};
use resolver::{CliResolution, CliResolver};
use runner::{CommandKind, CommandOutput, CommandRunError, CommandRunner, CommandSpec};

const DEFAULT_PROVIDER: &str = "codex";
const OUTPUT_LIMIT_BYTES: usize = 4 * 1024 * 1024;
const STDERR_LIMIT_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug)]
pub struct CliRefreshRequest {
    pub refresh_id: String,
    pub started_at: String,
    pub finished_at: String,
    pub providers: Vec<String>,
    pub selected_provider: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CliRefresh {
    pub snapshot: Snapshot,
    pub diagnostics: Vec<DiagnosticEvent>,
}

#[derive(Clone, Debug)]
pub struct CliTimeouts {
    pub version: Duration,
    pub usage: Duration,
    pub status: Duration,
    pub cost: Duration,
}

impl Default for CliTimeouts {
    fn default() -> Self {
        Self {
            version: Duration::from_secs(5),
            usage: Duration::from_secs(90),
            status: Duration::from_secs(90),
            cost: Duration::from_secs(30),
        }
    }
}

#[derive(Clone, Debug)]
pub struct UpstreamCliAdapter {
    resolver: CliResolver,
    runner: CommandRunner,
    timeouts: CliTimeouts,
}

impl UpstreamCliAdapter {
    pub fn from_paths(paths: &AppPaths) -> Self {
        Self::with_overrides(paths.upstream_cli_path.clone(), CliTimeouts::default())
    }

    pub fn with_overrides(override_path: Option<PathBuf>, timeouts: CliTimeouts) -> Self {
        Self {
            resolver: CliResolver::new(override_path),
            runner: CommandRunner,
            timeouts,
        }
    }

    pub async fn refresh(&self, request: CliRefreshRequest) -> AppResult<CliRefresh> {
        let providers = sanitize_provider_targets(&request.providers);
        let resolution = self.resolver.resolve();
        let CliResolution::Found { path } = resolution else {
            let cli = upstream_info_from_resolution(&resolution, None);
            let code = cli
                .diagnostic_code
                .clone()
                .unwrap_or_else(|| "upstream_cli_missing".to_string());
            let snapshot = error_snapshot(
                &request,
                &providers,
                cli,
                ProviderState::MissingDependency,
                &code,
            );
            validate_snapshot(&snapshot)?;
            return Ok(CliRefresh {
                snapshot,
                diagnostics: vec![diagnostic(
                    &code,
                    "Upstream CodexBar CLI is not available",
                    DiagnosticSeverity::Error,
                    &request.finished_at,
                    None,
                    command_details(CommandKind::Version, None),
                )],
            });
        };

        let mut diagnostics = vec![diagnostic(
            "upstream_cli_resolved",
            "Upstream CodexBar CLI executable resolved",
            DiagnosticSeverity::Info,
            &request.finished_at,
            None,
            BTreeMap::new(),
        )];
        let mut version = None;
        let mut cli_diagnostic_code = None;
        diagnostics.push(command_event(
            "upstream_cli_command_started",
            CommandKind::Version,
            None,
            &request.finished_at,
        ));
        match self.run(&path, version_spec(self.timeouts.version)).await {
            Ok(output) if output.success() => {
                version = sanitize_version(&String::from_utf8_lossy(&output.stdout));
                diagnostics.push(command_event(
                    "upstream_cli_command_finished",
                    CommandKind::Version,
                    None,
                    &request.finished_at,
                ));
                diagnostics.push(diagnostic(
                    "upstream_cli_version_detected",
                    "Upstream CodexBar CLI version detected",
                    DiagnosticSeverity::Info,
                    &request.finished_at,
                    None,
                    command_details(CommandKind::Version, Some(&output)),
                ));
            }
            Ok(output) => {
                diagnostics.push(command_event(
                    "upstream_cli_command_finished",
                    CommandKind::Version,
                    None,
                    &request.finished_at,
                ));
                let failure = CliFailure::from_output(&output);
                cli_diagnostic_code = Some(failure.code.to_string());
                diagnostics.push(failure.to_diagnostic(
                    CommandKind::Version,
                    None,
                    &request.finished_at,
                    Some(&output),
                ));
            }
            Err(err) => {
                diagnostics.push(command_event(
                    "upstream_cli_command_finished",
                    CommandKind::Version,
                    None,
                    &request.finished_at,
                ));
                let failure = CliFailure::from_run_error(err);
                cli_diagnostic_code = Some(failure.code.to_string());
                diagnostics.push(failure.to_diagnostic(
                    CommandKind::Version,
                    None,
                    &request.finished_at,
                    None,
                ));
            }
        }

        let cli = UpstreamCliInfo {
            path: Some(resolver::display_safe_path(&path)),
            version,
            available: true,
            diagnostic_code: cli_diagnostic_code,
        };

        diagnostics.push(command_event(
            "upstream_cli_command_started",
            CommandKind::Cost,
            None,
            &request.finished_at,
        ));
        let cost_by_provider = match self.run(&path, cost_spec(self.timeouts.cost)).await {
            Ok(output) => match parse_success_json(&output) {
                Ok(value) => {
                    diagnostics.push(command_event(
                        "upstream_cli_command_finished",
                        CommandKind::Cost,
                        None,
                        &request.finished_at,
                    ));
                    let costs = normalize_costs(&value);
                    if !costs.is_empty() {
                        diagnostics.push(diagnostic(
                            "upstream_cli_cost_normalized",
                            "Upstream CLI cost output normalized",
                            DiagnosticSeverity::Info,
                            &request.finished_at,
                            None,
                            command_details(CommandKind::Cost, Some(&output)),
                        ));
                    }
                    costs
                }
                Err(failure) => {
                    diagnostics.push(command_event(
                        "upstream_cli_command_finished",
                        CommandKind::Cost,
                        None,
                        &request.finished_at,
                    ));
                    diagnostics.push(failure.to_diagnostic(
                        CommandKind::Cost,
                        None,
                        &request.finished_at,
                        Some(&output),
                    ));
                    BTreeMap::new()
                }
            },
            Err(err) => {
                diagnostics.push(command_event(
                    "upstream_cli_command_finished",
                    CommandKind::Cost,
                    None,
                    &request.finished_at,
                ));
                let failure = CliFailure::from_run_error(err);
                diagnostics.push(failure.to_diagnostic(
                    CommandKind::Cost,
                    None,
                    &request.finished_at,
                    None,
                ));
                BTreeMap::new()
            }
        };

        let mut normalized = Vec::with_capacity(providers.len());
        for provider in &providers {
            diagnostics.push(command_event(
                "upstream_cli_command_started",
                CommandKind::Usage,
                Some(provider.clone()),
                &request.finished_at,
            ));
            let usage = self
                .run(&path, usage_spec(provider, self.timeouts.usage))
                .await;
            diagnostics.push(command_event(
                "upstream_cli_command_finished",
                CommandKind::Usage,
                Some(provider.clone()),
                &request.finished_at,
            ));
            diagnostics.push(command_event(
                "upstream_cli_command_started",
                CommandKind::Status,
                Some(provider.clone()),
                &request.finished_at,
            ));
            let status = self
                .run(&path, status_spec(provider, self.timeouts.status))
                .await;
            diagnostics.push(command_event(
                "upstream_cli_command_finished",
                CommandKind::Status,
                Some(provider.clone()),
                &request.finished_at,
            ));

            let mut provider_diagnostics = Vec::new();
            let usage_value = match usage {
                Ok(output) => match parse_success_json(&output) {
                    Ok(value) => {
                        diagnostics.push(diagnostic(
                            "upstream_cli_usage_normalized",
                            "Upstream CLI usage output normalized",
                            DiagnosticSeverity::Info,
                            &request.finished_at,
                            Some(provider.clone()),
                            command_details(CommandKind::Usage, Some(&output)),
                        ));
                        Some(value)
                    }
                    Err(failure) => {
                        provider_diagnostics.push(failure.clone());
                        diagnostics.push(failure.to_diagnostic(
                            CommandKind::Usage,
                            Some(provider),
                            &request.finished_at,
                            Some(&output),
                        ));
                        None
                    }
                },
                Err(err) => {
                    let failure = CliFailure::from_run_error(err);
                    provider_diagnostics.push(failure.clone());
                    diagnostics.push(failure.to_diagnostic(
                        CommandKind::Usage,
                        Some(provider),
                        &request.finished_at,
                        None,
                    ));
                    None
                }
            };

            let status_value = match status {
                Ok(output) => match parse_success_json(&output) {
                    Ok(value) => {
                        diagnostics.push(diagnostic(
                            "upstream_cli_status_normalized",
                            "Upstream CLI status output normalized",
                            DiagnosticSeverity::Info,
                            &request.finished_at,
                            Some(provider.clone()),
                            command_details(CommandKind::Status, Some(&output)),
                        ));
                        Some(value)
                    }
                    Err(failure) => {
                        diagnostics.push(failure.to_diagnostic(
                            CommandKind::Status,
                            Some(provider),
                            &request.finished_at,
                            Some(&output),
                        ));
                        None
                    }
                },
                Err(err) => {
                    let failure = CliFailure::from_run_error(err);
                    diagnostics.push(failure.to_diagnostic(
                        CommandKind::Status,
                        Some(provider),
                        &request.finished_at,
                        None,
                    ));
                    None
                }
            };

            let cost = cost_by_provider.get(provider).cloned();
            let normalized_provider = normalize_provider(
                provider,
                usage_value.as_ref(),
                status_value.as_ref(),
                cost,
                &provider_diagnostics,
                &request.finished_at,
            );
            normalized.push(normalized_provider);
        }

        let daemon_state = if normalized
            .iter()
            .all(|provider| provider.state == ProviderState::Ok)
        {
            DaemonState::Ok
        } else if normalized
            .iter()
            .any(|provider| provider.state == ProviderState::Ok)
        {
            DaemonState::Degraded
        } else {
            DaemonState::Error
        };

        let snapshot = Snapshot {
            schema_version: 1,
            generated_at: request.finished_at.clone(),
            stale: false,
            selected_provider: request
                .selected_provider
                .clone()
                .or_else(|| providers.first().cloned()),
            daemon: SnapshotDaemon {
                version: env!("CARGO_PKG_VERSION").to_string(),
                state: daemon_state,
                last_refresh_id: Some(request.refresh_id.clone()),
                last_refresh_started_at: Some(request.started_at.clone()),
                last_refresh_finished_at: Some(request.finished_at.clone()),
                upstream_cli: Some(cli),
            },
            providers: normalized,
        };
        validate_snapshot(&snapshot)?;
        diagnostics.push(diagnostic(
            "upstream_cli_redaction_applied",
            "Upstream CLI output was normalized through public redaction checks",
            DiagnosticSeverity::Info,
            &request.finished_at,
            None,
            BTreeMap::new(),
        ));
        Ok(CliRefresh {
            snapshot,
            diagnostics,
        })
    }

    async fn run(
        &self,
        path: &std::path::Path,
        spec: CommandSpec,
    ) -> Result<CommandOutput, CommandRunError> {
        self.runner.run(path, &spec).await
    }
}

pub fn resolve_info(paths: &AppPaths) -> UpstreamCliInfo {
    let resolver = CliResolver::new(paths.upstream_cli_path.clone());
    upstream_info_from_resolution(&resolver.resolve(), None)
}

pub fn target_providers(settings: &Settings, requested: &[String]) -> Vec<String> {
    if !requested.is_empty() {
        return sanitize_provider_targets(requested);
    }

    let configured = settings
        .providers
        .iter()
        .filter(|(_provider, settings)| {
            settings.enabled
                && settings.allow_cli_fallback
                && settings.preferred_source_adapter != crate::model::PreferredSourceAdapter::Off
        })
        .map(|(provider, _settings)| provider.clone())
        .collect::<Vec<_>>();
    sanitize_provider_targets(&configured)
}

fn sanitize_provider_targets(values: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut providers = values
        .iter()
        .filter(|value| is_safe_id(value))
        .filter(|value| seen.insert((*value).to_string()))
        .cloned()
        .collect::<Vec<_>>();
    if providers.is_empty() {
        providers.push(DEFAULT_PROVIDER.to_string());
    }
    providers
}

fn upstream_info_from_resolution(
    resolution: &CliResolution,
    version: Option<String>,
) -> UpstreamCliInfo {
    let (path, available, diagnostic_code) = resolution.info_parts();
    UpstreamCliInfo {
        path,
        version,
        available,
        diagnostic_code,
    }
}

fn version_spec(timeout: Duration) -> CommandSpec {
    CommandSpec {
        kind: CommandKind::Version,
        args: vec!["--version".to_string()],
        timeout,
        max_stdout_bytes: 16 * 1024,
        max_stderr_bytes: 16 * 1024,
    }
}

fn usage_spec(provider: &str, timeout: Duration) -> CommandSpec {
    CommandSpec {
        kind: CommandKind::Usage,
        args: vec![
            "--format".to_string(),
            "json".to_string(),
            "--json-only".to_string(),
            "--provider".to_string(),
            provider.to_string(),
            "--source".to_string(),
            "cli".to_string(),
        ],
        timeout,
        max_stdout_bytes: OUTPUT_LIMIT_BYTES,
        max_stderr_bytes: STDERR_LIMIT_BYTES,
    }
}

fn status_spec(provider: &str, timeout: Duration) -> CommandSpec {
    CommandSpec {
        kind: CommandKind::Status,
        args: vec![
            "--format".to_string(),
            "json".to_string(),
            "--json-only".to_string(),
            "--provider".to_string(),
            provider.to_string(),
            "--source".to_string(),
            "cli".to_string(),
            "--status".to_string(),
        ],
        timeout,
        max_stdout_bytes: OUTPUT_LIMIT_BYTES,
        max_stderr_bytes: STDERR_LIMIT_BYTES,
    }
}

fn cost_spec(timeout: Duration) -> CommandSpec {
    CommandSpec {
        kind: CommandKind::Cost,
        args: vec![
            "cost".to_string(),
            "--format".to_string(),
            "json".to_string(),
            "--json-only".to_string(),
            "--provider".to_string(),
            "all".to_string(),
        ],
        timeout,
        max_stdout_bytes: OUTPUT_LIMIT_BYTES,
        max_stderr_bytes: STDERR_LIMIT_BYTES,
    }
}

fn parse_success_json(output: &CommandOutput) -> Result<Value, CliFailure> {
    if output.timed_out {
        return Err(CliFailure::timeout());
    }
    if output.exit_code != Some(0) {
        return Err(CliFailure::nonzero(output));
    }
    match classify_stdout(output) {
        OutputClassification::SingleJson(value) => Ok(value),
        classification => Err(CliFailure::from_output_classification(
            diagnostic_code_for_stdout(&classification),
        )),
    }
}

fn normalize_provider(
    requested_provider: &str,
    usage_value: Option<&Value>,
    status_value: Option<&Value>,
    cost: Option<CostSummary>,
    failures: &[CliFailure],
    now: &str,
) -> Provider {
    let usage_object =
        usage_value.and_then(|value| find_provider_object(value, requested_provider));
    let status_object =
        status_value.and_then(|value| find_provider_object(value, requested_provider));
    let object = usage_object.or(status_object);

    let Some(object) = object else {
        let failure = failures
            .first()
            .cloned()
            .unwrap_or_else(CliFailure::parse_error);
        let mut provider = error_provider(
            requested_provider,
            failure.provider_state(),
            failure.code,
            failure.safe_message,
            now,
        );
        provider.cost = cost;
        return provider;
    };

    if object.get("error").is_some() {
        let failure = CliFailure::provider_error();
        return error_provider(
            requested_provider,
            failure.provider_state(),
            failure.code,
            failure.safe_message,
            now,
        );
    }

    let provider_id = string_field(object, "provider")
        .filter(|value| is_safe_id(value))
        .unwrap_or_else(|| requested_provider.to_string());
    let usage = object.get("usage").and_then(Value::as_object);
    let status = status_object
        .and_then(|status_object| status_object.get("status"))
        .or_else(|| object.get("status"))
        .and_then(Value::as_object);
    let credits = object.get("credits").and_then(Value::as_object);

    let mut diagnostic_codes = failures
        .iter()
        .map(|failure| failure.code.to_string())
        .collect::<Vec<_>>();
    if usage.is_none() {
        diagnostic_codes.push("upstream_cli_usage_missing".to_string());
    }

    Provider {
        provider: provider_id.clone(),
        display_name: display_name(&provider_id),
        version: string_field(object, "version").and_then(safe_string),
        source: source::map_upstream_source(string_field(object, "source").as_deref()),
        source_adapter: SourceAdapter::UpstreamCli,
        state: if usage.is_some() {
            ProviderState::Ok
        } else {
            ProviderState::ParseError
        },
        updated_at: usage
            .and_then(|usage| string_field(usage, "updatedAt"))
            .or_else(|| string_field(object, "updatedAt"))
            .and_then(valid_datetime)
            .or_else(|| Some(now.to_string())),
        stale_since: None,
        usage: normalize_usage(usage),
        credits: credits.map(|credits| normalize_credits(credits, usage, now)),
        identity: normalize_identity(usage),
        status: status.map(|status| normalize_status(status, now)),
        cost,
        dashboard_url: None,
        diagnostics_summary: if diagnostic_codes.is_empty() {
            None
        } else {
            Some("Upstream CLI returned partial diagnostics".to_string())
        },
        diagnostic_codes,
    }
}

fn normalize_usage(usage: Option<&Map<String, Value>>) -> Usage {
    Usage {
        primary: usage
            .and_then(|usage| usage.get("primary"))
            .and_then(|value| normalize_meter(value, "Session")),
        secondary: usage
            .and_then(|usage| usage.get("secondary"))
            .and_then(|value| normalize_meter(value, "Weekly")),
        tertiary: usage
            .and_then(|usage| usage.get("tertiary"))
            .and_then(|value| normalize_meter(value, "Daily")),
    }
}

fn normalize_meter(value: &Value, label: &str) -> Option<Meter> {
    let object = value.as_object()?;
    let used_percent = number_field(object, "usedPercent").map(clamp_percent);
    let remaining_percent = number_field(object, "remainingPercent")
        .map(clamp_percent)
        .or_else(|| used_percent.map(|used| clamp_percent(100.0 - used)));
    Some(Meter {
        used_percent,
        remaining_percent,
        window_minutes: integer_field(object, "windowMinutes"),
        resets_at: string_field(object, "resetsAt").and_then(valid_datetime),
        label: Some(label.to_string()),
        detail: remaining_percent.map(|remaining| format!("{remaining:.0}% remaining")),
    })
}

fn normalize_credits(
    credits: &Map<String, Value>,
    usage: Option<&Map<String, Value>>,
    now: &str,
) -> Credits {
    Credits {
        remaining: number_field(credits, "remaining"),
        remaining_percent: number_field(credits, "remainingPercent").map(clamp_percent),
        updated_at: string_field(credits, "updatedAt")
            .or_else(|| usage.and_then(|usage| string_field(usage, "updatedAt")))
            .and_then(valid_datetime)
            .or_else(|| Some(now.to_string())),
        unit: Some("credits".to_string()),
    }
}

fn normalize_identity(usage: Option<&Map<String, Value>>) -> Option<Identity> {
    let usage = usage?;
    let nested = usage.get("identity").and_then(Value::as_object);
    let email = nested
        .and_then(|identity| string_field(identity, "accountEmail"))
        .or_else(|| string_field(usage, "accountEmail"));
    let organization = nested
        .and_then(|identity| string_field(identity, "accountOrganization"))
        .or_else(|| string_field(usage, "accountOrganization"));
    let login_method = nested
        .and_then(|identity| string_field(identity, "loginMethod"))
        .or_else(|| string_field(usage, "loginMethod"))
        .and_then(safe_id_like_string);

    let identity = Identity {
        provider_account_id_hash: None,
        account_email_display: email.as_deref().and_then(mask_email_display),
        account_email_hash: None,
        account_organization_display: organization.as_deref().and_then(mask_organization_display),
        account_organization_hash: None,
        login_method,
    };

    if identity.provider_account_id_hash.is_none()
        && identity.account_email_display.is_none()
        && identity.account_email_hash.is_none()
        && identity.account_organization_display.is_none()
        && identity.account_organization_hash.is_none()
        && identity.login_method.is_none()
    {
        None
    } else {
        Some(identity)
    }
}

fn normalize_status(status: &Map<String, Value>, now: &str) -> ProviderStatus {
    ProviderStatus {
        indicator: string_field(status, "indicator").and_then(safe_string),
        description: string_field(status, "description").and_then(safe_string),
        updated_at: string_field(status, "updatedAt")
            .and_then(valid_datetime)
            .or_else(|| Some(now.to_string())),
        url: string_field(status, "url").and_then(safe_url),
    }
}

fn normalize_costs(value: &Value) -> BTreeMap<String, CostSummary> {
    provider_objects(value)
        .into_iter()
        .filter_map(|object| {
            let provider = string_field(object, "provider")?;
            if !is_safe_id(&provider) {
                return None;
            }
            let summary = normalize_cost(object);
            Some((provider, summary))
        })
        .collect()
}

fn normalize_cost(object: &Map<String, Value>) -> CostSummary {
    let total = object
        .get("totals")
        .and_then(Value::as_object)
        .and_then(|totals| number_field(totals, "totalCost"))
        .or_else(|| number_field(object, "last30DaysCostUSD"))
        .or_else(|| number_field(object, "sessionCostUSD"));
    let mut items = Vec::new();
    if let Some(amount) = number_field(object, "sessionCostUSD") {
        items.push(CostItem {
            label: "Session".to_string(),
            amount: Some(amount),
            currency: Some("USD".to_string()),
            detail: None,
        });
    }
    if let Some(amount) = object
        .get("totals")
        .and_then(Value::as_object)
        .and_then(|totals| number_field(totals, "totalCost"))
        .or_else(|| number_field(object, "last30DaysCostUSD"))
    {
        items.push(CostItem {
            label: "Last 30 days".to_string(),
            amount: Some(amount),
            currency: Some("USD".to_string()),
            detail: None,
        });
    }

    let (period_start_at, period_end_at) = cost_period(object);
    CostSummary {
        updated_at: string_field(object, "updatedAt").and_then(valid_datetime),
        currency: Some("USD".to_string()),
        total,
        period_start_at,
        period_end_at,
        items,
        diagnostic_codes: Vec::new(),
    }
}

fn cost_period(object: &Map<String, Value>) -> (Option<String>, Option<String>) {
    let Some(days) = object.get("daily").and_then(Value::as_array) else {
        return (None, None);
    };
    let mut dates = days
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|day| string_field(day, "date"))
        .filter(|date| is_date(date))
        .collect::<Vec<_>>();
    dates.sort();
    let start = dates.first().map(|date| format!("{date}T00:00:00Z"));
    let end = dates.last().map(|date| format!("{date}T23:59:59Z"));
    (start, end)
}

fn error_snapshot(
    request: &CliRefreshRequest,
    providers: &[String],
    cli: UpstreamCliInfo,
    state: ProviderState,
    code: &str,
) -> Snapshot {
    Snapshot {
        schema_version: 1,
        generated_at: request.finished_at.clone(),
        stale: false,
        selected_provider: request
            .selected_provider
            .clone()
            .or_else(|| providers.first().cloned()),
        daemon: SnapshotDaemon {
            version: env!("CARGO_PKG_VERSION").to_string(),
            state: DaemonState::Error,
            last_refresh_id: Some(request.refresh_id.clone()),
            last_refresh_started_at: Some(request.started_at.clone()),
            last_refresh_finished_at: Some(request.finished_at.clone()),
            upstream_cli: Some(cli),
        },
        providers: providers
            .iter()
            .map(|provider| {
                error_provider(
                    provider,
                    state,
                    code,
                    "Upstream CLI dependency is unavailable",
                    &request.finished_at,
                )
            })
            .collect(),
    }
}

fn error_provider(
    provider: &str,
    state: ProviderState,
    code: &str,
    summary: &str,
    now: &str,
) -> Provider {
    Provider {
        provider: provider.to_string(),
        display_name: display_name(provider),
        version: None,
        source: SemanticSource::Unknown,
        source_adapter: SourceAdapter::UpstreamCli,
        state,
        updated_at: Some(now.to_string()),
        stale_since: None,
        usage: Usage {
            primary: None,
            secondary: None,
            tertiary: None,
        },
        credits: None,
        identity: None,
        status: Some(ProviderStatus {
            indicator: Some(provider_state_label(state).to_string()),
            description: Some(summary.to_string()),
            updated_at: Some(now.to_string()),
            url: None,
        }),
        cost: None,
        dashboard_url: None,
        diagnostics_summary: Some(summary.to_string()),
        diagnostic_codes: vec![code.to_string()],
    }
}

fn find_provider_object<'a>(
    value: &'a Value,
    requested_provider: &str,
) -> Option<&'a Map<String, Value>> {
    let objects = provider_objects(value);
    objects
        .iter()
        .copied()
        .find(|object| string_field(object, "provider").as_deref() == Some(requested_provider))
        .or_else(|| objects.first().copied())
}

fn provider_objects(value: &Value) -> Vec<&Map<String, Value>> {
    if let Some(object) = value.as_object() {
        if let Some(providers) = object.get("providers").and_then(Value::as_array) {
            return providers.iter().filter_map(Value::as_object).collect();
        }
        return vec![object];
    }
    value
        .as_array()
        .map(|values| values.iter().filter_map(Value::as_object).collect())
        .unwrap_or_default()
}

fn string_field(object: &Map<String, Value>, key: &str) -> Option<String> {
    object.get(key)?.as_str().map(str::trim).map(str::to_string)
}

fn number_field(object: &Map<String, Value>, key: &str) -> Option<f64> {
    object.get(key)?.as_f64().filter(|value| value.is_finite())
}

fn integer_field(object: &Map<String, Value>, key: &str) -> Option<u64> {
    object.get(key)?.as_u64()
}

fn clamp_percent(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}

fn valid_datetime(value: String) -> Option<String> {
    if value.contains('T') && value.ends_with('Z') {
        safe_string(value)
    } else {
        None
    }
}

fn safe_url(value: String) -> Option<String> {
    if (value.starts_with("https://") || value.starts_with("http://"))
        && redact::validate_public_json_text(&value).is_ok()
    {
        Some(value)
    } else {
        None
    }
}

fn safe_string(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 512 {
        return None;
    }
    if redact::validate_public_json_text(trimmed).is_ok() {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn safe_id_like_string(value: String) -> Option<String> {
    let value = value.trim();
    if value.len() <= 64
        && !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | ':' | '-'))
    {
        Some(value.to_string())
    } else {
        None
    }
}

fn sanitize_version(text: &str) -> Option<String> {
    text.lines().find_map(|line| safe_string(line.to_string()))
}

fn mask_email_display(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.to_ascii_lowercase().contains("redacted") || value.contains("***@") {
        return Some("masked-account".to_string());
    }
    let at = value.find('@')?;
    let (local, domain_with_at) = value.split_at(at);
    let domain = &domain_with_at[1..];
    if local.is_empty()
        || domain.is_empty()
        || !domain
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-'))
    {
        return Some("masked-account".to_string());
    }
    let first = local.chars().next().unwrap_or('m');
    Some(format!("{first}***@{domain}"))
}

fn mask_organization_display(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some("masked-organization".to_string())
    }
}

fn display_name(provider: &str) -> String {
    let mut name = String::new();
    let mut uppercase_next = true;
    for ch in provider.chars() {
        if matches!(ch, '-' | '_' | '.') {
            name.push(' ');
            uppercase_next = true;
        } else if uppercase_next {
            name.extend(ch.to_uppercase());
            uppercase_next = false;
        } else {
            name.push(ch);
        }
    }
    if name.is_empty() {
        provider.to_string()
    } else {
        name
    }
}

fn provider_state_label(state: ProviderState) -> &'static str {
    match state {
        ProviderState::Loading => "loading",
        ProviderState::Ok => "ok",
        ProviderState::Stale => "stale",
        ProviderState::Unauthenticated => "unauthenticated",
        ProviderState::CookieRejected => "cookie_rejected",
        ProviderState::MissingDependency => "missing_dependency",
        ProviderState::ProviderUnavailable => "provider_unavailable",
        ProviderState::ParseError => "parse_error",
        ProviderState::Timeout => "timeout",
        ProviderState::Error => "error",
    }
}

fn is_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

#[derive(Clone, Debug)]
struct CliFailure {
    code: &'static str,
    safe_message: &'static str,
}

impl CliFailure {
    fn timeout() -> Self {
        Self {
            code: "upstream_cli_timeout",
            safe_message: "Upstream CLI command timed out",
        }
    }

    fn parse_error() -> Self {
        Self {
            code: "upstream_cli_parse_error",
            safe_message: "Upstream CLI output could not be parsed",
        }
    }

    fn from_output_classification(code: &'static str) -> Self {
        match code {
            "upstream_cli_output_truncated" => Self {
                code,
                safe_message: "Upstream CLI output exceeded the capture limit",
            },
            "upstream_cli_empty_stdout" => Self {
                code,
                safe_message: "Upstream CLI output was empty",
            },
            _ => Self::parse_error(),
        }
    }

    fn provider_error() -> Self {
        Self {
            code: "upstream_cli_provider_error",
            safe_message: "Upstream CLI returned a provider error",
        }
    }

    fn from_run_error(error: CommandRunError) -> Self {
        match error {
            CommandRunError::Spawn => Self {
                code: "upstream_cli_spawn_failed",
                safe_message: "Upstream CLI command could not start",
            },
            CommandRunError::Io | CommandRunError::Join => Self {
                code: "upstream_cli_io_error",
                safe_message: "Upstream CLI command failed during I/O",
            },
        }
    }

    fn from_output(output: &CommandOutput) -> Self {
        if output.timed_out {
            return Self::timeout();
        }
        if output.exit_code != Some(0) {
            return Self::nonzero(output);
        }
        Self::parse_error()
    }

    fn nonzero(output: &CommandOutput) -> Self {
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .to_ascii_lowercase();
        if text.contains("only supported on macos") || text.contains("requires web support") {
            Self {
                code: "upstream_cli_unsupported_source",
                safe_message: "Upstream CLI source is unavailable on this platform",
            }
        } else if text.contains("unauthorized")
            || text.contains("unauthenticated")
            || text.contains("not logged in")
            || text.contains("login")
        {
            Self {
                code: "upstream_cli_unauthenticated",
                safe_message: "Upstream CLI provider is unauthenticated",
            }
        } else if text.contains("provider") {
            Self::provider_error()
        } else {
            Self {
                code: "upstream_cli_nonzero_exit",
                safe_message: "Upstream CLI command exited with an error",
            }
        }
    }

    fn provider_state(&self) -> ProviderState {
        match self.code {
            "upstream_cli_timeout" => ProviderState::Timeout,
            "upstream_cli_parse_error"
            | "upstream_cli_output_truncated"
            | "upstream_cli_empty_stdout" => ProviderState::ParseError,
            "upstream_cli_missing" | "upstream_cli_spawn_failed" | "upstream_cli_io_error" => {
                ProviderState::MissingDependency
            }
            "upstream_cli_unauthenticated" => ProviderState::Unauthenticated,
            "upstream_cli_unsupported_source" | "upstream_cli_nonzero_exit" => {
                ProviderState::ProviderUnavailable
            }
            _ => ProviderState::Error,
        }
    }

    fn to_diagnostic(
        &self,
        command: CommandKind,
        provider: Option<&str>,
        timestamp: &str,
        output: Option<&CommandOutput>,
    ) -> DiagnosticEvent {
        diagnostic(
            self.code,
            self.safe_message,
            if self.provider_state() == ProviderState::Ok {
                DiagnosticSeverity::Info
            } else {
                DiagnosticSeverity::Warning
            },
            timestamp,
            provider.map(str::to_string),
            command_details(command, output),
        )
    }
}

fn command_details(
    command: CommandKind,
    output: Option<&CommandOutput>,
) -> BTreeMap<String, Value> {
    let mut details = BTreeMap::new();
    details.insert(
        "command".to_string(),
        Value::String(command.label().to_string()),
    );
    if let Some(output) = output {
        let summary = output.summary();
        details.insert(
            "command".to_string(),
            Value::String(summary.kind.label().to_string()),
        );
        if let Some(code) = summary.exit_code {
            details.insert("exitCode".to_string(), Value::Number(code.into()));
        }
        details.insert("timedOut".to_string(), Value::Bool(summary.timed_out));
        details.insert(
            "durationMs".to_string(),
            Value::Number(summary.duration_ms.into()),
        );
        details.insert(
            "stdoutBytes".to_string(),
            Value::Number((summary.stdout_bytes as u64).into()),
        );
        details.insert(
            "stderrBytes".to_string(),
            Value::Number((summary.stderr_bytes as u64).into()),
        );
        details.insert(
            "stdoutTruncated".to_string(),
            Value::Bool(summary.stdout_truncated),
        );
        details.insert(
            "stderrTruncated".to_string(),
            Value::Bool(summary.stderr_truncated),
        );
        details.insert(
            "stderrClass".to_string(),
            Value::String(stderr_class_label(classify_stderr(output)).to_string()),
        );
    }
    details
}

fn stderr_class_label(classification: StderrClassification) -> &'static str {
    match classification {
        StderrClassification::Empty => "empty",
        StderrClassification::Text => "text",
        StderrClassification::Json => "json",
        StderrClassification::Truncated => "truncated",
        StderrClassification::Binary => "binary",
    }
}

fn diagnostic(
    code: &str,
    safe_message: &str,
    severity: DiagnosticSeverity,
    timestamp: &str,
    provider: Option<String>,
    details: BTreeMap<String, Value>,
) -> DiagnosticEvent {
    DiagnosticEvent {
        code: code.to_string(),
        severity,
        safe_message: safe_message.to_string(),
        timestamp: timestamp.to_string(),
        provider,
        source_adapter: Some(SourceAdapter::UpstreamCli),
        recoverable: true,
        details,
        redacted: EventRedaction {
            applied: true,
            classes: vec![
                "secrets".to_string(),
                "identity".to_string(),
                "stdout".to_string(),
                "stderr".to_string(),
            ],
        },
    }
}

fn command_event(
    code: &str,
    command: CommandKind,
    provider: Option<String>,
    timestamp: &str,
) -> DiagnosticEvent {
    diagnostic(
        code,
        match code {
            "upstream_cli_command_started" => "Upstream CLI command started",
            "upstream_cli_command_finished" => "Upstream CLI command finished",
            _ => "Upstream CLI command event",
        },
        DiagnosticSeverity::Info,
        timestamp,
        provider,
        command_details(command, None),
    )
}
