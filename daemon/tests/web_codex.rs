mod common;

use std::collections::BTreeMap;
use std::fs;

use codexbar_linuxd::app::{App, AppRuntime, RefreshStart};
use codexbar_linuxd::browser::session_material::{ScopedCookie, SessionMaterial};
use codexbar_linuxd::fixtures;
use codexbar_linuxd::model::{
    ProviderState, RefreshResult, SemanticSource, Snapshot, SourceAdapter,
};
use codexbar_linuxd::web::client::{CodexWebFixture, FakeWebClient, WebClientError, WebResponse};
use codexbar_linuxd::web::diagnostics;
use codexbar_linuxd::web::policy::CodexWebPolicy;
use codexbar_linuxd::web::providers::codex;
use codexbar_linuxd::web::{self, WebRefreshRequest};

const NOW: &str = "2026-05-02T12:00:00Z";
const LINUX_WEB_REFRESH_OPTIONS_JSON: &str = r#"{"schemaVersion":1,"reason":"test","force":true,"sourceAdapterPolicy":{"mode":"only","adapters":["linux_web"]}}"#;

#[test]
fn codex_policy_allows_only_static_dashboard_target() {
    let policy = CodexWebPolicy::new();

    assert_eq!(
        policy.dashboard_url(),
        "https://chatgpt.com/codex/settings/usage"
    );
    assert_eq!(policy.request_hosts(), ["chatgpt.com"]);
    assert_eq!(policy.redirect_hosts(), ["chatgpt.com"]);
    assert_eq!(policy.cookie_domains(), ["chatgpt.com", "openai.com"]);
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
    assert!(result
        .diagnostic_codes
        .contains(&"stale_cache_fallback".to_string()));
    common::assert_public_json_safe(&fallback.snapshot_json);
    common::assert_public_json_safe(&fallback.result_json);
}

fn refresh_for_response(response: WebResponse) -> codexbar_linuxd::web::WebRefresh {
    let client = FakeWebClient::responding(response);
    web::refresh_with_client(web_request(Some(session())), &client).expect("web refresh")
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
