mod common;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};

use codexbar_linuxd::app::{App, AppRuntime};
use codexbar_linuxd::browser::cookie_store::copy_cookie_db_to_private_temp;
use codexbar_linuxd::browser::keyring::FakeDecryptorMode;
use codexbar_linuxd::browser::profile::BrowserDiscoveryRoots;
use codexbar_linuxd::model::{
    BrowserImportResult, BrowserImportStatus, BrowserProviderStatus, KeyringState,
};
use rusqlite::Connection;

#[test]
fn chromium_family_fake_roots_return_safe_profiles_and_counts() {
    let (tmp, paths) = common::temp_paths();
    create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "plaintext-default/schema.sql",
    );
    create_network_cookie_db(
        tmp.path(),
        ".config/chromium/Profile 1",
        "plaintext-default/schema.sql",
    );
    create_network_cookie_db(
        tmp.path(),
        ".config/BraveSoftware/Brave-Browser/Default",
        "plaintext-default/schema.sql",
    );

    let app = browser_app(paths, tmp.path(), FakeDecryptorMode::Success);
    let result = test_import(&app, r#"{"schemaVersion":1,"providers":["codex"]}"#);

    assert_eq!(result.status, BrowserImportStatus::Success);
    assert_eq!(result.profiles.len(), 3);
    assert!(result
        .profiles
        .iter()
        .all(|profile| !profile.profile_id.contains('/') && !profile.profile_id.contains('\\')));
    assert!(result
        .profiles
        .iter()
        .any(|profile| profile.profile_display_name == "Chrome Default"));
    assert!(result
        .profiles
        .iter()
        .any(|profile| profile.profile_display_name == "Chromium Profile 1"));
    assert!(result
        .profiles
        .iter()
        .any(|profile| profile.profile_display_name == "Brave Default"));
    assert_eq!(result.providers[0].status, BrowserProviderStatus::Success);
    assert_eq!(
        result.providers[0].diagnostic_codes,
        vec!["browser_cookie_found"]
    );

    let result_json = serde_json::to_string(&result).expect("result json");
    assert_public_browser_json_safe(tmp.path(), &result_json);
}

#[test]
fn chromium_snap_fake_root_is_bounded_and_supported() {
    let (tmp, paths) = common::temp_paths();
    create_network_cookie_db(
        tmp.path(),
        "snap/chromium/common/chromium/Default",
        "plaintext-default/schema.sql",
    );
    let app = browser_app(paths, tmp.path(), FakeDecryptorMode::Success);
    let result = test_import(&app, r#"{"schemaVersion":1,"providers":["codex"]}"#);

    assert_eq!(result.status, BrowserImportStatus::Success);
    assert_eq!(result.profiles.len(), 1);
    assert_eq!(result.profiles[0].profile_id, "chromium-snap-default");
    assert_eq!(
        result.profiles[0].profile_display_name,
        "Chromium Snap Default"
    );
}

#[test]
fn default_runtime_does_not_scan_real_or_fake_home() {
    let (tmp, paths) = common::temp_paths();
    let app = App::new(paths).expect("app");
    let result = test_import(&app, r#"{"schemaVersion":1,"providers":["codex"]}"#);

    assert_eq!(result.status, BrowserImportStatus::Unavailable);
    assert!(result.profiles.is_empty());
    assert_eq!(
        result.providers[0].status,
        BrowserProviderStatus::MissingDependency
    );
    assert!(result
        .diagnostic_codes
        .contains(&"browser_live_profiles_disabled".to_string()));
    assert!(!tmp.path().join(".config").join("chromium").exists());
    assert!(!tmp.path().join(".config").join("google-chrome").exists());
    assert!(!tmp.path().join(".config").join("BraveSoftware").exists());
}

#[test]
fn profile_id_filters_are_opaque_and_intersect_settings_allowlist() {
    let (tmp, paths) = common::temp_paths();
    create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "plaintext-default/schema.sql",
    );
    create_network_cookie_db(
        tmp.path(),
        ".config/chromium/Profile 1",
        "plaintext-default/schema.sql",
    );
    let app = browser_app(paths, tmp.path(), FakeDecryptorMode::Success);
    app.set_settings_patch_json(
        r#"{"schemaVersion":1,"browserImport":{"profileIdAllowlist":["chrome-default"]}}"#,
    )
    .expect("settings");
    let result = test_import(
        &app,
        r#"{"schemaVersion":1,"providers":["codex"],"profileIds":["chrome-default","chromium-profile-1"]}"#,
    );

    assert_eq!(result.profiles.len(), 1);
    assert_eq!(result.profiles[0].profile_id, "chrome-default");
    assert!(result
        .diagnostic_codes
        .contains(&"browser_profile_skipped".to_string()));
}

#[test]
#[cfg(unix)]
fn symlink_profile_escape_is_skipped_without_public_path_leakage() {
    let (tmp, paths) = common::temp_paths();
    let chrome_root = tmp.path().join(".config/google-chrome");
    fs::create_dir_all(&chrome_root).expect("chrome root");
    let outside = tmp.path().join("outside-profile");
    fs::create_dir_all(&outside).expect("outside");
    symlink(&outside, chrome_root.join("Default")).expect("profile symlink");

    let app = browser_app(paths, tmp.path(), FakeDecryptorMode::Success);
    let result_json = app
        .test_browser_import_json(r#"{"schemaVersion":1,"providers":["codex"]}"#)
        .expect("result json");
    common::assert_schema("browser-import-result.schema.json", &result_json);
    common::assert_public_json_safe(&result_json);
    let result: BrowserImportResult = serde_json::from_str(&result_json).expect("result");

    assert!(result.profiles.is_empty());
    assert!(result
        .diagnostic_codes
        .contains(&"browser_profile_skipped".to_string()));
    assert_public_browser_json_safe(tmp.path(), &result_json);
}

#[test]
#[cfg(unix)]
fn cookie_db_temp_copy_has_private_permissions_and_cleans_up() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db = create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "plaintext-default/schema.sql",
    );
    fs::write(
        PathBuf::from(format!("{}-shm", db.display())),
        b"synthetic-shm",
    )
    .expect("synthetic shm");
    let copy = copy_cookie_db_to_private_temp(&db).expect("copy");
    assert_eq!(mode(copy.dir()), 0o700);
    assert!(copy
        .copied_files()
        .iter()
        .any(|path| path.file_name().and_then(|name| name.to_str()) == Some("Cookies")));
    assert!(copy
        .copied_files()
        .iter()
        .any(|path| path.file_name().and_then(|name| name.to_str()) == Some("Cookies-shm")));
    for path in copy.copied_files() {
        assert_eq!(mode(path), 0o600);
    }
    let copy_dir = copy.dir().to_path_buf();
    drop(copy);
    assert!(!copy_dir.exists());
}

#[test]
fn wal_companion_rows_are_available_after_private_copy() {
    let (tmp, paths) = common::temp_paths();
    let profile_dir = tmp.path().join(".config/google-chrome/Default/Network");
    fs::create_dir_all(&profile_dir).expect("profile network");
    let db = profile_dir.join("Cookies");
    let connection = Connection::open(&db).expect("open wal db");
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .expect("wal");
    connection
        .execute_batch(&fixture_sql("locked-or-wal/schema.sql"))
        .expect("schema");
    connection
        .execute(
            "INSERT INTO cookies(creation_utc, host_key, name, value, encrypted_value, path, expires_utc, is_secure, is_httponly, last_access_utc) VALUES(1, 'codex.example.invalid', 'quota_marker', 'fixture-wal-value', X'', '/', 20000000000000000, 1, 1, 1)",
            [],
        )
        .expect("insert wal row");
    assert!(PathBuf::from(format!("{}-wal", db.display())).is_file());

    let app = browser_app(paths, tmp.path(), FakeDecryptorMode::Success);
    let result = test_import(&app, r#"{"schemaVersion":1,"providers":["codex"]}"#);
    assert_eq!(result.providers[0].status, BrowserProviderStatus::Success);
    drop(connection);
}

#[test]
fn fake_decryptor_success_and_failures_map_to_safe_diagnostics() {
    let cases = [
        (
            FakeDecryptorMode::Success,
            BrowserProviderStatus::Success,
            KeyringState::Unlocked,
            "browser_cookie_found",
        ),
        (
            FakeDecryptorMode::Failure,
            BrowserProviderStatus::MissingDependency,
            KeyringState::Unlocked,
            "browser_cookie_decryption_failed",
        ),
        (
            FakeDecryptorMode::Unavailable,
            BrowserProviderStatus::MissingDependency,
            KeyringState::Unavailable,
            "browser_keyring_unavailable",
        ),
        (
            FakeDecryptorMode::Locked,
            BrowserProviderStatus::MissingDependency,
            KeyringState::Locked,
            "browser_keyring_locked",
        ),
        (
            FakeDecryptorMode::PromptRequired,
            BrowserProviderStatus::MissingDependency,
            KeyringState::Locked,
            "browser_keyring_prompt_required",
        ),
    ];
    for (mode, expected_status, expected_keyring, expected_code) in cases {
        let (tmp, paths) = common::temp_paths();
        create_network_cookie_db(
            tmp.path(),
            ".config/google-chrome/Default",
            "encrypted-fake/schema.sql",
        );
        let app = browser_app(paths, tmp.path(), mode);
        let result = test_import(&app, r#"{"schemaVersion":1,"providers":["codex"]}"#);
        assert_eq!(result.providers[0].status, expected_status, "{mode:?}");
        assert_eq!(
            result.profiles[0].keyring_state, expected_keyring,
            "{mode:?}"
        );
        assert!(
            result.profiles[0]
                .diagnostic_codes
                .iter()
                .chain(result.providers[0].diagnostic_codes.iter())
                .any(|code| code == expected_code),
            "{mode:?} should include {expected_code}"
        );
        let result_json = serde_json::to_string(&result).expect("result json");
        assert_public_browser_json_safe(tmp.path(), &result_json);
    }
}

#[test]
fn missing_corrupt_and_unsupported_cookie_dbs_are_safe_failures() {
    let (missing_tmp, missing_paths) = common::temp_paths();
    fs::create_dir_all(
        missing_tmp
            .path()
            .join(".config/google-chrome/Default/Network"),
    )
    .expect("missing profile");
    let missing_app = browser_app(
        missing_paths,
        missing_tmp.path(),
        FakeDecryptorMode::Success,
    );
    let missing = test_import(&missing_app, r#"{"schemaVersion":1,"providers":["codex"]}"#);
    assert!(missing.profiles[0]
        .diagnostic_codes
        .contains(&"browser_cookie_db_missing".to_string()));

    let (corrupt_tmp, corrupt_paths) = common::temp_paths();
    let corrupt_db = corrupt_tmp
        .path()
        .join(".config/google-chrome/Default/Network/Cookies");
    fs::create_dir_all(corrupt_db.parent().expect("parent")).expect("corrupt parent");
    fs::write(&corrupt_db, b"not a sqlite database").expect("corrupt db");
    let corrupt_app = browser_app(
        corrupt_paths,
        corrupt_tmp.path(),
        FakeDecryptorMode::Success,
    );
    let corrupt = test_import(&corrupt_app, r#"{"schemaVersion":1,"providers":["codex"]}"#);
    assert!(corrupt.profiles[0]
        .diagnostic_codes
        .iter()
        .any(|code| code == "browser_cookie_db_schema_unsupported"
            || code == "browser_cookie_db_unreadable"));

    let (unsupported_tmp, unsupported_paths) = common::temp_paths();
    create_network_cookie_db(
        unsupported_tmp.path(),
        ".config/google-chrome/Default",
        "unsupported-schema/schema.sql",
    );
    let unsupported_app = browser_app(
        unsupported_paths,
        unsupported_tmp.path(),
        FakeDecryptorMode::Success,
    );
    let unsupported = test_import(
        &unsupported_app,
        r#"{"schemaVersion":1,"providers":["codex"]}"#,
    );
    assert_eq!(
        unsupported.providers[0].status,
        BrowserProviderStatus::ParseError
    );
    assert!(unsupported.profiles[0]
        .diagnostic_codes
        .contains(&"browser_cookie_db_schema_unsupported".to_string()));
}

#[test]
fn disabled_and_provider_gates_do_not_probe_cookie_values() {
    let (tmp, paths) = common::temp_paths();
    create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "plaintext-default/schema.sql",
    );
    let app = browser_app(paths, tmp.path(), FakeDecryptorMode::Success);
    app.set_settings_patch_json(r#"{"schemaVersion":1,"browserImport":{"enabled":false}}"#)
        .expect("disable");
    let disabled = test_import(&app, r#"{"schemaVersion":1,"providers":["codex"]}"#);
    assert_eq!(disabled.status, BrowserImportStatus::Unavailable);
    assert!(disabled.profiles.is_empty());

    let (tmp, paths) = common::temp_paths();
    create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "plaintext-default/schema.sql",
    );
    let app = browser_app(paths, tmp.path(), FakeDecryptorMode::Success);
    app.set_settings_patch_json(
        r#"{"schemaVersion":1,"providers":{"codex":{"enabled":true,"allowBrowserImport":false}}}"#,
    )
    .expect("provider gate");
    let provider_disabled = test_import(&app, r#"{"schemaVersion":1,"providers":["codex"]}"#);
    assert_eq!(
        provider_disabled.providers[0].status,
        BrowserProviderStatus::MissingDependency
    );
    assert_eq!(
        provider_disabled.providers[0].source_adapter,
        codexbar_linuxd::model::BrowserSourceAdapter::None
    );
}

fn browser_app(
    paths: codexbar_linuxd::paths::AppPaths,
    home: &Path,
    mode: FakeDecryptorMode,
) -> App {
    App::new_with_runtime(
        paths,
        AppRuntime::with_browser_roots_for_tests(BrowserDiscoveryRoots::synthetic_home(
            home.to_path_buf(),
        ))
        .with_browser_decryptor_mode(mode),
    )
    .expect("browser app")
}

fn test_import(app: &App, options: &str) -> BrowserImportResult {
    let result_json = app
        .test_browser_import_json(options)
        .expect("browser import result");
    common::assert_schema("browser-import-result.schema.json", &result_json);
    common::assert_public_json_safe(&result_json);
    serde_json::from_str(&result_json).expect("browser result")
}

fn create_network_cookie_db(home: &Path, profile_relative: &str, fixture: &str) -> PathBuf {
    let profile = home.join(profile_relative).join("Network");
    fs::create_dir_all(&profile).expect("profile network");
    let db = profile.join("Cookies");
    let connection = Connection::open(&db).expect("open fixture db");
    connection
        .execute_batch(&fixture_sql(fixture))
        .expect("execute fixture sql");
    db
}

fn fixture_sql(relative: &str) -> String {
    fs::read_to_string(common::repo_path("daemon/fixtures/browser/chromium").join(relative))
        .unwrap_or_else(|err| panic!("fixture SQL {relative}: {err}"))
}

#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    fs::metadata(path).expect("metadata").permissions().mode() & 0o777
}

fn assert_public_browser_json_safe(tmp_home: &Path, text: &str) {
    let lower = text.to_ascii_lowercase();
    for forbidden in [
        "fixture-value",
        "quota_marker",
        "usage_marker",
        "distractor",
        ".config",
        "network/cookies",
        "cookies.sqlite",
        "encrypted_value",
        "authorization",
        "bearer ",
        "set-cookie",
        "rawprofilepath",
        "rawcookie",
        "rawheader",
    ] {
        assert!(
            !lower.contains(forbidden),
            "browser result leaked forbidden marker {forbidden}: {text}"
        );
    }
    assert!(
        !text.contains(&tmp_home.display().to_string()),
        "browser result leaked temp home path"
    );
    assert!(
        !text.contains('@'),
        "browser result should not contain raw email-like data"
    );
}
