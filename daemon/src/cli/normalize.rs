use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use serde_json::Value;

use super::source::map_upstream_source;
use super::types::AdapterDiagnostic;
use crate::model::{
    CostItem, CostSummary, Credits, Identity, Meter, Provider, ProviderState, ProviderStatus,
    SourceAdapter, Usage,
};

#[derive(Clone, Debug, Default)]
pub struct NormalizedUsage {
    pub providers: Vec<Provider>,
    pub diagnostics: Vec<AdapterDiagnostic>,
    pub usage_success: bool,
}

#[derive(Clone, Debug, Default)]
pub struct NormalizedCost {
    pub costs: BTreeMap<String, CostSummary>,
    pub diagnostics: Vec<AdapterDiagnostic>,
}

pub fn normalize_usage(
    value: &Value,
    requested_provider: &str,
    timestamp: &str,
) -> NormalizedUsage {
    normalize_provider_array(value, requested_provider, timestamp, false)
}

pub fn normalize_status(
    value: &Value,
    requested_provider: &str,
    timestamp: &str,
) -> NormalizedUsage {
    normalize_provider_array(value, requested_provider, timestamp, true)
}

pub fn normalize_cost(value: &Value) -> NormalizedCost {
    let mut normalized = NormalizedCost::default();
    for entry in entries(value) {
        let Some(provider) = string_field(entry, "provider") else {
            normalized.diagnostics.push(AdapterDiagnostic::warning(
                "upstream_cli_cost_normalize_skipped",
                "Upstream CLI cost entry did not include a provider id",
                None,
            ));
            continue;
        };
        if entry.get("error").is_some() {
            normalized.diagnostics.push(AdapterDiagnostic::warning(
                "upstream_cli_provider_error",
                "Upstream CLI returned a provider cost error",
                Some(provider),
            ));
            continue;
        }

        let updated_at = string_field(entry, "updatedAt");
        let session_cost = number_field(entry, "sessionCostUSD");
        let last_30_days_cost = number_field(entry, "last30DaysCostUSD");
        let total_cost = entry
            .get("totals")
            .and_then(|totals| number_field(totals, "totalCost"))
            .or(last_30_days_cost)
            .or(session_cost);
        let mut items = Vec::new();
        if session_cost.is_some() {
            items.push(CostItem {
                label: "Session".to_string(),
                amount: session_cost,
                currency: Some("USD".to_string()),
                detail: None,
            });
        }
        if last_30_days_cost.is_some() {
            items.push(CostItem {
                label: "Last 30 days".to_string(),
                amount: last_30_days_cost,
                currency: Some("USD".to_string()),
                detail: None,
            });
        }
        if total_cost.is_some() {
            items.push(CostItem {
                label: "Total".to_string(),
                amount: total_cost,
                currency: Some("USD".to_string()),
                detail: None,
            });
        }

        normalized.costs.insert(
            provider.clone(),
            CostSummary {
                updated_at,
                currency: Some("USD".to_string()),
                total: total_cost,
                period_start_at: None,
                period_end_at: None,
                items,
                diagnostic_codes: Vec::new(),
            },
        );
        normalized.diagnostics.push(AdapterDiagnostic::info(
            "upstream_cli_cost_normalized",
            "Upstream CLI cost summary normalized",
            Some(provider),
        ));
    }
    normalized
}

pub fn attach_costs(providers: &mut [Provider], costs: &BTreeMap<String, CostSummary>) {
    for provider in providers {
        if let Some(cost) = costs.get(&provider.provider) {
            provider.cost = Some(cost.clone());
            if !provider
                .diagnostic_codes
                .iter()
                .any(|code| code == "upstream_cli_cost_normalized")
            {
                provider
                    .diagnostic_codes
                    .push("upstream_cli_cost_normalized".to_string());
            }
        }
    }
}

pub fn attach_status(
    providers: &mut [Provider],
    status: NormalizedUsage,
) -> Vec<AdapterDiagnostic> {
    let mut diagnostics = status.diagnostics;
    let mut by_provider = BTreeMap::new();
    for provider in status.providers {
        by_provider.insert(provider.provider.clone(), provider);
    }
    for provider in providers {
        if let Some(status_provider) = by_provider.remove(&provider.provider) {
            provider.status = status_provider.status;
            if !provider
                .diagnostic_codes
                .iter()
                .any(|code| code == "upstream_cli_status_normalized")
            {
                provider
                    .diagnostic_codes
                    .push("upstream_cli_status_normalized".to_string());
            }
        }
    }
    diagnostics.push(AdapterDiagnostic::info(
        "upstream_cli_status_normalized",
        "Upstream CLI status payload normalized",
        None,
    ));
    diagnostics
}

pub fn synthetic_provider(
    provider_id: &str,
    state: ProviderState,
    code: &str,
    message: &str,
    timestamp: &str,
) -> Provider {
    Provider {
        provider: provider_id.to_string(),
        display_name: display_name(provider_id),
        version: None,
        source: crate::model::SemanticSource::Unknown,
        source_adapter: SourceAdapter::UpstreamCli,
        state,
        updated_at: Some(timestamp.to_string()),
        stale_since: None,
        usage: empty_usage(),
        credits: None,
        identity: None,
        status: Some(ProviderStatus {
            indicator: Some(state_indicator(state).to_string()),
            description: Some(message.to_string()),
            updated_at: Some(timestamp.to_string()),
            url: None,
        }),
        cost: None,
        dashboard_url: None,
        diagnostics_summary: Some(message.to_string()),
        diagnostic_codes: vec![code.to_string()],
    }
}

fn normalize_provider_array(
    value: &Value,
    requested_provider: &str,
    timestamp: &str,
    status_only: bool,
) -> NormalizedUsage {
    let mut normalized = NormalizedUsage::default();
    let mut seen = BTreeSet::new();

    for entry in entries(value) {
        let provider_id = string_field(entry, "provider")
            .filter(|provider| provider != "cli")
            .unwrap_or_else(|| requested_provider.to_string());
        if !seen.insert(provider_id.clone()) {
            normalized.diagnostics.push(AdapterDiagnostic::warning(
                "upstream_cli_duplicate_provider",
                "Upstream CLI returned duplicate provider entries",
                Some(provider_id),
            ));
            continue;
        }

        if entry.get("error").is_some() {
            let code = provider_error_code(entry);
            normalized.diagnostics.push(AdapterDiagnostic::warning(
                code,
                provider_error_message(code),
                Some(provider_id.clone()),
            ));
            normalized.providers.push(synthetic_provider(
                &provider_id,
                state_for_error_code(code),
                code,
                provider_error_message(code),
                timestamp,
            ));
            continue;
        }

        let Some(provider) =
            normalize_success_provider(entry, &provider_id, timestamp, status_only)
        else {
            normalized.diagnostics.push(AdapterDiagnostic::warning(
                "upstream_cli_parse_error",
                "Upstream CLI provider entry could not be normalized",
                Some(provider_id.clone()),
            ));
            normalized.providers.push(synthetic_provider(
                &provider_id,
                ProviderState::ParseError,
                "upstream_cli_parse_error",
                "Upstream CLI provider entry could not be normalized",
                timestamp,
            ));
            continue;
        };
        normalized.usage_success |= !status_only;
        normalized.diagnostics.push(AdapterDiagnostic::info(
            if status_only {
                "upstream_cli_status_normalized"
            } else {
                "upstream_cli_usage_normalized"
            },
            if status_only {
                "Upstream CLI status payload normalized"
            } else {
                "Upstream CLI usage payload normalized"
            },
            Some(provider_id),
        ));
        normalized.providers.push(provider);
    }

    if normalized.providers.is_empty() {
        normalized.providers.push(synthetic_provider(
            requested_provider,
            ProviderState::ParseError,
            "upstream_cli_parse_error",
            "Upstream CLI payload contained no provider entries",
            timestamp,
        ));
    }
    normalized
}

fn normalize_success_provider(
    entry: &Value,
    provider_id: &str,
    timestamp: &str,
    status_only: bool,
) -> Option<Provider> {
    let usage_value = entry.get("usage")?;
    let updated_at = string_field(usage_value, "updatedAt")
        .or_else(|| string_field(entry, "updatedAt"))
        .unwrap_or_else(|| timestamp.to_string());
    let source = map_upstream_source(string_field(entry, "source").as_deref());
    let mut diagnostic_codes = if status_only {
        vec!["upstream_cli_status_normalized".to_string()]
    } else {
        vec!["upstream_cli_usage_normalized".to_string()]
    };
    let status = normalize_status_value(entry.get("status"), timestamp);
    if status.is_some()
        && !diagnostic_codes
            .iter()
            .any(|code| code == "upstream_cli_status_normalized")
    {
        diagnostic_codes.push("upstream_cli_status_normalized".to_string());
    }
    Some(Provider {
        provider: provider_id.to_string(),
        display_name: display_name(provider_id),
        version: string_field(entry, "version"),
        source,
        source_adapter: SourceAdapter::UpstreamCli,
        state: ProviderState::Ok,
        updated_at: Some(updated_at),
        stale_since: None,
        usage: Usage {
            primary: normalize_meter(usage_value.get("primary"), "Primary"),
            secondary: normalize_meter(usage_value.get("secondary"), "Secondary"),
            tertiary: normalize_meter(usage_value.get("tertiary"), "Tertiary"),
        },
        credits: normalize_credits(entry.get("credits")),
        identity: normalize_identity(usage_value),
        status,
        cost: None,
        dashboard_url: None,
        diagnostics_summary: None,
        diagnostic_codes,
    })
}

fn normalize_meter(value: Option<&Value>, fallback_label: &str) -> Option<Meter> {
    let value = value?;
    if value.is_null() {
        return None;
    }
    let used_percent = number_field(value, "usedPercent").map(clamp_percent);
    let remaining_percent = used_percent.map(|used| clamp_percent(100.0 - used));
    let window_minutes = value.get("windowMinutes").and_then(Value::as_u64);
    Some(Meter {
        used_percent,
        remaining_percent,
        window_minutes,
        resets_at: string_field(value, "resetsAt"),
        label: Some(label_for_window(window_minutes, fallback_label).to_string()),
        detail: string_field(value, "resetDescription"),
    })
}

fn normalize_credits(value: Option<&Value>) -> Option<Credits> {
    let value = value?;
    if value.is_null() {
        return None;
    }
    Some(Credits {
        remaining: number_field(value, "remaining"),
        remaining_percent: number_field(value, "remainingPercent").map(clamp_percent),
        updated_at: string_field(value, "updatedAt"),
        unit: string_field(value, "unit"),
    })
}

fn normalize_identity(usage: &Value) -> Option<Identity> {
    let identity = usage.get("identity");
    let email = string_field(usage, "accountEmail")
        .or_else(|| identity.and_then(|value| string_field(value, "accountEmail")))
        .or_else(|| string_field(usage, "signedInEmail"));
    let provider_account = identity
        .and_then(|value| string_field(value, "providerID"))
        .or_else(|| identity.and_then(|value| string_field(value, "providerAccountId")));
    let organization = string_field(usage, "accountOrganization")
        .or_else(|| identity.and_then(|value| string_field(value, "accountOrganization")));
    let login_method = string_field(usage, "loginMethod")
        .or_else(|| identity.and_then(|value| string_field(value, "loginMethod")))
        .filter(|value| is_safe_login_method(value));
    if email.is_none()
        && provider_account.is_none()
        && organization.is_none()
        && login_method.is_none()
    {
        return None;
    }
    Some(Identity {
        provider_account_id_hash: provider_account.as_deref().map(local_hash),
        account_email_display: email.as_deref().map(mask_email_display),
        account_email_hash: email.as_deref().map(local_hash),
        account_organization_display: organization.as_deref().map(mask_identity_display),
        account_organization_hash: organization.as_deref().map(local_hash),
        login_method,
    })
}

fn normalize_status_value(value: Option<&Value>, timestamp: &str) -> Option<ProviderStatus> {
    let value = value?;
    if value.is_null() {
        return None;
    }
    Some(ProviderStatus {
        indicator: string_field(value, "indicator"),
        description: string_field(value, "description"),
        updated_at: string_field(value, "updatedAt").or_else(|| Some(timestamp.to_string())),
        url: string_field(value, "url"),
    })
}

fn entries(value: &Value) -> Vec<&Value> {
    match value {
        Value::Array(items) => items.iter().collect(),
        Value::Object(_) => vec![value],
        _ => Vec::new(),
    }
}

fn provider_error_code(entry: &Value) -> &'static str {
    let message = entry
        .get("error")
        .and_then(|error| string_field(error, "message"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    if message.contains("web support") || message.contains("macos") {
        "upstream_cli_unsupported_source"
    } else if message.contains("authentication") || message.contains("unauthenticated") {
        "upstream_cli_unauthenticated"
    } else if message.contains("rate") {
        "upstream_cli_provider_unavailable"
    } else {
        "upstream_cli_provider_error"
    }
}

fn provider_error_message(code: &str) -> &'static str {
    match code {
        "upstream_cli_unsupported_source" => {
            "Upstream CLI reported that the requested source is unsupported on Linux"
        }
        "upstream_cli_unauthenticated" => {
            "Upstream CLI reported provider authentication is missing"
        }
        "upstream_cli_provider_unavailable" => "Upstream CLI reported provider unavailable",
        _ => "Upstream CLI returned a provider error",
    }
}

fn state_for_error_code(code: &str) -> ProviderState {
    match code {
        "upstream_cli_unauthenticated" => ProviderState::Unauthenticated,
        "upstream_cli_provider_unavailable" => ProviderState::ProviderUnavailable,
        "upstream_cli_unsupported_source" => ProviderState::MissingDependency,
        _ => ProviderState::Error,
    }
}

fn state_indicator(state: ProviderState) -> &'static str {
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

fn empty_usage() -> Usage {
    Usage {
        primary: None,
        secondary: None,
        tertiary: None,
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn number_field(value: &Value, key: &str) -> Option<f64> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
}

fn clamp_percent(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}

fn label_for_window(window_minutes: Option<u64>, fallback: &str) -> &'static str {
    match window_minutes {
        Some(300) => "Session",
        Some(10080) => "Weekly",
        _ if fallback == "Primary" => "Primary",
        _ if fallback == "Secondary" => "Secondary",
        _ => "Tertiary",
    }
}

fn display_name(provider: &str) -> String {
    let mut chars = provider.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => "Unknown".to_string(),
    }
}

fn mask_email_display(value: &str) -> String {
    if value.starts_with("[REDACTED_") {
        return "masked-account".to_string();
    }
    let Some((local, domain)) = value.split_once('@') else {
        return mask_identity_display(value);
    };
    let first = local.chars().next().unwrap_or('m');
    format!("{first}***@{domain}")
}

fn mask_identity_display(_value: &str) -> String {
    "masked-identity".to_string()
}

fn local_hash(value: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "codexbar-linux-v1".hash(&mut hasher);
    value.hash(&mut hasher);
    format!("local-hash-{:016x}", hasher.finish())
}

fn is_safe_login_method(value: &str) -> bool {
    value.len() <= 64
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-04-30T00:00:00Z";

    #[test]
    fn normalizes_codex_usage_without_raw_identity() {
        let value: Value = serde_json::from_str(include_str!(
            "../../fixtures/upstream-cli/usage/live_20260429T193209Z_usage_codex_cli_default_stdout.json"
        ))
        .expect("fixture");
        let normalized = normalize_usage(&value, "codex", NOW);
        assert!(normalized.usage_success);
        let provider = &normalized.providers[0];
        assert_eq!(provider.provider, "codex");
        assert_eq!(provider.source_adapter, SourceAdapter::UpstreamCli);
        assert_eq!(provider.source, crate::model::SemanticSource::Local);
        assert_eq!(provider.state, ProviderState::Ok);
        assert_eq!(
            provider.usage.primary.as_ref().unwrap().used_percent,
            Some(34.0)
        );
        assert_eq!(
            provider
                .usage
                .primary
                .as_ref()
                .unwrap()
                .resets_at
                .as_deref(),
            Some("2026-04-29T22:36:14Z")
        );
        let json = serde_json::to_string(provider).expect("json");
        assert!(!json.contains("\"accountEmail\":"));
        assert!(!json.contains("[REDACTED_EMAIL]"));
    }

    #[test]
    fn status_payload_merges_as_normal_provider_shape() {
        let value: Value = serde_json::from_str(include_str!(
            "../../fixtures/upstream-cli/status/live_20260429T193209Z_status_codex_cli_stdout.json"
        ))
        .expect("fixture");
        let normalized = normalize_status(&value, "codex", NOW);
        let provider = &normalized.providers[0];
        assert_eq!(
            provider.status.as_ref().unwrap().description.as_deref(),
            Some("All Systems Operational")
        );
    }

    #[test]
    fn cost_drops_daily_chronology() {
        let value: Value = serde_json::from_str(include_str!(
            "../../fixtures/upstream-cli/cost/live_20260429T181010Z_cost_all_stdout.json"
        ))
        .expect("fixture");
        let normalized = normalize_cost(&value);
        let cost = normalized.costs.get("codex").expect("codex cost");
        let json = serde_json::to_string(cost).expect("json");
        assert!(json.contains("Last 30 days"));
        assert!(!json.contains("daily"));
        assert!(!json.contains("modelBreakdowns"));
        assert!(!json.contains("modelsUsed"));
    }

    #[test]
    fn unsupported_source_maps_to_safe_provider_diagnostic() {
        let value: Value = serde_json::from_str(include_str!(
            "../../fixtures/upstream-cli/errors/live_20260429T181010Z_unsupported_web_source_stdout.json"
        ))
        .expect("fixture");
        let normalized = normalize_usage(&value, "codex", NOW);
        assert_eq!(
            normalized.providers[0].state,
            ProviderState::MissingDependency
        );
        assert_eq!(
            normalized.diagnostics[0].code,
            "upstream_cli_unsupported_source"
        );
    }
}
