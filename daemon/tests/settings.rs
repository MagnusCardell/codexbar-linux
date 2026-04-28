mod common;

use std::fs;

use codexbar_linuxd::app::App;
use codexbar_linuxd::config;
use codexbar_linuxd::error::AppError;
use codexbar_linuxd::model::{DiagnosticsVerbosity, Settings};

#[test]
fn default_settings_validate() {
    let settings = Settings::default();
    config::validate_settings(&settings).expect("default settings validate");
    let json = serde_json::to_string(&settings).expect("settings json");
    common::assert_schema("settings.schema.json", &json);
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
    assert!(settings.providers["codex"].allow_browser_import);
    assert_eq!(
        settings.diagnostics.verbosity,
        DiagnosticsVerbosity::Verbose
    );
}

#[test]
fn invalid_patch_maps_to_internal_typed_errors() {
    let schema_error =
        config::parse_settings_patch(r#"{"schemaVersion":1,"refresh":{"intervalSeconds":1}}"#)
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
