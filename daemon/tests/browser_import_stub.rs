mod common;

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
