mod common;

use std::fs;

use codexbar_linuxd::app::{App, RefreshStart};
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
        r#"{"sessionKey":"abc"}"#,
        r#"{"apiKey":"abc"}"#,
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
    let app = App::new(paths.clone()).expect("app");
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
