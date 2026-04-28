use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::clock;
use crate::error::{AppError, AppResult};
use crate::model::{ProviderState, Snapshot, SourceAdapter};
use crate::redact;

#[derive(Clone, Debug)]
pub struct SnapshotCache {
    dir: PathBuf,
    file: PathBuf,
}

#[derive(Clone, Debug)]
pub enum CacheLoad {
    Missing,
    Invalid,
    Loaded(Box<Snapshot>),
}

impl SnapshotCache {
    pub fn new(dir: PathBuf, file: PathBuf) -> Self {
        Self { dir, file }
    }

    pub fn file_path(&self) -> &Path {
        &self.file
    }

    pub fn load(&self) -> CacheLoad {
        let text = match fs::read_to_string(&self.file) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return CacheLoad::Missing,
            Err(_) => return CacheLoad::Invalid,
        };
        if redact::validate_public_json_text(&text).is_err() {
            return CacheLoad::Invalid;
        }
        match serde_json::from_str::<Snapshot>(&text) {
            Ok(snapshot) if validate_snapshot(&snapshot).is_ok() => {
                CacheLoad::Loaded(Box::new(snapshot))
            }
            _ => CacheLoad::Invalid,
        }
    }

    pub fn store(&self, snapshot: &Snapshot) -> AppResult<()> {
        validate_snapshot(snapshot)?;
        let text =
            serde_json::to_string_pretty(snapshot).map_err(|_| AppError::internal_redacted())?;
        redact::validate_public_json_text(&text).map_err(|_| AppError::internal_redacted())?;
        ensure_private_dir(&self.dir)?;

        let tmp_path = self.temp_file_path();
        let mut tmp = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp_path)
            .map_err(|_| AppError::internal_redacted())?;
        tmp.write_all(text.as_bytes())
            .and_then(|_| tmp.write_all(b"\n"))
            .and_then(|_| tmp.flush())
            .and_then(|_| tmp.sync_all())
            .map_err(|_| AppError::internal_redacted())?;
        drop(tmp);

        fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600))
            .map_err(|_| AppError::internal_redacted())?;
        fs::rename(&tmp_path, &self.file).map_err(|_| {
            let _ = fs::remove_file(&tmp_path);
            AppError::internal_redacted()
        })?;
        fs::set_permissions(&self.file, fs::Permissions::from_mode(0o600))
            .map_err(|_| AppError::internal_redacted())?;
        let _ = File::open(&self.dir).and_then(|dir| dir.sync_all());
        Ok(())
    }

    fn temp_file_path(&self) -> PathBuf {
        let stamp = clock::now_rfc3339().replace([':', '.', '+'], "-");
        self.dir
            .join(format!(".snapshot.{}.{}.tmp", std::process::id(), stamp))
    }
}

pub fn ensure_private_dir(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path).map_err(|_| AppError::internal_redacted())?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| AppError::internal_redacted())?;
    Ok(())
}

pub fn stale_mutated(mut snapshot: Snapshot, stale_since: &str) -> Snapshot {
    snapshot.stale = true;
    snapshot.daemon.state = crate::model::DaemonState::Degraded;
    for provider in &mut snapshot.providers {
        if provider.state.is_usable_for_stale_cache() {
            provider.state = ProviderState::Stale;
            if provider.stale_since.is_none() {
                provider.stale_since = Some(stale_since.to_string());
            }
        }
        if provider.source_adapter == SourceAdapter::None {
            provider.source_adapter = SourceAdapter::Cache;
        }
    }
    snapshot
}

pub fn validate_snapshot(snapshot: &Snapshot) -> AppResult<()> {
    if snapshot.schema_version != 1 {
        return Err(AppError::internal_redacted());
    }
    if snapshot.generated_at.is_empty() {
        return Err(AppError::internal_redacted());
    }
    for provider in &snapshot.providers {
        if provider.provider.is_empty() || provider.display_name.is_empty() {
            return Err(AppError::internal_redacted());
        }
    }
    let value = serde_json::to_value(snapshot).map_err(|_| AppError::internal_redacted())?;
    redact::validate_public_json_value(&value).map_err(|_| AppError::internal_redacted())?;
    Ok(())
}
