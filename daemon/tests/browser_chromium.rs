mod common;

use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};

use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
use aes::Aes128;
use codexbar_linuxd::app::{App, AppRuntime};
use codexbar_linuxd::browser::cookie_store::{
    copy_cookie_db_to_private_temp, DecryptionFailureClass,
};
use codexbar_linuxd::browser::keyring::{
    BrowserDecryptorMode, DecryptionStatus, DecryptorBackend, FakeDecryptorMode,
    SecretServiceProbeStatus,
};
use codexbar_linuxd::browser::profile::BrowserDiscoveryRoots;
use codexbar_linuxd::browser::{self, BrowserSessionRequest};
use codexbar_linuxd::model::{
    BrowserFamily, BrowserImportResult, BrowserImportStatus, BrowserProviderStatus, KeyringState,
};
use pbkdf2::pbkdf2_hmac;
use rusqlite::{params, Connection};
use sha1::Sha1;
use sha2::{Digest, Sha256};

const CHROMIUM_V10_PREFIX: &[u8] = b"v10";
const CHROMIUM_BASIC_PASSWORD: &[u8] = b"peanuts";
const CHROMIUM_BASIC_SALT: &[u8] = b"saltysalt";
const CHROMIUM_BASIC_ITERATIONS: u32 = 1;
const CHROMIUM_AES_BLOCK_LEN: usize = 16;
const CHROMIUM_BASIC_IV: [u8; CHROMIUM_AES_BLOCK_LEN] = [b' '; CHROMIUM_AES_BLOCK_LEN];

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
fn path_bearing_runtime_debug_output_is_redacted() {
    let (tmp, paths) = common::temp_paths();
    let roots = BrowserDiscoveryRoots::synthetic_home(tmp.path().to_path_buf());
    let runtime = AppRuntime::with_browser_roots_for_tests(roots.clone());
    let roots_debug = format!("{roots:?}");
    let paths_debug = format!("{paths:?}");
    let runtime_debug = format!("{runtime:?}");
    let tmp_path = tmp.path().display().to_string();

    for debug in [roots_debug, paths_debug, runtime_debug] {
        assert!(!debug.contains(&tmp_path), "{debug}");
        assert!(!debug.contains(".config"), "{debug}");
        assert!(debug.contains("[redacted]"), "{debug}");
    }
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
fn cookie_db_symlink_escape_is_skipped_without_public_path_leakage() {
    let (tmp, paths) = common::temp_paths();
    let outside_dir = tmp.path().join("outside-cookie-db");
    fs::create_dir_all(&outside_dir).expect("outside");
    let outside_db = outside_dir.join("Cookies");
    let connection = Connection::open(&outside_db).expect("outside db");
    connection
        .execute_batch(&fixture_sql("plaintext-default/schema.sql"))
        .expect("outside fixture sql");
    drop(connection);

    let network = tmp.path().join(".config/google-chrome/Default/Network");
    fs::create_dir_all(&network).expect("network");
    symlink(&outside_db, network.join("Cookies")).expect("cookie db symlink");

    let app = browser_app(paths, tmp.path(), FakeDecryptorMode::Success);
    let result_json = app
        .test_browser_import_json(r#"{"schemaVersion":1,"providers":["codex"]}"#)
        .expect("result json");
    common::assert_schema("browser-import-result.schema.json", &result_json);
    common::assert_public_json_safe(&result_json);
    let result: BrowserImportResult = serde_json::from_str(&result_json).expect("result");

    assert_eq!(result.profiles.len(), 1);
    assert!(result.profiles[0]
        .diagnostic_codes
        .contains(&"browser_cookie_db_missing".to_string()));
    assert_public_browser_json_safe(tmp.path(), &result_json);
    assert!(!result_json.contains(&outside_db.display().to_string()));
}

#[test]
fn test_browser_import_does_not_write_snapshot_cache() {
    let (tmp, paths) = common::temp_paths();
    let cache_file = paths.cache_file.clone();
    create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "plaintext-default/schema.sql",
    );
    let app = browser_app(paths, tmp.path(), FakeDecryptorMode::Success);
    assert!(!cache_file.exists());
    let result = test_import(&app, r#"{"schemaVersion":1,"providers":["codex"]}"#);

    assert_eq!(result.status, BrowserImportStatus::Success);
    assert!(!cache_file.exists());
}

#[test]
fn plaintext_cookie_material_does_not_require_keyring() {
    let (tmp, paths) = common::temp_paths();
    create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "plaintext-default/schema.sql",
    );
    let app = browser_app(paths, tmp.path(), FakeDecryptorMode::Success);
    let result = test_import(&app, r#"{"schemaVersion":1,"providers":["codex"]}"#);

    assert_eq!(result.providers[0].status, BrowserProviderStatus::Success);
    assert_eq!(result.profiles[0].keyring_state, KeyringState::NotRequired);
    assert!(result.profiles[0]
        .diagnostic_codes
        .contains(&"browser_cookie_found".to_string()));
    assert!(!result.profiles[0]
        .diagnostic_codes
        .contains(&"browser_cookie_decrypted".to_string()));
    assert!(!result.profiles[0]
        .diagnostic_codes
        .iter()
        .any(|code| code.starts_with("browser_keyring_")));
}

#[test]
fn v10_basic_cookie_material_uses_fake_decryptor_without_keyring() {
    let (tmp, paths) = common::temp_paths();
    create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "basic-v10/schema.sql",
    );
    let app = browser_app(paths, tmp.path(), FakeDecryptorMode::Success);
    let result = test_import(&app, r#"{"schemaVersion":1,"providers":["codex"]}"#);
    let result_json = serde_json::to_string(&result).expect("result json");

    assert_eq!(result.providers[0].status, BrowserProviderStatus::Success);
    assert_eq!(result.profiles[0].keyring_state, KeyringState::NotRequired);
    assert!(result.profiles[0]
        .diagnostic_codes
        .contains(&"browser_cookie_found".to_string()));
    assert!(result.profiles[0]
        .diagnostic_codes
        .contains(&"browser_cookie_decrypted".to_string()));
    assert!(result.providers[0]
        .diagnostic_codes
        .contains(&"browser_cookie_decrypted".to_string()));
    assert!(!result.profiles[0]
        .diagnostic_codes
        .iter()
        .any(|code| code.starts_with("browser_keyring_")));
    assert_public_browser_json_safe(tmp.path(), &result_json);
}

#[test]
fn v10_basic_cookie_material_uses_plain_backend_without_keyring() {
    let (tmp, paths) = common::temp_paths();
    create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "basic-v10/schema.sql",
    );
    let app = App::new_with_runtime(
        paths,
        AppRuntime::with_browser_roots_for_tests(BrowserDiscoveryRoots::synthetic_home(
            tmp.path().to_path_buf(),
        ))
        .with_browser_decryptor_backend(BrowserDecryptorMode::Plain),
    )
    .expect("browser app");
    let result = test_import(&app, r#"{"schemaVersion":1,"providers":["codex"]}"#);
    let result_json = serde_json::to_string(&result).expect("result json");

    assert_eq!(result.providers[0].status, BrowserProviderStatus::Success);
    assert_eq!(result.profiles[0].keyring_state, KeyringState::NotRequired);
    assert!(result.profiles[0]
        .diagnostic_codes
        .contains(&"browser_cookie_found".to_string()));
    assert!(result.profiles[0]
        .diagnostic_codes
        .contains(&"browser_cookie_decrypted".to_string()));
    assert!(!result.profiles[0]
        .diagnostic_codes
        .iter()
        .any(|code| code.starts_with("browser_keyring_")));
    assert_public_browser_json_safe(tmp.path(), &result_json);
}

#[test]
fn v10_basic_cookie_decryptor_unavailable_does_not_report_keyring_unavailable() {
    let (tmp, paths) = common::temp_paths();
    create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "basic-v10/schema.sql",
    );
    let app = browser_app(paths, tmp.path(), FakeDecryptorMode::Unavailable);
    let result = test_import(&app, r#"{"schemaVersion":1,"providers":["codex"]}"#);
    let result_json = serde_json::to_string(&result).expect("result json");

    assert_eq!(
        result.providers[0].status,
        BrowserProviderStatus::MissingDependency
    );
    assert_eq!(result.profiles[0].keyring_state, KeyringState::NotRequired);
    assert!(result.profiles[0]
        .diagnostic_codes
        .contains(&"browser_cookie_decryption_unavailable".to_string()));
    assert!(!result.profiles[0]
        .diagnostic_codes
        .contains(&"browser_keyring_unavailable".to_string()));
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
#[cfg(unix)]
fn cookie_db_temp_copy_skips_symlinked_companions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db = create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "plaintext-default/schema.sql",
    );
    let outside = tmp.path().join("outside-wal-marker");
    fs::write(&outside, b"outside marker").expect("outside marker");
    symlink(&outside, PathBuf::from(format!("{}-wal", db.display()))).expect("wal symlink");

    let copy = copy_cookie_db_to_private_temp(&db).expect("copy");
    assert!(copy
        .copied_files()
        .iter()
        .any(|path| path.file_name().and_then(|name| name.to_str()) == Some("Cookies")));
    assert!(!copy
        .copied_files()
        .iter()
        .any(|path| path.file_name().and_then(|name| name.to_str()) == Some("Cookies-wal")));
    for path in copy.copied_files() {
        let text = fs::read(path).expect("copied file");
        assert!(!text
            .windows(b"outside marker".len())
            .any(|window| window == b"outside marker"));
    }
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
            "browser_cookie_decrypted",
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
fn unknown_encrypted_cookie_material_is_a_safe_non_keyring_failure() {
    let (tmp, paths) = common::temp_paths();
    let db = create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "locked-or-wal/schema.sql",
    );
    let connection = Connection::open(&db).expect("open unknown-material db");
    insert_cookie_row(
        &connection,
        "codex.example.invalid",
        "quota_marker",
        "",
        b"fixture-unknown",
    );
    drop(connection);

    let app = browser_app(paths, tmp.path(), FakeDecryptorMode::Success);
    let result = test_import(&app, r#"{"schemaVersion":1,"providers":["codex"]}"#);
    let result_json = serde_json::to_string(&result).expect("result json");

    assert_eq!(
        result.providers[0].status,
        BrowserProviderStatus::MissingDependency
    );
    assert_eq!(result.profiles[0].keyring_state, KeyringState::Unknown);
    assert!(result.profiles[0]
        .diagnostic_codes
        .contains(&"browser_cookie_decryption_unavailable".to_string()));
    assert!(!result.profiles[0]
        .diagnostic_codes
        .iter()
        .any(|code| code.starts_with("browser_keyring_")));
    assert_public_browser_json_safe(tmp.path(), &result_json);
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

#[test]
fn live_codex_session_collection_reads_chatgpt_domain_only() {
    let (tmp, _paths) = common::temp_paths();
    let db = create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "plaintext-default/schema.sql",
    );
    let connection = Connection::open(&db).expect("open live-scope fixture db");
    for (host, name, value) in [
        (".chatgpt.com", "chatgpt_session", "fixture-chatgpt-live"),
        (".openai.com", "openai_session", "fixture-openai-live"),
    ] {
        connection
            .execute(
                "INSERT INTO cookies(creation_utc, host_key, name, value, encrypted_value, path, expires_utc, is_secure, is_httponly, last_access_utc) VALUES(1, ?1, ?2, ?3, X'', '/', 20000000000000000, 1, 1, 1)",
                [host, name, value],
            )
            .expect("insert live-scope cookie");
    }
    drop(connection);

    let collection = browser::collect_session_material(BrowserSessionRequest {
        providers: vec!["codex".to_string()],
        settings: Default::default(),
        roots: BrowserDiscoveryRoots::synthetic_home(tmp.path().to_path_buf()).canonicalized(),
        decryptor_mode: BrowserDecryptorMode::fake(FakeDecryptorMode::Success),
    });
    let material = collection.sessions.get("codex").expect("codex material");

    assert_eq!(material.cookie_count(), 1);
    assert_eq!(collection.material_summary.profiles_discovered, 1);
    assert_eq!(collection.material_summary.candidate_cookie_rows, 1);
    assert_eq!(collection.material_summary.plaintext_value_rows, 1);
    assert_eq!(collection.material_summary.encrypted_value_rows, 0);
    assert_eq!(collection.material_summary.usable_session_cookies, 1);
    assert_eq!(
        collection.material_summary.decryptor_backend,
        DecryptorBackend::Fake
    );
    assert_eq!(
        collection.material_summary.decryption_status,
        DecryptionStatus::NotNeeded
    );
    assert!(collection
        .provider_diagnostic_codes
        .get("codex")
        .is_some_and(|codes| codes.contains(&"browser_cookie_found".to_string())));
}

#[test]
fn live_codex_session_collection_constructs_material_from_v10_basic_cookie() {
    let (tmp, _paths) = common::temp_paths();
    let db = create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "locked-or-wal/schema.sql",
    );
    let connection = Connection::open(&db).expect("open v10 live-scope fixture db");
    insert_cookie_row(
        &connection,
        ".chatgpt.com",
        "chatgpt_session",
        "",
        b"v10fixture-chatgpt-basic",
    );
    drop(connection);

    let collection = browser::collect_session_material(BrowserSessionRequest {
        providers: vec!["codex".to_string()],
        settings: Default::default(),
        roots: BrowserDiscoveryRoots::synthetic_home(tmp.path().to_path_buf()).canonicalized(),
        decryptor_mode: BrowserDecryptorMode::fake(FakeDecryptorMode::Success),
    });
    let material = collection.sessions.get("codex").expect("codex material");
    let material_debug = format!("{material:?}");

    assert_eq!(material.cookie_count(), 1);
    assert_eq!(collection.material_summary.encrypted_value_rows, 1);
    assert_eq!(collection.material_summary.encrypted_prefixes.v10, 1);
    assert_eq!(collection.material_summary.usable_session_cookies, 1);
    assert_eq!(
        collection.material_summary.decryption_status,
        DecryptionStatus::Succeeded
    );
    assert!(collection
        .provider_diagnostic_codes
        .get("codex")
        .is_some_and(|codes| codes.contains(&"browser_cookie_found".to_string())
            && codes.contains(&"browser_cookie_decrypted".to_string())));
    assert!(!material_debug.contains("chatgpt_session"));
    assert!(!material_debug.contains("fixture-decrypted-value"));
}

#[test]
fn live_codex_session_collection_constructs_material_from_plain_v10_basic_cookie() {
    let (tmp, _paths) = common::temp_paths();
    let db = create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "locked-or-wal/schema.sql",
    );
    let connection = Connection::open(&db).expect("open v10 live-scope fixture db");
    insert_cookie_row(
        &connection,
        ".chatgpt.com",
        "chatgpt_session",
        "",
        &encrypt_v10_basic(".chatgpt.com", b"fixture-chatgpt-basic"),
    );
    drop(connection);

    let collection = browser::collect_session_material(BrowserSessionRequest {
        providers: vec!["codex".to_string()],
        settings: Default::default(),
        roots: BrowserDiscoveryRoots::synthetic_home(tmp.path().to_path_buf()).canonicalized(),
        decryptor_mode: BrowserDecryptorMode::Plain,
    });
    let material = collection.sessions.get("codex").expect("codex material");
    let material_debug = format!("{material:?}");
    let summary_json = serde_json::to_string(&collection.material_summary).expect("summary json");

    assert_eq!(material.cookie_count(), 1);
    assert_eq!(collection.material_summary.encrypted_value_rows, 1);
    assert_eq!(collection.material_summary.encrypted_prefixes.v10, 1);
    assert_eq!(collection.material_summary.usable_session_cookies, 1);
    assert_eq!(
        collection.material_summary.decryptor_backend,
        DecryptorBackend::Plain
    );
    assert_eq!(
        collection.material_summary.decryption_status,
        DecryptionStatus::Succeeded
    );
    assert_eq!(
        collection.material_summary.decryption_failure_class,
        DecryptionFailureClass::None
    );
    assert!(collection
        .provider_diagnostic_codes
        .get("codex")
        .is_some_and(|codes| codes.contains(&"browser_cookie_found".to_string())
            && codes.contains(&"browser_cookie_decrypted".to_string())));
    assert!(!material_debug.contains("chatgpt_session"));
    assert!(!material_debug.contains("fixture-chatgpt-basic"));
    assert!(!summary_json.contains("fixture-chatgpt-basic"));
    assert!(!summary_json.contains("chatgpt_session"));
    assert_public_browser_json_safe(tmp.path(), &summary_json);
}

#[test]
fn malformed_v10_and_wrong_host_hash_fail_closed_with_safe_classes() {
    for (encrypted_value, expected_class) in [
        (
            b"v10short".to_vec(),
            DecryptionFailureClass::MalformedCiphertext,
        ),
        (
            encrypt_v10_basic("codex.example.invalid", b"fixture-wrong-host"),
            DecryptionFailureClass::WrongKey,
        ),
    ] {
        let (tmp, _paths) = common::temp_paths();
        let db = create_network_cookie_db(
            tmp.path(),
            ".config/google-chrome/Default",
            "locked-or-wal/schema.sql",
        );
        let connection = Connection::open(&db).expect("open malformed v10 fixture db");
        insert_cookie_row(
            &connection,
            ".chatgpt.com",
            "plain_live",
            "fixture-chatgpt-live",
            b"",
        );
        insert_cookie_row(
            &connection,
            ".chatgpt.com",
            "bad_v10_live",
            "",
            &encrypted_value,
        );
        drop(connection);

        let collection = browser::collect_session_material(BrowserSessionRequest {
            providers: vec!["codex".to_string()],
            settings: Default::default(),
            roots: BrowserDiscoveryRoots::synthetic_home(tmp.path().to_path_buf()).canonicalized(),
            decryptor_mode: BrowserDecryptorMode::Plain,
        });
        let codes = collection
            .provider_diagnostic_codes
            .get("codex")
            .expect("codex diagnostics");
        let summary_json =
            serde_json::to_string(&collection.material_summary).expect("summary json");

        assert!(!collection.sessions.contains_key("codex"));
        assert!(codes.contains(&"browser_cookie_decryption_failed".to_string()));
        assert_eq!(collection.material_summary.usable_session_cookies, 0);
        assert_eq!(
            collection.material_summary.decryption_status,
            DecryptionStatus::Failed
        );
        assert_eq!(
            collection.material_summary.decryption_failure_class,
            expected_class
        );
        assert!(!summary_json.contains("plain_live"));
        assert!(!summary_json.contains("bad_v10_live"));
        assert!(!summary_json.contains("fixture-chatgpt-live"));
        assert_public_browser_json_safe(tmp.path(), &summary_json);
    }
}

#[test]
fn unsupported_encrypted_prefixes_classify_without_keyring_or_raw_material() {
    for (encrypted_value, prefix_counter) in [
        (b"v20fixture-v20".as_slice(), "v20"),
        (b"v24fixture-v24".as_slice(), "v24"),
    ] {
        let (tmp, _paths) = common::temp_paths();
        let db = create_network_cookie_db(
            tmp.path(),
            ".config/google-chrome/Default",
            "locked-or-wal/schema.sql",
        );
        let connection = Connection::open(&db).expect("open unsupported prefix fixture db");
        insert_cookie_row(
            &connection,
            ".chatgpt.com",
            "unsupported_live",
            "",
            encrypted_value,
        );
        drop(connection);

        let collection = browser::collect_session_material(BrowserSessionRequest {
            providers: vec!["codex".to_string()],
            settings: Default::default(),
            roots: BrowserDiscoveryRoots::synthetic_home(tmp.path().to_path_buf()).canonicalized(),
            decryptor_mode: BrowserDecryptorMode::Plain,
        });
        let codes = collection
            .provider_diagnostic_codes
            .get("codex")
            .expect("codex diagnostics");
        let summary_json =
            serde_json::to_string(&collection.material_summary).expect("summary json");

        assert!(!collection.sessions.contains_key("codex"));
        assert!(codes.contains(&"browser_cookie_decryption_unavailable".to_string()));
        assert!(!codes
            .iter()
            .any(|code| code.starts_with("browser_keyring_")));
        assert_eq!(
            collection.material_summary.decryption_failure_class,
            DecryptionFailureClass::UnsupportedFormat
        );
        match prefix_counter {
            "v20" => assert_eq!(collection.material_summary.encrypted_prefixes.v20, 1),
            "v24" => assert_eq!(collection.material_summary.encrypted_prefixes.v24, 1),
            _ => unreachable!(),
        }
        assert!(!summary_json.contains("unsupported_live"));
        assert_public_browser_json_safe(tmp.path(), &summary_json);
    }
}

#[test]
fn default_plain_backend_uses_plaintext_but_fails_closed_for_encrypted_rows() {
    let (tmp, _paths) = common::temp_paths();
    let db = create_network_cookie_db(
        tmp.path(),
        ".config/google-chrome/Default",
        "locked-or-wal/schema.sql",
    );
    let connection = Connection::open(&db).expect("open mixed live-scope fixture db");
    insert_cookie_row(
        &connection,
        ".chatgpt.com",
        "plain_live",
        "fixture-chatgpt-live",
        b"",
    );
    insert_cookie_row(
        &connection,
        ".chatgpt.com",
        "encrypted_live",
        "",
        b"v11fixture-chatgpt-keyring",
    );
    drop(connection);

    let collection = browser::collect_session_material(BrowserSessionRequest {
        providers: vec!["codex".to_string()],
        settings: Default::default(),
        roots: BrowserDiscoveryRoots::synthetic_home(tmp.path().to_path_buf()).canonicalized(),
        decryptor_mode: BrowserDecryptorMode::Plain,
    });
    let codes = collection
        .provider_diagnostic_codes
        .get("codex")
        .expect("codex diagnostics");
    let summary_json =
        serde_json::to_string(&collection.material_summary).expect("material summary json");

    assert!(!collection.sessions.contains_key("codex"));
    assert!(codes.contains(&"browser_cookie_decryption_unavailable".to_string()));
    assert!(codes.contains(&"browser_keyring_unavailable".to_string()));
    assert_eq!(collection.material_summary.profiles_discovered, 1);
    assert_eq!(collection.material_summary.candidate_cookie_rows, 2);
    assert_eq!(collection.material_summary.plaintext_value_rows, 1);
    assert_eq!(collection.material_summary.encrypted_value_rows, 1);
    assert_eq!(collection.material_summary.encrypted_prefixes.v11, 1);
    assert_eq!(collection.material_summary.usable_session_cookies, 0);
    assert_eq!(
        collection.material_summary.decryptor_backend,
        DecryptorBackend::Plain
    );
    assert_eq!(
        collection.material_summary.decryption_status,
        DecryptionStatus::Unavailable
    );
    assert_public_browser_json_safe(tmp.path(), &summary_json);
}

#[test]
fn secret_service_probe_statuses_map_without_prompting_or_extracting_secrets() {
    for (probe_status, expected_keyring, expected_code) in [
        (
            SecretServiceProbeStatus::Unavailable,
            KeyringState::Unavailable,
            "browser_keyring_unavailable",
        ),
        (
            SecretServiceProbeStatus::Locked,
            KeyringState::Locked,
            "browser_keyring_locked",
        ),
        (
            SecretServiceProbeStatus::PromptRequired,
            KeyringState::Locked,
            "browser_keyring_prompt_required",
        ),
    ] {
        let (tmp, paths) = common::temp_paths();
        let db = create_network_cookie_db(
            tmp.path(),
            ".config/google-chrome/Default",
            "locked-or-wal/schema.sql",
        );
        let connection = Connection::open(&db).expect("open keyring probe fixture db");
        insert_cookie_row(
            &connection,
            ".chatgpt.com",
            "encrypted_live",
            "",
            b"v11fixture-chatgpt-keyring",
        );
        insert_cookie_row(
            &connection,
            "codex.example.invalid",
            "quota_marker",
            "",
            b"v11fixture-codex-keyring",
        );
        drop(connection);

        let collection = browser::collect_session_material(BrowserSessionRequest {
            providers: vec!["codex".to_string()],
            settings: Default::default(),
            roots: BrowserDiscoveryRoots::synthetic_home(tmp.path().to_path_buf()).canonicalized(),
            decryptor_mode: BrowserDecryptorMode::SecretServiceProbe(probe_status),
        });
        let app = App::new_with_runtime(
            paths,
            AppRuntime::with_browser_roots_for_tests(BrowserDiscoveryRoots::synthetic_home(
                tmp.path().to_path_buf(),
            ))
            .with_browser_decryptor_backend(BrowserDecryptorMode::SecretServiceProbe(probe_status)),
        )
        .expect("probe app");
        let result = test_import(&app, r#"{"schemaVersion":1,"providers":["codex"]}"#);
        let codes = collection
            .provider_diagnostic_codes
            .get("codex")
            .expect("codex diagnostics");
        let summary_json =
            serde_json::to_string(&collection.material_summary).expect("summary json");

        assert!(!collection.sessions.contains_key("codex"));
        assert!(
            codes.contains(&expected_code.to_string()),
            "{probe_status:?}"
        );
        assert_eq!(result.profiles[0].keyring_state, expected_keyring);
        assert!(codes.contains(&"browser_cookie_decryption_unavailable".to_string()));
        assert_eq!(
            collection.material_summary.decryptor_backend,
            DecryptorBackend::SecretService
        );
        assert_eq!(
            collection.material_summary.decryption_status,
            match probe_status {
                SecretServiceProbeStatus::Unavailable => DecryptionStatus::Unavailable,
                SecretServiceProbeStatus::Locked => DecryptionStatus::Locked,
                SecretServiceProbeStatus::PromptRequired => DecryptionStatus::PromptRequired,
            }
        );
        assert!(
            collection
                .diagnostic_codes
                .contains(&"browser_import_finished".to_string()),
            "probe collection should complete without interactive prompting"
        );
        assert_public_browser_json_safe(tmp.path(), &summary_json);
    }
}

#[test]
#[ignore = "requires CODEXBAR_BROWSER_LIVE=1 and CODEXBAR_BROWSER_IMPORT_FAKE_HOME=/tmp/..."]
fn live_throwaway_browser_profile_smoke() {
    let Some(fake_home) = live_throwaway_fake_home() else {
        return;
    };
    assert!(
        observed_live_cookie_db_shape(&fake_home).is_some(),
        "live throwaway profile did not contain Cookies in a supported relative shape"
    );

    let (_tmp, paths) = common::temp_paths();
    let app = App::new_with_runtime(
        paths,
        AppRuntime::from_env().expect("live throwaway browser runtime"),
    )
    .expect("live browser app");
    let provider =
        env::var("CODEXBAR_BROWSER_IMPORT_LIVE_PROVIDER").unwrap_or_else(|_| "smoke".to_string());
    assert!(
        codexbar_linuxd::config::is_safe_id(&provider),
        "live smoke provider id must be safe"
    );
    let options = serde_json::json!({
        "schemaVersion": 1,
        "providers": [provider],
        "includeDiagnostics": true
    });
    let result_json = app
        .test_browser_import_json(&options.to_string())
        .expect("live browser import result");
    common::assert_schema("browser-import-result.schema.json", &result_json);
    common::assert_public_json_safe(&result_json);
    assert_public_browser_json_safe(&fake_home, &result_json);

    let result: BrowserImportResult = serde_json::from_str(&result_json).expect("result");
    assert!(
        !result.profiles.is_empty(),
        "live throwaway import did not discover a Chromium-family profile"
    );
    assert!(result.profiles.iter().any(|profile| matches!(
        profile.browser_family,
        BrowserFamily::Chrome | BrowserFamily::Chromium | BrowserFamily::Brave
    )));
    assert!(result.profiles.iter().all(|profile| {
        !profile.profile_id.contains('/')
            && !profile.profile_id.contains('\\')
            && !profile.profile_id.contains("..")
            && !profile.profile_id.to_ascii_lowercase().contains(".config")
    }));
    assert!(result.profiles.iter().all(|profile| {
        matches!(
            profile.profile_display_name.as_str(),
            "Chrome Default"
                | "Chrome Profile 1"
                | "Chromium Default"
                | "Chromium Profile 1"
                | "Brave Default"
                | "Brave Profile 1"
        )
    }));
    assert!(result
        .diagnostic_codes
        .contains(&"browser_profile_discovered".to_string()));
    if env::var("CODEXBAR_BROWSER_IMPORT_EXPECT_COOKIE")
        .ok()
        .as_deref()
        == Some("1")
    {
        assert_eq!(
            result.providers[0].status,
            BrowserProviderStatus::Success,
            "seeded throwaway cookie should be found"
        );
    } else {
        assert!(
            matches!(
                result.providers[0].status,
                BrowserProviderStatus::Success | BrowserProviderStatus::Unauthenticated
            ),
            "live throwaway provider status should be success or unauthenticated"
        );
    }
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
    match fixture {
        "basic-v10/schema.sql" => insert_cookie_row(
            &connection,
            "codex.example.invalid",
            "quota_marker",
            "",
            &encrypt_v10_basic("codex.example.invalid", b"fixture-basic-alpha"),
        ),
        "encrypted-fake/schema.sql" => {
            insert_cookie_row(
                &connection,
                "codex.example.invalid",
                "quota_marker",
                "",
                b"v11fixture-encrypted-alpha",
            );
            insert_cookie_row(
                &connection,
                "other.example.invalid",
                "quota_marker",
                "",
                b"v11fixture-encrypted-distractor",
            );
        }
        _ => {}
    }
    db
}

fn insert_cookie_row(
    connection: &Connection,
    host_key: &str,
    name: &str,
    value: &str,
    encrypted_value: &[u8],
) {
    connection
        .execute(
            "INSERT INTO cookies(creation_utc, host_key, name, value, encrypted_value, path, expires_utc, is_secure, is_httponly, last_access_utc) VALUES(1, ?1, ?2, ?3, ?4, '/', 20000000000000000, 1, 1, 1)",
            params![host_key, name, value, encrypted_value],
        )
        .expect("insert synthetic cookie row");
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

fn live_throwaway_fake_home() -> Option<PathBuf> {
    if env::var("CODEXBAR_BROWSER_LIVE").ok().as_deref() != Some("1") {
        eprintln!("skipping live throwaway browser smoke: CODEXBAR_BROWSER_LIVE=1 is not set");
        return None;
    }
    let value = env::var_os("CODEXBAR_BROWSER_IMPORT_FAKE_HOME")
        .expect("CODEXBAR_BROWSER_IMPORT_FAKE_HOME is required when CODEXBAR_BROWSER_LIVE=1");
    assert!(
        !value.is_empty(),
        "CODEXBAR_BROWSER_IMPORT_FAKE_HOME must not be empty"
    );
    let fake_home = fs::canonicalize(PathBuf::from(value))
        .expect("CODEXBAR_BROWSER_IMPORT_FAKE_HOME must exist");
    assert_ne!(
        fake_home,
        PathBuf::from("/"),
        "CODEXBAR_BROWSER_IMPORT_FAKE_HOME must not be /"
    );
    if let Some(real_home) = env::var_os("HOME").and_then(|path| fs::canonicalize(path).ok()) {
        assert_ne!(
            fake_home, real_home,
            "CODEXBAR_BROWSER_IMPORT_FAKE_HOME must not be the real HOME"
        );
        assert!(
            !fake_home.starts_with(real_home.join(".config")),
            "CODEXBAR_BROWSER_IMPORT_FAKE_HOME must not be under the real ~/.config"
        );
    }
    assert!(
        fake_home.join(".codexbar-throwaway-browser-root").is_file(),
        "CODEXBAR_BROWSER_IMPORT_FAKE_HOME must include the throwaway marker file"
    );
    Some(fake_home)
}

fn observed_live_cookie_db_shape(fake_home: &Path) -> Option<&'static str> {
    for (relative, shape) in [
        (
            ".config/google-chrome/Default/Network/Cookies",
            "chrome-network",
        ),
        (".config/google-chrome/Default/Cookies", "chrome-legacy"),
        (
            ".config/chromium/Default/Network/Cookies",
            "chromium-network",
        ),
        (".config/chromium/Default/Cookies", "chromium-legacy"),
        (
            ".config/BraveSoftware/Brave-Browser/Default/Network/Cookies",
            "brave-network",
        ),
        (
            ".config/BraveSoftware/Brave-Browser/Default/Cookies",
            "brave-legacy",
        ),
        (
            "snap/chromium/common/chromium/Default/Network/Cookies",
            "chromium-snap-network",
        ),
        (
            "snap/chromium/common/chromium/Default/Cookies",
            "chromium-snap-legacy",
        ),
    ] {
        if fake_home.join(relative).is_file() {
            return Some(shape);
        }
    }
    None
}
