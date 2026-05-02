use std::collections::BTreeSet;

pub const IMPORT_STARTED: &str = "browser_import_started";
pub const IMPORT_FINISHED: &str = "browser_import_finished";
pub const IMPORT_DISABLED: &str = "browser_import_disabled";
pub const LIVE_PROFILES_DISABLED: &str = "browser_live_profiles_disabled";
pub const FIREFOX_NOT_IMPLEMENTED: &str = "browser_firefox_not_implemented";
pub const NOT_FOUND: &str = "browser_not_found";
pub const PROFILE_DISCOVERED: &str = "browser_profile_discovered";
pub const PROFILE_SKIPPED: &str = "browser_profile_skipped";
pub const PROFILE_NOT_FOUND: &str = "browser_profile_not_found";
pub const PROFILE_UNREADABLE: &str = "browser_profile_unreadable";
pub const COOKIE_DB_MISSING: &str = "browser_cookie_db_missing";
pub const COOKIE_DB_UNREADABLE: &str = "browser_cookie_db_unreadable";
pub const COOKIE_DB_LOCKED: &str = "browser_cookie_db_locked";
pub const COOKIE_DB_SCHEMA_UNSUPPORTED: &str = "browser_cookie_db_schema_unsupported";
pub const COOKIE_DECRYPTION_UNAVAILABLE: &str = "browser_cookie_decryption_unavailable";
pub const COOKIE_DECRYPTION_FAILED: &str = "browser_cookie_decryption_failed";
pub const KEYRING_UNAVAILABLE: &str = "browser_keyring_unavailable";
pub const KEYRING_LOCKED: &str = "browser_keyring_locked";
pub const KEYRING_PROMPT_REQUIRED: &str = "browser_keyring_prompt_required";
pub const COOKIE_FOUND: &str = "browser_cookie_found";
pub const COOKIE_DECRYPTED: &str = "browser_cookie_decrypted";
pub const COOKIE_MISSING: &str = "browser_cookie_missing";

pub fn push_code(codes: &mut Vec<String>, code: &'static str) {
    if !codes.iter().any(|existing| existing == code) {
        codes.push(code.to_string());
    }
}

pub fn unique_codes(codes: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for code in codes {
        if seen.insert(code.clone()) {
            unique.push(code);
        }
    }
    unique
}

pub fn contains_dependency_failure(codes: &[String]) -> bool {
    codes.iter().any(|code| {
        matches!(
            code.as_str(),
            COOKIE_DB_MISSING
                | COOKIE_DB_UNREADABLE
                | COOKIE_DB_LOCKED
                | COOKIE_DB_SCHEMA_UNSUPPORTED
                | COOKIE_DECRYPTION_UNAVAILABLE
                | COOKIE_DECRYPTION_FAILED
                | KEYRING_UNAVAILABLE
                | KEYRING_LOCKED
                | KEYRING_PROMPT_REQUIRED
                | PROFILE_UNREADABLE
        )
    })
}
