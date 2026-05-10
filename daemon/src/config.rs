use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::cache::ensure_private_dir;
use crate::clock;
use crate::error::{AppError, AppResult};
use crate::model::{
    default_provider_settings, BrowserImportPolicy, BrowserImportSettingsPatch,
    DiagnosticsSettingsPatch, PreferredSourceAdapter, ProviderSettings, ProviderSettingsPatch,
    RefreshSettingsPatch, Settings, SettingsPatch,
};
use crate::redact;

#[derive(Clone)]
pub struct SettingsStore {
    dir: PathBuf,
    file: PathBuf,
}

impl fmt::Debug for SettingsStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SettingsStore")
            .field("dir", &"[redacted]")
            .field("file", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Debug)]
pub enum SettingsLoad {
    Loaded(Settings),
    Defaulted(Settings),
    Invalid(Settings),
}

impl SettingsStore {
    pub fn new(dir: PathBuf, file: PathBuf) -> Self {
        Self { dir, file }
    }

    pub fn file_path(&self) -> &Path {
        &self.file
    }

    pub fn load(&self) -> SettingsLoad {
        let text = match fs::read_to_string(&self.file) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return SettingsLoad::Defaulted(Settings::default())
            }
            Err(_) => return SettingsLoad::Invalid(Settings::default()),
        };
        if redact::validate_public_json_text(&text).is_err() {
            return SettingsLoad::Invalid(Settings::default());
        }
        match serde_json::from_str::<Settings>(&text) {
            Ok(mut settings) if validate_settings(&settings).is_ok() => {
                normalize_no_browser_settings(&mut settings);
                SettingsLoad::Loaded(settings)
            }
            _ => SettingsLoad::Invalid(Settings::default()),
        }
    }

    pub fn store(&self, settings: &Settings) -> AppResult<()> {
        validate_settings(settings)?;
        let text =
            serde_json::to_string_pretty(settings).map_err(|_| AppError::internal_redacted())?;
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
            .join(format!(".config.{}.{}.tmp", std::process::id(), stamp))
    }
}

pub fn parse_settings_patch(json: &str) -> AppResult<SettingsPatch> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| AppError::invalid_json())?;
    if redact::contains_null(&value) {
        return Err(AppError::invalid_json());
    }
    let patch: SettingsPatch =
        serde_json::from_value(value).map_err(|_| AppError::invalid_json())?;
    if patch.schema_version != 1 {
        return Err(AppError::invalid_json());
    }
    validate_settings_patch_policy(&patch)?;
    Ok(patch)
}

pub fn apply_settings_patch(mut settings: Settings, patch: SettingsPatch) -> AppResult<Settings> {
    if let Some(refresh) = patch.refresh {
        apply_refresh_patch(&mut settings, refresh);
    }
    if let Some(providers) = patch.providers {
        for (provider_id, provider_patch) in providers {
            if !is_safe_id(&provider_id) {
                return Err(AppError::invalid_settings_patch(
                    "provider id rejected by daemon policy",
                ));
            }
            let entry = settings.providers.entry(provider_id).or_default();
            apply_provider_patch(entry, provider_patch);
        }
    }
    if let Some(browser_import) = patch.browser_import {
        apply_browser_import_patch(&mut settings, browser_import)?;
    }
    if let Some(diagnostics) = patch.diagnostics {
        apply_diagnostics_patch(&mut settings, diagnostics);
    }
    normalize_no_browser_settings(&mut settings);
    validate_settings(&settings)?;
    Ok(settings)
}

pub fn normalize_no_browser_settings(settings: &mut Settings) {
    if settings.providers.is_empty() {
        settings.providers = default_provider_settings();
    }
    settings.browser_import.enabled = false;
    settings.browser_import.policy = BrowserImportPolicy::Off;
    settings.browser_import.profile_id_allowlist.clear();
    for provider in settings.providers.values_mut() {
        provider.allow_browser_import = false;
        if provider.preferred_source_adapter == PreferredSourceAdapter::LinuxWeb {
            provider.preferred_source_adapter = PreferredSourceAdapter::UpstreamCli;
        }
    }
}

pub fn validate_settings(settings: &Settings) -> AppResult<()> {
    if settings.schema_version != 1 {
        return Err(AppError::invalid_json());
    }
    if settings.refresh.interval_seconds != 0
        && !(30..=86400).contains(&settings.refresh.interval_seconds)
    {
        return Err(AppError::invalid_json());
    }
    for provider_id in settings.providers.keys() {
        if !is_safe_id(provider_id) {
            return Err(AppError::invalid_settings_patch(
                "provider id rejected by daemon policy",
            ));
        }
    }
    for profile_id in &settings.browser_import.profile_id_allowlist {
        if !is_safe_id(profile_id) {
            return Err(AppError::invalid_json());
        }
    }
    let value = serde_json::to_value(settings).map_err(|_| AppError::internal_redacted())?;
    redact::validate_public_json_value(&value).map_err(|_| AppError::internal_redacted())?;
    Ok(())
}

fn validate_settings_patch_policy(patch: &SettingsPatch) -> AppResult<()> {
    if let Some(providers) = &patch.providers {
        for provider_id in providers.keys() {
            if !is_safe_id(provider_id) {
                return Err(AppError::invalid_settings_patch(
                    "provider id rejected by daemon policy",
                ));
            }
        }
    }
    if let Some(browser_import) = &patch.browser_import {
        if let Some(profile_ids) = &browser_import.profile_id_allowlist {
            for profile_id in profile_ids {
                if !is_safe_id(profile_id) {
                    return Err(AppError::invalid_json());
                }
            }
        }
    }
    Ok(())
}

fn apply_refresh_patch(settings: &mut Settings, patch: RefreshSettingsPatch) {
    if let Some(value) = patch.interval_seconds {
        settings.refresh.interval_seconds = value;
    }
    if let Some(value) = patch.startup_refresh {
        settings.refresh.startup_refresh = value;
    }
    if let Some(value) = patch.allow_stale_cache_fallback {
        settings.refresh.allow_stale_cache_fallback = value;
    }
}

fn apply_provider_patch(settings: &mut ProviderSettings, patch: ProviderSettingsPatch) {
    if let Some(value) = patch.enabled {
        settings.enabled = value;
    }
    if let Some(value) = patch.preferred_source_adapter {
        settings.preferred_source_adapter = match value {
            PreferredSourceAdapter::LinuxWeb => PreferredSourceAdapter::UpstreamCli,
            value => value,
        };
    }
    if patch.allow_browser_import.is_some() {
        settings.allow_browser_import = false;
    }
    if let Some(value) = patch.allow_cli_fallback {
        settings.allow_cli_fallback = value;
    }
}

fn apply_browser_import_patch(
    settings: &mut Settings,
    patch: BrowserImportSettingsPatch,
) -> AppResult<()> {
    if patch.enabled.is_some() {
        settings.browser_import.enabled = false;
    }
    if patch.policy.is_some() {
        settings.browser_import.policy = BrowserImportPolicy::Off;
    }
    if let Some(value) = patch.profile_id_allowlist {
        for profile_id in &value {
            if !is_safe_id(profile_id) {
                return Err(AppError::invalid_json());
            }
        }
        settings.browser_import.profile_id_allowlist.clear();
    }
    if let Some(value) = patch.domain_allowlist_mode {
        settings.browser_import.domain_allowlist_mode = value;
    }
    Ok(())
}

fn apply_diagnostics_patch(settings: &mut Settings, patch: DiagnosticsSettingsPatch) {
    if let Some(value) = patch.verbosity {
        settings.diagnostics.verbosity = value;
    }
    if let Some(value) = patch.keep_redacted_artifacts {
        settings.diagnostics.keep_redacted_artifacts = value;
    }
}

pub fn is_safe_id(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    value.len() <= 128
        && first.is_ascii_alphanumeric()
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | ':' | '-'))
}
