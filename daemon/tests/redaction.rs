mod common;

use std::fs;

use codexbar_linuxd::app::RefreshStart;
use codexbar_linuxd::redact;

#[test]
fn obvious_forbidden_content_is_rejected() {
    for toxic in [
        r#"{"value":"Authorization: Bearer abc"}"#,
        r#"{"authorization":"Basic abc"}"#,
        r#"{"Cookie":"session=abc"}"#,
        r#"{"value":"Set-Cookie: session=abc"}"#,
        r#"{"value":"sk-test-secret"}"#,
        r#"{"value":"ghp_secret"}"#,
        r#"{"value":"xoxb-secret"}"#,
        r#"{"accessToken":"abc"}"#,
        r#"{"value":"refresh_token=abc"}"#,
        r#"{"value":"sid=abc"}"#,
        r#"{"sessionid":"abc"}"#,
        r#"{"sessionKey":"abc"}"#,
        r#"{"apiKey":"abc"}"#,
        r#"{"cookieName":"__Secure-fixture"}"#,
        r#"{"cookieNames":["quota_marker"]}"#,
        r#"{"hostKey":"codex.example.invalid"}"#,
        r#"{"domain":"codex.example.invalid"}"#,
        r#"{"requestHeaders":{"Cookie":"abc"}}"#,
        r#"{"responseHeaders":{"Set-Cookie":"abc"}}"#,
        r#"{"value":"__Host-fixture"}"#,
        r#"{"value":"encrypted_value"}"#,
        r#"{"value":"/tmp/codexbar/Cookies"}"#,
        r#"{"value":"cookies.sqlite"}"#,
        r#"{"value":"/home/user/.config/google-chrome/Default/Network/Cookies"}"#,
        r#"{"value":"/home/user/.config/chromium/Profile 1/Cookies"}"#,
        r#"{"value":"BraveSoftware/Brave-Browser/Default"}"#,
        r#"{"rawProfilePath":"redacted"}"#,
        r#"{"rawCookie":"redacted"}"#,
        r#"{"rawHeader":"redacted"}"#,
        r#"{"value":"raw@example.com"}"#,
        r#"{"rawPayload":"secret"}"#,
        r#"{"rawResponse":"secret"}"#,
        r#"{"headers":{"Authorization":"Bearer abc"}}"#,
        r#"{"value":"/home/user/.config/chromium/Profile 1/Cookies"}"#,
        r#"{"value":"~/.local/share/auth.json"}"#,
    ] {
        assert!(
            redact::validate_public_json_text(toxic).is_err(),
            "expected toxic content to be rejected: {toxic}"
        );
    }
}

#[tokio::test]
async fn daemon_public_payloads_and_cache_pass_redaction_scan() {
    let (tmp, paths) = common::temp_paths();
    let app = common::fixture_app(paths.clone());
    let refresh = app
        .start_refresh(common::FIXTURE_REFRESH_OPTIONS_JSON)
        .expect("start refresh");
    let RefreshStart::Started { refresh_id } = refresh else {
        panic!("expected started refresh");
    };
    let completion = app
        .finish_refresh(&refresh_id)
        .await
        .expect("finish refresh");
    assert!(tmp.path().is_dir());

    for payload in [
        app.get_snapshot_json().expect("snapshot"),
        app.get_daemon_info_json().expect("daemon info"),
        app.get_diagnostics_json("global").expect("diagnostics"),
        app.test_browser_import_json(r#"{"schemaVersion":1,"providers":["codex"]}"#)
            .expect("browser import"),
        completion.snapshot_json,
        completion.result_json,
    ] {
        common::assert_public_json_safe(&payload);
    }
    for (_provider, event_json) in completion.provider_events {
        common::assert_public_json_safe(&event_json);
    }
    let cache_text = fs::read_to_string(paths.cache_file).expect("cache");
    common::assert_public_json_safe(&cache_text);
}
