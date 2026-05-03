use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

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
const MAX_EMBEDDED_JSON_CANDIDATES: usize = 16;
const MAX_INLINE_STATE_SCRIPT_BYTES: usize = 128 * 1024;
const MAX_SAFE_KEY_CLASS_KEYS: usize = 64;
const MAX_SAFE_KEY_CLASS_DEPTH: usize = 6;
const INLINE_JSON_ASSIGNMENT_MARKERS: &[&str] = &[
    "window.__CODEX_DASHBOARD__",
    "window.__CODEXBAR_DASHBOARD__",
    "globalThis.__CODEX_DASHBOARD__",
    "globalThis.__CODEXBAR_DASHBOARD__",
];

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

    let parse_attempt = parse_dashboard_response(response.body());
    let parsed = match parse_attempt.parsed {
        Ok(parsed) => parsed,
        Err(_) => {
            events.push(diagnostics::event(
                diagnostics::FETCH_PARSE_ERROR,
                DiagnosticSeverity::Warning,
                "Codex web response could not be parsed",
                now,
                PROVIDER_ID,
                response_metadata_details_with_parser(
                    &response,
                    &redirect_trace,
                    content_type_class,
                    Some(&parse_attempt.recon),
                ),
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
                response_metadata_details_with_parser(
                    &response,
                    &redirect_trace,
                    content_type_class,
                    Some(&parse_attempt.recon),
                ),
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
                let mut details = response_metadata_details_with_parser(
                    &response,
                    &redirect_trace,
                    content_type_class,
                    Some(&parse_attempt.recon),
                );
                details.insert("accountMismatch".to_string(), Value::Bool(true));
                events.push(diagnostics::event(
                    diagnostics::ACCOUNT_MISMATCH,
                    DiagnosticSeverity::Warning,
                    "Codex web account identity did not match the expected account",
                    now,
                    PROVIDER_ID,
                    details,
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
                response_metadata_details_with_parser(
                    &response,
                    &redirect_trace,
                    content_type_class,
                    Some(&parse_attempt.recon),
                ),
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

struct DashboardParseAttempt {
    parsed: Result<ParsedDashboard, ()>,
    recon: ParserRecon,
}

#[derive(Clone, Debug)]
struct ParserRecon {
    parser_reached: bool,
    html_structure_class: &'static str,
    embedded_json_candidate_count: u64,
    embedded_json_safe_key_classes: String,
    parser_candidate: &'static str,
    parser_failure_class: &'static str,
}

impl ParserRecon {
    fn reached() -> Self {
        Self {
            parser_reached: true,
            html_structure_class: "unknown_html",
            embedded_json_candidate_count: 0,
            embedded_json_safe_key_classes: "none".to_string(),
            parser_candidate: "none",
            parser_failure_class: "no_candidate",
        }
    }

    fn append_details(&self, details: &mut BTreeMap<String, Value>) {
        details.insert(
            "htmlStructureClass".to_string(),
            Value::from(self.html_structure_class),
        );
        details.insert(
            "embeddedJsonCandidateCount".to_string(),
            Value::from(self.embedded_json_candidate_count),
        );
        details.insert(
            "embeddedJsonSafeKeyClasses".to_string(),
            Value::from(self.embedded_json_safe_key_classes.as_str()),
        );
        details.insert(
            "parserCandidate".to_string(),
            Value::from(self.parser_candidate),
        );
        details.insert(
            "parserFailureClass".to_string(),
            Value::from(self.parser_failure_class),
        );
        details.insert(
            "parserReached".to_string(),
            Value::Bool(self.parser_reached),
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParserCandidateKind {
    DirectJson,
    CodexbarFixture,
    NextData,
    ApplicationJsonScript,
    InlineJsonAssignment,
}

impl ParserCandidateKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::DirectJson | Self::CodexbarFixture | Self::ApplicationJsonScript => {
                "application_json_script"
            }
            Self::NextData => "next_data_script",
            Self::InlineJsonAssignment => "inline_state_script",
        }
    }

    fn html_structure_class(self) -> &'static str {
        match self {
            Self::DirectJson
            | Self::CodexbarFixture
            | Self::ApplicationJsonScript
            | Self::InlineJsonAssignment => "script_json",
            Self::NextData => "next_data",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct JsonCandidate<'a> {
    kind: ParserCandidateKind,
    json: &'a str,
}

#[derive(Default)]
struct JsonCandidateSet<'a> {
    candidates: Vec<JsonCandidate<'a>>,
    embedded_json_candidate_count: u64,
    safe_key_classes: BTreeSet<&'static str>,
    candidate_too_large: bool,
    too_many_candidates: bool,
    unsafe_candidate: bool,
}

impl<'a> JsonCandidateSet<'a> {
    fn add(&mut self, kind: ParserCandidateKind, json: &'a str, embedded: bool) {
        if embedded {
            self.embedded_json_candidate_count = self
                .embedded_json_candidate_count
                .saturating_add(1)
                .min(MAX_EMBEDDED_JSON_CANDIDATES as u64);
        }
        if json.len() > MAX_INLINE_STATE_SCRIPT_BYTES {
            self.candidate_too_large = true;
            return;
        }
        if self.candidates.len() >= MAX_EMBEDDED_JSON_CANDIDATES {
            self.too_many_candidates = true;
            return;
        }
        if let Ok(value) = serde_json::from_str::<Value>(json) {
            collect_safe_key_classes(&value, &mut self.safe_key_classes);
            if candidate_redaction_unsafe(&value) {
                self.unsafe_candidate = true;
            }
        }
        self.candidates.push(JsonCandidate { kind, json });
    }
}

fn parse_dashboard_response(body: &[u8]) -> DashboardParseAttempt {
    let mut recon = ParserRecon::reached();
    let text = match std::str::from_utf8(body) {
        Ok(text) => text,
        Err(_) => {
            recon.parser_failure_class = "unsupported_live_shape";
            return DashboardParseAttempt {
                parsed: Err(()),
                recon,
            };
        }
    };
    recon.html_structure_class = html_structure_class(text);

    let candidates = extract_json_candidates(text);
    recon.embedded_json_candidate_count = candidates.embedded_json_candidate_count;
    recon.embedded_json_safe_key_classes = safe_key_classes_text(&candidates.safe_key_classes);
    if candidates.unsafe_candidate {
        recon.parser_failure_class = "candidate_redaction_rejected";
        return DashboardParseAttempt {
            parsed: Err(()),
            recon,
        };
    }
    if candidates.candidates.is_empty() {
        if looks_like_login_required(body) {
            recon.html_structure_class = "login_shell";
            recon.parser_candidate = "html_text_fallback";
            recon.parser_failure_class = "no_candidate";
            return DashboardParseAttempt {
                parsed: Ok(ParsedDashboard::LoginRequired),
                recon,
            };
        }
        recon.parser_failure_class =
            if candidates.too_many_candidates || candidates.candidate_too_large {
                "unsupported_live_shape"
            } else {
                "no_candidate"
            };
        return DashboardParseAttempt {
            parsed: Err(()),
            recon,
        };
    }

    let mut failure_class = if candidates.too_many_candidates || candidates.candidate_too_large {
        "unsupported_live_shape"
    } else {
        "candidate_schema_unknown"
    };
    for candidate in candidates.candidates {
        recon.parser_candidate = candidate.kind.as_str();
        match parse_dashboard_candidate(candidate.json) {
            Ok(parsed) => {
                recon.html_structure_class = candidate.kind.html_structure_class();
                recon.parser_failure_class = "none";
                return DashboardParseAttempt {
                    parsed: Ok(parsed),
                    recon,
                };
            }
            Err(class) => {
                failure_class = class;
            }
        }
    }

    if looks_like_login_required(body) {
        recon.html_structure_class = "login_shell";
        recon.parser_candidate = "html_text_fallback";
        return DashboardParseAttempt {
            parsed: Ok(ParsedDashboard::LoginRequired),
            recon,
        };
    }
    recon.parser_failure_class = failure_class;
    DashboardParseAttempt {
        parsed: Err(()),
        recon,
    }
}

fn parse_dashboard_candidate(json_text: &str) -> Result<ParsedDashboard, &'static str> {
    let value: Value = serde_json::from_str(json_text).map_err(|_| "candidate_not_json")?;
    if candidate_redaction_unsafe(&value) {
        return Err("candidate_redaction_rejected");
    }
    let mut failure_class = "candidate_schema_unknown";
    for candidate in dashboard_payload_candidates(&value) {
        let Ok(payload) = serde_json::from_value::<DashboardPayload>(candidate.clone()) else {
            continue;
        };
        match dashboard_payload_to_parsed(payload) {
            Ok(parsed) => return Ok(parsed),
            Err(class) => failure_class = class,
        }
    }
    Err(failure_class)
}

fn dashboard_payload_to_parsed(payload: DashboardPayload) -> Result<ParsedDashboard, &'static str> {
    if payload.schema_version != 1 {
        return Err("candidate_schema_unknown");
    }
    match payload.state.as_deref() {
        Some("ok") | Some("account_mismatch") if payload_has_usage_fields(&payload) => {
            Ok(ParsedDashboard::Success(Box::new(payload)))
        }
        Some("ok") | Some("account_mismatch") => Err("candidate_missing_usage_fields"),
        Some("login_required") => Ok(ParsedDashboard::LoginRequired),
        _ => Err("candidate_schema_unknown"),
    }
}

fn payload_has_usage_fields(payload: &DashboardPayload) -> bool {
    payload.credits.is_some()
        || payload.usage.as_ref().is_some_and(|usage| {
            usage.session.is_some() || usage.weekly.is_some() || usage.code_review.is_some()
        })
}

fn dashboard_payload_candidates(value: &Value) -> Vec<&Value> {
    let mut candidates = vec![value];
    for path in [
        &["codexbarDashboard"][..],
        &["codexUsage"][..],
        &["dashboard"][..],
        &["data", "codexbarDashboard"][..],
        &["props", "pageProps", "codexbarDashboard"][..],
        &["props", "pageProps", "codexUsage"][..],
        &["props", "pageProps", "dashboard"][..],
        &["props", "pageProps", "usageDashboard"][..],
        &["props", "pageProps", "initialData", "codexbarDashboard"][..],
    ] {
        if let Some(candidate) = value_at_path(value, path) {
            candidates.push(candidate);
        }
    }
    candidates
}

fn value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.as_object()?.get(*key)?;
    }
    Some(current)
}

fn extract_json_candidates(text: &str) -> JsonCandidateSet<'_> {
    let mut set = JsonCandidateSet::default();
    if text.trim_start().starts_with('{') {
        set.add(ParserCandidateKind::DirectJson, text.trim(), false);
    }
    collect_script_json_candidates(text, &mut set);
    set
}

fn collect_script_json_candidates<'a>(text: &'a str, set: &mut JsonCandidateSet<'a>) {
    let lower = text.to_ascii_lowercase();
    let mut offset = 0;
    while let Some(script_start_relative) = lower[offset..].find("<script") {
        let script_start = offset + script_start_relative;
        let Some(tag_end_relative) = lower[script_start..].find('>') else {
            break;
        };
        let tag_end = script_start + tag_end_relative;
        let content_start = tag_end + 1;
        let Some(close_relative) = lower[content_start..].find("</script>") else {
            break;
        };
        let content_end = content_start + close_relative;
        let tag = &lower[script_start..=tag_end];
        let content = text[content_start..content_end].trim();

        if script_tag_has_attr_value(tag, "id", "codexbar-fixture") {
            set.add(ParserCandidateKind::CodexbarFixture, content, true);
        } else if script_tag_has_attr_value(tag, "id", "__next_data__") {
            set.add(ParserCandidateKind::NextData, content, true);
        } else if script_tag_attr_contains(tag, "type", "application/json") {
            set.add(ParserCandidateKind::ApplicationJsonScript, content, true);
        }

        if content.len() <= MAX_INLINE_STATE_SCRIPT_BYTES {
            for marker in INLINE_JSON_ASSIGNMENT_MARKERS {
                if let Some(json) = inline_assignment_json(content, marker) {
                    set.add(ParserCandidateKind::InlineJsonAssignment, json, true);
                }
            }
        }

        offset = content_end + "</script>".len();
    }
}

fn inline_assignment_json<'a>(content: &'a str, marker: &str) -> Option<&'a str> {
    let marker_start = content.find(marker)?;
    let after_marker = marker_start + marker.len();
    let equals_relative = content[after_marker..].find('=')?;
    let after_equals = after_marker + equals_relative + 1;
    let whitespace = content[after_equals..]
        .find(|ch: char| !ch.is_whitespace())
        .unwrap_or(0);
    balanced_json_slice(content, after_equals + whitespace)
}

fn balanced_json_slice(text: &str, start: usize) -> Option<&str> {
    let open = text[start..].chars().next()?;
    let close = match open {
        '{' => '}',
        '[' => ']',
        _ => return None,
    };
    let mut depth = 0_u32;
    let mut in_string = false;
    let mut escaped = false;
    for (relative, ch) in text[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
        } else if ch == open {
            depth = depth.saturating_add(1);
        } else if ch == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                let end = start + relative + ch.len_utf8();
                return Some(&text[start..end]);
            }
        }
    }
    None
}

fn script_tag_has_attr_value(tag: &str, attr: &str, value: &str) -> bool {
    script_tag_attr_contains(tag, attr, value)
}

fn script_tag_attr_contains(tag: &str, attr: &str, needle: &str) -> bool {
    for quote in ['"', '\''] {
        let marker = format!("{attr}={quote}");
        let mut offset = 0;
        while let Some(relative) = tag[offset..].find(&marker) {
            let value_start = offset + relative + marker.len();
            let Some(value_end_relative) = tag[value_start..].find(quote) else {
                return false;
            };
            let value = &tag[value_start..value_start + value_end_relative];
            if value.contains(needle) {
                return true;
            }
            offset = value_start + value_end_relative + quote.len_utf8();
        }
    }
    false
}

fn html_structure_class(text: &str) -> &'static str {
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') {
        return "script_json";
    }
    let lower = text.to_ascii_lowercase();
    if looks_like_login_required(text.as_bytes()) {
        "login_shell"
    } else if lower.contains("id=\"__next_data__\"") || lower.contains("id='__next_data__'") {
        "next_data"
    } else if lower.contains("id=\"codexbar-fixture\"")
        || lower.contains("id='codexbar-fixture'")
        || lower.contains("__codex_dashboard__")
        || lower.contains("__codexbar_dashboard__")
        || lower.contains("type=\"application/json\"")
        || lower.contains("type='application/json'")
    {
        "script_json"
    } else if lower.contains("static-app-shell")
        || lower.contains("id=\"app\"")
        || lower.contains("id='app'")
        || lower.contains("id=\"app-root\"")
        || lower.contains("id='app-root'")
    {
        "static_app_shell"
    } else if lower.contains("error")
        || lower.contains("unavailable")
        || lower.contains("something went wrong")
    {
        "error_page"
    } else {
        "unknown_html"
    }
}

fn collect_safe_key_classes(value: &Value, classes: &mut BTreeSet<&'static str>) {
    let mut visited = 0_usize;
    collect_safe_key_classes_inner(value, classes, 0, &mut visited);
}

fn collect_safe_key_classes_inner(
    value: &Value,
    classes: &mut BTreeSet<&'static str>,
    depth: usize,
    visited: &mut usize,
) {
    if depth > MAX_SAFE_KEY_CLASS_DEPTH || *visited >= MAX_SAFE_KEY_CLASS_KEYS {
        return;
    }
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if *visited >= MAX_SAFE_KEY_CLASS_KEYS {
                    return;
                }
                *visited += 1;
                classes.insert(safe_key_class(key));
                collect_safe_key_classes_inner(value, classes, depth + 1, visited);
            }
        }
        Value::Array(items) => {
            for item in items.iter().take(4) {
                collect_safe_key_classes_inner(item, classes, depth + 1, visited);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn safe_key_class(key: &str) -> &'static str {
    let normalized: String = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    match normalized.as_str() {
        "schemaversion" | "state" => "unknown",
        "props" | "pageprops" | "initialdata" | "buildid" | "page" | "query" | "runtimeconfig"
        | "scriptloader" | "gsp" | "nextdata" | "route" | "path" => "route",
        "codexbardashboard" | "codexusage" | "dashboard" | "usagedashboard" => "usage",
        "usage" | "session" | "weekly" | "codereview" | "usedpercent" | "remainingpercent"
        | "windowminutes" | "resetsat" | "label" | "detail" => "usage",
        "credits" | "remaining" | "unit" => "credits",
        "quota" | "limit" | "limits" | "reset" | "resets" => "quota",
        "signedinemail" | "account" | "identity" | "user" | "organization" | "workspace" => {
            "account"
        }
        "billing" | "subscription" | "invoice" | "plan" => "billing",
        "features" | "featureflags" | "featureflag" | "flags" => "featureFlags",
        _ => "unknown",
    }
}

fn candidate_redaction_unsafe(value: &Value) -> bool {
    candidate_redaction_unsafe_inner(value, 0, &mut 0)
}

fn candidate_redaction_unsafe_inner(value: &Value, depth: usize, visited: &mut usize) -> bool {
    if depth > MAX_SAFE_KEY_CLASS_DEPTH || *visited >= MAX_SAFE_KEY_CLASS_KEYS {
        return false;
    }
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                *visited += 1;
                if unsafe_candidate_key(key)
                    || candidate_redaction_unsafe_inner(value, depth + 1, visited)
                {
                    return true;
                }
            }
            false
        }
        Value::Array(items) => items
            .iter()
            .take(8)
            .any(|item| candidate_redaction_unsafe_inner(item, depth + 1, visited)),
        Value::String(value) => redact::validate_public_json_text(value).is_err(),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn unsafe_candidate_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    matches!(
        normalized.as_str(),
        "authorization"
            | "cookie"
            | "cookies"
            | "setcookie"
            | "headers"
            | "requestheaders"
            | "responseheaders"
            | "raw"
            | "rawpayload"
            | "rawresponse"
            | "rawbody"
            | "rawheader"
            | "rawcookie"
            | "rawurl"
            | "rawpath"
            | "rawquery"
            | "rawfragment"
            | "rawlocation"
            | "accesstoken"
            | "refreshtoken"
            | "sessiontoken"
            | "sessionkey"
            | "sessionid"
            | "sid"
            | "apikey"
            | "secret"
            | "password"
            | "provideraccountid"
            | "organizationid"
            | "workspaceid"
    )
}

fn safe_key_classes_text(classes: &BTreeSet<&'static str>) -> String {
    if classes.is_empty() {
        "none".to_string()
    } else {
        classes.iter().copied().collect::<Vec<_>>().join(",")
    }
}

fn looks_like_login_required(body: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(body) else {
        return false;
    };
    let lower = text.to_ascii_lowercase();
    (lower.contains("login_required")
        || lower.contains("/auth/login")
        || lower.contains("log in")
        || lower.contains("sign in"))
        && (lower.contains("chatgpt") || lower.contains("openai") || lower.contains("codex"))
}

enum ParsedDashboard {
    Success(Box<DashboardPayload>),
    LoginRequired,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DashboardPayload {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
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
    response_metadata_details_with_parser(response, redirect_trace, content_type_class, None)
}

fn response_metadata_details_with_parser(
    response: &WebResponse,
    redirect_trace: &RedirectTrace,
    content_type_class: &'static str,
    parser_recon: Option<&ParserRecon>,
) -> BTreeMap<String, Value> {
    let initial_response = redirect_trace.initial_response(response);
    let final_status = redirect_trace.final_status(response);
    let mut details = diagnostics::details(&[
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
    ]);
    if let Some(parser_recon) = parser_recon {
        parser_recon.append_details(&mut details);
    }
    details
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
