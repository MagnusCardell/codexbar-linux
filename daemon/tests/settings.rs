mod common;

use std::fs;

use codexbar_linuxd::app::App;
use codexbar_linuxd::app::RefreshStart;
use codexbar_linuxd::config;
use codexbar_linuxd::error::AppError;
use codexbar_linuxd::model::{
    BrowserImportPolicy, DiagnosticsVerbosity, PreferredSourceAdapter, RefreshResult,
    RefreshStatus, Settings,
};

#[test]
fn default_settings_validate() {
    let settings = Settings::default();
    assert_eq!(settings.refresh.interval_seconds, 300);
    config::validate_settings(&settings).expect("default settings validate");
    let json = serde_json::to_string(&settings).expect("settings json");
    common::assert_schema("settings.schema.json", &json);
}

#[test]
fn interval_zero_is_valid_manual_refresh_mode() {
    let (_tmp, paths) = common::temp_paths();
    let app = App::new(paths).expect("app");
    let settings_json = app
        .set_settings_patch_json(r#"{"schemaVersion":1,"refresh":{"intervalSeconds":0}}"#)
        .expect("manual refresh mode patch");
    common::assert_schema("settings.schema.json", &settings_json);
    let settings: Settings = serde_json::from_str(&settings_json).expect("settings");
    assert_eq!(settings.refresh.interval_seconds, 0);
    assert!(settings.refresh.startup_refresh);
}

#[test]
fn valid_patch_applies_persists_and_preserves_omitted_fields() {
    let (_tmp, paths) = common::temp_paths();
    let app = App::new(paths.clone()).expect("app");
    let settings_json = app
        .set_settings_patch_json(
            r#"{"schemaVersion":1,"refresh":{"intervalSeconds":300},"diagnostics":{"verbosity":"verbose"},"providers":{"codex":{"enabled":false}}}"#,
        )
        .expect("settings patch");
    common::assert_schema("settings.schema.json", &settings_json);

    assert_eq!(common::file_mode(&paths.config_dir), 0o700);
    assert_eq!(common::file_mode(&paths.config_file), 0o600);
    let persisted = fs::read_to_string(&paths.config_file).expect("config file");
    common::assert_public_json_safe(&persisted);
    let settings: Settings = serde_json::from_str(&persisted).expect("settings");
    assert_eq!(settings.refresh.interval_seconds, 300);
    assert!(settings.refresh.startup_refresh);
    assert!(!settings.providers["codex"].enabled);
    assert!(!settings.providers["codex"].allow_browser_import);
    assert_eq!(settings.browser_import.policy, BrowserImportPolicy::Off);
    assert!(!settings.browser_import.enabled);
    assert_eq!(
        settings.diagnostics.verbosity,
        DiagnosticsVerbosity::Verbose
    );
}

#[tokio::test]
async fn settings_patch_advances_scheduler_revision() {
    let (_tmp, paths) = common::temp_paths();
    let app = App::new(paths).expect("app");
    let observed = app.settings_revision();

    app.set_settings_patch_json(r#"{"schemaVersion":1,"refresh":{"intervalSeconds":300}}"#)
        .expect("settings patch");

    let revision = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        app.wait_for_settings_change(observed),
    )
    .await
    .expect("settings revision wake");
    assert!(revision > observed);
}

#[tokio::test]
async fn failed_refresh_can_be_unwedged_without_daemon_restart() {
    let (_tmp, paths) = common::temp_paths();
    let app = common::fixture_app(paths);
    let start = app
        .start_refresh(common::FIXTURE_REFRESH_OPTIONS_JSON)
        .expect("start refresh");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("first refresh should start");
    };

    let failed = app
        .fail_refresh(
            &refresh_id,
            "refresh_failed",
            "Refresh failed; details were redacted.",
        )
        .expect("fail active refresh");
    common::assert_schema("snapshot.schema.json", &failed.snapshot_json);
    common::assert_schema("refresh-result.schema.json", &failed.result_json);
    let result: RefreshResult = serde_json::from_str(&failed.result_json).expect("result json");
    assert_eq!(result.status, RefreshStatus::Error);
    assert_eq!(result.refresh_id, refresh_id);
    assert!(result
        .diagnostic_codes
        .iter()
        .any(|code| code == "refresh_failed"));

    let next = app
        .start_refresh(common::FIXTURE_REFRESH_OPTIONS_JSON)
        .expect("second refresh starts after failure");
    assert!(
        matches!(next, RefreshStart::Started { .. }),
        "refresh failure must clear the active refresh guard"
    );
}

#[test]
fn browser_settings_are_compatibility_only_and_normalized_off() {
    let (_tmp, paths) = common::temp_paths();
    let app = App::new(paths).expect("app");
    let settings_json = app
        .set_settings_patch_json(
            r#"{"schemaVersion":1,"providers":{"codex":{"preferredSourceAdapter":"linux_web","allowBrowserImport":true}},"browserImport":{"enabled":true,"policy":"chromium_family","profileIdAllowlist":["safe-profile"]}}"#,
        )
        .expect("settings patch");
    let settings: Settings = serde_json::from_str(&settings_json).expect("settings");
    assert!(!settings.browser_import.enabled);
    assert_eq!(settings.browser_import.policy, BrowserImportPolicy::Off);
    assert!(settings.browser_import.profile_id_allowlist.is_empty());
    assert!(!settings.providers["codex"].allow_browser_import);
    assert_eq!(
        settings.providers["codex"].preferred_source_adapter,
        PreferredSourceAdapter::UpstreamCli
    );
}

#[test]
fn invalid_patch_maps_to_internal_typed_errors() {
    let schema_error =
        config::parse_settings_patch(r#"{"schemaVersion":1,"refresh":{"intervalSeconds":29}}"#)
            .expect("patch parses before full settings validation");
    let err = config::apply_settings_patch(Settings::default(), schema_error)
        .expect_err("invalid interval");
    assert!(matches!(err, AppError::InvalidJson(_)));

    let err = config::parse_settings_patch(
        r#"{"schemaVersion":1,"providers":{"/home/user/profile":{"enabled":true}}}"#,
    )
    .expect_err("policy reject");
    assert!(matches!(err, AppError::InvalidSettingsPatch(_)));
}

#[test]
fn profile_id_allowlist_rejects_absolute_paths() {
    let err = config::parse_settings_patch(
        r#"{"schemaVersion":1,"browserImport":{"profileIdAllowlist":["/home/user/.config/chromium/Profile 1"]}}"#,
    )
    .expect_err("absolute profile path rejected");
    assert!(matches!(err, AppError::InvalidJson(_)));
}

#[test]
fn null_fields_do_not_delete_or_reset() {
    let err =
        config::parse_settings_patch(r#"{"schemaVersion":1,"refresh":{"intervalSeconds":null}}"#)
            .expect_err("null rejected by patch schema");
    assert!(matches!(err, AppError::InvalidJson(_)));
}
