use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{copy, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::{params_from_iter, Connection, OpenFlags};

use crate::browser::diagnostics;
use crate::browser::keyring::{
    CookieDecryptContext, CookieDecryptor, DecryptError, DecryptionStatus, DecryptorBackend,
};
use crate::browser::profile::{path_is_under, BrowserProfileDescriptor};
use crate::browser::session_material::{
    cookie_domain_matches, cookie_path_matches, CookieRequestTarget, ScopedCookie, SessionMaterial,
    SessionMaterialError, SessionMaterialPolicy,
};
use crate::model::KeyringState;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Eq, PartialEq)]
pub struct CookieQuery {
    provider: String,
    domains: Vec<String>,
    names: Vec<String>,
    target_url: Option<&'static str>,
}

impl CookieQuery {
    pub fn for_provider(provider: &str) -> Self {
        match provider {
            "codex" => Self {
                provider: provider.to_string(),
                domains: vec![
                    "codex.example.invalid".to_string(),
                    "openai.example.invalid".to_string(),
                ],
                names: vec!["quota_marker".to_string(), "usage_marker".to_string()],
                target_url: None,
            },
            "claude" => Self {
                provider: provider.to_string(),
                domains: vec!["claude.example.invalid".to_string()],
                names: vec!["quota_marker".to_string()],
                target_url: None,
            },
            _ => Self {
                provider: provider.to_string(),
                domains: vec![format!(
                    "{}.example.invalid",
                    provider_domain_label(provider)
                )],
                names: vec!["quota_marker".to_string()],
                target_url: None,
            },
        }
    }

    pub fn for_live_web_provider(provider: &str) -> Self {
        match provider {
            "codex" => Self {
                provider: provider.to_string(),
                domains: vec!["chatgpt.com".to_string()],
                names: Vec::new(),
                target_url: Some(SessionMaterialPolicy::codex_dashboard().target_url()),
            },
            _ => Self::for_provider(provider),
        }
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    fn request_target(&self) -> Option<CookieRequestTarget> {
        self.target_url.and_then(CookieRequestTarget::parse)
    }

    fn host_keys(&self) -> Vec<String> {
        let mut values = Vec::with_capacity(self.domains.len() * 2);
        for domain in &self.domains {
            values.push(domain.clone());
            values.push(format!(".{domain}"));
        }
        values
    }
}

impl fmt::Debug for CookieQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CookieQuery")
            .field("provider", &self.provider)
            .field("domain_count", &self.domains.len())
            .field("name_count", &self.names.len())
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct CookieStoreProfileOutcome {
    pub cookies_found: u64,
    pub keyring_state: KeyringState,
    pub diagnostic_codes: Vec<String>,
    pub provider_counts: BTreeMap<String, u64>,
    pub provider_diagnostic_codes: BTreeMap<String, Vec<String>>,
    pub material_summary: BrowserCookieMaterialSummary,
}

impl Default for CookieStoreProfileOutcome {
    fn default() -> Self {
        Self {
            cookies_found: 0,
            keyring_state: KeyringState::Unknown,
            diagnostic_codes: Vec::new(),
            provider_counts: BTreeMap::new(),
            provider_diagnostic_codes: BTreeMap::new(),
            material_summary: BrowserCookieMaterialSummary::default(),
        }
    }
}

impl CookieStoreProfileOutcome {
    fn push_code(&mut self, code: &'static str) {
        diagnostics::push_code(&mut self.diagnostic_codes, code);
    }

    fn push_provider_code(&mut self, provider: &str, code: &'static str) {
        let codes = self
            .provider_diagnostic_codes
            .entry(provider.to_string())
            .or_default();
        diagnostics::push_code(codes, code);
    }

    fn add_provider_count(&mut self, provider: &str, count: u64) {
        *self
            .provider_counts
            .entry(provider.to_string())
            .or_default() += count;
        self.cookies_found += count;
    }
}

pub struct CookieStoreSessionOutcome {
    pub profile: CookieStoreProfileOutcome,
    pub sessions: BTreeMap<String, SessionMaterial>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCookieMaterialSummary {
    pub profiles_discovered: u64,
    pub candidate_cookie_rows: u64,
    pub domain_matched_rows: u64,
    pub path_matched_rows: u64,
    pub secure_matched_rows: u64,
    pub plaintext_value_rows: u64,
    pub encrypted_value_rows: u64,
    pub encrypted_prefixes: EncryptedPrefixCounts,
    pub expired_rows: u64,
    pub decrypted_rows: u64,
    pub header_eligible_rows: u64,
    pub header_rejected_rows: u64,
    pub header_rejected_by_class: HeaderRejectedByClassCounts,
    pub cookie_header_status: CookieHeaderStatus,
    pub usable_session_cookies: u64,
    pub decryptor_backend: DecryptorBackend,
    pub decryption_status: DecryptionStatus,
    pub decryption_failure_class: DecryptionFailureClass,
}

impl Default for BrowserCookieMaterialSummary {
    fn default() -> Self {
        Self {
            profiles_discovered: 0,
            candidate_cookie_rows: 0,
            domain_matched_rows: 0,
            path_matched_rows: 0,
            secure_matched_rows: 0,
            plaintext_value_rows: 0,
            encrypted_value_rows: 0,
            encrypted_prefixes: EncryptedPrefixCounts::default(),
            expired_rows: 0,
            decrypted_rows: 0,
            header_eligible_rows: 0,
            header_rejected_rows: 0,
            header_rejected_by_class: HeaderRejectedByClassCounts::default(),
            cookie_header_status: CookieHeaderStatus::NotAttempted,
            usable_session_cookies: 0,
            decryptor_backend: DecryptorBackend::Unavailable,
            decryption_status: DecryptionStatus::NotNeeded,
            decryption_failure_class: DecryptionFailureClass::None,
        }
    }
}

impl BrowserCookieMaterialSummary {
    pub fn with_backend(backend: DecryptorBackend) -> Self {
        Self {
            decryptor_backend: backend,
            ..Self::default()
        }
    }

    pub fn add_profiles_discovered(&mut self, count: usize) {
        self.profiles_discovered += count as u64;
    }

    pub fn combine_profile(&mut self, other: BrowserCookieMaterialSummary) {
        self.candidate_cookie_rows += other.candidate_cookie_rows;
        self.domain_matched_rows += other.domain_matched_rows;
        self.path_matched_rows += other.path_matched_rows;
        self.secure_matched_rows += other.secure_matched_rows;
        self.plaintext_value_rows += other.plaintext_value_rows;
        self.encrypted_value_rows += other.encrypted_value_rows;
        self.encrypted_prefixes.combine(other.encrypted_prefixes);
        self.expired_rows += other.expired_rows;
        self.decrypted_rows += other.decrypted_rows;
        self.header_eligible_rows += other.header_eligible_rows;
        self.header_rejected_rows += other.header_rejected_rows;
        self.header_rejected_by_class
            .combine(other.header_rejected_by_class);
        self.cookie_header_status =
            combine_cookie_header_status(self.cookie_header_status, other.cookie_header_status);
        self.usable_session_cookies += other.usable_session_cookies;
        self.decryption_status =
            combine_decryption_status(self.decryption_status, other.decryption_status);
        self.decryption_failure_class = combine_decryption_failure_class(
            self.decryption_failure_class,
            other.decryption_failure_class,
        );
    }

    fn observe_rows(&mut self, rows: &[CookieRow]) {
        for row in rows {
            self.candidate_cookie_rows += 1;
            if row.encrypted_value.is_empty() {
                self.plaintext_value_rows += 1;
            }
            if !row.encrypted_value.is_empty() {
                self.encrypted_value_rows += 1;
                self.encrypted_prefixes
                    .record(encrypted_prefix(&row.encrypted_value));
            }
            if row.is_expired() {
                self.expired_rows += 1;
            }
        }
    }

    fn record_domain_match(&mut self) {
        self.domain_matched_rows += 1;
    }

    fn record_path_match(&mut self) {
        self.path_matched_rows += 1;
    }

    fn record_secure_match(&mut self) {
        self.secure_matched_rows += 1;
    }

    fn record_decrypted_row(&mut self) {
        self.decrypted_rows += 1;
        self.decryption_status =
            combine_decryption_status(self.decryption_status, DecryptionStatus::Succeeded);
    }

    fn record_header_eligible_row(&mut self) {
        self.header_eligible_rows += 1;
    }

    fn record_header_rejection(&mut self, class: HeaderRejectedClass) {
        self.header_rejected_rows += 1;
        self.header_rejected_by_class.record(class);
    }

    fn record_cookie_header_status(&mut self, status: CookieHeaderStatus) {
        self.cookie_header_status = combine_cookie_header_status(self.cookie_header_status, status);
    }

    fn record_usable_session_cookies(&mut self, count: u64, decrypted: bool) {
        self.usable_session_cookies += count;
        if decrypted {
            self.decryption_status =
                combine_decryption_status(self.decryption_status, DecryptionStatus::Succeeded);
        }
    }

    fn record_decryption_error(&mut self, error: CookieRowDecryptError) {
        let status = match error.error {
            DecryptError::Unavailable | DecryptError::UnsupportedFormat => {
                DecryptionStatus::Unavailable
            }
            DecryptError::Locked => DecryptionStatus::Locked,
            DecryptError::PromptRequired => DecryptionStatus::PromptRequired,
            DecryptError::Failed
            | DecryptError::MalformedCiphertext
            | DecryptError::WrongKey
            | DecryptError::InvalidMaterial
            | DecryptError::HeaderTooLarge
            | DecryptError::TooManyCookies => DecryptionStatus::Failed,
        };
        self.decryption_status = combine_decryption_status(self.decryption_status, status);
        self.decryption_failure_class = combine_decryption_failure_class(
            self.decryption_failure_class,
            DecryptionFailureClass::from_row_error(error),
        );
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecryptionFailureClass {
    #[default]
    None,
    KeyringNeeded,
    UnsupportedFormat,
    MalformedCiphertext,
    WrongKey,
    InvalidMaterial,
    HeaderTooLarge,
    TooManyCookies,
    Unavailable,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedPrefixCounts {
    pub v10: u64,
    pub v11: u64,
    pub v20: u64,
    pub v24: u64,
    pub unknown: u64,
}

impl EncryptedPrefixCounts {
    fn record(&mut self, prefix: EncryptedCookiePrefix) {
        match prefix {
            EncryptedCookiePrefix::V10 => self.v10 += 1,
            EncryptedCookiePrefix::V11 => self.v11 += 1,
            EncryptedCookiePrefix::V20 => self.v20 += 1,
            EncryptedCookiePrefix::V24 => self.v24 += 1,
            EncryptedCookiePrefix::Unknown => self.unknown += 1,
        }
    }

    fn combine(&mut self, other: Self) {
        self.v10 += other.v10;
        self.v11 += other.v11;
        self.v20 += other.v20;
        self.v24 += other.v24;
        self.unknown += other.unknown;
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CookieHeaderStatus {
    #[default]
    NotAttempted,
    Built,
    Empty,
    HeaderTooLarge,
    TooManyCookies,
    InvalidMaterial,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct HeaderRejectedByClassCounts {
    pub invalid_name: u64,
    pub invalid_value: u64,
    pub empty_name: u64,
    pub too_long: u64,
    pub expired: u64,
    pub domain_mismatch: u64,
    pub path_mismatch: u64,
    pub secure_mismatch: u64,
    pub unsupported_prefix: u64,
    pub decrypt_failed: u64,
}

impl HeaderRejectedByClassCounts {
    fn record(&mut self, class: HeaderRejectedClass) {
        match class {
            HeaderRejectedClass::InvalidName => self.invalid_name += 1,
            HeaderRejectedClass::InvalidValue => self.invalid_value += 1,
            HeaderRejectedClass::EmptyName => self.empty_name += 1,
            HeaderRejectedClass::TooLong => self.too_long += 1,
            HeaderRejectedClass::Expired => self.expired += 1,
            HeaderRejectedClass::DomainMismatch => self.domain_mismatch += 1,
            HeaderRejectedClass::PathMismatch => self.path_mismatch += 1,
            HeaderRejectedClass::SecureMismatch => self.secure_mismatch += 1,
            HeaderRejectedClass::UnsupportedPrefix => self.unsupported_prefix += 1,
            HeaderRejectedClass::DecryptFailed => self.decrypt_failed += 1,
        }
    }

    fn combine(&mut self, other: Self) {
        self.invalid_name += other.invalid_name;
        self.invalid_value += other.invalid_value;
        self.empty_name += other.empty_name;
        self.too_long += other.too_long;
        self.expired += other.expired;
        self.domain_mismatch += other.domain_mismatch;
        self.path_mismatch += other.path_mismatch;
        self.secure_mismatch += other.secure_mismatch;
        self.unsupported_prefix += other.unsupported_prefix;
        self.decrypt_failed += other.decrypt_failed;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HeaderRejectedClass {
    InvalidName,
    InvalidValue,
    EmptyName,
    TooLong,
    Expired,
    DomainMismatch,
    PathMismatch,
    SecureMismatch,
    UnsupportedPrefix,
    DecryptFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EncryptedCookiePrefix {
    V10,
    V11,
    V20,
    V24,
    Unknown,
}

fn encrypted_prefix(value: &[u8]) -> EncryptedCookiePrefix {
    if value.starts_with(b"v10") {
        EncryptedCookiePrefix::V10
    } else if value.starts_with(b"v11") {
        EncryptedCookiePrefix::V11
    } else if value.starts_with(b"v20") {
        EncryptedCookiePrefix::V20
    } else if value.starts_with(b"v24") {
        EncryptedCookiePrefix::V24
    } else {
        EncryptedCookiePrefix::Unknown
    }
}

fn combine_decryption_status(
    current: DecryptionStatus,
    next: DecryptionStatus,
) -> DecryptionStatus {
    use DecryptionStatus::{Failed, Locked, NotNeeded, PromptRequired, Succeeded, Unavailable};
    match (current, next) {
        (Failed, _) | (_, Failed) => Failed,
        (Locked, _) | (_, Locked) => Locked,
        (PromptRequired, _) | (_, PromptRequired) => PromptRequired,
        (Unavailable, _) | (_, Unavailable) => Unavailable,
        (Succeeded, _) | (_, Succeeded) => Succeeded,
        (NotNeeded, NotNeeded) => NotNeeded,
    }
}

fn combine_cookie_header_status(
    current: CookieHeaderStatus,
    next: CookieHeaderStatus,
) -> CookieHeaderStatus {
    use CookieHeaderStatus::{
        Built, Empty, HeaderTooLarge, InvalidMaterial, NotAttempted, TooManyCookies,
    };
    match (current, next) {
        (HeaderTooLarge, _) | (_, HeaderTooLarge) => HeaderTooLarge,
        (TooManyCookies, _) | (_, TooManyCookies) => TooManyCookies,
        (InvalidMaterial, _) | (_, InvalidMaterial) => InvalidMaterial,
        (Built, _) | (_, Built) => Built,
        (Empty, _) | (_, Empty) => Empty,
        (NotAttempted, NotAttempted) => NotAttempted,
    }
}

fn combine_decryption_failure_class(
    current: DecryptionFailureClass,
    next: DecryptionFailureClass,
) -> DecryptionFailureClass {
    use DecryptionFailureClass::{
        Failed, HeaderTooLarge, InvalidMaterial, KeyringNeeded, MalformedCiphertext, None,
        TooManyCookies, Unavailable, UnsupportedFormat, WrongKey,
    };
    match (current, next) {
        (WrongKey, _) | (_, WrongKey) => WrongKey,
        (MalformedCiphertext, _) | (_, MalformedCiphertext) => MalformedCiphertext,
        (UnsupportedFormat, _) | (_, UnsupportedFormat) => UnsupportedFormat,
        (KeyringNeeded, _) | (_, KeyringNeeded) => KeyringNeeded,
        (HeaderTooLarge, _) | (_, HeaderTooLarge) => HeaderTooLarge,
        (TooManyCookies, _) | (_, TooManyCookies) => TooManyCookies,
        (InvalidMaterial, _) | (_, InvalidMaterial) => InvalidMaterial,
        (Unavailable, _) | (_, Unavailable) => Unavailable,
        (Failed, _) | (_, Failed) => Failed,
        (None, None) => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CookieDbFailure {
    Missing,
    Unreadable,
    Locked,
    UnsupportedSchema,
}

impl CookieDbFailure {
    fn code(&self) -> &'static str {
        match self {
            Self::Missing => diagnostics::COOKIE_DB_MISSING,
            Self::Unreadable => diagnostics::COOKIE_DB_UNREADABLE,
            Self::Locked => diagnostics::COOKIE_DB_LOCKED,
            Self::UnsupportedSchema => diagnostics::COOKIE_DB_SCHEMA_UNSUPPORTED,
        }
    }
}

pub fn read_profile_cookies(
    profile: &BrowserProfileDescriptor,
    queries: &[CookieQuery],
    decryptor: &dyn CookieDecryptor,
) -> CookieStoreProfileOutcome {
    let mut outcome = CookieStoreProfileOutcome {
        keyring_state: KeyringState::NotRequired,
        material_summary: BrowserCookieMaterialSummary::with_backend(decryptor.backend()),
        ..CookieStoreProfileOutcome::default()
    };
    let Some(db_path) = find_chromium_cookie_db(profile.profile_path()) else {
        outcome.push_code(diagnostics::COOKIE_DB_MISSING);
        for query in queries {
            outcome.push_provider_code(query.provider(), diagnostics::COOKIE_DB_MISSING);
        }
        return outcome;
    };
    let temp_copy = match copy_cookie_db_to_private_temp(&db_path) {
        Ok(copy) => copy,
        Err(failure) => {
            outcome.push_code(failure.code());
            for query in queries {
                outcome.push_provider_code(query.provider(), failure.code());
            }
            return outcome;
        }
    };
    let connection = match open_read_only(temp_copy.db_path()) {
        Ok(connection) => connection,
        Err(failure) => {
            outcome.push_code(failure.code());
            for query in queries {
                outcome.push_provider_code(query.provider(), failure.code());
            }
            return outcome;
        }
    };
    let columns = match cookie_columns(&connection) {
        Ok(columns) if required_columns_present(&columns) => columns,
        Ok(_) => {
            outcome.push_code(diagnostics::COOKIE_DB_SCHEMA_UNSUPPORTED);
            for query in queries {
                outcome.push_provider_code(
                    query.provider(),
                    diagnostics::COOKIE_DB_SCHEMA_UNSUPPORTED,
                );
            }
            return outcome;
        }
        Err(failure) => {
            outcome.push_code(failure.code());
            for query in queries {
                outcome.push_provider_code(query.provider(), failure.code());
            }
            return outcome;
        }
    };
    let encrypted_value_has_host_hash = encrypted_values_include_host_hash(&connection);

    for query in queries {
        match query_cookie_rows(&connection, &columns, query, encrypted_value_has_host_hash) {
            Ok(rows) => {
                outcome.material_summary.observe_rows(&rows);
                let mut provider_count = 0;
                let mut provider_used_keyring = false;
                let mut provider_decrypted = false;
                let mut provider_had_material_error = false;
                for row in rows {
                    if row.is_expired() {
                        continue;
                    }
                    match row.session_material(query.provider(), decryptor) {
                        Ok(Some((kind, material))) => {
                            provider_count += material.cookie_count() as u64;
                            provider_used_keyring |= kind.requires_keyring();
                            provider_decrypted |= kind.was_decrypted();
                            drop(material);
                        }
                        Ok(None) => {}
                        Err(error) => {
                            provider_had_material_error = true;
                            outcome.material_summary.record_decryption_error(error);
                            record_decrypt_error(&mut outcome, query.provider(), error);
                        }
                    }
                }
                if provider_had_material_error {
                    provider_count = 0;
                }
                if provider_count > 0 {
                    if provider_used_keyring
                        || (!provider_decrypted
                            && rows_required_keyring(
                                &connection,
                                &columns,
                                query,
                                encrypted_value_has_host_hash,
                            ))
                    {
                        record_keyring_state(&mut outcome, KeyringState::Unlocked);
                    }
                    outcome
                        .material_summary
                        .record_usable_session_cookies(provider_count, provider_decrypted);
                    outcome.add_provider_count(query.provider(), provider_count);
                    outcome.push_code(diagnostics::COOKIE_FOUND);
                    outcome.push_provider_code(query.provider(), diagnostics::COOKIE_FOUND);
                    if provider_decrypted {
                        outcome.push_code(diagnostics::COOKIE_DECRYPTED);
                        outcome.push_provider_code(query.provider(), diagnostics::COOKIE_DECRYPTED);
                    }
                } else if !outcome
                    .provider_diagnostic_codes
                    .get(query.provider())
                    .is_some_and(|codes| diagnostics::contains_dependency_failure(codes))
                {
                    outcome.push_provider_code(query.provider(), diagnostics::COOKIE_MISSING);
                }
            }
            Err(failure) => {
                outcome.push_code(failure.code());
                outcome.push_provider_code(query.provider(), failure.code());
            }
        }
    }
    if outcome.cookies_found == 0
        && !diagnostics::contains_dependency_failure(&outcome.diagnostic_codes)
    {
        outcome.push_code(diagnostics::COOKIE_MISSING);
    }
    outcome
}

pub fn read_profile_session_material(
    profile: &BrowserProfileDescriptor,
    queries: &[CookieQuery],
    decryptor: &dyn CookieDecryptor,
) -> CookieStoreSessionOutcome {
    let mut outcome = CookieStoreProfileOutcome {
        keyring_state: KeyringState::NotRequired,
        material_summary: BrowserCookieMaterialSummary::with_backend(decryptor.backend()),
        ..CookieStoreProfileOutcome::default()
    };
    let mut sessions = BTreeMap::new();
    let Some(db_path) = find_chromium_cookie_db(profile.profile_path()) else {
        outcome.push_code(diagnostics::COOKIE_DB_MISSING);
        for query in queries {
            outcome.push_provider_code(query.provider(), diagnostics::COOKIE_DB_MISSING);
        }
        return CookieStoreSessionOutcome {
            profile: outcome,
            sessions,
        };
    };
    let temp_copy = match copy_cookie_db_to_private_temp(&db_path) {
        Ok(copy) => copy,
        Err(failure) => {
            outcome.push_code(failure.code());
            for query in queries {
                outcome.push_provider_code(query.provider(), failure.code());
            }
            return CookieStoreSessionOutcome {
                profile: outcome,
                sessions,
            };
        }
    };
    let connection = match open_read_only(temp_copy.db_path()) {
        Ok(connection) => connection,
        Err(failure) => {
            outcome.push_code(failure.code());
            for query in queries {
                outcome.push_provider_code(query.provider(), failure.code());
            }
            return CookieStoreSessionOutcome {
                profile: outcome,
                sessions,
            };
        }
    };
    let columns = match cookie_columns(&connection) {
        Ok(columns) if required_columns_present(&columns) => columns,
        Ok(_) => {
            outcome.push_code(diagnostics::COOKIE_DB_SCHEMA_UNSUPPORTED);
            for query in queries {
                outcome.push_provider_code(
                    query.provider(),
                    diagnostics::COOKIE_DB_SCHEMA_UNSUPPORTED,
                );
            }
            return CookieStoreSessionOutcome {
                profile: outcome,
                sessions,
            };
        }
        Err(failure) => {
            outcome.push_code(failure.code());
            for query in queries {
                outcome.push_provider_code(query.provider(), failure.code());
            }
            return CookieStoreSessionOutcome {
                profile: outcome,
                sessions,
            };
        }
    };
    let encrypted_value_has_host_hash = encrypted_values_include_host_hash(&connection);

    for query in queries {
        match query_cookie_rows(&connection, &columns, query, encrypted_value_has_host_hash) {
            Ok(mut rows) => {
                outcome.material_summary.observe_rows(&rows);
                rows.sort_by(|left, right| {
                    right
                        .path
                        .len()
                        .cmp(&left.path.len())
                        .then(left.creation_utc.cmp(&right.creation_utc))
                });
                let target = query.request_target();
                let mut cookies = Vec::new();
                let mut decrypted_cookie = false;
                let mut used_keyring = false;
                let mut provider_had_hard_material_error = false;
                let mut provider_had_header_reject = false;
                for row in rows {
                    row.record_request_matches(target.as_ref(), &mut outcome.material_summary);
                    if let Some(class) = row.request_rejection(target.as_ref()) {
                        outcome.material_summary.record_header_rejection(class);
                        continue;
                    }
                    match row.scoped_cookie_for_header(decryptor) {
                        Ok(Some((kind, cookie))) => {
                            decrypted_cookie |= kind.was_decrypted();
                            used_keyring |= kind.requires_keyring();
                            if kind.was_decrypted() {
                                outcome.material_summary.record_decrypted_row();
                            }
                            outcome.material_summary.record_header_eligible_row();
                            cookies.push(cookie);
                        }
                        Ok(None) => {}
                        Err(CookieRowHeaderError::Decrypt(error)) => {
                            provider_had_hard_material_error = true;
                            outcome
                                .material_summary
                                .record_header_rejection(header_rejected_class_from_decrypt(error));
                            outcome.material_summary.record_decryption_error(error);
                            record_decrypt_error(&mut outcome, query.provider(), error);
                        }
                        Err(CookieRowHeaderError::Header { kind, error }) => {
                            provider_had_header_reject = true;
                            if kind.was_decrypted() {
                                decrypted_cookie = true;
                                outcome.material_summary.record_decrypted_row();
                            }
                            outcome.material_summary.record_header_rejection(
                                header_rejected_class_from_session_material(error),
                            );
                        }
                    }
                }
                if provider_had_hard_material_error {
                    cookies.clear();
                }
                if !cookies.is_empty() {
                    let provider_count = cookies.len() as u64;
                    match SessionMaterial::try_new(query.provider(), cookies) {
                        Ok(material) => {
                            outcome
                                .material_summary
                                .record_cookie_header_status(CookieHeaderStatus::Built);
                            if used_keyring {
                                record_keyring_state(&mut outcome, KeyringState::Unlocked);
                            }
                            outcome
                                .material_summary
                                .record_usable_session_cookies(provider_count, decrypted_cookie);
                            outcome.add_provider_count(query.provider(), provider_count);
                            outcome.push_code(diagnostics::COOKIE_FOUND);
                            outcome.push_provider_code(query.provider(), diagnostics::COOKIE_FOUND);
                            if decrypted_cookie {
                                outcome.push_code(diagnostics::COOKIE_DECRYPTED);
                                outcome.push_provider_code(
                                    query.provider(),
                                    diagnostics::COOKIE_DECRYPTED,
                                );
                            }
                            sessions.insert(query.provider().to_string(), material);
                        }
                        Err(error) => {
                            outcome.material_summary.record_cookie_header_status(
                                cookie_header_status_from_session_material(error),
                            );
                            let error = CookieRowDecryptError {
                                kind: if decrypted_cookie {
                                    CookieMaterialKind::BasicEncrypted
                                } else {
                                    CookieMaterialKind::Plaintext
                                },
                                error: decrypt_error_from_session_material(error),
                            };
                            outcome.material_summary.record_decryption_error(error);
                            record_decrypt_error(&mut outcome, query.provider(), error);
                        }
                    }
                } else if provider_had_header_reject {
                    outcome
                        .material_summary
                        .record_cookie_header_status(CookieHeaderStatus::InvalidMaterial);
                    let error = CookieRowDecryptError {
                        kind: if decrypted_cookie {
                            CookieMaterialKind::BasicEncrypted
                        } else {
                            CookieMaterialKind::Plaintext
                        },
                        error: DecryptError::InvalidMaterial,
                    };
                    outcome.material_summary.record_decryption_error(error);
                    record_decrypt_error(&mut outcome, query.provider(), error);
                } else if !outcome
                    .provider_diagnostic_codes
                    .get(query.provider())
                    .is_some_and(|codes| diagnostics::contains_dependency_failure(codes))
                {
                    outcome
                        .material_summary
                        .record_cookie_header_status(CookieHeaderStatus::Empty);
                    outcome.push_provider_code(query.provider(), diagnostics::COOKIE_MISSING);
                }
            }
            Err(failure) => {
                outcome.push_code(failure.code());
                outcome.push_provider_code(query.provider(), failure.code());
            }
        }
    }
    if outcome.cookies_found == 0
        && !diagnostics::contains_dependency_failure(&outcome.diagnostic_codes)
    {
        outcome.push_code(diagnostics::COOKIE_MISSING);
    }
    CookieStoreSessionOutcome {
        profile: outcome,
        sessions,
    }
}

pub fn find_chromium_cookie_db(profile_path: &Path) -> Option<PathBuf> {
    let canonical_profile = fs::canonicalize(profile_path).ok()?;
    [
        profile_path.join("Network").join("Cookies"),
        profile_path.join("Cookies"),
    ]
    .into_iter()
    .find_map(|path| {
        let metadata = fs::symlink_metadata(&path).ok()?;
        if !metadata.file_type().is_file() {
            return None;
        }
        let canonical_db = fs::canonicalize(path).ok()?;
        if path_is_under(&canonical_db, &canonical_profile) {
            Some(canonical_db)
        } else {
            None
        }
    })
}

pub struct TempCookieDbCopy {
    dir: PathBuf,
    db_path: PathBuf,
    copied_files: Vec<PathBuf>,
}

impl TempCookieDbCopy {
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn copied_files(&self) -> &[PathBuf] {
        &self.copied_files
    }
}

impl Drop for TempCookieDbCopy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

pub fn copy_cookie_db_to_private_temp(
    source_db: &Path,
) -> Result<TempCookieDbCopy, CookieDbFailure> {
    if !source_db.is_file() {
        return Err(CookieDbFailure::Missing);
    }
    let temp_root = std::env::temp_dir();
    let dir = create_private_temp_dir(&temp_root)?;
    let db_path = dir.join("Cookies");
    copy_file_private(source_db, &db_path).map_err(|_| {
        let _ = fs::remove_dir_all(&dir);
        CookieDbFailure::Unreadable
    })?;
    let mut copied_files = vec![db_path.clone()];
    for suffix in ["-wal", "-shm"] {
        let companion = PathBuf::from(format!("{}{}", source_db.display(), suffix));
        if safe_companion_file(source_db, &companion) {
            let target = dir.join(format!("Cookies{suffix}"));
            copy_file_private(&companion, &target).map_err(|_| {
                let _ = fs::remove_dir_all(&dir);
                CookieDbFailure::Unreadable
            })?;
            copied_files.push(target);
        }
    }
    Ok(TempCookieDbCopy {
        dir,
        db_path,
        copied_files,
    })
}

fn safe_companion_file(source_db: &Path, companion: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(companion) else {
        return false;
    };
    if !metadata.file_type().is_file() {
        return false;
    }
    let Ok(canonical_companion) = fs::canonicalize(companion) else {
        return false;
    };
    source_db
        .parent()
        .is_some_and(|parent| canonical_companion.parent() == Some(parent))
}

fn create_private_temp_dir(temp_root: &Path) -> Result<PathBuf, CookieDbFailure> {
    for _ in 0..100 {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = temp_root.join(format!("codexbar-browser-{}-{counter}", std::process::id()));
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        match builder.create(&dir) {
            Ok(()) => {
                fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
                    .map_err(|_| CookieDbFailure::Unreadable)?;
                return Ok(dir);
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(CookieDbFailure::Unreadable),
        }
    }
    Err(CookieDbFailure::Unreadable)
}

fn copy_file_private(source: &Path, target: &Path) -> std::io::Result<()> {
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(target)?;
    copy(&mut input, &mut output)?;
    output.flush()?;
    fs::set_permissions(target, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn open_read_only(path: &Path) -> Result<Connection, CookieDbFailure> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(classify_sqlite_error)
}

fn cookie_columns(connection: &Connection) -> Result<BTreeSet<String>, CookieDbFailure> {
    let mut statement = connection
        .prepare("PRAGMA table_info(cookies)")
        .map_err(classify_sqlite_error)?;
    let mut rows = statement.query([]).map_err(classify_sqlite_error)?;
    let mut columns = BTreeSet::new();
    while let Some(row) = rows.next().map_err(classify_sqlite_error)? {
        let name: String = row.get(1).map_err(classify_sqlite_error)?;
        columns.insert(name);
    }
    if columns.is_empty() {
        return Err(CookieDbFailure::UnsupportedSchema);
    }
    Ok(columns)
}

fn required_columns_present(columns: &BTreeSet<String>) -> bool {
    [
        "host_key",
        "name",
        "path",
        "value",
        "encrypted_value",
        "expires_utc",
        "is_secure",
        "is_httponly",
    ]
    .into_iter()
    .all(|column| columns.contains(column))
}

fn encrypted_values_include_host_hash(connection: &Connection) -> bool {
    let Ok(version) =
        connection.query_row("SELECT value FROM meta WHERE key = 'version'", [], |row| {
            row.get::<_, String>(0)
        })
    else {
        return false;
    };
    version.parse::<u32>().is_ok_and(|version| version >= 24)
}

fn query_cookie_rows(
    connection: &Connection,
    columns: &BTreeSet<String>,
    query: &CookieQuery,
    encrypted_value_has_host_hash: bool,
) -> Result<Vec<CookieRow>, CookieDbFailure> {
    let host_keys = query.host_keys();
    if host_keys.is_empty() {
        return Ok(Vec::new());
    }
    let host_placeholders = placeholders(host_keys.len());
    let samesite = optional_column(columns, "samesite");
    let source_scheme = optional_column(columns, "source_scheme");
    let source_port = optional_column(columns, "source_port");
    let mut sql = format!(
        "SELECT creation_utc, host_key, name, path, value, encrypted_value, expires_utc, is_secure, is_httponly, {samesite}, {source_scheme}, {source_port} FROM cookies WHERE host_key IN ({host_placeholders})"
    );
    if !query.names.is_empty() {
        sql.push_str(" AND name IN (");
        sql.push_str(&placeholders(query.names.len()));
        sql.push(')');
    }
    let params = host_keys.iter().chain(query.names.iter());
    let mut statement = connection.prepare(&sql).map_err(classify_sqlite_error)?;
    let rows = statement
        .query_map(params_from_iter(params), |row| {
            CookieRow::from_row(row, encrypted_value_has_host_hash)
        })
        .map_err(classify_sqlite_error)?;
    let mut output = Vec::new();
    for row in rows {
        output.push(row.map_err(classify_sqlite_error)?);
    }
    Ok(output)
}

fn rows_required_keyring(
    connection: &Connection,
    columns: &BTreeSet<String>,
    query: &CookieQuery,
    encrypted_value_has_host_hash: bool,
) -> bool {
    query_cookie_rows(connection, columns, query, encrypted_value_has_host_hash)
        .map(|rows| {
            rows.into_iter()
                .any(|row| !row.is_expired() && row.material_kind().requires_keyring())
        })
        .unwrap_or(false)
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(",")
}

fn optional_column(columns: &BTreeSet<String>, name: &str) -> &'static str {
    if columns.contains(name) {
        match name {
            "samesite" => "samesite",
            "source_scheme" => "source_scheme",
            "source_port" => "source_port",
            _ => "NULL",
        }
    } else {
        "NULL"
    }
}

struct CookieRow {
    creation_utc: i64,
    host_key: String,
    name: String,
    path: String,
    value: String,
    encrypted_value: Vec<u8>,
    expires_utc: i64,
    is_secure: bool,
    encrypted_value_has_host_hash: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CookieMaterialKind {
    Plaintext,
    BasicEncrypted,
    KeyringEncrypted,
    UnknownEncrypted,
    Empty,
}

impl DecryptionFailureClass {
    fn from_row_error(error: CookieRowDecryptError) -> Self {
        match error.error {
            DecryptError::Unavailable => {
                if error.kind.requires_keyring() {
                    Self::KeyringNeeded
                } else if error.kind == CookieMaterialKind::UnknownEncrypted {
                    Self::UnsupportedFormat
                } else {
                    Self::Unavailable
                }
            }
            DecryptError::Locked | DecryptError::PromptRequired => Self::KeyringNeeded,
            DecryptError::UnsupportedFormat => Self::UnsupportedFormat,
            DecryptError::MalformedCiphertext => Self::MalformedCiphertext,
            DecryptError::WrongKey => Self::WrongKey,
            DecryptError::InvalidMaterial => Self::InvalidMaterial,
            DecryptError::HeaderTooLarge => Self::HeaderTooLarge,
            DecryptError::TooManyCookies => Self::TooManyCookies,
            DecryptError::Failed => Self::Failed,
        }
    }
}

impl CookieMaterialKind {
    fn requires_keyring(self) -> bool {
        matches!(self, Self::KeyringEncrypted)
    }

    fn was_decrypted(self) -> bool {
        matches!(self, Self::BasicEncrypted | Self::KeyringEncrypted)
    }

    fn keyring_state_for_error(self, error: DecryptError) -> KeyringState {
        match self {
            Self::KeyringEncrypted => error.keyring_state(),
            Self::UnknownEncrypted => KeyringState::Unknown,
            Self::BasicEncrypted | Self::Plaintext | Self::Empty => KeyringState::NotRequired,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CookieRowDecryptError {
    kind: CookieMaterialKind,
    error: DecryptError,
}

impl CookieRow {
    fn from_row(
        row: &rusqlite::Row<'_>,
        encrypted_value_has_host_hash: bool,
    ) -> rusqlite::Result<Self> {
        let creation_utc: i64 = row.get(0)?;
        let host_key: String = row.get(1)?;
        let name: String = row.get(2)?;
        let path: String = row.get(3)?;
        let value: String = row.get(4)?;
        let encrypted_value: Vec<u8> = row.get(5)?;
        let expires_utc: i64 = row.get(6)?;
        let secure: i64 = row.get(7)?;
        let _http_only: i64 = row.get(8)?;
        let _same_site: Option<i64> = row.get(9)?;
        let _source_scheme: Option<i64> = row.get(10)?;
        let _source_port: Option<i64> = row.get(11)?;
        Ok(Self {
            creation_utc,
            host_key,
            name,
            path,
            value,
            encrypted_value,
            expires_utc,
            is_secure: secure != 0,
            encrypted_value_has_host_hash,
        })
    }

    fn is_expired(&self) -> bool {
        if self.expires_utc == 0 {
            return false;
        }
        self.expires_utc <= chromium_now_utc()
    }

    fn request_rejection(
        &self,
        target: Option<&CookieRequestTarget>,
    ) -> Option<HeaderRejectedClass> {
        let Some(target) = target else {
            return self.is_expired().then_some(HeaderRejectedClass::Expired);
        };
        if self.is_expired() {
            return Some(HeaderRejectedClass::Expired);
        }
        if !cookie_domain_matches(&self.host_key, target.host()) {
            return Some(HeaderRejectedClass::DomainMismatch);
        }
        if !cookie_path_matches(&self.path, target.path()) {
            return Some(HeaderRejectedClass::PathMismatch);
        }
        if self.is_secure && !target.is_https() {
            return Some(HeaderRejectedClass::SecureMismatch);
        }
        None
    }

    fn record_request_matches(
        &self,
        target: Option<&CookieRequestTarget>,
        summary: &mut BrowserCookieMaterialSummary,
    ) {
        let Some(target) = target else {
            return;
        };
        if self.is_expired() {
            return;
        }
        if !cookie_domain_matches(&self.host_key, target.host()) {
            return;
        }
        summary.record_domain_match();
        if !cookie_path_matches(&self.path, target.path()) {
            return;
        }
        summary.record_path_match();
        if self.is_secure && !target.is_https() {
            return;
        }
        summary.record_secure_match();
    }

    fn session_material(
        &self,
        provider: &str,
        decryptor: &dyn CookieDecryptor,
    ) -> Result<Option<(CookieMaterialKind, SessionMaterial)>, CookieRowDecryptError> {
        let Some((kind, cookie)) = self.scoped_cookie(decryptor)? else {
            return Ok(None);
        };
        SessionMaterial::try_new(provider, vec![cookie])
            .map(|material| Some((kind, material)))
            .map_err(|error| CookieRowDecryptError {
                kind,
                error: decrypt_error_from_session_material(error),
            })
    }

    fn scoped_cookie(
        &self,
        decryptor: &dyn CookieDecryptor,
    ) -> Result<Option<(CookieMaterialKind, ScopedCookie)>, CookieRowDecryptError> {
        match self.material_kind() {
            CookieMaterialKind::Plaintext => self
                .scoped_cookie_from_value(self.value.clone(), CookieMaterialKind::Plaintext)
                .map(Some),
            CookieMaterialKind::BasicEncrypted | CookieMaterialKind::KeyringEncrypted => {
                let kind = self.material_kind();
                let value = decryptor
                    .decrypt(
                        &self.encrypted_value,
                        CookieDecryptContext {
                            host_key: &self.host_key,
                            db_version: self.encrypted_value_has_host_hash.then_some(24),
                        },
                    )
                    .map_err(|error| CookieRowDecryptError { kind, error })?;
                self.scoped_cookie_from_value(value, kind).map(Some)
            }
            CookieMaterialKind::UnknownEncrypted => Err(CookieRowDecryptError {
                kind: CookieMaterialKind::UnknownEncrypted,
                error: DecryptError::Unavailable,
            }),
            CookieMaterialKind::Empty => Ok(None),
        }
    }

    fn scoped_cookie_from_value(
        &self,
        value: String,
        kind: CookieMaterialKind,
    ) -> Result<(CookieMaterialKind, ScopedCookie), CookieRowDecryptError> {
        ScopedCookie::try_new_for_domain(&self.host_key, &self.path, self.name.clone(), value)
            .map(|cookie| (kind, cookie))
            .map_err(|error| CookieRowDecryptError {
                kind,
                error: decrypt_error_from_session_material(error),
            })
    }

    fn scoped_cookie_for_header(
        &self,
        decryptor: &dyn CookieDecryptor,
    ) -> Result<Option<(CookieMaterialKind, ScopedCookie)>, CookieRowHeaderError> {
        match self.material_kind() {
            CookieMaterialKind::Plaintext => self
                .scoped_cookie_from_value_for_header(
                    self.value.clone(),
                    CookieMaterialKind::Plaintext,
                )
                .map(Some),
            CookieMaterialKind::BasicEncrypted | CookieMaterialKind::KeyringEncrypted => {
                let kind = self.material_kind();
                let value = decryptor
                    .decrypt(
                        &self.encrypted_value,
                        CookieDecryptContext {
                            host_key: &self.host_key,
                            db_version: self.encrypted_value_has_host_hash.then_some(24),
                        },
                    )
                    .map_err(|error| {
                        CookieRowHeaderError::Decrypt(CookieRowDecryptError { kind, error })
                    })?;
                self.scoped_cookie_from_value_for_header(value, kind)
                    .map(Some)
            }
            CookieMaterialKind::UnknownEncrypted => {
                Err(CookieRowHeaderError::Decrypt(CookieRowDecryptError {
                    kind: CookieMaterialKind::UnknownEncrypted,
                    error: DecryptError::Unavailable,
                }))
            }
            CookieMaterialKind::Empty => Ok(None),
        }
    }

    fn scoped_cookie_from_value_for_header(
        &self,
        value: String,
        kind: CookieMaterialKind,
    ) -> Result<(CookieMaterialKind, ScopedCookie), CookieRowHeaderError> {
        ScopedCookie::try_new_for_domain_with_secure(
            &self.host_key,
            &self.path,
            self.is_secure,
            self.name.clone(),
            value,
        )
        .map(|cookie| (kind, cookie))
        .map_err(|error| CookieRowHeaderError::Header { kind, error })
    }

    fn material_kind(&self) -> CookieMaterialKind {
        if self.encrypted_value.starts_with(b"v10") {
            CookieMaterialKind::BasicEncrypted
        } else if self.encrypted_value.starts_with(b"v11") {
            CookieMaterialKind::KeyringEncrypted
        } else if !self.encrypted_value.is_empty() {
            CookieMaterialKind::UnknownEncrypted
        } else if self.value.is_empty() {
            CookieMaterialKind::Empty
        } else {
            CookieMaterialKind::Plaintext
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CookieRowHeaderError {
    Decrypt(CookieRowDecryptError),
    Header {
        kind: CookieMaterialKind,
        error: SessionMaterialError,
    },
}

fn decrypt_error_from_session_material(error: SessionMaterialError) -> DecryptError {
    match error {
        SessionMaterialError::TooManyCookies => DecryptError::TooManyCookies,
        SessionMaterialError::HeaderTooLarge => DecryptError::HeaderTooLarge,
        SessionMaterialError::InvalidProvider
        | SessionMaterialError::EmptyCookieName
        | SessionMaterialError::CookieNameTooLong
        | SessionMaterialError::InvalidCookieName
        | SessionMaterialError::CookieValueTooLong
        | SessionMaterialError::InvalidCookieValue
        | SessionMaterialError::InvalidCookieDomain
        | SessionMaterialError::InvalidCookiePath => DecryptError::InvalidMaterial,
    }
}

fn header_rejected_class_from_session_material(error: SessionMaterialError) -> HeaderRejectedClass {
    match error {
        SessionMaterialError::EmptyCookieName => HeaderRejectedClass::EmptyName,
        SessionMaterialError::InvalidCookieName => HeaderRejectedClass::InvalidName,
        SessionMaterialError::InvalidCookieValue => HeaderRejectedClass::InvalidValue,
        SessionMaterialError::CookieNameTooLong | SessionMaterialError::CookieValueTooLong => {
            HeaderRejectedClass::TooLong
        }
        SessionMaterialError::InvalidCookieDomain | SessionMaterialError::InvalidCookiePath => {
            HeaderRejectedClass::InvalidValue
        }
        SessionMaterialError::InvalidProvider
        | SessionMaterialError::TooManyCookies
        | SessionMaterialError::HeaderTooLarge => HeaderRejectedClass::DecryptFailed,
    }
}

fn cookie_header_status_from_session_material(error: SessionMaterialError) -> CookieHeaderStatus {
    match error {
        SessionMaterialError::TooManyCookies => CookieHeaderStatus::TooManyCookies,
        SessionMaterialError::HeaderTooLarge => CookieHeaderStatus::HeaderTooLarge,
        SessionMaterialError::InvalidProvider
        | SessionMaterialError::EmptyCookieName
        | SessionMaterialError::CookieNameTooLong
        | SessionMaterialError::InvalidCookieName
        | SessionMaterialError::CookieValueTooLong
        | SessionMaterialError::InvalidCookieValue
        | SessionMaterialError::InvalidCookieDomain
        | SessionMaterialError::InvalidCookiePath => CookieHeaderStatus::InvalidMaterial,
    }
}

fn header_rejected_class_from_decrypt(error: CookieRowDecryptError) -> HeaderRejectedClass {
    match error.error {
        DecryptError::UnsupportedFormat => HeaderRejectedClass::UnsupportedPrefix,
        DecryptError::Unavailable if error.kind == CookieMaterialKind::UnknownEncrypted => {
            HeaderRejectedClass::UnsupportedPrefix
        }
        DecryptError::Unavailable
        | DecryptError::Locked
        | DecryptError::PromptRequired
        | DecryptError::Failed
        | DecryptError::MalformedCiphertext
        | DecryptError::WrongKey
        | DecryptError::InvalidMaterial
        | DecryptError::HeaderTooLarge
        | DecryptError::TooManyCookies => HeaderRejectedClass::DecryptFailed,
    }
}

fn record_decrypt_error(
    outcome: &mut CookieStoreProfileOutcome,
    provider: &str,
    error: CookieRowDecryptError,
) {
    record_keyring_state(outcome, error.kind.keyring_state_for_error(error.error));
    match error.error {
        DecryptError::Unavailable | DecryptError::UnsupportedFormat => {
            outcome.push_code(diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
            outcome.push_provider_code(provider, diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
            if error.kind.requires_keyring() {
                outcome.push_code(diagnostics::KEYRING_UNAVAILABLE);
                outcome.push_provider_code(provider, diagnostics::KEYRING_UNAVAILABLE);
            }
        }
        DecryptError::Locked => {
            outcome.push_code(diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
            outcome.push_provider_code(provider, diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
            if error.kind.requires_keyring() {
                outcome.push_code(diagnostics::KEYRING_LOCKED);
                outcome.push_provider_code(provider, diagnostics::KEYRING_LOCKED);
            }
        }
        DecryptError::PromptRequired => {
            outcome.push_code(diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
            outcome.push_provider_code(provider, diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
            if error.kind.requires_keyring() {
                outcome.push_code(diagnostics::KEYRING_PROMPT_REQUIRED);
                outcome.push_provider_code(provider, diagnostics::KEYRING_PROMPT_REQUIRED);
            }
        }
        DecryptError::Failed
        | DecryptError::MalformedCiphertext
        | DecryptError::WrongKey
        | DecryptError::InvalidMaterial
        | DecryptError::HeaderTooLarge
        | DecryptError::TooManyCookies => {
            outcome.push_code(diagnostics::COOKIE_DECRYPTION_FAILED);
            outcome.push_provider_code(provider, diagnostics::COOKIE_DECRYPTION_FAILED);
        }
    }
}

fn record_keyring_state(outcome: &mut CookieStoreProfileOutcome, state: KeyringState) {
    match state {
        KeyringState::NotRequired => {}
        KeyringState::Unknown => {
            if outcome.keyring_state == KeyringState::NotRequired {
                outcome.keyring_state = KeyringState::Unknown;
            }
        }
        _ => {
            if outcome.keyring_state == KeyringState::NotRequired
                || outcome.keyring_state == KeyringState::Unknown
            {
                outcome.keyring_state = state;
            }
        }
    }
}

fn chromium_now_utc() -> i64 {
    let unix_seconds = time::OffsetDateTime::now_utc().unix_timestamp();
    (unix_seconds + 11_644_473_600) * 1_000_000
}

fn classify_sqlite_error(error: rusqlite::Error) -> CookieDbFailure {
    match error {
        rusqlite::Error::SqliteFailure(err, _) => match err.code {
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                CookieDbFailure::Locked
            }
            rusqlite::ErrorCode::NotADatabase
            | rusqlite::ErrorCode::DatabaseCorrupt
            | rusqlite::ErrorCode::SchemaChanged => CookieDbFailure::UnsupportedSchema,
            _ => CookieDbFailure::Unreadable,
        },
        rusqlite::Error::QueryReturnedNoRows => CookieDbFailure::UnsupportedSchema,
        _ => CookieDbFailure::Unreadable,
    }
}

fn provider_domain_label(provider: &str) -> String {
    let label: String = provider
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let label = label.trim_matches('-');
    if label.is_empty() {
        "provider".to_string()
    } else {
        label.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_policy_uses_synthetic_domains_only() {
        let query = CookieQuery::for_provider("codex");
        assert_eq!(query.provider(), "codex");
        assert!(query
            .domains
            .iter()
            .all(|domain| domain.ends_with(".invalid")));
        assert!(query.host_keys().iter().any(|host| host.starts_with('.')));
    }

    #[test]
    fn live_codex_query_is_domain_bounded_without_cookie_name_guessing() {
        let query = CookieQuery::for_live_web_provider("codex");
        assert_eq!(query.provider(), "codex");
        assert_eq!(query.domains, vec!["chatgpt.com"]);
        assert!(query.names.is_empty());
        assert_eq!(query.host_keys(), vec!["chatgpt.com", ".chatgpt.com"]);
    }
}
