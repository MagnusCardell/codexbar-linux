mod common;

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use codexbar_linuxd::app::{App, AppRuntime, RefreshStart};
use codexbar_linuxd::browser::diagnostics as browser_diagnostics;
use codexbar_linuxd::browser::session_material::{ScopedCookie, SessionMaterial};
use codexbar_linuxd::fixtures;
use codexbar_linuxd::model::{
    Provider, ProviderState, RefreshProviderResult, RefreshProviderStatus, RefreshReason,
    RefreshResult, RefreshStatus, SemanticSource, Snapshot, SourceAdapter,
};
use codexbar_linuxd::web::client::{
    CodexWebFixture, FakeWebClient, ReqwestStaticGetClient, WebClientError, WebRequest, WebResponse,
};
use codexbar_linuxd::web::diagnostics;
use codexbar_linuxd::web::policy::CodexWebPolicy;
use codexbar_linuxd::web::providers::codex;
use codexbar_linuxd::web::{self, WebRefreshRequest};

const NOW: &str = "2026-05-02T12:00:00Z";
const LINUX_WEB_LIVE_HTTP_DISABLED: &str = "linux_web_live_http_disabled";
const LINUX_WEB_REFRESH_OPTIONS_JSON: &str = r#"{"schemaVersion":1,"reason":"test","force":true,"sourceAdapterPolicy":{"mode":"only","adapters":["linux_web"]}}"#;
const CODEX_LIVE_WEB_REFRESH_OPTIONS_JSON: &str = r#"{"schemaVersion":1,"reason":"test","force":true,"providers":["codex"],"sourceAdapterPolicy":{"mode":"only","adapters":["linux_web"]}}"#;

#[test]
fn codex_policy_allows_only_static_dashboard_target() {
    let policy = CodexWebPolicy::new();

    assert_eq!(
        policy.dashboard_url(),
        "https://chatgpt.com/codex/settings/usage"
    );
    assert_eq!(policy.request_hosts(), ["chatgpt.com"]);
    assert_eq!(policy.redirect_hosts(), ["chatgpt.com"]);
    assert_eq!(policy.cookie_domains(), ["chatgpt.com"]);
    assert!(policy
        .validate_dashboard_url(policy.dashboard_url())
        .is_ok());

    for rejected in [
        "https://codex-test.example.invalid/codex/settings/usage",
        "https://user@chatgpt.com/codex/settings/usage",
        "http://chatgpt.com/codex/settings/usage",
        "https://chatgpt.com/codex/settings/usage?token=fixture",
        "https://chatgpt.com/other",
        "https://localhost/codex/settings/usage",
        "https://127.0.0.1/codex/settings/usage",
        "https://10.0.0.1/codex/settings/usage",
        "https://172.16.0.1/codex/settings/usage",
        "https://192.168.1.1/codex/settings/usage",
        "https://169.254.169.254/codex/settings/usage",
        "https://[::1]/codex/settings/usage",
        "https://[fe80::1]/codex/settings/usage",
    ] {
        assert!(
            policy.validate_dashboard_url(rejected).is_err(),
            "policy accepted {rejected}"
        );
    }
}

#[test]
fn codex_policy_blocks_wrong_redirect_hosts_and_tokenized_redirects() {
    let policy = CodexWebPolicy::new();
    assert!(policy
        .validate_redirect_url("https://chatgpt.com/codex/settings/usage")
        .is_ok());
    for rejected in [
        "https://codex-test.example.invalid/callback",
        "https://chatgpt.com/callback?token=fixture",
        "https://user@chatgpt.com/callback",
        "http://chatgpt.com/callback",
        "https://127.0.0.1/callback",
        "https://[::1]/callback",
    ] {
        assert!(
            policy.validate_redirect_url(rejected).is_err(),
            "redirect policy accepted {rejected}"
        );
    }
}

#[test]
fn codex_policy_allows_openai_redirect_only_when_explicitly_configured() {
    let default_policy = CodexWebPolicy::new();
    assert!(default_policy
        .validate_redirect_url("https://openai.com/codex/settings/usage")
        .is_err());

    let openai_policy =
        CodexWebPolicy::with_redirect_hosts_for_tests(&["chatgpt.com", "openai.com"]);
    assert!(openai_policy
        .validate_redirect_url("https://openai.com/codex/settings/usage")
        .is_ok());
}

#[test]
fn reqwest_live_client_rejects_non_static_hosts_before_network() {
    let request = WebRequest::new("https://attacker.example.invalid/codex/settings/usage");
    assert_eq!(
        ReqwestStaticGetClient::validate_request_for_tests(&request).unwrap_err(),
        WebClientError::TransportUnavailable
    );
}

#[test]
fn fake_success_response_normalizes_to_schema_valid_linux_web_snapshot() {
    let client = FakeWebClient::responding(html_response("dashboard_success.html"));
    let refresh =
        web::refresh_with_client(web_request(Some(session())), &client).expect("web refresh");

    assert_eq!(client.requests().len(), 1);
    let provider = &refresh.snapshot.providers[0];
    assert_eq!(provider.provider, "codex");
    assert_eq!(provider.state, ProviderState::Ok);
    assert_eq!(provider.source, SemanticSource::Web);
    assert_eq!(provider.source_adapter, SourceAdapter::LinuxWeb);
    assert_eq!(
        provider.usage.primary.as_ref().unwrap().used_percent,
        Some(34.0)
    );
    assert_eq!(provider.credits.as_ref().unwrap().remaining, Some(112.4));
    assert_eq!(
        provider
            .identity
            .as_ref()
            .unwrap()
            .account_email_display
            .as_deref(),
        Some("masked-account")
    );
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_FINISHED.to_string()));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn absent_session_material_maps_to_unauthenticated_without_client_call() {
    let client = FakeWebClient::responding(html_response("dashboard_success.html"));
    let refresh = web::refresh_with_client(web_request(None), &client).expect("web refresh");

    assert!(client.requests().is_empty());
    let provider = &refresh.snapshot.providers[0];
    assert_eq!(provider.state, ProviderState::Unauthenticated);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::COOKIE_ABSENT.to_string()));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn browser_preflight_no_profile_maps_to_missing_dependency_without_client_call() {
    let client = FakeWebClient::responding(html_response("dashboard_success.html"));
    let mut request = web_request(None);
    request.session_diagnostic_codes.insert(
        "codex".to_string(),
        vec!["browser_profile_not_found".to_string()],
    );
    let refresh = web::refresh_with_client(request, &client).expect("web refresh");

    assert!(client.requests().is_empty());
    let provider = &refresh.snapshot.providers[0];
    assert_eq!(provider.state, ProviderState::MissingDependency);
    assert!(provider
        .diagnostic_codes
        .contains(&"browser_profile_not_found".to_string()));
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::COOKIE_ABSENT.to_string()));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn browser_preflight_cookie_decrypted_code_is_preserved_on_success() {
    let client = FakeWebClient::responding(html_response("dashboard_success.html"));
    let mut request = web_request(Some(session()));
    request.session_diagnostic_codes.insert(
        "codex".to_string(),
        vec![
            "browser_cookie_found".to_string(),
            "browser_cookie_decrypted".to_string(),
        ],
    );
    let refresh = web::refresh_with_client(request, &client).expect("web refresh");
    let provider = &refresh.snapshot.providers[0];

    assert_eq!(provider.state, ProviderState::Ok);
    assert!(provider
        .diagnostic_codes
        .contains(&"browser_cookie_decrypted".to_string()));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn openai_scoped_cookie_is_not_sent_to_chatgpt_static_get() {
    let material = SessionMaterial::new(
        "codex",
        vec![
            ScopedCookie::try_new_for_domain(".openai.com", "/", "openai_only", "fixture-value")
                .expect("openai scoped cookie"),
        ],
    );
    let client = FakeWebClient::responding(html_response("dashboard_success.html"));
    let refresh =
        web::refresh_with_client(web_request(Some(material)), &client).expect("web refresh");

    assert!(client.requests().is_empty());
    assert_eq!(
        refresh.snapshot.providers[0].state,
        ProviderState::Unauthenticated
    );
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn login_required_response_maps_to_cookie_rejected() {
    let refresh = refresh_for_response(html_response("dashboard_login_required.html"));
    let provider = &refresh.snapshot.providers[0];
    assert_eq!(provider.state, ProviderState::CookieRejected);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::COOKIE_REJECTED.to_string()));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn account_mismatch_maps_to_safe_diagnostic_without_raw_email() {
    let client = FakeWebClient::responding(html_response("dashboard_account_mismatch.html"));
    let result = codex::fetch_dashboard_with_client(
        &client,
        Some(&session()),
        NOW,
        Some("other@example.invalid"),
    );
    let payload = serde_json::to_string(&(&result.provider, &result.diagnostics))
        .expect("diagnostic payload json");

    assert_eq!(result.provider.state, ProviderState::CookieRejected);
    assert!(result
        .provider
        .diagnostic_codes
        .contains(&diagnostics::ACCOUNT_MISMATCH.to_string()));
    assert!(!payload.contains("user@example.invalid"));
    assert!(!payload.contains("other@example.invalid"));
    common::assert_public_json_safe(&payload);
}

#[test]
fn non_200_response_maps_to_provider_unavailable() {
    let descriptor = fixture_json("non_200.json");
    let status = descriptor["status"].as_u64().expect("status") as u16;
    let refresh = refresh_for_response(WebResponse::new(
        status,
        CodexWebPolicy::new().dashboard_url(),
        "{}",
    ));
    let provider = &refresh.snapshot.providers[0];
    assert_eq!(provider.state, ProviderState::ProviderUnavailable);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_NONZERO_STATUS.to_string()));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn auth_rejection_status_maps_to_cookie_rejected_without_body_exposure() {
    for status in [401, 403] {
        let refresh = refresh_for_response(WebResponse::new(
            status,
            CodexWebPolicy::new().dashboard_url(),
            "Authorization: Bearer fixture-secret",
        ));
        let payload = serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics))
            .expect("refresh json");
        let provider = &refresh.snapshot.providers[0];

        assert_eq!(provider.state, ProviderState::CookieRejected);
        assert!(provider
            .diagnostic_codes
            .contains(&diagnostics::COOKIE_REJECTED.to_string()));
        assert!(provider
            .diagnostic_codes
            .contains(&diagnostics::FETCH_NONZERO_STATUS.to_string()));
        assert!(!payload.contains("Authorization"));
        assert!(!payload.contains("Bearer"));
        assert!(!payload.contains("fixture-secret"));
        assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
    }
}

#[test]
fn timeout_maps_to_timeout_state() {
    let client = FakeWebClient::failing(WebClientError::Timeout);
    let refresh =
        web::refresh_with_client(web_request(Some(session())), &client).expect("web refresh");
    let provider = &refresh.snapshot.providers[0];
    assert_eq!(provider.state, ProviderState::Timeout);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_TIMEOUT.to_string()));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn transport_body_cap_error_maps_to_response_too_large() {
    let client = FakeWebClient::failing(WebClientError::ResponseTooLarge);
    let refresh =
        web::refresh_with_client(web_request(Some(session())), &client).expect("web refresh");
    let provider = &refresh.snapshot.providers[0];

    assert_eq!(provider.state, ProviderState::ParseError);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::RESPONSE_TOO_LARGE.to_string()));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn too_large_response_maps_to_parse_error_without_body_leakage() {
    let oversized = "x".repeat(CodexWebPolicy::new().response_size_limit() + 1);
    let refresh = refresh_for_response(WebResponse::new(
        200,
        CodexWebPolicy::new().dashboard_url(),
        oversized.as_bytes(),
    ));
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");
    let provider = &refresh.snapshot.providers[0];

    assert_eq!(provider.state, ProviderState::ParseError);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::RESPONSE_TOO_LARGE.to_string()));
    assert!(!payload.contains(&oversized));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn parse_error_response_never_exposes_raw_body() {
    let body = fixture_text("dashboard_parse_error.html");
    let refresh = refresh_for_response(WebResponse::new(
        200,
        CodexWebPolicy::new().dashboard_url(),
        body.as_bytes(),
    ));
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");
    let provider = &refresh.snapshot.providers[0];

    assert_eq!(provider.state, ProviderState::ParseError);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_PARSE_ERROR.to_string()));
    assert!(!payload.contains("codexbar-response-body-marker"));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn unsupported_content_type_maps_to_parse_error_without_body_leakage() {
    let refresh = refresh_for_response(
        WebResponse::new(
            200,
            CodexWebPolicy::new().dashboard_url(),
            "codexbar-unsupported-content-type-body-marker",
        )
        .with_content_type("application/octet-stream"),
    );
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");
    let provider = &refresh.snapshot.providers[0];

    assert_eq!(provider.state, ProviderState::ParseError);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_PARSE_ERROR.to_string()));
    assert!(refresh.diagnostics.iter().any(|event| {
        event.details.get("contentTypeClass") == Some(&serde_json::Value::from("other"))
    }));
    assert!(!payload.contains("codexbar-unsupported-content-type-body-marker"));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn redirect_to_wrong_host_is_blocked_without_following() {
    let descriptor = fixture_json("redirect_wrong_host.json");
    let location = descriptor["location"].as_str().expect("location");
    let client = FakeWebClient::responding(
        WebResponse::new(200, CodexWebPolicy::new().dashboard_url(), "{}").with_redirect(location),
    );
    let refresh =
        web::refresh_with_client(web_request(Some(session())), &client).expect("web refresh");
    let provider = &refresh.snapshot.providers[0];

    assert_eq!(provider.state, ProviderState::ProviderUnavailable);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::REDIRECT_BLOCKED.to_string()));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn allowed_login_redirect_maps_to_cookie_rejected_without_location_exposure() {
    let client = FakeWebClient::responding(
        WebResponse::new(302, CodexWebPolicy::new().dashboard_url(), "")
            .with_redirect("https://chatgpt.com/auth/login"),
    );
    let refresh =
        web::refresh_with_client(web_request(Some(session())), &client).expect("web refresh");
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");
    let provider = &refresh.snapshot.providers[0];

    assert_eq!(provider.state, ProviderState::CookieRejected);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::COOKIE_REJECTED.to_string()));
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_NONZERO_STATUS.to_string()));
    assert!(!payload.contains("auth/login"));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn final_url_same_host_wrong_path_is_blocked() {
    let client = FakeWebClient::responding(WebResponse::new(
        200,
        "https://chatgpt.com/not-codex/settings/usage",
        "{}",
    ));
    let refresh =
        web::refresh_with_client(web_request(Some(session())), &client).expect("web refresh");
    let provider = &refresh.snapshot.providers[0];

    assert_eq!(provider.state, ProviderState::ProviderUnavailable);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::REDIRECT_BLOCKED.to_string()));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn redirect_allowed_host_cannot_mask_disallowed_final_url() {
    let client = FakeWebClient::responding(
        WebResponse::new(200, "https://attacker.example.invalid/final", "{}")
            .with_redirect("https://chatgpt.com/codex/settings/usage"),
    );
    let refresh =
        web::refresh_with_client(web_request(Some(session())), &client).expect("web refresh");
    let provider = &refresh.snapshot.providers[0];

    assert_eq!(provider.state, ProviderState::ProviderUnavailable);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::REDIRECT_BLOCKED.to_string()));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn raw_header_and_token_like_body_is_rejected_without_exposure() {
    let client = FakeWebClient::responding(WebResponse::new(
        200,
        CodexWebPolicy::new().dashboard_url(),
        r#"{"schemaVersion":1,"state":"ok","rawResponse":"Authorization: Bearer fixture-secret"}"#,
    ));
    let refresh =
        web::refresh_with_client(web_request(Some(session())), &client).expect("web refresh");
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");

    assert_eq!(
        refresh.snapshot.providers[0].state,
        ProviderState::ParseError
    );
    assert!(!payload.contains("Authorization"));
    assert!(!payload.contains("Bearer"));
    assert!(!payload.contains("fixture-secret"));
    common::assert_public_json_safe(&payload);
}

#[test]
fn session_material_debug_is_redacted_and_fake_request_records_only_counts() {
    let material = session();
    let debug = format!("{material:?}");
    assert!(debug.contains("cookie_count"));
    assert!(!debug.contains("fixture_session"));
    assert!(!debug.contains("fixture-value"));

    let client = FakeWebClient::responding(html_response("dashboard_success.html"));
    let _refresh =
        web::refresh_with_client(web_request(Some(material)), &client).expect("web refresh");
    let request = client.requests().into_iter().next().expect("request");
    assert!(request.session_material_attached);
    assert!(request.session_material_bytes > 0);
    let request_debug = format!("{request:?}");
    assert!(!request_debug.contains("fixture_session"));
    assert!(!request_debug.contains("fixture-value"));
}

#[test]
fn web_request_and_response_debug_redact_url_secrets_and_bodies() {
    let request_debug = format!(
        "{:?}",
        WebRequest::new("https://user@chatgpt.com/codex/settings/usage?token=fixture-secret")
    );
    assert!(request_debug.contains("chatgpt.com/codex/settings/usage"));
    assert!(!request_debug.contains("user@"));
    assert!(!request_debug.contains("token="));
    assert!(!request_debug.contains("fixture-secret"));

    let response_debug = format!(
        "{:?}",
        WebResponse::new(
            302,
            "https://chatgpt.com/codex/settings/usage?token=fixture-secret",
            "raw-body-marker",
        )
        .with_redirect("https://attacker.example.invalid/callback?token=fixture-secret")
        .with_content_type("text/html; charset=utf-8")
    );
    assert!(response_debug.contains("redirect_present"));
    assert!(!response_debug.contains("attacker.example.invalid"));
    assert!(!response_debug.contains("token="));
    assert!(!response_debug.contains("fixture-secret"));
    assert!(!response_debug.contains("raw-body-marker"));
}

#[test]
fn live_recon_summary_filters_to_safe_stable_fields() {
    let refresh = refresh_for_response(html_response("dashboard_success.html"));
    let mut provider = refresh.snapshot.providers[0].clone();
    provider.diagnostic_codes.extend([
        browser_diagnostics::COOKIE_DECRYPTED.to_string(),
        diagnostics::FETCH_FINISHED.to_string(),
        "Authorization: Bearer fixture-secret".to_string(),
        "https://chatgpt.com/codex/settings/usage?token=fixture-secret".to_string(),
        "/home/example/.config/google-chrome/Default/Network/Cookies".to_string(),
        "rawResponse".to_string(),
        "user@example.invalid".to_string(),
    ]);
    let result = recon_refresh_result(
        RefreshStatus::Partial,
        false,
        [
            browser_diagnostics::COOKIE_DECRYPTED,
            diagnostics::FETCH_REDACTION_APPLIED,
            "Set-Cookie: fixture-secret",
        ],
    );

    let summary = LiveReconSummary::from_provider_and_result(&provider, &result);
    let summary_json = serde_json::to_string(&summary).expect("summary json");

    assert_eq!(summary.provider, "codex");
    assert_eq!(summary.provider_state, ProviderState::Ok);
    assert_eq!(summary.source, SemanticSource::Web);
    assert_eq!(summary.source_adapter, SourceAdapter::LinuxWeb);
    assert_eq!(
        summary.classification,
        LiveReconClassification::ParserSucceeded
    );
    assert_eq!(summary.cookie_presence, LiveReconCookiePresence::Decrypted);
    assert_eq!(summary.web_fetch, LiveReconWebFetch::Finished);
    assert_eq!(
        summary.diagnostic_codes,
        vec![
            diagnostics::FETCH_STARTED.to_string(),
            diagnostics::FETCH_FINISHED.to_string(),
            diagnostics::FETCH_REDACTION_APPLIED.to_string(),
            browser_diagnostics::COOKIE_DECRYPTED.to_string(),
        ]
    );
    assert!(summary.redaction_applied);
    assert!(!summary_json.contains("Authorization"));
    assert!(!summary_json.contains("Bearer"));
    assert!(!summary_json.contains("fixture-secret"));
    assert!(!summary_json.contains("Set-Cookie"));
    assert!(!summary_json.contains("token="));
    assert!(!summary_json.contains("/home/"));
    assert!(!summary_json.contains("Network/Cookies"));
    assert!(!summary_json.contains("rawResponse"));
    assert!(!summary_json.contains("user@example.invalid"));
    common::assert_public_json_safe(&summary_json);
    assert_no_live_web_secret_markers("live Codex web summary", &summary_json);
}

#[test]
fn live_recon_summary_classifies_required_outcomes() {
    for (state, codes, classification, cookie_presence, web_fetch) in [
        (
            ProviderState::MissingDependency,
            vec![LINUX_WEB_LIVE_HTTP_DISABLED],
            LiveReconClassification::LinuxWebLiveHttpDisabled,
            LiveReconCookiePresence::Unknown,
            LiveReconWebFetch::Blocked,
        ),
        (
            ProviderState::MissingDependency,
            vec![
                browser_diagnostics::PROFILE_NOT_FOUND,
                diagnostics::COOKIE_ABSENT,
            ],
            LiveReconClassification::BrowserProfileNotFound,
            LiveReconCookiePresence::Unavailable,
            LiveReconWebFetch::NotAttempted,
        ),
        (
            ProviderState::ProviderUnavailable,
            vec![
                browser_diagnostics::COOKIE_DB_LOCKED,
                diagnostics::COOKIE_ABSENT,
            ],
            LiveReconClassification::UnknownSafeFailure,
            LiveReconCookiePresence::Unavailable,
            LiveReconWebFetch::NotAttempted,
        ),
        (
            ProviderState::MissingDependency,
            vec![
                browser_diagnostics::KEYRING_UNAVAILABLE,
                diagnostics::COOKIE_ABSENT,
            ],
            LiveReconClassification::BrowserKeyringUnavailable,
            LiveReconCookiePresence::Unavailable,
            LiveReconWebFetch::NotAttempted,
        ),
        (
            ProviderState::Unauthenticated,
            vec![
                browser_diagnostics::COOKIE_MISSING,
                diagnostics::COOKIE_ABSENT,
            ],
            LiveReconClassification::BrowserCookieMissing,
            LiveReconCookiePresence::None,
            LiveReconWebFetch::NotAttempted,
        ),
        (
            ProviderState::ProviderUnavailable,
            vec![
                browser_diagnostics::COOKIE_FOUND,
                diagnostics::FETCH_STARTED,
                diagnostics::FETCH_FINISHED,
            ],
            LiveReconClassification::BrowserCookieFound,
            LiveReconCookiePresence::Found,
            LiveReconWebFetch::Finished,
        ),
        (
            ProviderState::CookieRejected,
            vec![
                browser_diagnostics::COOKIE_DECRYPTED,
                diagnostics::COOKIE_REJECTED,
            ],
            LiveReconClassification::ProviderCookieRejected,
            LiveReconCookiePresence::Decrypted,
            LiveReconWebFetch::NotAttempted,
        ),
        (
            ProviderState::CookieRejected,
            vec![
                browser_diagnostics::COOKIE_DECRYPTED,
                diagnostics::FETCH_STARTED,
                diagnostics::COOKIE_REJECTED,
            ],
            LiveReconClassification::LoginRequired,
            LiveReconCookiePresence::Decrypted,
            LiveReconWebFetch::Attempted,
        ),
        (
            ProviderState::ProviderUnavailable,
            vec![
                browser_diagnostics::COOKIE_DECRYPTED,
                diagnostics::FETCH_STARTED,
                diagnostics::REDIRECT_BLOCKED,
            ],
            LiveReconClassification::RedirectBlocked,
            LiveReconCookiePresence::Decrypted,
            LiveReconWebFetch::Blocked,
        ),
        (
            ProviderState::ProviderUnavailable,
            vec![
                browser_diagnostics::COOKIE_DECRYPTED,
                diagnostics::FETCH_STARTED,
                diagnostics::FETCH_NONZERO_STATUS,
            ],
            LiveReconClassification::Non200,
            LiveReconCookiePresence::Decrypted,
            LiveReconWebFetch::Finished,
        ),
        (
            ProviderState::ParseError,
            vec![
                browser_diagnostics::COOKIE_FOUND,
                diagnostics::FETCH_STARTED,
                diagnostics::FETCH_FINISHED,
                diagnostics::FETCH_PARSE_ERROR,
            ],
            LiveReconClassification::ParseError,
            LiveReconCookiePresence::Found,
            LiveReconWebFetch::Finished,
        ),
        (
            ProviderState::Timeout,
            vec![
                browser_diagnostics::COOKIE_DECRYPTED,
                diagnostics::FETCH_STARTED,
                diagnostics::FETCH_TIMEOUT,
            ],
            LiveReconClassification::Timeout,
            LiveReconCookiePresence::Decrypted,
            LiveReconWebFetch::Timeout,
        ),
        (
            ProviderState::ParseError,
            vec![
                browser_diagnostics::COOKIE_DECRYPTED,
                diagnostics::FETCH_STARTED,
                diagnostics::RESPONSE_TOO_LARGE,
            ],
            LiveReconClassification::ResponseTooLarge,
            LiveReconCookiePresence::Decrypted,
            LiveReconWebFetch::ParseError,
        ),
        (
            ProviderState::Ok,
            vec![
                browser_diagnostics::COOKIE_FOUND,
                diagnostics::FETCH_STARTED,
                diagnostics::FETCH_FINISHED,
            ],
            LiveReconClassification::DashboardReachable,
            LiveReconCookiePresence::Found,
            LiveReconWebFetch::Finished,
        ),
        (
            ProviderState::Ok,
            vec![
                browser_diagnostics::COOKIE_DECRYPTED,
                diagnostics::FETCH_STARTED,
                diagnostics::FETCH_FINISHED,
                diagnostics::FETCH_REDACTION_APPLIED,
            ],
            LiveReconClassification::ParserSucceeded,
            LiveReconCookiePresence::Decrypted,
            LiveReconWebFetch::Finished,
        ),
    ] {
        let provider = recon_provider(state, codes);
        let result = recon_refresh_result(RefreshStatus::Error, false, []);
        let summary = LiveReconSummary::from_provider_and_result(&provider, &result);

        assert_eq!(summary.classification, classification);
        assert_eq!(summary.cookie_presence, cookie_presence);
        assert_eq!(summary.web_fetch, web_fetch);
        let summary_json = serde_json::to_string(&summary).expect("summary json");
        common::assert_public_json_safe(&summary_json);
        assert_no_live_web_secret_markers("live Codex web summary", &summary_json);
    }
}

#[test]
fn live_recon_summary_does_not_copy_diagnostic_details() {
    let provider = recon_provider(
        ProviderState::ParseError,
        vec![
            browser_diagnostics::COOKIE_FOUND,
            diagnostics::FETCH_PARSE_ERROR,
        ],
    );
    let result = recon_refresh_result(RefreshStatus::Partial, false, []);
    let raw_detail_json = serde_json::json!({
        "rawResponse": "Authorization: Bearer fixture-secret",
        "profilePath": "/tmp/codexbar-web-live/profile",
        "accountEmail": "user@example.invalid",
        "redirectUrl": "https://chatgpt.com/auth/login?token=fixture-secret"
    })
    .to_string();

    let summary = LiveReconSummary::from_provider_and_result(&provider, &result);
    let summary_json = serde_json::to_string(&summary).expect("summary json");

    for forbidden in [
        "rawResponse",
        "Authorization",
        "Bearer",
        "fixture-secret",
        "/tmp/codexbar-web-live",
        "user@example.invalid",
        "redirectUrl",
        "token=",
    ] {
        assert!(
            raw_detail_json.contains(forbidden),
            "test setup should include {forbidden}"
        );
        assert!(
            !summary_json.contains(forbidden),
            "summary copied diagnostic detail marker {forbidden}"
        );
    }
    common::assert_public_json_safe(&summary_json);
    assert_no_live_web_secret_markers("live Codex web summary", &summary_json);
}

#[tokio::test]
async fn production_linux_web_refresh_has_no_live_client_by_default() {
    let (_tmp, paths) = common::temp_paths();
    let app = App::new(paths).expect("production app");
    let start = app
        .start_refresh(LINUX_WEB_REFRESH_OPTIONS_JSON)
        .expect("refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("refresh should start");
    };
    let completion = app.finish_refresh(&refresh_id).await.expect("refresh");
    let snapshot: Snapshot = serde_json::from_str(&completion.snapshot_json).expect("snapshot");
    let result: RefreshResult =
        serde_json::from_str(&completion.result_json).expect("refresh result");

    assert_eq!(
        snapshot.providers[0].state,
        ProviderState::MissingDependency
    );
    assert_eq!(
        snapshot.providers[0].source_adapter,
        SourceAdapter::LinuxWeb
    );
    assert!(!result.cache_written);
    assert!(snapshot.providers[0]
        .diagnostic_codes
        .contains(&"linux_web_live_http_disabled".to_string()));
    common::assert_public_json_safe(&completion.snapshot_json);
    common::assert_public_json_safe(&completion.result_json);
    let diagnostics_json = app.get_diagnostics_json("codex").expect("diagnostics");
    common::assert_schema("diagnostics.schema.json", &diagnostics_json);
    common::assert_public_json_safe(&diagnostics_json);
}

#[tokio::test]
async fn live_transport_gate_requires_explicit_codex_provider() {
    let (_tmp, paths) = common::temp_paths();
    let app = App::new_with_runtime(
        paths,
        AppRuntime::production().with_codex_web_live_transport_for_tests(true),
    )
    .expect("app");
    let start = app
        .start_refresh(LINUX_WEB_REFRESH_OPTIONS_JSON)
        .expect("refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("refresh should start");
    };
    let completion = app.finish_refresh(&refresh_id).await.expect("refresh");
    let snapshot: Snapshot = serde_json::from_str(&completion.snapshot_json).expect("snapshot");

    assert!(snapshot.providers[0]
        .diagnostic_codes
        .contains(&"linux_web_live_http_disabled".to_string()));
    common::assert_public_json_safe(&completion.snapshot_json);
}

#[tokio::test]
async fn failed_linux_web_refresh_preserves_previous_stale_snapshot() {
    let (_tmp, paths) = common::temp_paths();
    let fake_app = App::new_with_runtime(
        paths.clone(),
        AppRuntime::with_codex_web_fixture_for_tests(CodexWebFixture::Success),
    )
    .expect("fake web app");
    let start = fake_app
        .start_refresh(LINUX_WEB_REFRESH_OPTIONS_JSON)
        .expect("fake refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("fake refresh should start");
    };
    let successful = fake_app.finish_refresh(&refresh_id).await.expect("refresh");
    let successful_result: RefreshResult =
        serde_json::from_str(&successful.result_json).expect("successful result");
    assert!(successful_result.cache_written);
    assert!(fake_app.cache_file_path().is_file());
    let fake_diagnostics_json = fake_app.get_diagnostics_json("codex").expect("diagnostics");
    common::assert_schema("diagnostics.schema.json", &fake_diagnostics_json);
    common::assert_public_json_safe(&fake_diagnostics_json);
    for (_provider, event_json) in &successful.provider_events {
        common::assert_public_json_safe(event_json);
    }

    let production_app = App::new(paths).expect("production app");
    let start = production_app
        .start_refresh(LINUX_WEB_REFRESH_OPTIONS_JSON)
        .expect("fallback refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("fallback refresh should start");
    };
    let fallback = production_app
        .finish_refresh(&refresh_id)
        .await
        .expect("fallback refresh");
    let snapshot: Snapshot = serde_json::from_str(&fallback.snapshot_json).expect("snapshot");
    let result: RefreshResult = serde_json::from_str(&fallback.result_json).expect("result");

    assert!(snapshot.stale);
    assert_eq!(snapshot.providers[0].state, ProviderState::Stale);
    assert_eq!(
        snapshot.providers[0].source_adapter,
        SourceAdapter::LinuxWeb
    );
    assert!(!result.cache_written);
    assert_eq!(result.status, RefreshStatus::Partial);
    assert!(result
        .diagnostic_codes
        .contains(&"stale_cache_fallback".to_string()));
    common::assert_public_json_safe(&fallback.snapshot_json);
    common::assert_public_json_safe(&fallback.result_json);
}

#[tokio::test]
#[ignore = "requires CODEXBAR_CODEX_WEB_LIVE=1 and CODEXBAR_BROWSER_IMPORT_FAKE_HOME=/tmp/... throwaway profile"]
async fn codex_web_live_throwaway_recon_smoke_redacts_outputs() {
    let Some(fake_home) = live_throwaway_fake_home() else {
        return;
    };
    let (_tmp, paths) = common::temp_paths();
    let app = App::new_with_runtime(
        paths,
        AppRuntime::from_env().expect("live Codex web runtime"),
    )
    .expect("live Codex web app");
    let start = app
        .start_refresh(CODEX_LIVE_WEB_REFRESH_OPTIONS_JSON)
        .expect("live web refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("live web refresh should start");
    };
    let completion = app
        .finish_refresh(&refresh_id)
        .await
        .expect("live web refresh");
    let snapshot: Snapshot = serde_json::from_str(&completion.snapshot_json).expect("snapshot");
    let result: RefreshResult = serde_json::from_str(&completion.result_json).expect("result");

    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);
    common::assert_schema("refresh-result.schema.json", &completion.result_json);
    common::assert_public_json_safe(&completion.snapshot_json);
    common::assert_public_json_safe(&completion.result_json);
    assert_no_live_web_secret_markers("live Codex web snapshot", &completion.snapshot_json);
    assert_no_live_web_secret_markers("live Codex web result", &completion.result_json);
    assert_no_live_web_fake_home_path(
        "live Codex web snapshot",
        &completion.snapshot_json,
        &fake_home,
    );
    assert_no_live_web_fake_home_path("live Codex web result", &completion.result_json, &fake_home);
    assert!(!result.cache_written || app.cache_file_path().is_file());

    let provider = snapshot
        .providers
        .iter()
        .find(|provider| provider.provider == "codex")
        .expect("codex provider");
    assert_eq!(provider.source, SemanticSource::Web);
    assert_eq!(provider.source_adapter, SourceAdapter::LinuxWeb);
    assert!(
        matches!(
            provider.state,
            ProviderState::Ok
                | ProviderState::Unauthenticated
                | ProviderState::CookieRejected
                | ProviderState::MissingDependency
                | ProviderState::ProviderUnavailable
                | ProviderState::ParseError
                | ProviderState::Timeout
        ),
        "unexpected live Codex web state: {:?}",
        provider.state
    );
    assert!(provider.diagnostic_codes.iter().any(|code| matches!(
        code.as_str(),
        "browser_live_profiles_disabled"
            | "browser_profile_not_found"
            | "browser_cookie_db_locked"
            | "browser_keyring_locked"
            | "browser_cookie_missing"
            | "browser_cookie_found"
            | "browser_cookie_decrypted"
            | "provider_cookie_absent"
            | "provider_cookie_rejected"
            | "provider_redirect_blocked"
            | "provider_web_fetch_nonzero_status"
            | "provider_web_fetch_parse_error"
            | "provider_web_fetch_finished"
            | "provider_web_fetch_timeout"
            | "provider_response_too_large"
    )));

    let diagnostics_json = app.get_diagnostics_json("codex").expect("diagnostics");
    common::assert_public_json_safe(&diagnostics_json);
    assert_no_live_web_secret_markers("live Codex web diagnostics", &diagnostics_json);
    assert_no_live_web_fake_home_path("live Codex web diagnostics", &diagnostics_json, &fake_home);
    for (_provider, event_json) in completion.provider_events {
        common::assert_public_json_safe(&event_json);
        assert_no_live_web_secret_markers("live Codex web event", &event_json);
        assert_no_live_web_fake_home_path("live Codex web event", &event_json, &fake_home);
    }
    if app.cache_file_path().is_file() {
        let cache_json = fs::read_to_string(app.cache_file_path()).expect("cache");
        common::assert_public_json_safe(&cache_json);
        assert_no_live_web_secret_markers("live Codex web cache", &cache_json);
        assert_no_live_web_fake_home_path("live Codex web cache", &cache_json, &fake_home);
    }

    let summary = LiveReconSummary::from_provider_and_result(provider, &result);
    let summary_json = serde_json::to_string(&summary).expect("live recon summary json");
    common::assert_public_json_safe(&summary_json);
    assert_no_live_web_secret_markers("live Codex web summary", &summary_json);
    assert_no_live_web_fake_home_path("live Codex web summary", &summary_json, &fake_home);
    eprintln!("{summary_json}");
}

fn refresh_for_response(response: WebResponse) -> codexbar_linuxd::web::WebRefresh {
    let client = FakeWebClient::responding(response);
    web::refresh_with_client(web_request(Some(session())), &client).expect("web refresh")
}

fn live_throwaway_fake_home() -> Option<PathBuf> {
    if env::var("CODEXBAR_CODEX_WEB_LIVE").ok().as_deref() != Some("1") {
        eprintln!("skipping live Codex web recon: CODEXBAR_CODEX_WEB_LIVE=1 is not set");
        return None;
    }
    let Some(value) = env::var_os("CODEXBAR_BROWSER_IMPORT_FAKE_HOME") else {
        eprintln!("skipping live Codex web recon: CODEXBAR_BROWSER_IMPORT_FAKE_HOME is not set");
        return None;
    };
    let fake_home = fs::canonicalize(PathBuf::from(value)).expect("throwaway fake home must exist");
    assert_ne!(
        fake_home,
        PathBuf::from("/"),
        "throwaway fake home must not be /"
    );
    if let Some(real_home) = env::var_os("HOME").and_then(|path| fs::canonicalize(path).ok()) {
        assert_ne!(fake_home, real_home, "throwaway fake home must not be HOME");
        assert!(
            !fake_home.starts_with(real_home.join(".config")),
            "throwaway fake home must not be under the real config home"
        );
    }
    assert!(
        fake_home.join(".codexbar-throwaway-browser-root").is_file(),
        "throwaway fake home marker is required"
    );
    Some(fake_home)
}

fn assert_no_live_web_secret_markers(label: &str, text: &str) {
    let lower = text.to_ascii_lowercase();
    for needle in [
        "/home/",
        "~/.local/share",
        "auth.json",
        "authorization:",
        "bearer ",
        "cookie:",
        "set-cookie",
        "access_token",
        "accesstoken",
        "refresh_token",
        "refreshtoken",
        "session_token",
        "sessionkey",
        "sessiontoken",
        "apikey",
        "api_key",
        "ghp_",
        "github_pat",
        "xoxb-",
        "rawresponse",
        "raw_response",
        "rawpayload",
        "raw_payload",
    ] {
        assert!(
            !lower.contains(needle),
            "{label} contains forbidden live secret marker {needle:?}"
        );
    }
}

fn assert_no_live_web_fake_home_path(label: &str, text: &str, fake_home: &std::path::Path) {
    let fake_home = fake_home.to_string_lossy();
    assert!(
        !text.contains(fake_home.as_ref()),
        "{label} contains throwaway fake home path"
    );
    for needle in [
        ".codexbar-throwaway-browser-root",
        ".config/google-chrome",
        ".config/chromium",
        "BraveSoftware",
        "Network/Cookies",
        "cookies.sqlite",
    ] {
        assert!(
            !text.contains(needle),
            "{label} contains forbidden browser profile marker {needle:?}"
        );
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveReconSummary {
    provider: String,
    provider_state: ProviderState,
    refresh_status: RefreshStatus,
    cache_written: bool,
    source: SemanticSource,
    source_adapter: SourceAdapter,
    classification: LiveReconClassification,
    diagnostic_codes: Vec<String>,
    cookie_presence: LiveReconCookiePresence,
    web_fetch: LiveReconWebFetch,
    redaction_applied: bool,
}

impl LiveReconSummary {
    fn from_provider_and_result(provider: &Provider, result: &RefreshResult) -> Self {
        let diagnostic_codes = safe_recon_diagnostic_codes(provider, result);
        let classification = classify_live_recon(provider.state, &diagnostic_codes);
        let cookie_presence = classify_cookie_presence(&diagnostic_codes);
        let web_fetch = classify_web_fetch(&diagnostic_codes);
        Self {
            provider: "codex".to_string(),
            provider_state: provider.state,
            refresh_status: result.status,
            cache_written: result.cache_written,
            source: SemanticSource::Web,
            source_adapter: SourceAdapter::LinuxWeb,
            classification,
            diagnostic_codes,
            cookie_presence,
            web_fetch,
            redaction_applied: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum LiveReconClassification {
    DashboardReachable,
    ParserSucceeded,
    LoginRequired,
    ProviderCookieRejected,
    RedirectBlocked,
    Non200,
    ParseError,
    Timeout,
    ResponseTooLarge,
    BrowserCookieMissing,
    BrowserCookieFound,
    BrowserKeyringUnavailable,
    BrowserProfileNotFound,
    LinuxWebLiveHttpDisabled,
    UnknownSafeFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum LiveReconCookiePresence {
    None,
    Found,
    Decrypted,
    Unavailable,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum LiveReconWebFetch {
    NotAttempted,
    Attempted,
    Finished,
    Blocked,
    Timeout,
    ParseError,
}

fn safe_recon_diagnostic_codes(provider: &Provider, result: &RefreshResult) -> Vec<String> {
    let mut codes = Vec::new();
    for code in &provider.diagnostic_codes {
        push_safe_recon_diagnostic_code(&mut codes, code);
    }
    for provider_result in result
        .providers
        .iter()
        .filter(|provider_result| provider_result.provider == "codex")
    {
        for code in &provider_result.diagnostic_codes {
            push_safe_recon_diagnostic_code(&mut codes, code);
        }
    }
    for code in &result.diagnostic_codes {
        push_safe_recon_diagnostic_code(&mut codes, code);
    }
    codes
}

fn push_safe_recon_diagnostic_code(codes: &mut Vec<String>, code: &str) {
    if is_safe_recon_diagnostic_code(code) && !codes.iter().any(|existing| existing == code) {
        codes.push(code.to_string());
    }
}

fn is_safe_recon_diagnostic_code(code: &str) -> bool {
    matches!(
        code,
        LINUX_WEB_LIVE_HTTP_DISABLED
            | browser_diagnostics::LIVE_PROFILES_DISABLED
            | browser_diagnostics::NOT_FOUND
            | browser_diagnostics::PROFILE_NOT_FOUND
            | browser_diagnostics::PROFILE_UNREADABLE
            | browser_diagnostics::PROFILE_SKIPPED
            | browser_diagnostics::COOKIE_DB_MISSING
            | browser_diagnostics::COOKIE_DB_UNREADABLE
            | browser_diagnostics::COOKIE_DB_LOCKED
            | browser_diagnostics::COOKIE_DB_SCHEMA_UNSUPPORTED
            | browser_diagnostics::COOKIE_DECRYPTION_UNAVAILABLE
            | browser_diagnostics::COOKIE_DECRYPTION_FAILED
            | browser_diagnostics::KEYRING_UNAVAILABLE
            | browser_diagnostics::KEYRING_LOCKED
            | browser_diagnostics::KEYRING_PROMPT_REQUIRED
            | browser_diagnostics::COOKIE_FOUND
            | browser_diagnostics::COOKIE_DECRYPTED
            | browser_diagnostics::COOKIE_MISSING
            | diagnostics::FETCH_STARTED
            | diagnostics::FETCH_FINISHED
            | diagnostics::FETCH_TIMEOUT
            | diagnostics::FETCH_NONZERO_STATUS
            | diagnostics::FETCH_RATE_LIMITED
            | diagnostics::FETCH_PARSE_ERROR
            | diagnostics::FETCH_REDACTION_APPLIED
            | diagnostics::DOMAIN_NOT_ALLOWED
            | diagnostics::REDIRECT_BLOCKED
            | diagnostics::RESPONSE_TOO_LARGE
            | diagnostics::COOKIE_ABSENT
            | diagnostics::COOKIE_REJECTED
            | diagnostics::ACCOUNT_MISMATCH
    )
}

fn classify_live_recon(provider_state: ProviderState, codes: &[String]) -> LiveReconClassification {
    if provider_state == ProviderState::Ok {
        if has_code(codes, diagnostics::FETCH_REDACTION_APPLIED) {
            return LiveReconClassification::ParserSucceeded;
        }
        return LiveReconClassification::DashboardReachable;
    }
    if has_code(codes, LINUX_WEB_LIVE_HTTP_DISABLED) {
        return LiveReconClassification::LinuxWebLiveHttpDisabled;
    }
    if has_code(codes, diagnostics::REDIRECT_BLOCKED) {
        return LiveReconClassification::RedirectBlocked;
    }
    if has_code(codes, diagnostics::FETCH_TIMEOUT) {
        return LiveReconClassification::Timeout;
    }
    if has_code(codes, diagnostics::RESPONSE_TOO_LARGE) {
        return LiveReconClassification::ResponseTooLarge;
    }
    if has_code(codes, diagnostics::COOKIE_REJECTED) {
        if has_code(codes, diagnostics::FETCH_STARTED) {
            return LiveReconClassification::LoginRequired;
        }
        return LiveReconClassification::ProviderCookieRejected;
    }
    if has_code(codes, diagnostics::ACCOUNT_MISMATCH)
        || provider_state == ProviderState::CookieRejected
    {
        return LiveReconClassification::ProviderCookieRejected;
    }
    if has_any_code(
        codes,
        &[
            diagnostics::FETCH_NONZERO_STATUS,
            diagnostics::FETCH_RATE_LIMITED,
        ],
    ) {
        return LiveReconClassification::Non200;
    }
    if has_code(codes, diagnostics::FETCH_PARSE_ERROR)
        || provider_state == ProviderState::ParseError
    {
        return LiveReconClassification::ParseError;
    }
    if has_any_code(
        codes,
        &[
            browser_diagnostics::PROFILE_NOT_FOUND,
            browser_diagnostics::NOT_FOUND,
            browser_diagnostics::LIVE_PROFILES_DISABLED,
        ],
    ) {
        return LiveReconClassification::BrowserProfileNotFound;
    }
    if has_any_code(
        codes,
        &[
            browser_diagnostics::COOKIE_DECRYPTION_UNAVAILABLE,
            browser_diagnostics::COOKIE_DECRYPTION_FAILED,
            browser_diagnostics::KEYRING_LOCKED,
            browser_diagnostics::KEYRING_PROMPT_REQUIRED,
            browser_diagnostics::KEYRING_UNAVAILABLE,
        ],
    ) {
        return LiveReconClassification::BrowserKeyringUnavailable;
    }
    if has_any_code(
        codes,
        &[
            browser_diagnostics::COOKIE_DB_MISSING,
            browser_diagnostics::COOKIE_DB_UNREADABLE,
            browser_diagnostics::COOKIE_DB_LOCKED,
            browser_diagnostics::COOKIE_DB_SCHEMA_UNSUPPORTED,
        ],
    ) {
        return LiveReconClassification::UnknownSafeFailure;
    }
    if has_any_code(
        codes,
        &[
            browser_diagnostics::COOKIE_MISSING,
            diagnostics::COOKIE_ABSENT,
        ],
    ) || provider_state == ProviderState::Unauthenticated
    {
        return LiveReconClassification::BrowserCookieMissing;
    }
    if has_any_code(
        codes,
        &[
            browser_diagnostics::COOKIE_FOUND,
            browser_diagnostics::COOKIE_DECRYPTED,
        ],
    ) {
        return LiveReconClassification::BrowserCookieFound;
    }
    LiveReconClassification::UnknownSafeFailure
}

fn classify_cookie_presence(codes: &[String]) -> LiveReconCookiePresence {
    if has_code(codes, browser_diagnostics::COOKIE_DECRYPTED) {
        LiveReconCookiePresence::Decrypted
    } else if has_code(codes, browser_diagnostics::COOKIE_FOUND) {
        LiveReconCookiePresence::Found
    } else if has_any_code(
        codes,
        &[
            browser_diagnostics::PROFILE_NOT_FOUND,
            browser_diagnostics::PROFILE_UNREADABLE,
            browser_diagnostics::PROFILE_SKIPPED,
            browser_diagnostics::NOT_FOUND,
            browser_diagnostics::LIVE_PROFILES_DISABLED,
            browser_diagnostics::COOKIE_DB_MISSING,
            browser_diagnostics::COOKIE_DB_UNREADABLE,
            browser_diagnostics::COOKIE_DB_LOCKED,
            browser_diagnostics::COOKIE_DB_SCHEMA_UNSUPPORTED,
            browser_diagnostics::COOKIE_DECRYPTION_UNAVAILABLE,
            browser_diagnostics::COOKIE_DECRYPTION_FAILED,
            browser_diagnostics::KEYRING_UNAVAILABLE,
            browser_diagnostics::KEYRING_LOCKED,
            browser_diagnostics::KEYRING_PROMPT_REQUIRED,
        ],
    ) {
        LiveReconCookiePresence::Unavailable
    } else if has_any_code(
        codes,
        &[
            browser_diagnostics::COOKIE_MISSING,
            diagnostics::COOKIE_ABSENT,
        ],
    ) {
        LiveReconCookiePresence::None
    } else {
        LiveReconCookiePresence::Unknown
    }
}

fn classify_web_fetch(codes: &[String]) -> LiveReconWebFetch {
    if has_code(codes, LINUX_WEB_LIVE_HTTP_DISABLED)
        || has_code(codes, diagnostics::REDIRECT_BLOCKED)
    {
        return LiveReconWebFetch::Blocked;
    }
    if has_code(codes, diagnostics::FETCH_TIMEOUT) {
        return LiveReconWebFetch::Timeout;
    }
    if has_any_code(
        codes,
        &[
            diagnostics::FETCH_FINISHED,
            diagnostics::FETCH_NONZERO_STATUS,
            diagnostics::FETCH_RATE_LIMITED,
        ],
    ) {
        return LiveReconWebFetch::Finished;
    }
    if has_any_code(
        codes,
        &[
            diagnostics::FETCH_PARSE_ERROR,
            diagnostics::RESPONSE_TOO_LARGE,
        ],
    ) {
        return LiveReconWebFetch::ParseError;
    }
    if has_code(codes, diagnostics::FETCH_STARTED) {
        return LiveReconWebFetch::Attempted;
    }
    LiveReconWebFetch::NotAttempted
}

fn has_code(codes: &[String], expected: &str) -> bool {
    codes.iter().any(|code| code == expected)
}

fn has_any_code(codes: &[String], expected: &[&str]) -> bool {
    expected.iter().any(|expected| has_code(codes, expected))
}

fn recon_provider(state: ProviderState, codes: Vec<&'static str>) -> Provider {
    let mut refresh = refresh_for_response(html_response("dashboard_success.html"));
    let provider = &mut refresh.snapshot.providers[0];
    provider.state = state;
    provider.diagnostic_codes = codes.into_iter().map(str::to_string).collect();
    provider.clone()
}

fn recon_refresh_result(
    status: RefreshStatus,
    cache_written: bool,
    codes: impl IntoIterator<Item = &'static str>,
) -> RefreshResult {
    let diagnostic_codes = codes.into_iter().map(str::to_string).collect::<Vec<_>>();
    RefreshResult {
        schema_version: 1,
        refresh_id: "recon-summary-test".to_string(),
        status,
        started_at: NOW.to_string(),
        finished_at: NOW.to_string(),
        duration_ms: 0,
        reason: RefreshReason::Test,
        providers: vec![RefreshProviderResult {
            provider: "codex".to_string(),
            status: RefreshProviderStatus::Ok,
            source_adapter: Some(SourceAdapter::LinuxWeb),
            diagnostic_codes: diagnostic_codes.clone(),
        }],
        cache_written,
        snapshot_generated_at: Some(NOW.to_string()),
        diagnostic_codes,
    }
}

fn html_response(name: &str) -> WebResponse {
    WebResponse::new(
        200,
        CodexWebPolicy::new().dashboard_url(),
        fixture_text(name).into_bytes(),
    )
    .with_content_type("text/html")
}

fn web_request(session: Option<SessionMaterial>) -> WebRefreshRequest {
    let mut sessions = BTreeMap::new();
    if let Some(session) = session {
        sessions.insert("codex".to_string(), session);
    }
    WebRefreshRequest {
        refresh_id: "web-refresh-test".to_string(),
        started_at: NOW.to_string(),
        finished_at: NOW.to_string(),
        providers: vec!["codex".to_string()],
        selected_provider: None,
        upstream_cli: fixtures::task01_upstream_cli(),
        sessions,
        session_diagnostic_codes: BTreeMap::new(),
    }
}

fn session() -> SessionMaterial {
    SessionMaterial::new(
        "codex",
        vec![ScopedCookie::new("fixture_session", "fixture-value")],
    )
}

fn fixture_text(name: &str) -> String {
    fs::read_to_string(common::repo_path("daemon/fixtures/web/codex").join(name))
        .unwrap_or_else(|err| panic!("fixture {name}: {err}"))
}

fn fixture_json(name: &str) -> serde_json::Value {
    serde_json::from_str(&fixture_text(name)).expect("fixture json")
}

fn assert_payloads_are_schema_valid_and_public(
    snapshot: &Snapshot,
    diagnostics: &[codexbar_linuxd::model::DiagnosticEvent],
) {
    let snapshot_json = serde_json::to_string(snapshot).expect("snapshot json");
    let diagnostics_json = serde_json::to_string(diagnostics).expect("diagnostics json");
    common::assert_schema("snapshot.schema.json", &snapshot_json);
    common::assert_public_json_safe(&snapshot_json);
    common::assert_public_json_safe(&diagnostics_json);
}
