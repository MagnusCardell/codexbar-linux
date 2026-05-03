use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

use crate::browser;
use crate::browser::session_material::SessionMaterial;
use crate::model::{
    Credits, DiagnosticEvent, DiagnosticSeverity, Identity, Meter, Provider, ProviderState,
    ProviderStatus, SemanticSource, SourceAdapter, Usage,
};
use crate::redact;
use crate::web::client::{WebClient, WebClientError, WebRequest, WebResponse};
use crate::web::diagnostics;
use crate::web::policy::{CodexWebPolicy, RedirectTargetClass, RedirectTargetSummary};

pub const PROVIDER_ID: &str = "codex";
pub const DISPLAY_NAME: &str = "Codex";

#[derive(Clone, Debug)]
pub struct CodexWebFetchResult {
    pub provider: Provider,
    pub diagnostics: Vec<DiagnosticEvent>,
}

pub async fn fetch_dashboard_with_client<C>(
    client: &C,
    session: Option<&SessionMaterial>,
    now: &str,
    expected_account_email: Option<&str>,
) -> CodexWebFetchResult
where
    C: WebClient,
{
    fetch_dashboard_with_client_with_session_codes(
        client,
        session,
        now,
        expected_account_email,
        &[],
    )
    .await
}

pub async fn fetch_dashboard_with_client_with_session_codes<C>(
    client: &C,
    session: Option<&SessionMaterial>,
    now: &str,
    expected_account_email: Option<&str>,
    session_diagnostic_codes: &[String],
) -> CodexWebFetchResult
where
    C: WebClient,
{
    let policy = CodexWebPolicy::new();
    let preflight_codes = unique_string_codes(session_diagnostic_codes);
    let Some(material) =
        session.filter(|material| material.provider() == PROVIDER_ID && !material.is_empty())
    else {
        return failure_with_extra_codes(
            state_from_session_codes(&preflight_codes),
            diagnostics::COOKIE_ABSENT,
            "Browser session material was not available for Codex web",
            now,
            Vec::new(),
            &preflight_codes,
        );
    };
    let Some(session_header) = material
        .cookie_header_for_url(policy.dashboard_url())
        .ok()
        .flatten()
    else {
        return failure_with_extra_codes(
            state_from_session_codes(&preflight_codes),
            diagnostics::COOKIE_ABSENT,
            "Browser session material was not available for Codex web",
            now,
            Vec::new(),
            &preflight_codes,
        );
    };

    let request = policy
        .dashboard_request()
        .with_session_header(session_header.clone());
    let mut events = vec![diagnostics::event(
        diagnostics::FETCH_STARTED,
        DiagnosticSeverity::Info,
        "Codex web fetch started",
        now,
        PROVIDER_ID,
        request_metadata_details(&request),
    )];
    let response = match client.request(request).await {
        Ok(response) => response,
        Err(WebClientError::Timeout) => {
            events.push(diagnostics::event(
                diagnostics::FETCH_TIMEOUT,
                DiagnosticSeverity::Warning,
                "Codex web fetch timed out",
                now,
                PROVIDER_ID,
                BTreeMap::new(),
            ));
            return failure_with_extra_codes(
                ProviderState::Timeout,
                diagnostics::FETCH_TIMEOUT,
                "Codex web fetch timed out",
                now,
                events,
                &preflight_codes,
            );
        }
        Err(WebClientError::ResponseTooLarge) => {
            events.push(diagnostics::event(
                diagnostics::RESPONSE_TOO_LARGE,
                DiagnosticSeverity::Warning,
                "Codex web response exceeded the configured size limit",
                now,
                PROVIDER_ID,
                BTreeMap::new(),
            ));
            return failure_with_extra_codes(
                ProviderState::ParseError,
                diagnostics::RESPONSE_TOO_LARGE,
                "Codex web response exceeded the configured size limit",
                now,
                events,
                &preflight_codes,
            );
        }
        Err(WebClientError::TransportUnavailable) => {
            events.push(diagnostics::event(
                diagnostics::FETCH_FINISHED,
                DiagnosticSeverity::Warning,
                "Codex web fetch failed safely",
                now,
                PROVIDER_ID,
                BTreeMap::new(),
            ));
            return failure_with_extra_codes(
                ProviderState::ProviderUnavailable,
                diagnostics::FETCH_FINISHED,
                "Codex web fetch failed safely",
                now,
                events,
                &preflight_codes,
            );
        }
    };

    let mut response = response;
    let mut redirect_trace = RedirectTrace::new(&policy, &response);
    let mut content_type_class = response_content_type_class(response.content_type());
    let final_url_allowed = policy.validate_dashboard_url(response.final_url()).is_ok();
    if response.redirect_invalid()
        || !final_url_allowed
        || redirect_target_policy_failed(redirect_trace.target_class)
    {
        let mut details = response_metadata_details(&response, &redirect_trace, content_type_class);
        details.insert("redirectBlocked".to_string(), Value::Bool(true));
        events.push(diagnostics::event(
            diagnostics::REDIRECT_BLOCKED,
            DiagnosticSeverity::Warning,
            "Codex web redirect target was blocked by policy",
            now,
            PROVIDER_ID,
            details,
        ));
        return failure_with_extra_codes(
            ProviderState::ProviderUnavailable,
            diagnostics::REDIRECT_BLOCKED,
            "Codex web redirect target was blocked by policy",
            now,
            events,
            &preflight_codes,
        );
    }

    if redirect_trace.target_class == RedirectTargetClass::SameHostLoginPath {
        if !(200..=299).contains(&response.status()) {
            events.push(diagnostics::event(
                diagnostics::FETCH_NONZERO_STATUS,
                DiagnosticSeverity::Warning,
                "Codex web fetch returned an authentication redirect status",
                now,
                PROVIDER_ID,
                response_metadata_details(&response, &redirect_trace, content_type_class),
            ));
        }
        events.push(diagnostics::event(
            diagnostics::COOKIE_REJECTED,
            DiagnosticSeverity::Warning,
            "Codex web redirected to an authentication flow",
            now,
            PROVIDER_ID,
            BTreeMap::new(),
        ));
        return failure_with_extra_codes(
            ProviderState::CookieRejected,
            diagnostics::COOKIE_REJECTED,
            "Codex web rejected the supplied browser session material",
            now,
            events,
            &preflight_codes,
        );
    }

    if (300..=399).contains(&response.status()) {
        if let Some(redirect_url) = response
            .redirect_url()
            .filter(|url| policy.should_follow_redirect(url))
        {
            let Some(follow_session_header) =
                material.cookie_header_for_url(redirect_url).ok().flatten()
            else {
                return failure_with_extra_codes(
                    state_from_session_codes(&preflight_codes),
                    diagnostics::COOKIE_ABSENT,
                    "Browser session material was not available for Codex web redirect",
                    now,
                    events,
                    &preflight_codes,
                );
            };
            let follow_request = match policy.redirect_follow_request(redirect_url) {
                Ok(request) => request.with_session_header(follow_session_header),
                Err(_) => {
                    let mut details =
                        response_metadata_details(&response, &redirect_trace, content_type_class);
                    details.insert("redirectBlocked".to_string(), Value::Bool(true));
                    events.push(diagnostics::event(
                        diagnostics::REDIRECT_BLOCKED,
                        DiagnosticSeverity::Warning,
                        "Codex web redirect was not followed by policy",
                        now,
                        PROVIDER_ID,
                        details,
                    ));
                    return failure_with_extra_codes(
                        ProviderState::ProviderUnavailable,
                        diagnostics::REDIRECT_BLOCKED,
                        "Codex web redirect was not followed by policy",
                        now,
                        events,
                        &preflight_codes,
                    );
                }
            };
            let initial_response = response.clone();
            response = match client.request(follow_request).await {
                Ok(response) => response,
                Err(WebClientError::Timeout) => {
                    events.push(diagnostics::event(
                        diagnostics::FETCH_TIMEOUT,
                        DiagnosticSeverity::Warning,
                        "Codex web fetch timed out",
                        now,
                        PROVIDER_ID,
                        BTreeMap::new(),
                    ));
                    return failure_with_extra_codes(
                        ProviderState::Timeout,
                        diagnostics::FETCH_TIMEOUT,
                        "Codex web fetch timed out",
                        now,
                        events,
                        &preflight_codes,
                    );
                }
                Err(WebClientError::ResponseTooLarge) => {
                    events.push(diagnostics::event(
                        diagnostics::RESPONSE_TOO_LARGE,
                        DiagnosticSeverity::Warning,
                        "Codex web response exceeded the configured size limit",
                        now,
                        PROVIDER_ID,
                        BTreeMap::new(),
                    ));
                    return failure_with_extra_codes(
                        ProviderState::ParseError,
                        diagnostics::RESPONSE_TOO_LARGE,
                        "Codex web response exceeded the configured size limit",
                        now,
                        events,
                        &preflight_codes,
                    );
                }
                Err(WebClientError::TransportUnavailable) => {
                    events.push(diagnostics::event(
                        diagnostics::FETCH_FINISHED,
                        DiagnosticSeverity::Warning,
                        "Codex web fetch failed safely",
                        now,
                        PROVIDER_ID,
                        BTreeMap::new(),
                    ));
                    return failure_with_extra_codes(
                        ProviderState::ProviderUnavailable,
                        diagnostics::FETCH_FINISHED,
                        "Codex web fetch failed safely",
                        now,
                        events,
                        &preflight_codes,
                    );
                }
            };
            redirect_trace =
                RedirectTrace::followed(initial_response, redirect_trace.target_summary);
            content_type_class = response_content_type_class(response.content_type());
            let final_url_allowed = policy
                .validate_follow_response_url(response.final_url())
                .is_ok();
            if response.redirect_invalid()
                || !final_url_allowed
                || response.redirect_present()
                || (300..=399).contains(&response.status())
            {
                let mut details =
                    response_metadata_details(&response, &redirect_trace, content_type_class);
                details.insert("redirectBlocked".to_string(), Value::Bool(true));
                events.push(diagnostics::event(
                    diagnostics::REDIRECT_BLOCKED,
                    DiagnosticSeverity::Warning,
                    "Codex web redirect was not followed beyond one hop",
                    now,
                    PROVIDER_ID,
                    details,
                ));
                return failure_with_extra_codes(
                    ProviderState::ProviderUnavailable,
                    diagnostics::REDIRECT_BLOCKED,
                    "Codex web redirect was not followed beyond one hop",
                    now,
                    events,
                    &preflight_codes,
                );
            }
        } else {
            let mut details =
                response_metadata_details(&response, &redirect_trace, content_type_class);
            details.insert("redirectBlocked".to_string(), Value::Bool(true));
            events.push(diagnostics::event(
                diagnostics::REDIRECT_BLOCKED,
                DiagnosticSeverity::Warning,
                "Codex web redirect was not followed by policy",
                now,
                PROVIDER_ID,
                details,
            ));
            return failure_with_extra_codes(
                ProviderState::ProviderUnavailable,
                diagnostics::REDIRECT_BLOCKED,
                "Codex web redirect was not followed by policy",
                now,
                events,
                &preflight_codes,
            );
        }
    }

    if response.body().len() > policy.response_size_limit() {
        events.push(diagnostics::event(
            diagnostics::RESPONSE_TOO_LARGE,
            DiagnosticSeverity::Warning,
            "Codex web response exceeded the configured size limit",
            now,
            PROVIDER_ID,
            response_metadata_details(&response, &redirect_trace, content_type_class),
        ));
        return failure_with_extra_codes(
            ProviderState::ParseError,
            diagnostics::RESPONSE_TOO_LARGE,
            "Codex web fixture response exceeded the configured size limit",
            now,
            events,
            &preflight_codes,
        );
    }

    if response.status() == 429 {
        events.push(diagnostics::event(
            diagnostics::FETCH_RATE_LIMITED,
            DiagnosticSeverity::Warning,
            "Codex web fetch was rate limited",
            now,
            PROVIDER_ID,
            response_metadata_details(&response, &redirect_trace, content_type_class),
        ));
        return failure_with_extra_codes(
            ProviderState::ProviderUnavailable,
            diagnostics::FETCH_RATE_LIMITED,
            "Codex web fetch was rate limited",
            now,
            events,
            &preflight_codes,
        );
    }

    if provider_rejected_status(response.status()) {
        events.push(diagnostics::event(
            diagnostics::FETCH_NONZERO_STATUS,
            DiagnosticSeverity::Warning,
            "Codex web fetch returned an authentication rejection status",
            now,
            PROVIDER_ID,
            response_metadata_details(&response, &redirect_trace, content_type_class),
        ));
        events.push(diagnostics::event(
            diagnostics::COOKIE_REJECTED,
            DiagnosticSeverity::Warning,
            "Codex web rejected the supplied browser session material",
            now,
            PROVIDER_ID,
            BTreeMap::new(),
        ));
        return failure_with_extra_codes(
            ProviderState::CookieRejected,
            diagnostics::COOKIE_REJECTED,
            "Codex web rejected the supplied browser session material",
            now,
            events,
            &preflight_codes,
        );
    }

    if !(200..=299).contains(&response.status()) {
        events.push(diagnostics::event(
            diagnostics::FETCH_NONZERO_STATUS,
            DiagnosticSeverity::Warning,
            "Codex web fetch returned an unsuccessful status",
            now,
            PROVIDER_ID,
            response_metadata_details(&response, &redirect_trace, content_type_class),
        ));
        return failure_with_extra_codes(
            ProviderState::ProviderUnavailable,
            diagnostics::FETCH_NONZERO_STATUS,
            "Codex web fetch returned an unsuccessful status",
            now,
            events,
            &preflight_codes,
        );
    }

    if !content_type_allowed(content_type_class) {
        events.push(diagnostics::event(
            diagnostics::FETCH_PARSE_ERROR,
            DiagnosticSeverity::Warning,
            "Codex web response content type was not supported",
            now,
            PROVIDER_ID,
            response_metadata_details(&response, &redirect_trace, content_type_class),
        ));
        return failure_with_extra_codes(
            ProviderState::ParseError,
            diagnostics::FETCH_PARSE_ERROR,
            "Codex web response content type was not supported",
            now,
            events,
            &preflight_codes,
        );
    }

    let parsed = match parse_dashboard_response(response.body()) {
        Ok(parsed) => parsed,
        Err(()) if looks_like_login_required(response.body()) => ParsedDashboard::LoginRequired,
        Err(()) => {
            events.push(diagnostics::event(
                diagnostics::FETCH_PARSE_ERROR,
                DiagnosticSeverity::Warning,
                "Codex web response could not be parsed",
                now,
                PROVIDER_ID,
                response_metadata_details(&response, &redirect_trace, content_type_class),
            ));
            return failure_with_extra_codes(
                ProviderState::ParseError,
                diagnostics::FETCH_PARSE_ERROR,
                "Codex web response could not be parsed",
                now,
                events,
                &preflight_codes,
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
            failure_with_extra_codes(
                ProviderState::CookieRejected,
                diagnostics::COOKIE_REJECTED,
                "Codex web rejected the supplied browser session material",
                now,
                events,
                &preflight_codes,
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
                return failure_with_extra_codes(
                    ProviderState::CookieRejected,
                    diagnostics::ACCOUNT_MISMATCH,
                    "Codex web account identity did not match the expected account",
                    now,
                    events,
                    &preflight_codes,
                );
            }
            events.push(diagnostics::event(
                diagnostics::FETCH_FINISHED,
                DiagnosticSeverity::Info,
                "Codex web fetch finished",
                now,
                PROVIDER_ID,
                response_metadata_details(&response, &redirect_trace, content_type_class),
            ));
            events.push(diagnostics::event(
                diagnostics::FETCH_REDACTION_APPLIED,
                DiagnosticSeverity::Info,
                "Codex web output was normalized with redaction",
                now,
                PROVIDER_ID,
                BTreeMap::new(),
            ));
            let codes = event_codes(&events);
            CodexWebFetchResult {
                provider: success_provider(*payload, now, merged_codes(codes, &preflight_codes)),
                diagnostics: events,
            }
        }
    }
}

fn failure_with_extra_codes(
    state: ProviderState,
    code: &'static str,
    message: &'static str,
    timestamp: &str,
    mut diagnostics_events: Vec<DiagnosticEvent>,
    extra_codes: &[String],
) -> CodexWebFetchResult {
    if !diagnostics_events
        .iter()
        .any(|event| event.code == diagnostics::FETCH_REDACTION_APPLIED)
    {
        diagnostics_events.push(diagnostics::event(
            diagnostics::FETCH_REDACTION_APPLIED,
            DiagnosticSeverity::Info,
            "Codex web output was normalized with redaction",
            timestamp,
            PROVIDER_ID,
            BTreeMap::new(),
        ));
    }
    let mut codes = merged_codes(event_codes(&diagnostics_events), extra_codes);
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
            account_email_hash: None,
            account_organization_display: None,
            account_organization_hash: None,
            login_method: Some("browser_cookie".to_string()),
        }),
        status: Some(ProviderStatus {
            indicator: Some("ok".to_string()),
            description: Some("Codex web normalized".to_string()),
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

fn looks_like_login_required(body: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(body) else {
        return false;
    };
    let lower = text.to_ascii_lowercase();
    (lower.contains("login_required") || lower.contains("/auth/login") || lower.contains("log in"))
        && (lower.contains("chatgpt") || lower.contains("openai"))
}

enum ParsedDashboard {
    Success(Box<DashboardPayload>),
    LoginRequired,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DashboardPayload {
    #[serde(rename = "schemaVersion")]
    _schema_version: u8,
    state: Option<String>,
    signed_in_email: Option<String>,
    usage: Option<DashboardUsage>,
    credits: Option<DashboardCredits>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DashboardUsage {
    session: Option<MeterPayload>,
    weekly: Option<MeterPayload>,
    code_review: Option<MeterPayload>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MeterPayload {
    used_percent: Option<f64>,
    remaining_percent: Option<f64>,
    window_minutes: Option<u64>,
    resets_at: Option<String>,
    label: Option<String>,
    detail: Option<String>,
}

#[derive(Deserialize)]
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
        100..=199 => "informational",
        200..=299 => "success",
        300..=399 => "redirect",
        400..=499 => "client_error",
        500..=599 => "server_error",
        _ => "unknown",
    }
}

fn provider_rejected_status(status: u16) -> bool {
    matches!(status, 401 | 403)
}

fn request_metadata_details(request: &WebRequest) -> BTreeMap<String, Value> {
    diagnostics::details(&[(
        "requestHeaderProfile",
        Value::from(request.request_header_profile()),
    )])
}

#[derive(Clone, Debug)]
struct RedirectTrace {
    initial_response: Option<WebResponse>,
    target_summary: RedirectTargetSummary,
    target_class: RedirectTargetClass,
    followed: bool,
    hop_count: u64,
}

impl RedirectTrace {
    fn new(policy: &CodexWebPolicy, response: &WebResponse) -> Self {
        let target_summary = policy
            .classify_redirect_target_summary(response.redirect_url(), response.redirect_invalid());
        Self {
            initial_response: None,
            target_summary,
            target_class: target_summary.target_class(),
            followed: false,
            hop_count: 0,
        }
    }

    fn followed(initial_response: WebResponse, target_summary: RedirectTargetSummary) -> Self {
        Self {
            initial_response: Some(initial_response),
            target_summary,
            target_class: target_summary.target_class(),
            followed: true,
            hop_count: 1,
        }
    }

    fn initial_response<'a>(&'a self, response: &'a WebResponse) -> &'a WebResponse {
        self.initial_response.as_ref().unwrap_or(response)
    }

    fn final_status(&self, response: &WebResponse) -> Option<u16> {
        self.followed.then_some(response.status())
    }
}

fn redirect_target_policy_failed(target_class: RedirectTargetClass) -> bool {
    matches!(
        target_class,
        RedirectTargetClass::Invalid
            | RedirectTargetClass::BlockedHost
            | RedirectTargetClass::SameHostOther
            | RedirectTargetClass::AllowedHostOther
    )
}

fn response_metadata_details(
    response: &WebResponse,
    redirect_trace: &RedirectTrace,
    content_type_class: &'static str,
) -> BTreeMap<String, Value> {
    let initial_response = redirect_trace.initial_response(response);
    let final_status = redirect_trace.final_status(response);
    diagnostics::details(&[
        (
            "httpStatusCode",
            Value::from(initial_response.status() as u64),
        ),
        (
            "httpStatusClass",
            Value::from(status_class(initial_response.status())),
        ),
        (
            "redirectPresent",
            Value::Bool(initial_response.redirect_present()),
        ),
        (
            "redirectHostClass",
            Value::from(redirect_host_class(
                initial_response,
                redirect_trace.target_class,
            )),
        ),
        (
            "redirectTargetClass",
            Value::from(redirect_trace.target_summary.target_class_str()),
        ),
        (
            "redirectPathFamily",
            Value::from(redirect_trace.target_summary.path_family_str()),
        ),
        (
            "redirectPathDepth",
            Value::from(redirect_trace.target_summary.path_depth_str()),
        ),
        (
            "redirectQueryClass",
            Value::from(redirect_trace.target_summary.query_class_str()),
        ),
        (
            "redirectCanFollow",
            Value::Bool(redirect_trace.target_summary.can_follow()),
        ),
        ("redirectFollowed", Value::Bool(redirect_trace.followed)),
        ("redirectHopCount", Value::from(redirect_trace.hop_count)),
        (
            "finalHttpStatusCode",
            final_status.map_or(Value::Null, |status| Value::from(status as u64)),
        ),
        (
            "finalHttpStatusClass",
            Value::from(final_status.map_or("none", status_class)),
        ),
        ("contentTypeClass", Value::from(content_type_class)),
        (
            "responseBodyClass",
            Value::from(response_body_class(response)),
        ),
        (
            "responseSizeBucket",
            Value::from(response_size_bucket(response)),
        ),
    ])
}

fn redirect_host_class(response: &WebResponse, target_class: RedirectTargetClass) -> &'static str {
    if response.redirect_invalid() {
        "invalid"
    } else if response.redirect_url().is_some() {
        match target_class {
            RedirectTargetClass::SameHostCanonical
            | RedirectTargetClass::SameHostUsagePath
            | RedirectTargetClass::SameHostLoginPath
            | RedirectTargetClass::SameHostOther
            | RedirectTargetClass::AllowedHostOther => "allowed",
            RedirectTargetClass::BlockedHost => "blocked",
            RedirectTargetClass::Invalid => "invalid",
            RedirectTargetClass::None => "none",
        }
    } else if (300..=399).contains(&response.status()) {
        "missing"
    } else {
        "none"
    }
}

fn response_body_class(response: &WebResponse) -> &'static str {
    if response.body().len() > CodexWebPolicy::new().response_size_limit() {
        "too_large"
    } else if response.body().is_empty() {
        "empty"
    } else if std::str::from_utf8(response.body()).is_err() {
        "invalid_encoding"
    } else {
        "within_cap"
    }
}

fn response_size_bucket(response: &WebResponse) -> &'static str {
    let bytes = response.body().len();
    if bytes > CodexWebPolicy::new().response_size_limit() {
        "capped"
    } else if bytes == 0 {
        "zero"
    } else if bytes <= 16 * 1024 {
        "small"
    } else if bytes <= 128 * 1024 {
        "medium"
    } else {
        "large"
    }
}

fn response_content_type_class(content_type: Option<&str>) -> &'static str {
    let Some(content_type) = content_type else {
        return "missing";
    };
    let lower = content_type.to_ascii_lowercase();
    if lower.contains("text/html") {
        "html"
    } else if lower.contains("application/json") || lower.ends_with("+json") {
        "json"
    } else if lower.starts_with("text/") {
        "text"
    } else {
        "other"
    }
}

fn content_type_allowed(class: &str) -> bool {
    matches!(class, "missing" | "html" | "json")
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

fn merged_codes(mut codes: Vec<String>, extra_codes: &[String]) -> Vec<String> {
    for code in extra_codes {
        if !codes.iter().any(|existing| existing == code) {
            codes.push(code.clone());
        }
    }
    codes
}

fn unique_string_codes(codes: &[String]) -> Vec<String> {
    let mut unique = Vec::new();
    for code in codes {
        if !unique.iter().any(|existing| existing == code) {
            unique.push(code.clone());
        }
    }
    unique
}

fn state_from_session_codes(codes: &[String]) -> ProviderState {
    if codes
        .iter()
        .any(|code| code == browser::diagnostics::COOKIE_DB_LOCKED)
    {
        ProviderState::ProviderUnavailable
    } else if codes
        .iter()
        .any(|code| code == browser::diagnostics::COOKIE_DB_SCHEMA_UNSUPPORTED)
    {
        ProviderState::ParseError
    } else if codes.iter().any(|code| {
        matches!(
            code.as_str(),
            browser::diagnostics::COOKIE_DB_UNREADABLE
                | browser::diagnostics::COOKIE_DECRYPTION_UNAVAILABLE
                | browser::diagnostics::COOKIE_DECRYPTION_FAILED
                | browser::diagnostics::KEYRING_UNAVAILABLE
                | browser::diagnostics::KEYRING_LOCKED
                | browser::diagnostics::KEYRING_PROMPT_REQUIRED
                | browser::diagnostics::LIVE_PROFILES_DISABLED
                | browser::diagnostics::NOT_FOUND
                | browser::diagnostics::PROFILE_NOT_FOUND
                | browser::diagnostics::PROFILE_UNREADABLE
        )
    }) {
        ProviderState::MissingDependency
    } else {
        ProviderState::Unauthenticated
    }
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
