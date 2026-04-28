mod common;

use codexbar_linuxd::app::{App, RefreshStart};
use codexbar_linuxd::fixtures;
use codexbar_linuxd::model::{
    BrowserImportOptions, BrowserImportPolicy, ProviderEvent, ProviderEventReason, Settings,
};

#[test]
fn daemon_generated_payloads_validate_against_schemas() {
    let (_tmp, paths) = common::temp_paths();
    let app = App::new(paths).expect("app");

    let snapshot_json = app.get_snapshot_json().expect("snapshot");
    common::assert_public_json_safe(&snapshot_json);
    common::assert_schema("snapshot.schema.json", &snapshot_json);

    let daemon_info_json = app.get_daemon_info_json().expect("daemon info");
    common::assert_public_json_safe(&daemon_info_json);
    common::assert_schema("daemon-info.schema.json", &daemon_info_json);

    let diagnostics_json = app.get_diagnostics_json("global").expect("diagnostics");
    common::assert_public_json_safe(&diagnostics_json);
    common::assert_schema("diagnostics.schema.json", &diagnostics_json);

    let settings_json = app
        .set_settings_patch_json(
            r#"{"schemaVersion":1,"refresh":{"intervalSeconds":180},"providers":{"codex":{"enabled":true}}}"#,
        )
        .expect("settings patch");
    common::assert_public_json_safe(&settings_json);
    common::assert_schema("settings.schema.json", &settings_json);

    let browser_json = app
        .test_browser_import_json(r#"{"schemaVersion":1,"providers":["codex"]}"#)
        .expect("browser import result");
    common::assert_public_json_safe(&browser_json);
    common::assert_schema("browser-import-result.schema.json", &browser_json);

    let refresh = app
        .start_refresh(r#"{"schemaVersion":1,"reason":"test","force":true}"#)
        .expect("start refresh");
    let RefreshStart::Started { refresh_id } = refresh else {
        panic!("expected new refresh");
    };
    let completion = app.finish_refresh(&refresh_id).expect("finish refresh");
    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);
    common::assert_schema("refresh-result.schema.json", &completion.result_json);
    for (_provider_id, event_json) in completion.provider_events {
        common::assert_public_json_safe(&event_json);
        common::assert_schema("provider-event.schema.json", &event_json);
    }
}

#[test]
fn fixture_models_and_manual_provider_event_validate() {
    let now = codexbar_linuxd::clock::now_rfc3339();
    let snapshot = fixtures::refreshed_snapshot("fixture-test", &now, &now).expect("snapshot");
    let snapshot_json = serde_json::to_string(&snapshot).expect("snapshot json");
    common::assert_schema("snapshot.schema.json", &snapshot_json);

    let provider = snapshot
        .providers
        .first()
        .expect("fixture provider")
        .clone();
    let event = ProviderEvent {
        schema_version: 1,
        event_id: "event-test".to_string(),
        emitted_at: now,
        reason: ProviderEventReason::RefreshFinished,
        provider_id: provider.provider.clone(),
        provider,
        diagnostic_codes: Vec::new(),
    };
    let event_json = serde_json::to_string(&event).expect("event json");
    common::assert_schema("provider-event.schema.json", &event_json);
}

#[test]
fn defaults_validate_against_settings_and_browser_option_schemas() {
    let settings_json = serde_json::to_string(&Settings::default()).expect("settings");
    common::assert_schema("settings.schema.json", &settings_json);

    let options = BrowserImportOptions {
        schema_version: 1,
        policy: BrowserImportPolicy::Auto,
        providers: vec!["codex".to_string()],
        profile_ids: Vec::new(),
        include_diagnostics: true,
    };
    let options_json = serde_json::to_string(&options).expect("options");
    common::assert_schema("browser-import-options.schema.json", &options_json);
}
