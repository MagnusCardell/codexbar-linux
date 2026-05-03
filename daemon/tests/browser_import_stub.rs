mod common;

use std::fs;

use codexbar_linuxd::app::App;
use codexbar_linuxd::error::AppError;
use codexbar_linuxd::model::BrowserImportResult;

#[test]
fn browser_import_stub_returns_not_implemented_without_browser_access() {
    let (tmp, paths) = common::temp_paths();
    let app = App::new(paths).expect("app");
    let result_json = app
        .test_browser_import_json(
            r#"{"schemaVersion":1,"policy":"auto","providers":["codex","claude"],"profileIds":["safe-profile-1"]}"#,
        )
        .expect("browser import result");
    common::assert_schema("browser-import-result.schema.json", &result_json);
    common::assert_public_json_safe(&result_json);
    let result: BrowserImportResult = serde_json::from_str(&result_json).expect("result");
    assert_eq!(
        result.status,
        codexbar_linuxd::model::BrowserImportStatus::NotImplemented
    );
    assert!(result.profiles.is_empty());
    assert_eq!(result.providers.len(), 2);
    assert!(result.providers.iter().all(|provider| provider.status
        == codexbar_linuxd::model::BrowserProviderStatus::NotImplemented
        && provider.source_adapter == codexbar_linuxd::model::BrowserSourceAdapter::None));

    assert!(!tmp.path().join(".config").join("chromium").exists());
    assert!(!tmp.path().join(".config").join("google-chrome").exists());
    assert!(!tmp.path().join(".mozilla").join("firefox").exists());
}

#[test]
fn browser_import_noop_does_not_touch_existing_browser_like_state() {
    let (tmp, paths) = common::temp_paths();
    let chromium = tmp.path().join(".config").join("chromium").join("Default");
    let chrome = tmp
        .path()
        .join(".config")
        .join("google-chrome")
        .join("Profile 1");
    let firefox = tmp
        .path()
        .join(".mozilla")
        .join("firefox")
        .join("safe.default-release");
    for dir in [&chromium, &chrome, &firefox] {
        fs::create_dir_all(dir).expect("browser-like dir");
    }
    fs::write(chromium.join("Cookies"), "sentinel chromium cookies").expect("chromium cookies");
    fs::write(chrome.join("Login Data"), "sentinel chrome login data").expect("chrome login");
    fs::write(firefox.join("cookies.sqlite"), "sentinel firefox cookies").expect("firefox cookies");

    let app = App::new(paths.clone()).expect("app");
    let result_json = app
        .test_browser_import_json(r#"{"schemaVersion":1,"providers":["codex"]}"#)
        .expect("browser import result");
    common::assert_schema("browser-import-result.schema.json", &result_json);
    let result: BrowserImportResult = serde_json::from_str(&result_json).expect("result");
    assert_eq!(
        result.status,
        codexbar_linuxd::model::BrowserImportStatus::NotImplemented
    );
    assert!(result.profiles.is_empty());
    assert_eq!(result.providers.len(), 1);
    assert_eq!(
        result.providers[0].status,
        codexbar_linuxd::model::BrowserProviderStatus::NotImplemented
    );
    assert_eq!(
        result.providers[0].source_adapter,
        codexbar_linuxd::model::BrowserSourceAdapter::None
    );

    assert_eq!(
        fs::read_to_string(chromium.join("Cookies")).expect("chromium sentinel"),
        "sentinel chromium cookies"
    );
    assert_eq!(
        fs::read_to_string(chrome.join("Login Data")).expect("chrome sentinel"),
        "sentinel chrome login data"
    );
    assert_eq!(
        fs::read_to_string(firefox.join("cookies.sqlite")).expect("firefox sentinel"),
        "sentinel firefox cookies"
    );
    assert!(!paths.cache_file.exists());
    assert!(!paths.config_file.exists());
}

#[test]
fn browser_import_options_reject_profile_paths() {
    let (_tmp, paths) = common::temp_paths();
    let app = App::new(paths).expect("app");
    let err = app
        .test_browser_import_json(
            r#"{"schemaVersion":1,"profileIds":["/home/user/.config/chromium/Profile 1"]}"#,
        )
        .expect_err("profile path rejected");
    assert!(matches!(err, AppError::InvalidJson(_)));
}
