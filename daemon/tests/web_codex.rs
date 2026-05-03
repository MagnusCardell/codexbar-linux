mod common;

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
use aes::Aes128;
use codexbar_linuxd::app::{App, AppRuntime, RefreshStart};
use codexbar_linuxd::browser::cookie_store::DecryptionFailureClass;
use codexbar_linuxd::browser::diagnostics as browser_diagnostics;
use codexbar_linuxd::browser::keyring::{
    BrowserDecryptorMode, DecryptionStatus, DecryptorBackend, FakeDecryptorMode,
};
use codexbar_linuxd::browser::profile::BrowserDiscoveryRoots;
use codexbar_linuxd::browser::session_material::{ScopedCookie, SessionMaterial};
use codexbar_linuxd::browser::{self, BrowserSessionRequest};
use codexbar_linuxd::fixtures;
use codexbar_linuxd::model::{
    DiagnosticEvent, Diagnostics, Provider, ProviderState, RefreshProviderResult,
    RefreshProviderStatus, RefreshReason, RefreshResult, RefreshStatus, SemanticSource, Snapshot,
    SourceAdapter,
};
use codexbar_linuxd::web::client::{
    CodexWebFixture, FakeWebClient, ReqwestStaticGetClient, WebClient, WebClientError, WebRequest,
    WebResponse,
};
use codexbar_linuxd::web::diagnostics;
use codexbar_linuxd::web::policy::{CodexWebPolicy, RedirectTargetClass};
use codexbar_linuxd::web::providers::codex;
use codexbar_linuxd::web::{self, WebRefreshRequest};
use pbkdf2::pbkdf2_hmac;
use rusqlite::{params, Connection};
use sha1::Sha1;
use sha2::{Digest, Sha256};

const NOW: &str = "2026-05-02T12:00:00Z";
const LINUX_WEB_LIVE_HTTP_DISABLED: &str = "linux_web_live_http_disabled";
const LINUX_WEB_REFRESH_OPTIONS_JSON: &str = r#"{"schemaVersion":1,"reason":"test","force":true,"sourceAdapterPolicy":{"mode":"only","adapters":["linux_web"]}}"#;
const CODEX_LIVE_WEB_REFRESH_OPTIONS_JSON: &str = r#"{"schemaVersion":1,"reason":"test","force":true,"providers":["codex"],"sourceAdapterPolicy":{"mode":"only","adapters":["linux_web"]}}"#;
const CHROMIUM_V10_PREFIX: &[u8] = b"v10";
const CHROMIUM_BASIC_PASSWORD: &[u8] = b"peanuts";
const CHROMIUM_BASIC_SALT: &[u8] = b"saltysalt";
const CHROMIUM_BASIC_ITERATIONS: u32 = 1;
const CHROMIUM_AES_BLOCK_LEN: usize = 16;
const CHROMIUM_BASIC_IV: [u8; CHROMIUM_AES_BLOCK_LEN] = [b' '; CHROMIUM_AES_BLOCK_LEN];

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
    assert!(policy
        .validate_redirect_url("https://chatgpt.com/codex/settings/usage/")
        .is_ok());
    assert!(policy
        .validate_redirect_url("https://chatgpt.com/codex/settings/usage?")
        .is_ok());
    assert!(policy
        .validate_redirect_url("https://chatgpt.com/codex/cloud/settings/usage")
        .is_ok());
    assert!(policy
        .validate_redirect_url("https://chatgpt.com/codex/settings/usage?utm_campaign=limits")
        .is_err());
    assert!(policy
        .validate_redirect_url("https://chatgpt.com/codex/cloud/settings/usage?")
        .is_ok());
    assert!(policy
        .validate_redirect_url("https://chatgpt.com/codex/cloud/settings/usage?token=fixture")
        .is_err());
    assert_eq!(
        policy.classify_redirect_target(Some("https://chatgpt.com/codex/settings/usage"), false,),
        RedirectTargetClass::SameHostUsagePath
    );
    assert_eq!(
        policy.classify_redirect_target(Some("https://chatgpt.com/codex/settings/usage/"), false,),
        RedirectTargetClass::SameHostUsagePath
    );
    assert_eq!(
        policy.classify_redirect_target(
            Some("https://chatgpt.com/codex/settings/usage?utm_campaign=limits"),
            false,
        ),
        RedirectTargetClass::SameHostUsagePath
    );
    assert_eq!(
        policy.classify_redirect_target(
            Some("https://chatgpt.com/codex/cloud/settings/usage"),
            false,
        ),
        RedirectTargetClass::SameHostUsagePath
    );
    assert_eq!(
        policy.classify_redirect_target(
            Some("https://chatgpt.com/codex/cloud/settings/usage?token=fixture"),
            false,
        ),
        RedirectTargetClass::Invalid
    );
    assert_eq!(
        policy.classify_redirect_target(Some("https://chatgpt.com/codex/cloud/other"), false),
        RedirectTargetClass::SameHostOther
    );
    assert_eq!(
        policy.classify_redirect_target(
            Some("https://chatgpt.com:443/codex/cloud/settings/usage"),
            false,
        ),
        RedirectTargetClass::Invalid
    );
    assert_eq!(
        policy.classify_redirect_target(Some("https://chatgpt.com/auth/login"), false),
        RedirectTargetClass::SameHostLoginPath
    );
    assert_eq!(
        policy.classify_redirect_target(Some("https://chatgpt.com/not-codex"), false),
        RedirectTargetClass::SameHostOther
    );
    for rejected in [
        "https://openai.com/codex/settings/usage",
        "https://openai.com/codex/cloud/settings/usage",
        "https://codex-test.example.invalid/callback",
        "https://chatgpt.com/callback",
        "https://chatgpt.com/callback?token=fixture",
        "https://chatgpt.com/callback#access_token=fixture",
        "https://user@chatgpt.com/callback",
        "http://chatgpt.com/callback",
        "https://chatgpt.com/codex/settings/usage?token=fixture",
        "https://chatgpt.com/codex/settings/usage#fragment",
        "http://chatgpt.com/codex/cloud/settings/usage",
        "https://user@chatgpt.com/codex/cloud/settings/usage",
        "https://chatgpt.com:443/codex/cloud/settings/usage",
        "https://chatgpt.com:444/codex/cloud/settings/usage",
        "https://chatgpt.com:443/codex/cloud/settings/usage?",
        "https://chatgpt.com/codex/cloud/settings/usage/",
        "https://chatgpt.com/codex/cloud/settings/usage?token=fixture",
        "https://chatgpt.com/codex/cloud/settings/usage#fragment",
        "https://chatgpt.com/codex/cloud/settings/usage#",
        "https://chatgpt.com/codex/cloud/other",
        "https://chatgpt.com/auth/login",
        "https://127.0.0.1/callback",
        "https://10.0.0.1/callback",
        "https://172.16.0.1/callback",
        "https://192.168.1.1/callback",
        "https://169.254.169.254/callback",
        "https://[::1]/callback",
    ] {
        assert!(
            policy.validate_redirect_url(rejected).is_err(),
            "redirect policy accepted {rejected}"
        );
    }
    assert_eq!(
        policy.classify_redirect_target(Some("https://chatgpt.com/callback?token=fixture"), false,),
        RedirectTargetClass::Invalid
    );
    assert_eq!(
        policy.classify_redirect_target(
            Some("https://chatgpt.com/callback#access_token=fixture"),
            false,
        ),
        RedirectTargetClass::Invalid
    );
    assert_eq!(
        policy.classify_redirect_target(Some("https://user@chatgpt.com/callback"), false),
        RedirectTargetClass::Invalid
    );
    assert_eq!(
        policy.classify_redirect_target(Some("https://127.0.0.1/callback"), false),
        RedirectTargetClass::BlockedHost
    );
}

#[test]
fn codex_policy_allows_openai_redirect_class_only_when_explicitly_configured() {
    let default_policy = CodexWebPolicy::new();
    assert!(default_policy
        .validate_redirect_url("https://openai.com/codex/settings/usage")
        .is_err());
    assert_eq!(
        default_policy
            .classify_redirect_target(Some("https://openai.com/codex/settings/usage"), false,),
        RedirectTargetClass::BlockedHost
    );

    let openai_policy =
        CodexWebPolicy::with_redirect_hosts_for_tests(&["chatgpt.com", "openai.com"]);
    assert!(openai_policy
        .validate_redirect_url("https://openai.com/codex/settings/usage")
        .is_err());
    assert!(openai_policy
        .validate_follow_redirect_url("https://openai.com/codex/settings/usage")
        .is_err());
    assert_eq!(
        openai_policy
            .classify_redirect_target(Some("https://openai.com/codex/settings/usage"), false,),
        RedirectTargetClass::AllowedHostOther
    );
}

#[test]
fn reqwest_live_client_rejects_non_static_hosts_before_network() {
    let request = WebRequest::new("https://attacker.example.invalid/codex/settings/usage");
    assert_eq!(
        ReqwestStaticGetClient::validate_request_for_tests(&request).unwrap_err(),
        WebClientError::TransportUnavailable
    );

    let same_host_other_path = WebRequest::new("https://chatgpt.com/not-codex/settings/usage");
    assert_eq!(
        ReqwestStaticGetClient::validate_request_for_tests(&same_host_other_path).unwrap_err(),
        WebClientError::TransportUnavailable
    );

    let direct_redirect_target = WebRequest::new("https://chatgpt.com/codex/settings/usage/");
    assert_eq!(
        ReqwestStaticGetClient::validate_request_for_tests(&direct_redirect_target).unwrap_err(),
        WebClientError::TransportUnavailable
    );

    let direct_cloud_redirect_target =
        WebRequest::new("https://chatgpt.com/codex/cloud/settings/usage");
    assert_eq!(
        ReqwestStaticGetClient::validate_request_for_tests(&direct_cloud_redirect_target)
            .unwrap_err(),
        WebClientError::TransportUnavailable
    );
}

#[test]
fn redirect_resolution_rejects_explicit_ports_before_policy_normalization() {
    let base = CodexWebPolicy::new().dashboard_url();

    assert_eq!(
        ReqwestStaticGetClient::resolve_redirect_url_for_tests(base, "/codex/cloud/settings/usage")
            .as_deref(),
        Some("https://chatgpt.com/codex/cloud/settings/usage")
    );
    assert_eq!(
        ReqwestStaticGetClient::resolve_redirect_url_for_tests(
            base,
            "//chatgpt.com/codex/cloud/settings/usage"
        )
        .as_deref(),
        Some("https://chatgpt.com/codex/cloud/settings/usage")
    );
    assert!(ReqwestStaticGetClient::resolve_redirect_url_for_tests(
        base,
        "https://chatgpt.com:443/codex/cloud/settings/usage",
    )
    .is_none());
    assert!(ReqwestStaticGetClient::resolve_redirect_url_for_tests(
        base,
        "https://chatgpt.com:444/codex/cloud/settings/usage",
    )
    .is_none());
    assert!(ReqwestStaticGetClient::resolve_redirect_url_for_tests(
        base,
        "//chatgpt.com:443/codex/cloud/settings/usage",
    )
    .is_none());
}

#[test]
fn codex_dashboard_get_uses_static_browser_like_header_profile() {
    let request = CodexWebPolicy::new().dashboard_request();
    assert_eq!(request.request_header_profile(), "browser_like");

    let headers = ReqwestStaticGetClient::static_browser_like_headers_for_tests();
    assert_eq!(
        headers,
        vec![
            (
                "User-Agent",
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ),
            (
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
            ("Accept-Language", "en-US,en;q=0.9"),
            ("Cache-Control", "no-cache"),
            ("Pragma", "no-cache"),
            ("Sec-Fetch-Dest", "document"),
            ("Sec-Fetch-Mode", "navigate"),
            ("Sec-Fetch-Site", "none"),
        ]
    );
    for (name, _value) in headers {
        assert!(
            !matches!(
                name.to_ascii_lowercase().as_str(),
                "cookie" | "authorization" | "set-cookie" | "origin" | "referer" | "x-csrf-token"
            ),
            "static header profile exposed forbidden header {name}"
        );
    }
}

#[tokio::test]
async fn async_reqwest_live_client_drops_inside_tokio_context_without_panic() {
    let client = ReqwestStaticGetClient::new();
    let request = WebRequest::new(CodexWebPolicy::new().dashboard_url());

    assert_eq!(
        client.request(request).await.unwrap_err(),
        WebClientError::TransportUnavailable
    );
    drop(client);
}

#[tokio::test]
async fn fake_web_client_is_async_compatible_without_network() {
    let client = FakeWebClient::responding(html_response("dashboard_success.html"));
    let refresh = web::refresh_with_client(web_request(Some(session())), &client)
        .await
        .expect("web refresh");

    assert_eq!(client.request_count(), 1);
    assert_eq!(refresh.snapshot.providers[0].state, ProviderState::Ok);
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn fake_success_response_normalizes_to_schema_valid_linux_web_snapshot() {
    let client = FakeWebClient::responding(html_response("dashboard_success.html"));
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");

    assert_eq!(client.requests().len(), 1);
    assert_eq!(client.requests()[0].request_header_profile, "browser_like");
    let started = find_diagnostic(&refresh.diagnostics, diagnostics::FETCH_STARTED);
    assert_allowed_response_detail_keys(started);
    assert_eq!(
        started.details.get("requestHeaderProfile"),
        Some(&serde_json::Value::from("browser_like"))
    );
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
    let refresh =
        block_on_web(web::refresh_with_client(web_request(None), &client)).expect("web refresh");

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
    let refresh = block_on_web(web::refresh_with_client(request, &client)).expect("web refresh");

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
    let refresh = block_on_web(web::refresh_with_client(request, &client)).expect("web refresh");
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
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(material)),
        &client,
    ))
    .expect("web refresh");

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
    let result = block_on_web(codex::fetch_dashboard_with_client(
        &client,
        Some(&session()),
        NOW,
        Some("other@example.invalid"),
    ));
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
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::FETCH_NONZERO_STATUS);
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("httpStatusCode"),
        Some(&serde_json::Value::from(status as u64))
    );
    assert_eq!(
        event.details.get("httpStatusClass"),
        Some(&serde_json::Value::from("server_error"))
    );
    assert_eq!(
        event.details.get("redirectPresent"),
        Some(&serde_json::Value::Bool(false))
    );
    assert_eq!(
        event.details.get("redirectHostClass"),
        Some(&serde_json::Value::from("none"))
    );
    assert_eq!(
        event.details.get("responseBodyClass"),
        Some(&serde_json::Value::from("within_cap"))
    );
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
        let event = find_diagnostic(&refresh.diagnostics, diagnostics::FETCH_NONZERO_STATUS);
        assert_allowed_response_detail_keys(event);
        assert_eq!(
            event.details.get("httpStatusCode"),
            Some(&serde_json::Value::from(status as u64))
        );
        assert_eq!(
            event.details.get("httpStatusClass"),
            Some(&serde_json::Value::from("client_error"))
        );
        assert!(!payload.contains("Authorization"));
        assert!(!payload.contains("Bearer"));
        assert!(!payload.contains("fixture-secret"));
        assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
    }
}

#[test]
fn rate_limit_status_maps_to_provider_unavailable_with_safe_summary() {
    let refresh = refresh_for_response(WebResponse::new(
        429,
        CodexWebPolicy::new().dashboard_url(),
        "rate-limit body marker",
    ));
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");
    let provider = &refresh.snapshot.providers[0];
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::FETCH_RATE_LIMITED);

    assert_eq!(provider.state, ProviderState::ProviderUnavailable);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_RATE_LIMITED.to_string()));
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("httpStatusCode"),
        Some(&serde_json::Value::from(429_u64))
    );
    assert_eq!(
        event.details.get("httpStatusClass"),
        Some(&serde_json::Value::from("client_error"))
    );
    assert!(!payload.contains("rate-limit body marker"));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn timeout_maps_to_timeout_state() {
    let client = FakeWebClient::failing(WebClientError::Timeout);
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
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
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
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
        WebResponse::new(302, CodexWebPolicy::new().dashboard_url(), "{}").with_redirect(location),
    );
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
    let provider = &refresh.snapshot.providers[0];

    assert_eq!(client.request_count(), 1);
    assert_eq!(provider.state, ProviderState::ProviderUnavailable);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::REDIRECT_BLOCKED.to_string()));
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::REDIRECT_BLOCKED);
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("httpStatusCode"),
        Some(&serde_json::Value::from(302_u64))
    );
    assert_eq!(
        event.details.get("httpStatusClass"),
        Some(&serde_json::Value::from("redirect"))
    );
    assert_eq!(
        event.details.get("redirectPresent"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectHostClass"),
        Some(&serde_json::Value::from("blocked"))
    );
    assert_eq!(
        event.details.get("redirectTargetClass"),
        Some(&serde_json::Value::from("blocked_host"))
    );
    assert_eq!(
        event.details.get("redirectFollowed"),
        Some(&serde_json::Value::Bool(false))
    );
    assert_eq!(
        event.details.get("redirectHopCount"),
        Some(&serde_json::Value::from(0_u64))
    );
    assert_eq!(
        event.details.get("finalHttpStatusCode"),
        Some(&serde_json::Value::Null)
    );
    assert_eq!(
        event.details.get("finalHttpStatusClass"),
        Some(&serde_json::Value::from("none"))
    );
    assert_eq!(
        event.details.get("redirectBlocked"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn same_path_redirect_is_followed_once_with_safe_final_status_metadata() {
    let client = FakeWebClient::responding_sequence(vec![
        WebResponse::new(307, CodexWebPolicy::new().dashboard_url(), "")
            .with_redirect(CodexWebPolicy::new().dashboard_url()),
        html_response("dashboard_success.html"),
    ]);
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");
    let provider = &refresh.snapshot.providers[0];
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::FETCH_FINISHED);

    assert_eq!(client.request_count(), 2);
    assert_eq!(
        client.requests()[0].url,
        CodexWebPolicy::new().dashboard_url()
    );
    assert_eq!(
        client.requests()[1].url,
        CodexWebPolicy::new().dashboard_url()
    );
    for request in client.requests() {
        assert_eq!(request.request_header_profile, "browser_like");
        assert!(request.session_material_attached);
        assert!(request.session_material_bytes > 0);
        assert_eq!(request.timeout_ms, 15_000);
        assert_eq!(
            request.response_size_limit,
            CodexWebPolicy::new().response_size_limit()
        );
        let request_debug = format!("{request:?}");
        assert!(!request_debug.contains("fixture_session"));
        assert!(!request_debug.contains("fixture-value"));
    }
    assert_eq!(provider.state, ProviderState::Ok);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_FINISHED.to_string()));
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("httpStatusCode"),
        Some(&serde_json::Value::from(307_u64))
    );
    assert_eq!(
        event.details.get("httpStatusClass"),
        Some(&serde_json::Value::from("redirect"))
    );
    assert_eq!(
        event.details.get("redirectPresent"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectHostClass"),
        Some(&serde_json::Value::from("allowed"))
    );
    assert_eq!(
        event.details.get("redirectTargetClass"),
        Some(&serde_json::Value::from("same_host_usage_path"))
    );
    assert_eq!(
        event.details.get("redirectPathFamily"),
        Some(&serde_json::Value::from("codex_usage"))
    );
    assert_eq!(
        event.details.get("redirectPathDepth"),
        Some(&serde_json::Value::from("three"))
    );
    assert_eq!(
        event.details.get("redirectQueryClass"),
        Some(&serde_json::Value::from("none"))
    );
    assert_eq!(
        event.details.get("redirectCanFollow"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectFollowed"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectHopCount"),
        Some(&serde_json::Value::from(1_u64))
    );
    assert_eq!(
        event.details.get("finalHttpStatusCode"),
        Some(&serde_json::Value::from(200_u64))
    );
    assert_eq!(
        event.details.get("finalHttpStatusClass"),
        Some(&serde_json::Value::from("success"))
    );
    assert!(!payload.contains("Location"));
    assert!(!payload.contains("fixture_session"));
    assert!(!payload.contains("fixture-value"));
    assert!(!payload.contains("Cookie:"));
    assert!(!payload.contains("Set-Cookie"));
    assert!(!payload.contains("Authorization"));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn trailing_slash_redirect_is_followed_once() {
    let trailing = "https://chatgpt.com/codex/settings/usage/";
    let client = FakeWebClient::responding_sequence(vec![
        WebResponse::new(308, CodexWebPolicy::new().dashboard_url(), "").with_redirect(trailing),
        html_response_at(trailing, "dashboard_success.html"),
    ]);
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
    let provider = &refresh.snapshot.providers[0];
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::FETCH_FINISHED);

    assert_eq!(client.request_count(), 2);
    assert_eq!(client.requests()[1].url, trailing);
    assert_eq!(provider.state, ProviderState::Ok);
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("redirectTargetClass"),
        Some(&serde_json::Value::from("same_host_usage_path"))
    );
    assert_eq!(
        event.details.get("redirectPathFamily"),
        Some(&serde_json::Value::from("codex_usage"))
    );
    assert_eq!(
        event.details.get("redirectPathDepth"),
        Some(&serde_json::Value::from("three"))
    );
    assert_eq!(
        event.details.get("redirectQueryClass"),
        Some(&serde_json::Value::from("none"))
    );
    assert_eq!(
        event.details.get("redirectCanFollow"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectFollowed"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectHopCount"),
        Some(&serde_json::Value::from(1_u64))
    );
    assert_eq!(
        event.details.get("finalHttpStatusCode"),
        Some(&serde_json::Value::from(200_u64))
    );
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn codex_cloud_usage_redirect_is_followed_once_with_safe_final_status_metadata() {
    let cloud_usage = "https://chatgpt.com/codex/cloud/settings/usage";
    let client = FakeWebClient::responding_sequence(vec![
        WebResponse::new(307, CodexWebPolicy::new().dashboard_url(), "").with_redirect(cloud_usage),
        html_response_at(cloud_usage, "dashboard_success.html"),
    ]);
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");
    let provider = &refresh.snapshot.providers[0];
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::FETCH_FINISHED);

    assert_eq!(client.request_count(), 2);
    assert_eq!(client.requests()[1].url, cloud_usage);
    assert_eq!(provider.state, ProviderState::Ok);
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("httpStatusCode"),
        Some(&serde_json::Value::from(307_u64))
    );
    assert_eq!(
        event.details.get("httpStatusClass"),
        Some(&serde_json::Value::from("redirect"))
    );
    assert_eq!(
        event.details.get("redirectPresent"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectHostClass"),
        Some(&serde_json::Value::from("allowed"))
    );
    assert_eq!(
        event.details.get("redirectTargetClass"),
        Some(&serde_json::Value::from("same_host_usage_path"))
    );
    assert_eq!(
        event.details.get("redirectPathFamily"),
        Some(&serde_json::Value::from("codex_usage"))
    );
    assert_eq!(
        event.details.get("redirectPathDepth"),
        Some(&serde_json::Value::from("many"))
    );
    assert_eq!(
        event.details.get("redirectQueryClass"),
        Some(&serde_json::Value::from("none"))
    );
    assert_eq!(
        event.details.get("redirectCanFollow"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectFollowed"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectHopCount"),
        Some(&serde_json::Value::from(1_u64))
    );
    assert_eq!(
        event.details.get("finalHttpStatusCode"),
        Some(&serde_json::Value::from(200_u64))
    );
    assert_eq!(
        event.details.get("finalHttpStatusClass"),
        Some(&serde_json::Value::from("success"))
    );
    assert!(!payload.contains("/codex/cloud/settings/usage"));
    assert!(!payload.contains("Location"));
    assert!(!payload.contains("Cookie:"));
    assert!(!payload.contains("Set-Cookie"));
    assert!(!payload.contains("Authorization"));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn codex_cloud_usage_redirect_with_present_query_is_not_followed_and_redacts_query() {
    let cloud_usage = "https://chatgpt.com/codex/cloud/settings/usage?utm_campaign=limits";
    let client = FakeWebClient::responding(
        WebResponse::new(302, CodexWebPolicy::new().dashboard_url(), "").with_redirect(cloud_usage),
    );
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");
    let provider = &refresh.snapshot.providers[0];
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::REDIRECT_BLOCKED);

    assert_eq!(client.request_count(), 1);
    assert_eq!(provider.state, ProviderState::ProviderUnavailable);
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("redirectTargetClass"),
        Some(&serde_json::Value::from("same_host_usage_path"))
    );
    assert_eq!(
        event.details.get("redirectPathFamily"),
        Some(&serde_json::Value::from("codex_usage"))
    );
    assert_eq!(
        event.details.get("redirectPathDepth"),
        Some(&serde_json::Value::from("many"))
    );
    assert_eq!(
        event.details.get("redirectQueryClass"),
        Some(&serde_json::Value::from("present"))
    );
    assert_eq!(
        event.details.get("redirectCanFollow"),
        Some(&serde_json::Value::Bool(false))
    );
    assert_eq!(
        event.details.get("redirectFollowed"),
        Some(&serde_json::Value::Bool(false))
    );
    assert_eq!(
        event.details.get("redirectHopCount"),
        Some(&serde_json::Value::from(0_u64))
    );
    assert_eq!(
        event.details.get("redirectBlocked"),
        Some(&serde_json::Value::Bool(true))
    );
    assert!(!payload.contains("/codex/cloud/settings/usage"));
    assert!(!payload.contains("utm_campaign"));
    assert!(!payload.contains("limits"));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn codex_cloud_usage_redirect_parser_failure_after_final_200_is_redacted_parse_error() {
    let cloud_usage = "https://chatgpt.com/codex/cloud/settings/usage";
    let body_marker = "codexbar-cloud-final-body-marker Location: https://chatgpt.com/codex/cloud/settings/usage?token=fixture-secret Set-Cookie: fixture_session=fixture-value /home/example/.config/google-chrome/Default/Network/Cookies";
    let client = FakeWebClient::responding_sequence(vec![
        WebResponse::new(307, CodexWebPolicy::new().dashboard_url(), "").with_redirect(cloud_usage),
        WebResponse::new(200, cloud_usage, body_marker).with_content_type("text/html"),
    ]);
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");
    let provider = &refresh.snapshot.providers[0];
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::FETCH_PARSE_ERROR);

    assert_eq!(client.request_count(), 2);
    assert_eq!(provider.state, ProviderState::ParseError);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_PARSE_ERROR.to_string()));
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("httpStatusCode"),
        Some(&serde_json::Value::from(307_u64))
    );
    assert_eq!(
        event.details.get("redirectTargetClass"),
        Some(&serde_json::Value::from("same_host_usage_path"))
    );
    assert_eq!(
        event.details.get("redirectPathFamily"),
        Some(&serde_json::Value::from("codex_usage"))
    );
    assert_eq!(
        event.details.get("redirectCanFollow"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectFollowed"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectHopCount"),
        Some(&serde_json::Value::from(1_u64))
    );
    assert_eq!(
        event.details.get("finalHttpStatusCode"),
        Some(&serde_json::Value::from(200_u64))
    );
    assert_eq!(
        event.details.get("finalHttpStatusClass"),
        Some(&serde_json::Value::from("success"))
    );
    assert!(!payload.contains(body_marker));
    assert!(!payload.contains("/codex/cloud/settings/usage"));
    assert!(!payload.contains("token="));
    assert!(!payload.contains("Location"));
    assert!(!payload.contains("Set-Cookie"));
    assert!(!payload.contains("fixture_session"));
    assert!(!payload.contains("fixture-value"));
    assert!(!payload.contains("/home/example"));
    assert!(!payload.contains("Network/Cookies"));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn codex_usage_redirect_with_present_query_is_not_followed_and_redacts_query() {
    let usage = "https://chatgpt.com/codex/usage?utm_campaign=limits";
    let client = FakeWebClient::responding(
        WebResponse::new(302, CodexWebPolicy::new().dashboard_url(), "").with_redirect(usage),
    );
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");
    let provider = &refresh.snapshot.providers[0];
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::REDIRECT_BLOCKED);

    assert_eq!(client.request_count(), 1);
    assert_eq!(provider.state, ProviderState::ProviderUnavailable);
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("redirectTargetClass"),
        Some(&serde_json::Value::from("same_host_usage_path"))
    );
    assert_eq!(
        event.details.get("redirectPathFamily"),
        Some(&serde_json::Value::from("codex_usage"))
    );
    assert_eq!(
        event.details.get("redirectPathDepth"),
        Some(&serde_json::Value::from("two"))
    );
    assert_eq!(
        event.details.get("redirectQueryClass"),
        Some(&serde_json::Value::from("present"))
    );
    assert_eq!(
        event.details.get("redirectCanFollow"),
        Some(&serde_json::Value::Bool(false))
    );
    assert_eq!(
        event.details.get("redirectFollowed"),
        Some(&serde_json::Value::Bool(false))
    );
    assert_eq!(
        event.details.get("redirectHopCount"),
        Some(&serde_json::Value::from(0_u64))
    );
    assert_eq!(
        event.details.get("redirectBlocked"),
        Some(&serde_json::Value::Bool(true))
    );
    assert!(!payload.contains("/codex/usage"));
    assert!(!payload.contains("utm_campaign"));
    assert!(!payload.contains("limits"));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn codex_settings_redirect_family_is_followed_once() {
    let settings = "https://chatgpt.com/codex/settings";
    let client = FakeWebClient::responding_sequence(vec![
        WebResponse::new(302, CodexWebPolicy::new().dashboard_url(), "").with_redirect(settings),
        html_response_at(settings, "dashboard_success.html"),
    ]);
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
    let provider = &refresh.snapshot.providers[0];
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::FETCH_FINISHED);

    assert_eq!(client.request_count(), 2);
    assert_eq!(client.requests()[1].url, settings);
    assert_eq!(provider.state, ProviderState::Ok);
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("redirectTargetClass"),
        Some(&serde_json::Value::from("same_host_canonical"))
    );
    assert_eq!(
        event.details.get("redirectPathFamily"),
        Some(&serde_json::Value::from("codex_settings"))
    );
    assert_eq!(
        event.details.get("redirectPathDepth"),
        Some(&serde_json::Value::from("two"))
    );
    assert_eq!(
        event.details.get("redirectQueryClass"),
        Some(&serde_json::Value::from("none"))
    );
    assert_eq!(
        event.details.get("redirectCanFollow"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectFollowed"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn multi_hop_redirect_stops_after_one_follow() {
    let client = FakeWebClient::responding_sequence(vec![
        WebResponse::new(307, CodexWebPolicy::new().dashboard_url(), "")
            .with_redirect(CodexWebPolicy::new().dashboard_url()),
        WebResponse::new(302, CodexWebPolicy::new().dashboard_url(), "")
            .with_redirect("https://chatgpt.com/codex/settings/usage/"),
    ]);
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");
    let provider = &refresh.snapshot.providers[0];
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::REDIRECT_BLOCKED);

    assert_eq!(client.request_count(), 2);
    assert_eq!(provider.state, ProviderState::ProviderUnavailable);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::REDIRECT_BLOCKED.to_string()));
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("httpStatusCode"),
        Some(&serde_json::Value::from(307_u64))
    );
    assert_eq!(
        event.details.get("redirectTargetClass"),
        Some(&serde_json::Value::from("same_host_usage_path"))
    );
    assert_eq!(
        event.details.get("redirectPathFamily"),
        Some(&serde_json::Value::from("codex_usage"))
    );
    assert_eq!(
        event.details.get("redirectCanFollow"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectFollowed"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectHopCount"),
        Some(&serde_json::Value::from(1_u64))
    );
    assert_eq!(
        event.details.get("finalHttpStatusCode"),
        Some(&serde_json::Value::from(302_u64))
    );
    assert_eq!(
        event.details.get("finalHttpStatusClass"),
        Some(&serde_json::Value::from("redirect"))
    );
    assert_eq!(
        event.details.get("redirectBlocked"),
        Some(&serde_json::Value::Bool(true))
    );
    assert!(!payload.contains("Location"));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn same_host_unknown_redirect_path_is_not_followed() {
    let client = FakeWebClient::responding(
        WebResponse::new(302, CodexWebPolicy::new().dashboard_url(), "")
            .with_redirect("https://chatgpt.com/not-codex"),
    );
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");
    let provider = &refresh.snapshot.providers[0];
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::REDIRECT_BLOCKED);

    assert_eq!(client.request_count(), 1);
    assert_eq!(provider.state, ProviderState::ProviderUnavailable);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::REDIRECT_BLOCKED.to_string()));
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("redirectTargetClass"),
        Some(&serde_json::Value::from("same_host_other"))
    );
    assert_eq!(
        event.details.get("redirectFollowed"),
        Some(&serde_json::Value::Bool(false))
    );
    assert_eq!(
        event.details.get("redirectBlocked"),
        Some(&serde_json::Value::Bool(true))
    );
    assert!(!payload.contains("not-codex"));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn same_host_redirect_path_families_are_classified_without_following_unsafe_routes() {
    for (location, target_class, path_family, path_depth, query_class, diagnostic, state) in [
        (
            "https://chatgpt.com/signin",
            "same_host_login_path",
            "auth_login",
            "one",
            "none",
            diagnostics::FETCH_NONZERO_STATUS,
            ProviderState::CookieRejected,
        ),
        (
            "https://chatgpt.com/oauth/callback",
            "same_host_login_path",
            "auth_callback",
            "two",
            "none",
            diagnostics::FETCH_NONZERO_STATUS,
            ProviderState::CookieRejected,
        ),
        (
            "https://chatgpt.com/api/data",
            "same_host_other",
            "api",
            "two",
            "none",
            diagnostics::REDIRECT_BLOCKED,
            ProviderState::ProviderUnavailable,
        ),
        (
            "https://chatgpt.com/_next/static/chunk.js",
            "same_host_other",
            "static_asset",
            "three",
            "none",
            diagnostics::REDIRECT_BLOCKED,
            ProviderState::ProviderUnavailable,
        ),
        (
            "https://chatgpt.com/",
            "same_host_other",
            "root",
            "zero",
            "none",
            diagnostics::REDIRECT_BLOCKED,
            ProviderState::ProviderUnavailable,
        ),
        (
            "https://chatgpt.com/codex/other",
            "same_host_other",
            "codex_other",
            "two",
            "none",
            diagnostics::REDIRECT_BLOCKED,
            ProviderState::ProviderUnavailable,
        ),
        (
            "https://chatgpt.com/codex/cloud/other",
            "same_host_other",
            "codex_other",
            "three",
            "none",
            diagnostics::REDIRECT_BLOCKED,
            ProviderState::ProviderUnavailable,
        ),
        (
            "https://chatgpt.com/codex/cloud/settings/usage/",
            "same_host_other",
            "codex_other",
            "many",
            "none",
            diagnostics::REDIRECT_BLOCKED,
            ProviderState::ProviderUnavailable,
        ),
        (
            "https://chatgpt.com/opaque",
            "same_host_other",
            "unknown",
            "one",
            "none",
            diagnostics::REDIRECT_BLOCKED,
            ProviderState::ProviderUnavailable,
        ),
        (
            "https://chatgpt.com/codex/settings/usage?token=fixture-secret",
            "invalid",
            "codex_usage",
            "three",
            "token_like",
            diagnostics::REDIRECT_BLOCKED,
            ProviderState::ProviderUnavailable,
        ),
        (
            "https://chatgpt.com/codex/cloud/settings/usage?token=fixture-secret",
            "invalid",
            "codex_usage",
            "many",
            "token_like",
            diagnostics::REDIRECT_BLOCKED,
            ProviderState::ProviderUnavailable,
        ),
    ] {
        let client = FakeWebClient::responding(
            WebResponse::new(302, CodexWebPolicy::new().dashboard_url(), "")
                .with_redirect(location),
        );
        let refresh = block_on_web(web::refresh_with_client(
            web_request(Some(session())),
            &client,
        ))
        .expect("web refresh");
        let payload = serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics))
            .expect("refresh json");
        let event = find_diagnostic(&refresh.diagnostics, diagnostic);

        assert_eq!(client.request_count(), 1);
        assert_eq!(refresh.snapshot.providers[0].state, state);
        assert_redirect_decision(
            event,
            target_class,
            path_family,
            path_depth,
            query_class,
            false,
            false,
        );
        assert!(!payload.contains(location));
        assert!(!payload.contains("signin"));
        assert!(!payload.contains("oauth"));
        assert!(!payload.contains("_next"));
        assert!(!payload.contains("chunk.js"));
        assert!(!payload.contains("opaque"));
        assert!(!payload.contains("token="));
        assert!(!payload.contains("fixture-secret"));
        assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
    }
}

#[test]
fn openai_redirect_is_not_followed_by_default() {
    let client = FakeWebClient::responding(
        WebResponse::new(302, CodexWebPolicy::new().dashboard_url(), "")
            .with_redirect("https://openai.com/codex/settings/usage"),
    );
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::REDIRECT_BLOCKED);

    assert_eq!(client.request_count(), 1);
    assert_eq!(
        refresh.snapshot.providers[0].state,
        ProviderState::ProviderUnavailable
    );
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("redirectHostClass"),
        Some(&serde_json::Value::from("blocked"))
    );
    assert_eq!(
        event.details.get("redirectTargetClass"),
        Some(&serde_json::Value::from("blocked_host"))
    );
    assert_eq!(
        event.details.get("redirectFollowed"),
        Some(&serde_json::Value::Bool(false))
    );
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn invalid_redirect_location_is_blocked_without_location_exposure() {
    let client = FakeWebClient::responding(
        WebResponse::new(302, CodexWebPolicy::new().dashboard_url(), "")
            .with_invalid_redirect_for_tests(),
    );
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");
    let provider = &refresh.snapshot.providers[0];
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::REDIRECT_BLOCKED);

    assert_eq!(provider.state, ProviderState::ProviderUnavailable);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::REDIRECT_BLOCKED.to_string()));
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("redirectPresent"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        event.details.get("redirectHostClass"),
        Some(&serde_json::Value::from("invalid"))
    );
    assert_eq!(
        event.details.get("redirectTargetClass"),
        Some(&serde_json::Value::from("invalid"))
    );
    assert_eq!(
        event.details.get("redirectFollowed"),
        Some(&serde_json::Value::Bool(false))
    );
    assert!(!payload.contains("Location"));
    assert_payloads_are_schema_valid_and_public(&refresh.snapshot, &refresh.diagnostics);
}

#[test]
fn unsafe_same_host_redirect_shapes_are_rejected_without_location_exposure() {
    for (location, forbidden) in [
        (
            "https://user:fixture-secret@chatgpt.com/codex/settings/usage",
            "fixture-secret",
        ),
        (
            "https://chatgpt.com/codex/settings/usage#access_token=fixture-secret",
            "access_token",
        ),
        (
            "https://chatgpt.com/codex/settings/usage?token=fixture-secret",
            "token=",
        ),
        ("https://chatgpt.com:443/codex/cloud/settings/usage", ":443"),
        (
            "https://chatgpt.com/codex/cloud/settings/usage#",
            "/codex/cloud/settings/usage",
        ),
        (
            "https://chatgpt.com/codex/cloud/settings/usage#fragment",
            "fragment",
        ),
        (
            "https://chatgpt.com/codex/cloud/settings/usage?token=fixture-secret",
            "token=",
        ),
    ] {
        let client = FakeWebClient::responding(
            WebResponse::new(302, CodexWebPolicy::new().dashboard_url(), "")
                .with_redirect(location),
        );
        let refresh = block_on_web(web::refresh_with_client(
            web_request(Some(session())),
            &client,
        ))
        .expect("web refresh");
        let payload = serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics))
            .expect("refresh json");
        let event = find_diagnostic(&refresh.diagnostics, diagnostics::REDIRECT_BLOCKED);

        assert_eq!(client.request_count(), 1);
        assert_eq!(
            refresh.snapshot.providers[0].state,
            ProviderState::ProviderUnavailable
        );
        assert_allowed_response_detail_keys(event);
        assert_eq!(
            event.details.get("redirectTargetClass"),
            Some(&serde_json::Value::from("invalid"))
        );
        assert_eq!(
            event.details.get("redirectFollowed"),
            Some(&serde_json::Value::Bool(false))
        );
        assert!(!payload.contains("Location"));
        assert!(!payload.contains(forbidden));
        assert!(!payload.contains("fixture-secret"));
        common::assert_public_json_safe(&payload);
    }
}

#[test]
fn allowed_login_redirect_maps_to_cookie_rejected_without_location_exposure() {
    let client = FakeWebClient::responding(
        WebResponse::new(302, CodexWebPolicy::new().dashboard_url(), "")
            .with_redirect("https://chatgpt.com/auth/login"),
    );
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
    let payload =
        serde_json::to_string(&(&refresh.snapshot, &refresh.diagnostics)).expect("refresh json");
    let provider = &refresh.snapshot.providers[0];

    assert_eq!(client.request_count(), 1);
    assert_eq!(provider.state, ProviderState::CookieRejected);
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::COOKIE_REJECTED.to_string()));
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_NONZERO_STATUS.to_string()));
    let event = find_diagnostic(&refresh.diagnostics, diagnostics::FETCH_NONZERO_STATUS);
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("httpStatusClass"),
        Some(&serde_json::Value::from("redirect"))
    );
    assert_eq!(
        event.details.get("redirectHostClass"),
        Some(&serde_json::Value::from("allowed"))
    );
    assert_eq!(
        event.details.get("redirectTargetClass"),
        Some(&serde_json::Value::from("same_host_login_path"))
    );
    assert_eq!(
        event.details.get("redirectFollowed"),
        Some(&serde_json::Value::Bool(false))
    );
    assert_eq!(
        event.details.get("finalHttpStatusCode"),
        Some(&serde_json::Value::Null)
    );
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
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
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
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
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
    let refresh = block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh");
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
    let _refresh = block_on_web(web::refresh_with_client(
        web_request(Some(material)),
        &client,
    ))
    .expect("web refresh");
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
        WebRequest::new(
            "https://user@chatgpt.com/auth/login/fixture-secret?token=fixture-secret#access_token=fixture-secret"
        )
    );
    assert!(request_debug.contains("chatgpt.com/[path_depth"));
    assert!(!request_debug.contains("user@"));
    assert!(!request_debug.contains("auth/login"));
    assert!(!request_debug.contains("codex/settings/usage"));
    assert!(!request_debug.contains("access_token"));
    assert!(!request_debug.contains("token="));
    assert!(!request_debug.contains("fixture-secret"));
    assert!(!request_debug.contains("Mozilla/5.0"));
    assert!(!request_debug.contains("text/html,application/xhtml+xml"));
    assert!(!request_debug.contains("en-US,en;q=0.9"));
    assert!(request_debug.contains("request_header_profile"));

    let response_debug = format!(
        "{:?}",
        WebResponse::new(
            302,
            "https://chatgpt.com/auth/callback/fixture-secret?token=fixture-secret#access_token=fixture-secret",
            "raw-body-marker",
        )
        .with_redirect("https://attacker.example.invalid/callback/fixture-secret?token=fixture-secret")
        .with_content_type("text/html; charset=utf-8")
    );
    assert!(response_debug.contains("redirect_present"));
    assert!(!response_debug.contains("attacker.example.invalid"));
    assert!(!response_debug.contains("auth/callback"));
    assert!(!response_debug.contains("callback/fixture"));
    assert!(!response_debug.contains("access_token"));
    assert!(!response_debug.contains("token="));
    assert!(!response_debug.contains("fixture-secret"));
    assert!(!response_debug.contains("raw-body-marker"));

    let cloud_response_debug = format!(
        "{:?}",
        WebResponse::new(
            302,
            "https://chatgpt.com/codex/cloud/settings/usage",
            "cloud-body-marker",
        )
        .with_redirect("https://chatgpt.com/codex/cloud/settings/usage?token=fixture-secret")
    );
    assert!(cloud_response_debug.contains("chatgpt.com/[path_depth"));
    assert!(!cloud_response_debug.contains("/codex/cloud/settings/usage"));
    assert!(!cloud_response_debug.contains("token="));
    assert!(!cloud_response_debug.contains("fixture-secret"));
    assert!(!cloud_response_debug.contains("cloud-body-marker"));
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
    assert_eq!(summary.request_header_profile, "browser_like");
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
fn live_recon_summary_includes_safe_http_response_metadata_only() {
    let refresh = refresh_for_response(
        WebResponse::new(
            503,
            CodexWebPolicy::new().dashboard_url(),
            "summary response body marker",
        )
        .with_content_type("text/plain; charset=utf-8"),
    );
    let provider = &refresh.snapshot.providers[0];
    let result = recon_refresh_result(RefreshStatus::Error, false, []);

    let summary = LiveReconSummary::from_provider_result_material_and_diagnostics(
        provider,
        &result,
        codexbar_linuxd::browser::cookie_store::BrowserCookieMaterialSummary::default(),
        &refresh.diagnostics,
    );
    let summary_json = serde_json::to_string(&summary).expect("summary json");

    assert_eq!(summary.classification, LiveReconClassification::Non200);
    assert_eq!(summary.request_header_profile, "browser_like");
    assert_eq!(summary.http_response.http_status_code, Some(503));
    assert_eq!(summary.http_response.http_status_class, "server_error");
    assert!(!summary.http_response.redirect_present);
    assert_eq!(summary.http_response.redirect_host_class, "none");
    assert_eq!(summary.http_response.redirect_target_class, "none");
    assert_eq!(summary.http_response.redirect_path_family, "none");
    assert_eq!(summary.http_response.redirect_path_depth, "unknown");
    assert_eq!(summary.http_response.redirect_query_class, "none");
    assert!(!summary.http_response.redirect_can_follow);
    assert!(!summary.http_response.redirect_followed);
    assert_eq!(summary.http_response.redirect_hop_count, 0);
    assert_eq!(summary.http_response.final_http_status_code, None);
    assert_eq!(summary.http_response.final_http_status_class, "none");
    assert_eq!(summary.http_response.content_type_class, "text");
    assert_eq!(summary.http_response.response_body_class, "within_cap");
    assert_eq!(summary.http_response.response_size_bucket, "small");
    assert!(summary_json.contains(r#""httpStatusCode":503"#));
    assert!(summary_json.contains(r#""httpStatusClass":"server_error""#));
    assert!(summary_json.contains(r#""redirectTargetClass":"none""#));
    assert!(summary_json.contains(r#""redirectPathFamily":"none""#));
    assert!(summary_json.contains(r#""redirectPathDepth":"unknown""#));
    assert!(summary_json.contains(r#""redirectQueryClass":"none""#));
    assert!(summary_json.contains(r#""redirectCanFollow":false"#));
    assert!(summary_json.contains(r#""redirectFollowed":false"#));
    assert!(summary_json.contains(r#""redirectHopCount":0"#));
    assert!(summary_json.contains(r#""finalHttpStatusCode":null"#));
    assert!(summary_json.contains(r#""finalHttpStatusClass":"none""#));
    assert!(!summary_json.contains("summary response body marker"));
    assert!(!summary_json.contains("Location"));
    assert!(!summary_json.contains("Set-Cookie"));
    assert!(!summary_json.contains("Cookie:"));
    assert!(!summary_json.contains("Authorization"));
    common::assert_public_json_safe(&summary_json);
    assert_no_live_web_secret_markers("live Codex web response summary", &summary_json);
}

#[test]
fn live_recon_summary_includes_redirect_family_classes_without_raw_target() {
    let refresh = refresh_for_response(
        WebResponse::new(
            302,
            CodexWebPolicy::new().dashboard_url(),
            "redirect body marker",
        )
        .with_redirect("https://chatgpt.com/codex/cloud/settings/usage?token=fixture-secret"),
    );
    let provider = &refresh.snapshot.providers[0];
    let result = recon_refresh_result(RefreshStatus::Error, false, []);

    let summary = LiveReconSummary::from_provider_result_material_and_diagnostics(
        provider,
        &result,
        codexbar_linuxd::browser::cookie_store::BrowserCookieMaterialSummary::default(),
        &refresh.diagnostics,
    );
    let summary_json = serde_json::to_string(&summary).expect("summary json");

    assert_eq!(
        summary.classification,
        LiveReconClassification::RedirectBlocked
    );
    assert_eq!(summary.web_fetch, LiveReconWebFetch::Blocked);
    assert!(summary.http_response.redirect_present);
    assert_eq!(summary.http_response.redirect_host_class, "invalid");
    assert_eq!(summary.http_response.redirect_target_class, "invalid");
    assert_eq!(summary.http_response.redirect_path_family, "codex_usage");
    assert_eq!(summary.http_response.redirect_path_depth, "many");
    assert_eq!(summary.http_response.redirect_query_class, "token_like");
    assert!(!summary.http_response.redirect_can_follow);
    assert!(!summary.http_response.redirect_followed);
    assert_eq!(summary.http_response.redirect_hop_count, 0);
    assert!(summary_json.contains(r#""redirectPathFamily":"codex_usage""#));
    assert!(summary_json.contains(r#""redirectPathDepth":"many""#));
    assert!(summary_json.contains(r#""redirectQueryClass":"token_like""#));
    assert!(summary_json.contains(r#""redirectCanFollow":false"#));
    assert!(!summary_json.contains("/codex/cloud/settings/usage"));
    assert!(!summary_json.contains("token="));
    assert!(!summary_json.contains("fixture-secret"));
    assert!(!summary_json.contains("redirect body marker"));
    assert!(!summary_json.contains("Location"));
    common::assert_public_json_safe(&summary_json);
    assert_no_live_web_secret_markers("live Codex web redirect summary", &summary_json);
}

#[test]
fn live_recon_summary_includes_safe_cookie_material_counts_only() {
    let (tmp, _paths) = common::temp_paths();
    create_chatgpt_cookie_db(
        tmp.path(),
        &[
            ChatgptCookieRow::plaintext("plain_live", "fixture-chatgpt-live"),
            ChatgptCookieRow::encrypted_v11("encrypted_live"),
        ],
    );
    let material_summary = browser::collect_session_material(BrowserSessionRequest {
        providers: vec!["codex".to_string()],
        settings: Default::default(),
        roots: BrowserDiscoveryRoots::synthetic_home(tmp.path().to_path_buf()).canonicalized(),
        decryptor_mode: BrowserDecryptorMode::Plain,
    })
    .material_summary;
    let provider = recon_provider(
        ProviderState::MissingDependency,
        vec![
            browser_diagnostics::COOKIE_DECRYPTION_UNAVAILABLE,
            diagnostics::COOKIE_ABSENT,
        ],
    );
    let result = recon_refresh_result(RefreshStatus::Error, false, []);

    let summary =
        LiveReconSummary::from_provider_result_and_material(&provider, &result, material_summary);
    let summary_json = serde_json::to_string(&summary).expect("summary json");

    assert_eq!(summary.cookie_material.profiles_discovered, 1);
    assert_eq!(summary.cookie_material.candidate_cookie_rows, 2);
    assert_eq!(summary.cookie_material.plaintext_value_rows, 1);
    assert_eq!(summary.cookie_material.encrypted_value_rows, 1);
    assert_eq!(summary.cookie_material.encrypted_prefixes.v11, 1);
    assert_eq!(summary.cookie_material.usable_session_cookies, 0);
    assert_eq!(
        summary.cookie_material.decryptor_backend,
        DecryptorBackend::Plain
    );
    assert_eq!(
        summary.cookie_material.decryption_status,
        DecryptionStatus::Unavailable
    );
    assert_eq!(
        summary.cookie_material.decryption_failure_class,
        DecryptionFailureClass::KeyringNeeded
    );
    assert!(!summary_json.contains("plain_live"));
    assert!(!summary_json.contains("encrypted_live"));
    assert!(!summary_json.contains("fixture-chatgpt-live"));
    assert!(!summary_json.contains(".chatgpt.com"));
    assert!(!summary_json.contains(&tmp.path().display().to_string()));
    assert!(!summary_json.contains("Network/Cookies"));
    common::assert_public_json_safe(&summary_json);
    assert_no_live_web_secret_markers("live Codex web material summary", &summary_json);
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
            ProviderState::MissingDependency,
            vec![
                browser_diagnostics::COOKIE_DECRYPTION_UNAVAILABLE,
                diagnostics::COOKIE_ABSENT,
            ],
            LiveReconClassification::UnknownSafeFailure,
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
        if classification == LiveReconClassification::Non200 {
            assert!(summary_json.contains(r#""classification":"non_200""#));
            assert!(!summary_json.contains(r#""classification":"non200""#));
        }
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
async fn live_transport_with_plaintext_cookie_builds_session_material() {
    let (tmp, paths) = common::temp_paths();
    create_chatgpt_cookie_db(
        tmp.path(),
        &[ChatgptCookieRow::plaintext(
            "plain_live",
            "fixture-chatgpt-live",
        )],
    );
    let app = App::new_with_runtime(
        paths,
        AppRuntime::production()
            .with_browser_roots(BrowserDiscoveryRoots::synthetic_home(
                tmp.path().to_path_buf(),
            ))
            .with_codex_web_live_transport_for_tests(true)
            .with_codex_web_fixture(CodexWebFixture::Success),
    )
    .expect("app");
    let start = app
        .start_refresh(CODEX_LIVE_WEB_REFRESH_OPTIONS_JSON)
        .expect("refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("refresh should start");
    };
    let completion = app.finish_refresh(&refresh_id).await.expect("refresh");
    let snapshot: Snapshot = serde_json::from_str(&completion.snapshot_json).expect("snapshot");
    let result: RefreshResult = serde_json::from_str(&completion.result_json).expect("result");
    let provider = &snapshot.providers[0];
    let payload =
        serde_json::to_string(&(&snapshot, &result, &completion.provider_events)).expect("payload");

    assert_eq!(provider.state, ProviderState::Ok);
    assert!(result.cache_written);
    assert!(provider
        .diagnostic_codes
        .contains(&browser_diagnostics::COOKIE_FOUND.to_string()));
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_STARTED.to_string()));
    assert!(!provider
        .diagnostic_codes
        .contains(&diagnostics::COOKIE_ABSENT.to_string()));
    assert!(!payload.contains("plain_live"));
    assert!(!payload.contains("fixture-chatgpt-live"));
    common::assert_public_json_safe(&payload);
    assert_no_live_web_secret_markers("plaintext live transport payload", &payload);
}

#[tokio::test]
async fn live_transport_plain_backend_decrypts_v10_basic_cookie_and_fetches() {
    let (tmp, paths) = common::temp_paths();
    create_chatgpt_cookie_db(tmp.path(), &[ChatgptCookieRow::encrypted_v10_basic()]);
    let app = App::new_with_runtime(
        paths,
        AppRuntime::production()
            .with_browser_roots(BrowserDiscoveryRoots::synthetic_home(
                tmp.path().to_path_buf(),
            ))
            .with_codex_web_live_transport_for_tests(true)
            .with_codex_web_fixture(CodexWebFixture::Success),
    )
    .expect("app");
    let start = app
        .start_refresh(CODEX_LIVE_WEB_REFRESH_OPTIONS_JSON)
        .expect("refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("refresh should start");
    };
    let completion = app.finish_refresh(&refresh_id).await.expect("refresh");
    let snapshot: Snapshot = serde_json::from_str(&completion.snapshot_json).expect("snapshot");
    let result: RefreshResult = serde_json::from_str(&completion.result_json).expect("result");
    let provider = &snapshot.providers[0];
    let payload =
        serde_json::to_string(&(&snapshot, &result, &completion.provider_events)).expect("payload");

    assert_eq!(provider.state, ProviderState::Ok);
    assert!(result.cache_written);
    assert!(provider
        .diagnostic_codes
        .contains(&browser_diagnostics::COOKIE_DECRYPTED.to_string()));
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_STARTED.to_string()));
    assert!(!provider
        .diagnostic_codes
        .contains(&diagnostics::COOKIE_ABSENT.to_string()));
    assert!(!payload.contains("v10_basic_live"));
    assert!(!payload.contains("fixture-chatgpt-basic"));
    common::assert_public_json_safe(&payload);
    assert_no_live_web_secret_markers("v10 basic live transport payload", &payload);
}

#[tokio::test]
async fn live_transport_plain_backend_does_not_fetch_with_encrypted_cookie_only() {
    let (tmp, paths) = common::temp_paths();
    create_chatgpt_cookie_db(
        tmp.path(),
        &[ChatgptCookieRow::encrypted_v11("encrypted_live")],
    );
    let app = App::new_with_runtime(
        paths,
        AppRuntime::production()
            .with_browser_roots(BrowserDiscoveryRoots::synthetic_home(
                tmp.path().to_path_buf(),
            ))
            .with_codex_web_live_transport_for_tests(true)
            .with_codex_web_fixture(CodexWebFixture::Success),
    )
    .expect("app");
    let start = app
        .start_refresh(CODEX_LIVE_WEB_REFRESH_OPTIONS_JSON)
        .expect("refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("refresh should start");
    };
    let completion = app.finish_refresh(&refresh_id).await.expect("refresh");
    let snapshot: Snapshot = serde_json::from_str(&completion.snapshot_json).expect("snapshot");
    let result: RefreshResult = serde_json::from_str(&completion.result_json).expect("result");
    let provider = &snapshot.providers[0];
    let payload =
        serde_json::to_string(&(&snapshot, &result, &completion.provider_events)).expect("payload");

    assert_eq!(provider.state, ProviderState::MissingDependency);
    assert!(!result.cache_written);
    assert!(provider
        .diagnostic_codes
        .contains(&browser_diagnostics::COOKIE_DECRYPTION_UNAVAILABLE.to_string()));
    assert!(provider
        .diagnostic_codes
        .contains(&browser_diagnostics::KEYRING_UNAVAILABLE.to_string()));
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::COOKIE_ABSENT.to_string()));
    assert!(!provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_STARTED.to_string()));
    assert!(!payload.contains("encrypted_live"));
    common::assert_public_json_safe(&payload);
    assert_no_live_web_secret_markers("encrypted unavailable live payload", &payload);
}

#[tokio::test]
async fn live_transport_fake_decryptor_success_is_test_only_and_fetches() {
    let (tmp, paths) = common::temp_paths();
    create_chatgpt_cookie_db(
        tmp.path(),
        &[ChatgptCookieRow::encrypted_v11("encrypted_live")],
    );
    let app = App::new_with_runtime(
        paths,
        AppRuntime::production()
            .with_browser_roots(BrowserDiscoveryRoots::synthetic_home(
                tmp.path().to_path_buf(),
            ))
            .with_browser_decryptor_mode(FakeDecryptorMode::Success)
            .with_codex_web_live_transport_for_tests(true)
            .with_codex_web_fixture(CodexWebFixture::Success),
    )
    .expect("app");
    let start = app
        .start_refresh(CODEX_LIVE_WEB_REFRESH_OPTIONS_JSON)
        .expect("refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("refresh should start");
    };
    let completion = app.finish_refresh(&refresh_id).await.expect("refresh");
    let snapshot: Snapshot = serde_json::from_str(&completion.snapshot_json).expect("snapshot");
    let result: RefreshResult = serde_json::from_str(&completion.result_json).expect("result");
    let provider = &snapshot.providers[0];

    assert_eq!(provider.state, ProviderState::Ok);
    assert!(result.cache_written);
    assert!(provider
        .diagnostic_codes
        .contains(&browser_diagnostics::COOKIE_DECRYPTED.to_string()));
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_STARTED.to_string()));
    common::assert_public_json_safe(&completion.snapshot_json);
    assert_no_live_web_secret_markers("fake decryptor live payload", &completion.snapshot_json);
}

#[tokio::test]
async fn live_transport_mixed_plaintext_and_failed_encrypted_cookie_fails_closed() {
    let (tmp, paths) = common::temp_paths();
    create_chatgpt_cookie_db(
        tmp.path(),
        &[
            ChatgptCookieRow::plaintext("plain_live", "fixture-chatgpt-live"),
            ChatgptCookieRow::encrypted_v11("encrypted_live"),
        ],
    );
    let app = App::new_with_runtime(
        paths,
        AppRuntime::production()
            .with_browser_roots(BrowserDiscoveryRoots::synthetic_home(
                tmp.path().to_path_buf(),
            ))
            .with_browser_decryptor_mode(FakeDecryptorMode::Failure)
            .with_codex_web_live_transport_for_tests(true)
            .with_codex_web_fixture(CodexWebFixture::Success),
    )
    .expect("app");
    let start = app
        .start_refresh(CODEX_LIVE_WEB_REFRESH_OPTIONS_JSON)
        .expect("refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("refresh should start");
    };
    let completion = app.finish_refresh(&refresh_id).await.expect("refresh");
    let snapshot: Snapshot = serde_json::from_str(&completion.snapshot_json).expect("snapshot");
    let result: RefreshResult = serde_json::from_str(&completion.result_json).expect("result");
    let provider = &snapshot.providers[0];
    let payload = serde_json::to_string(&(&snapshot, &result)).expect("payload");

    assert_eq!(provider.state, ProviderState::MissingDependency);
    assert!(!result.cache_written);
    assert!(provider
        .diagnostic_codes
        .contains(&browser_diagnostics::COOKIE_DECRYPTION_FAILED.to_string()));
    assert!(provider
        .diagnostic_codes
        .contains(&diagnostics::COOKIE_ABSENT.to_string()));
    assert!(!provider
        .diagnostic_codes
        .contains(&diagnostics::FETCH_STARTED.to_string()));
    assert!(!payload.contains("plain_live"));
    assert!(!payload.contains("encrypted_live"));
    assert!(!payload.contains("fixture-chatgpt-live"));
    common::assert_public_json_safe(&payload);
    assert_no_live_web_secret_markers("mixed failed material payload", &payload);
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
            | "browser_cookie_decryption_unavailable"
            | "browser_keyring_unavailable"
            | "browser_keyring_locked"
            | "browser_keyring_prompt_required"
            | "browser_cookie_missing"
            | "browser_cookie_found"
            | "browser_cookie_decrypted"
            | "provider_cookie_absent"
            | "provider_cookie_rejected"
            | "provider_redirect_blocked"
            | "provider_web_fetch_nonzero_status"
            | "provider_web_fetch_rate_limited"
            | "provider_web_fetch_parse_error"
            | "provider_web_fetch_finished"
            | "provider_web_fetch_timeout"
            | "provider_response_too_large"
    )));

    let diagnostics_json = app.get_diagnostics_json("codex").expect("diagnostics");
    let diagnostics_payload: Diagnostics =
        serde_json::from_str(&diagnostics_json).expect("diagnostics payload");
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

    let material_summary = live_cookie_material_summary(&fake_home);
    let summary = LiveReconSummary::from_provider_result_material_and_diagnostics(
        provider,
        &result,
        material_summary,
        &diagnostics_payload.events,
    );
    let summary_json = serde_json::to_string(&summary).expect("live recon summary json");
    common::assert_public_json_safe(&summary_json);
    assert_no_live_web_secret_markers("live Codex web summary", &summary_json);
    assert_no_live_web_fake_home_path("live Codex web summary", &summary_json, &fake_home);
    eprintln!("{summary_json}");
}

fn refresh_for_response(response: WebResponse) -> codexbar_linuxd::web::WebRefresh {
    let client = FakeWebClient::responding(response);
    block_on_web(web::refresh_with_client(
        web_request(Some(session())),
        &client,
    ))
    .expect("web refresh")
}

fn block_on_web<T>(future: impl std::future::Future<Output = T>) -> T {
    assert!(
        tokio::runtime::Handle::try_current().is_err(),
        "block_on_web must not create a runtime inside an async context"
    );
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
        .block_on(future)
}

#[derive(Clone)]
struct ChatgptCookieRow<'a> {
    name: &'a str,
    value: &'a str,
    encrypted_value: Vec<u8>,
}

impl<'a> ChatgptCookieRow<'a> {
    fn plaintext(name: &'a str, value: &'a str) -> Self {
        Self {
            name,
            value,
            encrypted_value: Vec::new(),
        }
    }

    fn encrypted_v11(name: &'a str) -> Self {
        Self {
            name,
            value: "",
            encrypted_value: b"v11fixture-chatgpt-keyring".to_vec(),
        }
    }

    fn encrypted_v10_basic() -> Self {
        Self {
            name: "v10_basic_live",
            value: "",
            encrypted_value: encrypt_v10_basic(".chatgpt.com", b"fixture-chatgpt-basic"),
        }
    }
}

fn create_chatgpt_cookie_db(home: &Path, rows: &[ChatgptCookieRow<'_>]) -> PathBuf {
    let profile = home.join(".config/google-chrome/Default/Network");
    fs::create_dir_all(&profile).expect("profile network");
    let db = profile.join("Cookies");
    let connection = Connection::open(&db).expect("open chatgpt fixture db");
    connection
        .execute_batch(
            r#"
CREATE TABLE meta(key LONGVARCHAR NOT NULL UNIQUE PRIMARY KEY, value LONGVARCHAR);
INSERT INTO meta(key, value) VALUES('version', '24');
CREATE TABLE cookies(
  creation_utc INTEGER NOT NULL,
  host_key TEXT NOT NULL,
  top_frame_site_key TEXT NOT NULL DEFAULT '',
  name TEXT NOT NULL,
  value TEXT NOT NULL,
  encrypted_value BLOB NOT NULL DEFAULT X'',
  path TEXT NOT NULL,
  expires_utc INTEGER NOT NULL,
  is_secure INTEGER NOT NULL,
  is_httponly INTEGER NOT NULL,
  last_access_utc INTEGER NOT NULL,
  has_expires INTEGER NOT NULL DEFAULT 1,
  is_persistent INTEGER NOT NULL DEFAULT 1,
  priority INTEGER NOT NULL DEFAULT 1,
  samesite INTEGER NOT NULL DEFAULT -1,
  source_scheme INTEGER NOT NULL DEFAULT 2,
  source_port INTEGER NOT NULL DEFAULT 443,
  last_update_utc INTEGER NOT NULL DEFAULT 0,
  source_type INTEGER NOT NULL DEFAULT 0,
  has_cross_site_ancestor INTEGER NOT NULL DEFAULT 0
);
"#,
        )
        .expect("chatgpt fixture schema");
    for row in rows {
        connection
            .execute(
                "INSERT INTO cookies(creation_utc, host_key, name, value, encrypted_value, path, expires_utc, is_secure, is_httponly, last_access_utc) VALUES(1, '.chatgpt.com', ?1, ?2, ?3, '/', 20000000000000000, 1, 1, 1)",
                params![row.name, row.value, row.encrypted_value.as_slice()],
            )
            .expect("insert chatgpt fixture cookie");
    }
    db
}

fn encrypt_v10_basic(host_key: &str, value: &[u8]) -> Vec<u8> {
    type Aes128CbcEncryptor = cbc::Encryptor<Aes128>;

    let mut key = [0_u8; CHROMIUM_AES_BLOCK_LEN];
    pbkdf2_hmac::<Sha1>(
        CHROMIUM_BASIC_PASSWORD,
        CHROMIUM_BASIC_SALT,
        CHROMIUM_BASIC_ITERATIONS,
        &mut key,
    );

    let mut plaintext = Vec::new();
    plaintext.extend_from_slice(Sha256::digest(host_key.as_bytes()).as_slice());
    plaintext.extend_from_slice(value);

    let padded_len = ((plaintext.len() / CHROMIUM_AES_BLOCK_LEN) + 1) * CHROMIUM_AES_BLOCK_LEN;
    let mut buffer = vec![0_u8; padded_len];
    buffer[..plaintext.len()].copy_from_slice(&plaintext);
    let ciphertext = Aes128CbcEncryptor::new_from_slices(&key, &CHROMIUM_BASIC_IV)
        .expect("fixed key and IV lengths")
        .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
        .expect("test encryption");

    let mut encrypted = CHROMIUM_V10_PREFIX.to_vec();
    encrypted.extend_from_slice(ciphertext);
    encrypted
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

fn live_cookie_material_summary(
    fake_home: &Path,
) -> codexbar_linuxd::browser::cookie_store::BrowserCookieMaterialSummary {
    browser::collect_session_material(BrowserSessionRequest {
        providers: vec!["codex".to_string()],
        settings: Default::default(),
        roots: BrowserDiscoveryRoots::synthetic_home(fake_home.to_path_buf()).canonicalized(),
        decryptor_mode: BrowserDecryptorMode::Plain,
    })
    .material_summary
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
    request_header_profile: String,
    #[serde(flatten)]
    http_response: LiveReconHttpResponseSummary,
    classification: LiveReconClassification,
    diagnostic_codes: Vec<String>,
    cookie_material: codexbar_linuxd::browser::cookie_store::BrowserCookieMaterialSummary,
    cookie_presence: LiveReconCookiePresence,
    web_fetch: LiveReconWebFetch,
    redaction_applied: bool,
}

impl LiveReconSummary {
    fn from_provider_result_material_and_diagnostics(
        provider: &Provider,
        result: &RefreshResult,
        cookie_material: codexbar_linuxd::browser::cookie_store::BrowserCookieMaterialSummary,
        diagnostics: &[DiagnosticEvent],
    ) -> Self {
        let diagnostic_codes = safe_recon_diagnostic_codes(provider, result);
        let classification = classify_live_recon(provider.state, &diagnostic_codes);
        let cookie_presence = classify_cookie_presence(&diagnostic_codes);
        let web_fetch = classify_web_fetch(&diagnostic_codes);
        let request_header_profile = recon_request_header_profile(diagnostics);
        let http_response =
            LiveReconHttpResponseSummary::from_diagnostics(diagnostics, &diagnostic_codes);
        Self {
            provider: "codex".to_string(),
            provider_state: provider.state,
            refresh_status: result.status,
            cache_written: result.cache_written,
            source: SemanticSource::Web,
            source_adapter: SourceAdapter::LinuxWeb,
            request_header_profile,
            http_response,
            classification,
            diagnostic_codes,
            cookie_material,
            cookie_presence,
            web_fetch,
            redaction_applied: true,
        }
    }

    fn from_provider_result_and_material(
        provider: &Provider,
        result: &RefreshResult,
        cookie_material: codexbar_linuxd::browser::cookie_store::BrowserCookieMaterialSummary,
    ) -> Self {
        Self::from_provider_result_material_and_diagnostics(provider, result, cookie_material, &[])
    }

    fn from_provider_and_result(provider: &Provider, result: &RefreshResult) -> Self {
        Self::from_provider_result_and_material(
            provider,
            result,
            codexbar_linuxd::browser::cookie_store::BrowserCookieMaterialSummary::default(),
        )
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveReconHttpResponseSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    http_status_code: Option<u16>,
    http_status_class: String,
    redirect_present: bool,
    redirect_host_class: String,
    redirect_target_class: String,
    redirect_path_family: String,
    redirect_path_depth: String,
    redirect_query_class: String,
    redirect_can_follow: bool,
    redirect_followed: bool,
    redirect_hop_count: u64,
    final_http_status_code: Option<u16>,
    final_http_status_class: String,
    content_type_class: String,
    response_body_class: String,
    response_size_bucket: String,
}

impl LiveReconHttpResponseSummary {
    fn from_diagnostics(events: &[DiagnosticEvent], codes: &[String]) -> Self {
        let mut summary = Self {
            http_status_code: None,
            http_status_class: "unknown".to_string(),
            redirect_present: false,
            redirect_host_class: "none".to_string(),
            redirect_target_class: "none".to_string(),
            redirect_path_family: "none".to_string(),
            redirect_path_depth: "unknown".to_string(),
            redirect_query_class: "none".to_string(),
            redirect_can_follow: false,
            redirect_followed: false,
            redirect_hop_count: 0,
            final_http_status_code: None,
            final_http_status_class: "none".to_string(),
            content_type_class: "missing".to_string(),
            response_body_class: if has_code(codes, diagnostics::RESPONSE_TOO_LARGE) {
                "too_large".to_string()
            } else {
                "not_read".to_string()
            },
            response_size_bucket: if has_code(codes, diagnostics::RESPONSE_TOO_LARGE) {
                "capped".to_string()
            } else {
                "zero".to_string()
            },
        };

        let Some(event) = events
            .iter()
            .rev()
            .find(|event| event.details.contains_key("httpStatusCode"))
        else {
            return summary;
        };

        summary.http_status_code = event
            .details
            .get("httpStatusCode")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u16::try_from(value).ok());
        summary.http_status_class = safe_detail_class(
            event,
            "httpStatusClass",
            &[
                "informational",
                "success",
                "redirect",
                "client_error",
                "server_error",
                "unknown",
            ],
            "unknown",
        );
        summary.redirect_present = event
            .details
            .get("redirectPresent")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        summary.redirect_host_class = safe_detail_class(
            event,
            "redirectHostClass",
            &["none", "allowed", "blocked", "missing", "invalid"],
            "none",
        );
        summary.redirect_target_class = safe_detail_class(
            event,
            "redirectTargetClass",
            &[
                "none",
                "same_host_canonical",
                "same_host_usage_path",
                "same_host_login_path",
                "same_host_other",
                "allowed_host_other",
                "blocked_host",
                "invalid",
            ],
            "none",
        );
        summary.redirect_path_family = safe_detail_class(
            event,
            "redirectPathFamily",
            &[
                "none",
                "codex_usage",
                "codex_settings",
                "codex_other",
                "auth_login",
                "auth_callback",
                "root",
                "static_asset",
                "api",
                "unknown",
                "invalid",
            ],
            "none",
        );
        summary.redirect_path_depth = safe_detail_class(
            event,
            "redirectPathDepth",
            &["zero", "one", "two", "three", "many", "unknown"],
            "unknown",
        );
        summary.redirect_query_class = safe_detail_class(
            event,
            "redirectQueryClass",
            &["none", "safe_empty", "token_like", "present", "unknown"],
            "unknown",
        );
        summary.redirect_can_follow = event
            .details
            .get("redirectCanFollow")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        summary.redirect_followed = event
            .details
            .get("redirectFollowed")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        summary.redirect_hop_count = event
            .details
            .get("redirectHopCount")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        summary.final_http_status_code = event
            .details
            .get("finalHttpStatusCode")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u16::try_from(value).ok());
        summary.final_http_status_class = safe_detail_class(
            event,
            "finalHttpStatusClass",
            &[
                "none",
                "informational",
                "success",
                "redirect",
                "client_error",
                "server_error",
                "unknown",
            ],
            "none",
        );
        summary.content_type_class = safe_detail_class(
            event,
            "contentTypeClass",
            &["html", "json", "text", "other", "missing"],
            "missing",
        );
        summary.response_body_class = safe_detail_class(
            event,
            "responseBodyClass",
            &[
                "not_read",
                "empty",
                "within_cap",
                "too_large",
                "invalid_encoding",
            ],
            "not_read",
        );
        summary.response_size_bucket = safe_detail_class(
            event,
            "responseSizeBucket",
            &["zero", "small", "medium", "large", "capped"],
            "zero",
        );
        summary
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
    #[serde(rename = "non_200")]
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
            browser_diagnostics::COOKIE_DECRYPTION_UNAVAILABLE,
            browser_diagnostics::COOKIE_DECRYPTION_FAILED,
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

fn recon_request_header_profile(events: &[DiagnosticEvent]) -> String {
    events
        .iter()
        .find(|event| event.code == diagnostics::FETCH_STARTED)
        .and_then(|event| event.details.get("requestHeaderProfile"))
        .and_then(serde_json::Value::as_str)
        .filter(|profile| matches!(*profile, "minimal" | "browser_like"))
        .unwrap_or("browser_like")
        .to_string()
}

fn safe_detail_class(
    event: &DiagnosticEvent,
    key: &str,
    allowed: &[&str],
    fallback: &str,
) -> String {
    event
        .details
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| allowed.iter().any(|allowed| allowed == value))
        .unwrap_or(fallback)
        .to_string()
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
    html_response_at(CodexWebPolicy::new().dashboard_url(), name)
}

fn html_response_at(url: &str, name: &str) -> WebResponse {
    WebResponse::new(200, url, fixture_text(name).into_bytes()).with_content_type("text/html")
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

fn find_diagnostic<'a>(events: &'a [DiagnosticEvent], code: &str) -> &'a DiagnosticEvent {
    events
        .iter()
        .find(|event| event.code == code)
        .unwrap_or_else(|| panic!("diagnostic event {code} not found"))
}

fn assert_redirect_decision(
    event: &DiagnosticEvent,
    target_class: &str,
    path_family: &str,
    path_depth: &str,
    query_class: &str,
    can_follow: bool,
    followed: bool,
) {
    assert_allowed_response_detail_keys(event);
    assert_eq!(
        event.details.get("redirectTargetClass"),
        Some(&serde_json::Value::from(target_class))
    );
    assert_eq!(
        event.details.get("redirectPathFamily"),
        Some(&serde_json::Value::from(path_family))
    );
    assert_eq!(
        event.details.get("redirectPathDepth"),
        Some(&serde_json::Value::from(path_depth))
    );
    assert_eq!(
        event.details.get("redirectQueryClass"),
        Some(&serde_json::Value::from(query_class))
    );
    assert_eq!(
        event.details.get("redirectCanFollow"),
        Some(&serde_json::Value::Bool(can_follow))
    );
    assert_eq!(
        event.details.get("redirectFollowed"),
        Some(&serde_json::Value::Bool(followed))
    );
}

fn assert_allowed_response_detail_keys(event: &DiagnosticEvent) {
    for key in event.details.keys() {
        assert!(
            matches!(
                key.as_str(),
                "contentTypeClass"
                    | "finalHttpStatusClass"
                    | "finalHttpStatusCode"
                    | "httpStatusCode"
                    | "httpStatusClass"
                    | "redirectBlocked"
                    | "redirectCanFollow"
                    | "redirectFollowed"
                    | "redirectHostClass"
                    | "redirectHopCount"
                    | "redirectPathDepth"
                    | "redirectPathFamily"
                    | "redirectPresent"
                    | "redirectQueryClass"
                    | "redirectTargetClass"
                    | "requestHeaderProfile"
                    | "responseBodyClass"
                    | "responseSizeBucket"
            ),
            "unexpected safe response detail key {key}"
        );
    }
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
