use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{copy, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::{params_from_iter, Connection, OpenFlags};

use crate::browser::diagnostics;
use crate::browser::keyring::{CookieDecryptor, DecryptError};
use crate::browser::profile::{path_is_under, BrowserProfileDescriptor};
use crate::browser::session_material::{ScopedCookie, SessionMaterial};
use crate::model::KeyringState;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Eq, PartialEq)]
pub struct CookieQuery {
    provider: String,
    domains: Vec<String>,
    names: Vec<String>,
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
            },
            "claude" => Self {
                provider: provider.to_string(),
                domains: vec!["claude.example.invalid".to_string()],
                names: vec!["quota_marker".to_string()],
            },
            _ => Self {
                provider: provider.to_string(),
                domains: vec![format!(
                    "{}.example.invalid",
                    provider_domain_label(provider)
                )],
                names: vec!["quota_marker".to_string()],
            },
        }
    }

    pub fn for_live_web_provider(provider: &str) -> Self {
        match provider {
            "codex" => Self {
                provider: provider.to_string(),
                domains: vec!["chatgpt.com".to_string()],
                names: Vec::new(),
            },
            _ => Self::for_provider(provider),
        }
    }

    pub fn provider(&self) -> &str {
        &self.provider
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
}

impl Default for CookieStoreProfileOutcome {
    fn default() -> Self {
        Self {
            cookies_found: 0,
            keyring_state: KeyringState::Unknown,
            diagnostic_codes: Vec::new(),
            provider_counts: BTreeMap::new(),
            provider_diagnostic_codes: BTreeMap::new(),
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

    for query in queries {
        match query_cookie_rows(&connection, &columns, query) {
            Ok(rows) => {
                let mut provider_count = 0;
                for row in rows {
                    if row.is_expired() {
                        continue;
                    }
                    match row.session_material(query.provider(), decryptor) {
                        Ok(Some(material)) => {
                            provider_count += material.cookie_count() as u64;
                            drop(material);
                        }
                        Ok(None) => {}
                        Err(error) => {
                            let state = error.keyring_state();
                            if outcome.keyring_state == KeyringState::NotRequired
                                || outcome.keyring_state == KeyringState::Unknown
                            {
                                outcome.keyring_state = state;
                            }
                            match error {
                                DecryptError::Unavailable => {
                                    outcome.push_code(diagnostics::KEYRING_UNAVAILABLE);
                                    outcome.push_code(diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
                                    outcome.push_provider_code(
                                        query.provider(),
                                        diagnostics::KEYRING_UNAVAILABLE,
                                    );
                                    outcome.push_provider_code(
                                        query.provider(),
                                        diagnostics::COOKIE_DECRYPTION_UNAVAILABLE,
                                    );
                                }
                                DecryptError::Locked => {
                                    outcome.push_code(diagnostics::KEYRING_LOCKED);
                                    outcome.push_code(diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
                                    outcome.push_provider_code(
                                        query.provider(),
                                        diagnostics::KEYRING_LOCKED,
                                    );
                                    outcome.push_provider_code(
                                        query.provider(),
                                        diagnostics::COOKIE_DECRYPTION_UNAVAILABLE,
                                    );
                                }
                                DecryptError::PromptRequired => {
                                    outcome.push_code(diagnostics::KEYRING_PROMPT_REQUIRED);
                                    outcome.push_code(diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
                                    outcome.push_provider_code(
                                        query.provider(),
                                        diagnostics::KEYRING_PROMPT_REQUIRED,
                                    );
                                    outcome.push_provider_code(
                                        query.provider(),
                                        diagnostics::COOKIE_DECRYPTION_UNAVAILABLE,
                                    );
                                }
                                DecryptError::Failed => {
                                    outcome.push_code(diagnostics::COOKIE_DECRYPTION_FAILED);
                                    outcome.push_provider_code(
                                        query.provider(),
                                        diagnostics::COOKIE_DECRYPTION_FAILED,
                                    );
                                }
                            }
                        }
                    }
                }
                if provider_count > 0 {
                    outcome.keyring_state = match outcome.keyring_state {
                        KeyringState::NotRequired | KeyringState::Unknown => {
                            if rows_required_decryption(&connection, &columns, query) {
                                KeyringState::Unlocked
                            } else {
                                KeyringState::NotRequired
                            }
                        }
                        state => state,
                    };
                    outcome.add_provider_count(query.provider(), provider_count);
                    outcome.push_code(diagnostics::COOKIE_FOUND);
                    outcome.push_provider_code(query.provider(), diagnostics::COOKIE_FOUND);
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

    for query in queries {
        match query_cookie_rows(&connection, &columns, query) {
            Ok(rows) => {
                let mut cookies = Vec::new();
                let mut decrypted_cookie = false;
                for row in rows {
                    if row.is_expired() {
                        continue;
                    }
                    match row.scoped_cookie(decryptor) {
                        Ok(Some((was_decrypted, cookie))) => {
                            decrypted_cookie |= was_decrypted;
                            cookies.push(cookie);
                        }
                        Ok(None) => {}
                        Err(error) => record_decrypt_error(&mut outcome, query.provider(), error),
                    }
                }
                if !cookies.is_empty() {
                    let provider_count = cookies.len() as u64;
                    match SessionMaterial::try_new(query.provider(), cookies) {
                        Ok(material) => {
                            outcome.keyring_state = match outcome.keyring_state {
                                KeyringState::NotRequired | KeyringState::Unknown => {
                                    if decrypted_cookie {
                                        KeyringState::Unlocked
                                    } else {
                                        KeyringState::NotRequired
                                    }
                                }
                                state => state,
                            };
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
                        Err(_) => {
                            outcome.push_code(diagnostics::COOKIE_DECRYPTION_FAILED);
                            outcome.push_provider_code(
                                query.provider(),
                                diagnostics::COOKIE_DECRYPTION_FAILED,
                            );
                        }
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

fn query_cookie_rows(
    connection: &Connection,
    columns: &BTreeSet<String>,
    query: &CookieQuery,
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
        "SELECT host_key, name, path, value, encrypted_value, expires_utc, is_secure, is_httponly, {samesite}, {source_scheme}, {source_port} FROM cookies WHERE host_key IN ({host_placeholders})"
    );
    if !query.names.is_empty() {
        sql.push_str(" AND name IN (");
        sql.push_str(&placeholders(query.names.len()));
        sql.push(')');
    }
    let params = host_keys.iter().chain(query.names.iter());
    let mut statement = connection.prepare(&sql).map_err(classify_sqlite_error)?;
    let rows = statement
        .query_map(params_from_iter(params), CookieRow::from_row)
        .map_err(classify_sqlite_error)?;
    let mut output = Vec::new();
    for row in rows {
        output.push(row.map_err(classify_sqlite_error)?);
    }
    Ok(output)
}

fn rows_required_decryption(
    connection: &Connection,
    columns: &BTreeSet<String>,
    query: &CookieQuery,
) -> bool {
    query_cookie_rows(connection, columns, query)
        .map(|rows| {
            rows.into_iter()
                .any(|row| !row.is_expired() && !row.encrypted_value.is_empty())
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
    host_key: String,
    name: String,
    path: String,
    value: String,
    encrypted_value: Vec<u8>,
    expires_utc: i64,
}

impl CookieRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        let host_key: String = row.get(0)?;
        let name: String = row.get(1)?;
        let path: String = row.get(2)?;
        let value: String = row.get(3)?;
        let encrypted_value: Vec<u8> = row.get(4)?;
        let expires_utc: i64 = row.get(5)?;
        let _secure: i64 = row.get(6)?;
        let _http_only: i64 = row.get(7)?;
        let _same_site: Option<i64> = row.get(8)?;
        let _source_scheme: Option<i64> = row.get(9)?;
        let _source_port: Option<i64> = row.get(10)?;
        Ok(Self {
            host_key,
            name,
            path,
            value,
            encrypted_value,
            expires_utc,
        })
    }

    fn is_expired(&self) -> bool {
        if self.expires_utc == 0 {
            return false;
        }
        self.expires_utc <= chromium_now_utc()
    }

    fn session_material(
        &self,
        provider: &str,
        decryptor: &dyn CookieDecryptor,
    ) -> Result<Option<SessionMaterial>, DecryptError> {
        let Some((_was_decrypted, cookie)) = self.scoped_cookie(decryptor)? else {
            return Ok(None);
        };
        Ok(SessionMaterial::try_new(provider, vec![cookie]).ok())
    }

    fn scoped_cookie(
        &self,
        decryptor: &dyn CookieDecryptor,
    ) -> Result<Option<(bool, ScopedCookie)>, DecryptError> {
        if !self.encrypted_value.is_empty() {
            let value = decryptor.decrypt(&self.encrypted_value)?;
            return Ok(ScopedCookie::try_new_for_domain(
                &self.host_key,
                &self.path,
                self.name.clone(),
                value,
            )
            .ok()
            .map(|cookie| (true, cookie)));
        }
        if self.value.is_empty() {
            return Ok(None);
        }
        Ok(ScopedCookie::try_new_for_domain(
            &self.host_key,
            &self.path,
            self.name.clone(),
            self.value.clone(),
        )
        .ok()
        .map(|cookie| (false, cookie)))
    }
}

fn record_decrypt_error(
    outcome: &mut CookieStoreProfileOutcome,
    provider: &str,
    error: DecryptError,
) {
    let state = error.keyring_state();
    if outcome.keyring_state == KeyringState::NotRequired
        || outcome.keyring_state == KeyringState::Unknown
    {
        outcome.keyring_state = state;
    }
    match error {
        DecryptError::Unavailable => {
            outcome.push_code(diagnostics::KEYRING_UNAVAILABLE);
            outcome.push_code(diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
            outcome.push_provider_code(provider, diagnostics::KEYRING_UNAVAILABLE);
            outcome.push_provider_code(provider, diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
        }
        DecryptError::Locked => {
            outcome.push_code(diagnostics::KEYRING_LOCKED);
            outcome.push_code(diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
            outcome.push_provider_code(provider, diagnostics::KEYRING_LOCKED);
            outcome.push_provider_code(provider, diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
        }
        DecryptError::PromptRequired => {
            outcome.push_code(diagnostics::KEYRING_PROMPT_REQUIRED);
            outcome.push_code(diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
            outcome.push_provider_code(provider, diagnostics::KEYRING_PROMPT_REQUIRED);
            outcome.push_provider_code(provider, diagnostics::COOKIE_DECRYPTION_UNAVAILABLE);
        }
        DecryptError::Failed => {
            outcome.push_code(diagnostics::COOKIE_DECRYPTION_FAILED);
            outcome.push_provider_code(provider, diagnostics::COOKIE_DECRYPTION_FAILED);
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
