use std::collections::BTreeMap;

use serde_json::Value;

use crate::model::{DiagnosticEvent, DiagnosticSeverity, EventRedaction, SourceAdapter};

pub const FETCH_STARTED: &str = "provider_web_fetch_started";
pub const FETCH_FINISHED: &str = "provider_web_fetch_finished";
pub const FETCH_TIMEOUT: &str = "provider_web_fetch_timeout";
pub const FETCH_NONZERO_STATUS: &str = "provider_web_fetch_nonzero_status";
pub const FETCH_RATE_LIMITED: &str = "provider_web_fetch_rate_limited";
pub const FETCH_PARSE_ERROR: &str = "provider_web_fetch_parse_error";
pub const FETCH_REDACTION_APPLIED: &str = "provider_web_fetch_redaction_applied";
pub const DOMAIN_NOT_ALLOWED: &str = "provider_domain_not_allowed";
pub const REDIRECT_BLOCKED: &str = "provider_redirect_blocked";
pub const RESPONSE_TOO_LARGE: &str = "provider_response_too_large";
pub const COOKIE_ABSENT: &str = "provider_cookie_absent";
pub const COOKIE_REJECTED: &str = "provider_cookie_rejected";
pub const ACCOUNT_MISMATCH: &str = "provider_account_mismatch";

pub fn event(
    code: &'static str,
    severity: DiagnosticSeverity,
    safe_message: &'static str,
    timestamp: &str,
    provider: &'static str,
    details: BTreeMap<String, Value>,
) -> DiagnosticEvent {
    DiagnosticEvent {
        code: code.to_string(),
        severity,
        safe_message: safe_message.to_string(),
        timestamp: timestamp.to_string(),
        provider: Some(provider.to_string()),
        source_adapter: Some(SourceAdapter::LinuxWeb),
        recoverable: true,
        details,
        redacted: EventRedaction {
            applied: true,
            classes: vec![
                "secrets".to_string(),
                "identity".to_string(),
                "provider_payload".to_string(),
            ],
        },
    }
}

pub fn push_code(codes: &mut Vec<String>, code: &'static str) {
    if !codes.iter().any(|existing| existing == code) {
        codes.push(code.to_string());
    }
}

pub fn details(values: &[(&str, Value)]) -> BTreeMap<String, Value> {
    values
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect()
}
