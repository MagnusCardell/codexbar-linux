use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use serde::Deserialize;
use serde_json::Value;

use crate::browser::session_material::SessionMaterial;
use crate::model::{
    Credits, DiagnosticEvent, DiagnosticSeverity, Identity, Meter, Provider, ProviderState,
    ProviderStatus, SemanticSource, SourceAdapter, Usage,
};
use crate::redact;
use crate::web::client::{WebClient, WebClientError};
use crate::web::diagnostics;
use crate::web::policy::CodexWebPolicy;

pub const PROVIDER_ID: &str = "codex";
pub const DISPLAY_NAME: &str = "Codex";

#[derive(Clone, Debug)]
pub struct CodexWebFetchResult {
    pub provider: Provider,
    pub diagnostics: Vec<DiagnosticEvent>,
}

pub fn fetch_dashboard_with_client<C>(
    client: &C,
    session: Option<&SessionMaterial>,
    now: &str,
    expected_account_email: Option<&str>,
) -> CodexWebFetchResult
where
    C: WebClient,
{
    let policy = CodexWebPolicy::new();
    let Some(material) =
        session.filter(|material| material.provider() == PROVIDER_ID && !material.is_empty())
    else {
        return failure(
            ProviderState::Unauthenticated,
            diagnostics::COOKIE_ABSENT,
            "Browser session material was not available for Codex web",
            now,
            Vec::new(),
        );
    };
    let Some(session_header) = material.cookie_header_value() else {
        return failure(
            ProviderState::Unauthenticated,
            diagnostics::COOKIE_ABSENT,
            "Browser session material was not available for Codex web",
            now,
            Vec::new(),
        );
    };

    let mut events = vec![diagnostics::event(
        diagnostics::FETCH_STARTED,
        DiagnosticSeverity::Info,
        "Codex web fixture fetch started",
        now,
        PROVIDER_ID,
        BTreeMap::new(),
    )];
    let request = policy
        .dashboard_request()
        .with_session_header(session_header);
    let response = match client.request(request) {
        Ok(response) => response,
        Err(WebClientError::Timeout) => {
            events.push(diagnostics::event(
                diagnostics::FETCH_TIMEOUT,
                DiagnosticSeverity::Warning,
                "Codex web fixture fetch timed out",
                now,
                PROVIDER_ID,
                BTreeMap::new(),
            ));
            return failure(
                ProviderState::Timeout,
                diagnostics::FETCH_TIMEOUT,
                "Codex web fixture fetch timed out",
                now,
                events,
            );
        }
        Err(WebClientError::TransportUnavailable) => {
            events.push(diagnostics::event(
                diagnostics::FETCH_FINISHED,
                DiagnosticSeverity::Warning,
                "Codex web fixture fetch failed safely",
                now,
                PROVIDER_ID,
                BTreeMap::new(),
            ));
            return failure(
                ProviderState::ProviderUnavailable,
                diagnostics::FETCH_FINISHED,
                "Codex web fixture fetch failed safely",
                now,
                events,
            );
        }
    };

    if response
        .redirect_url()
        .map_or_else(
            || policy.validate_candidate_url(response.final_url()),
            |url| policy.validate_redirect_url(url),
        )
        .is_err()
    {
        events.push(diagnostics::event(
            diagnostics::REDIRECT_BLOCKED,
            DiagnosticSeverity::Warning,
            "Codex web redirect target was blocked by policy",
            now,
            PROVIDER_ID,
            diagnostics::details(&[("redirectBlocked", Value::Bool(true))]),
        ));
        return failure(
            ProviderState::ProviderUnavailable,
            diagnostics::REDIRECT_BLOCKED,
            "Codex web redirect target was blocked by policy",
            now,
            events,
        );
    }

    if response.body().len() > policy.response_size_limit() {
        events.push(diagnostics::event(
            diagnostics::RESPONSE_TOO_LARGE,
            DiagnosticSeverity::Warning,
            "Codex web fixture response exceeded the configured size limit",
            now,
            PROVIDER_ID,
            diagnostics::details(&[("responseBytes", Value::from(response.body().len() as u64))]),
        ));
        return failure(
            ProviderState::ParseError,
            diagnostics::RESPONSE_TOO_LARGE,
            "Codex web fixture response exceeded the configured size limit",
            now,
            events,
        );
    }

    if response.status() == 429 {
        events.push(diagnostics::event(
            diagnostics::FETCH_RATE_LIMITED,
            DiagnosticSeverity::Warning,
            "Codex web fixture fetch was rate limited",
            now,
            PROVIDER_ID,
            diagnostics::details(&[("httpStatusClass", Value::from("4xx"))]),
        ));
        return failure(
            ProviderState::ProviderUnavailable,
            diagnostics::FETCH_RATE_LIMITED,
            "Codex web fixture fetch was rate limited",
            now,
            events,
        );
    }

    if !(200..=299).contains(&response.status()) {
        events.push(diagnostics::event(
            diagnostics::FETCH_NONZERO_STATUS,
            DiagnosticSeverity::Warning,
            "Codex web fixture fetch returned an unsuccessful status",
            now,
            PROVIDER_ID,
            diagnostics::details(&[(
                "httpStatusClass",
                Value::from(status_class(response.status())),
            )]),
        ));
        return failure(
            ProviderState::ProviderUnavailable,
            diagnostics::FETCH_NONZERO_STATUS,
            "Codex web fixture fetch returned an unsuccessful status",
            now,
            events,
        );
    }

    let parsed = match parse_dashboard_response(response.body()) {
        Ok(parsed) => parsed,
        Err(()) => {
            events.push(diagnostics::event(
                diagnostics::FETCH_PARSE_ERROR,
                DiagnosticSeverity::Warning,
                "Codex web fixture response could not be parsed",
                now,
                PROVIDER_ID,
                BTreeMap::new(),
            ));
            return failure(
                ProviderState::ParseError,
                diagnostics::FETCH_PARSE_ERROR,
                "Codex web fixture response could not be parsed",
                now,
                events,
            );
        }
    };

    match parsed {
        ParsedDashboard::LoginRequired => {
            events.push(diagnostics::event(
                diagnostics::COOKIE_REJECTED,
                DiagnosticSeverity::Warning,
                "Codex web rejected the supplied browser session material",
                now,
                PROVIDER_ID,
                BTreeMap::new(),
            ));
            failure(
                ProviderState::CookieRejected,
                diagnostics::COOKIE_REJECTED,
                "Codex web rejected the supplied browser session material",
                now,
                events,
            )
        }
        ParsedDashboard::Success(payload) => {
            if account_mismatched(expected_account_email, payload.signed_in_email.as_deref()) {
                events.push(diagnostics::event(
                    diagnostics::ACCOUNT_MISMATCH,
                    DiagnosticSeverity::Warning,
                    "Codex web account identity did not match the expected account",
                    now,
                    PROVIDER_ID,
                    diagnostics::details(&[("accountMismatch", Value::Bool(true))]),
                ));
                return failure(
                    ProviderState::CookieRejected,
                    diagnostics::ACCOUNT_MISMATCH,
                    "Codex web account identity did not match the expected account",
                    now,
                    events,
                );
            }
            events.push(diagnostics::event(
                diagnostics::FETCH_FINISHED,
                DiagnosticSeverity::Info,
                "Codex web fixture fetch finished",
                now,
                PROVIDER_ID,
                BTreeMap::new(),
            ));
            events.push(diagnostics::event(
                diagnostics::FETCH_REDACTION_APPLIED,
                DiagnosticSeverity::Info,
                "Codex web fixture output was normalized with redaction",
                now,
                PROVIDER_ID,
                BTreeMap::new(),
            ));
            let codes = event_codes(&events);
            CodexWebFetchResult {
                provider: success_provider(*payload, now, codes),
                diagnostics: events,
            }
        }
    }
}

fn failure(
    state: ProviderState,
    code: &'static str,
    message: &'static str,
    timestamp: &str,
    mut diagnostics_events: Vec<DiagnosticEvent>,
) -> CodexWebFetchResult {
    if !diagnostics_events
        .iter()
        .any(|event| event.code == diagnostics::FETCH_REDACTION_APPLIED)
    {
        diagnostics_events.push(diagnostics::event(
            diagnostics::FETCH_REDACTION_APPLIED,
            DiagnosticSeverity::Info,
            "Codex web fixture output was normalized with redaction",
            timestamp,
            PROVIDER_ID,
            BTreeMap::new(),
        ));
    }
    let mut codes = event_codes(&diagnostics_events);
    diagnostics::push_code(&mut codes, code);
    CodexWebFetchResult {
        provider: Provider {
            provider: PROVIDER_ID.to_string(),
            display_name: DISPLAY_NAME.to_string(),
            version: None,
            source: SemanticSource::Web,
            source_adapter: SourceAdapter::LinuxWeb,
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
            diagnostic_codes: codes,
        },
        diagnostics: diagnostics_events,
    }
}

fn success_provider(payload: DashboardPayload, timestamp: &str, codes: Vec<String>) -> Provider {
    Provider {
        provider: PROVIDER_ID.to_string(),
        display_name: DISPLAY_NAME.to_string(),
        version: None,
        source: SemanticSource::Web,
        source_adapter: SourceAdapter::LinuxWeb,
        state: ProviderState::Ok,
        updated_at: Some(timestamp.to_string()),
        stale_since: None,
        usage: Usage {
            primary: payload
                .usage
                .as_ref()
                .and_then(|usage| meter_from_payload(usage.session.as_ref(), "Session")),
            secondary: payload
                .usage
                .as_ref()
                .and_then(|usage| meter_from_payload(usage.weekly.as_ref(), "Weekly")),
            tertiary: payload
                .usage
                .as_ref()
                .and_then(|usage| meter_from_payload(usage.code_review.as_ref(), "Code review")),
        },
        credits: payload.credits.map(|credits| Credits {
            remaining: credits.remaining,
            remaining_percent: credits.remaining_percent.map(clamp_percent),
            updated_at: Some(timestamp.to_string()),
            unit: credits.unit,
        }),
        identity: payload.signed_in_email.as_deref().map(|email| Identity {
            provider_account_id_hash: None,
            account_email_display: Some(mask_email_display(email)),
            account_email_hash: Some(local_hash(email)),
            account_organization_display: None,
            account_organization_hash: None,
            login_method: Some("browser_cookie".to_string()),
        }),
        status: Some(ProviderStatus {
            indicator: Some("ok".to_string()),
            description: Some("Codex web fixture normalized".to_string()),
            updated_at: Some(timestamp.to_string()),
            url: None,
        }),
        cost: None,
        dashboard_url: Some(CodexWebPolicy::new().dashboard_url().to_string()),
        diagnostics_summary: None,
        diagnostic_codes: codes,
    }
}

fn parse_dashboard_response(body: &[u8]) -> Result<ParsedDashboard, ()> {
    let text = std::str::from_utf8(body).map_err(|_| ())?;
    redact::validate_public_json_text(text).map_err(|_| ())?;
    let json_text = if text.trim_start().starts_with('{') {
        text.trim()
    } else {
        embedded_fixture_json(text).ok_or(())?
    };
    let payload: DashboardPayload = serde_json::from_str(json_text).map_err(|_| ())?;
    match payload.state.as_deref() {
        Some("ok") | Some("account_mismatch") => Ok(ParsedDashboard::Success(Box::new(payload))),
        Some("login_required") => Ok(ParsedDashboard::LoginRequired),
        _ => Err(()),
    }
}

fn embedded_fixture_json(text: &str) -> Option<&str> {
    let marker = "<script id=\"codexbar-fixture\" type=\"application/json\">";
    let start = text.find(marker)? + marker.len();
    let rest = &text[start..];
    let end = rest.find("</script>")?;
    Some(rest[..end].trim())
}

#[derive(Debug)]
enum ParsedDashboard {
    Success(Box<DashboardPayload>),
    LoginRequired,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DashboardPayload {
    #[serde(rename = "schemaVersion")]
    _schema_version: u8,
    state: Option<String>,
    signed_in_email: Option<String>,
    usage: Option<DashboardUsage>,
    credits: Option<DashboardCredits>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DashboardUsage {
    session: Option<MeterPayload>,
    weekly: Option<MeterPayload>,
    code_review: Option<MeterPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MeterPayload {
    used_percent: Option<f64>,
    remaining_percent: Option<f64>,
    window_minutes: Option<u64>,
    resets_at: Option<String>,
    label: Option<String>,
    detail: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DashboardCredits {
    remaining: Option<f64>,
    remaining_percent: Option<f64>,
    unit: Option<String>,
}

fn meter_from_payload(value: Option<&MeterPayload>, fallback_label: &str) -> Option<Meter> {
    let value = value?;
    let used_percent = value
        .used_percent
        .or_else(|| value.remaining_percent.map(|remaining| 100.0 - remaining))
        .map(clamp_percent);
    let remaining_percent = value
        .remaining_percent
        .or_else(|| used_percent.map(|used| 100.0 - used))
        .map(clamp_percent);
    Some(Meter {
        used_percent,
        remaining_percent,
        window_minutes: value.window_minutes,
        resets_at: value.resets_at.clone(),
        label: Some(
            value
                .label
                .clone()
                .unwrap_or_else(|| fallback_label.to_string()),
        ),
        detail: value.detail.clone(),
    })
}

fn empty_usage() -> Usage {
    Usage {
        primary: None,
        secondary: None,
        tertiary: None,
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

fn status_class(status: u16) -> &'static str {
    match status {
        100..=199 => "1xx",
        200..=299 => "2xx",
        300..=399 => "3xx",
        400..=499 => "4xx",
        500..=599 => "5xx",
        _ => "unknown",
    }
}

fn event_codes(events: &[DiagnosticEvent]) -> Vec<String> {
    let mut codes = Vec::new();
    for event in events {
        if !codes.iter().any(|code| code == &event.code) {
            codes.push(event.code.clone());
        }
    }
    codes
}

fn account_mismatched(expected: Option<&str>, actual: Option<&str>) -> bool {
    match (expected, actual) {
        (Some(expected), Some(actual)) => !expected.eq_ignore_ascii_case(actual),
        _ => false,
    }
}

fn clamp_percent(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}

fn mask_email_display(value: &str) -> String {
    if value.starts_with("[REDACTED_") {
        return "masked-account".to_string();
    }
    let Some((local, domain)) = value.split_once('@') else {
        return "masked-account".to_string();
    };
    let first = local.chars().next().unwrap_or('m');
    format!("{first}***@{domain}")
}

fn local_hash(value: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "codexbar-linux-v1".hash(&mut hasher);
    value.hash(&mut hasher);
    format!("local-hash-{:016x}", hasher.finish())
}
