use std::env;
use std::fmt;
use std::path::PathBuf;

use crate::APP_ID;

#[derive(Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub cache_dir: PathBuf,
    pub cache_file: PathBuf,
    pub upstream_config_file_hint: Option<String>,
    pub upstream_cli_path: Option<PathBuf>,
}

impl fmt::Debug for AppPaths {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppPaths")
            .field("config_dir", &"[redacted]")
            .field("config_file", &"[redacted]")
            .field("cache_dir", &"[redacted]")
            .field("cache_file", &"[redacted]")
            .field("upstream_config_file_hint", &self.upstream_config_file_hint)
            .field(
                "upstream_cli_path",
                &self.upstream_cli_path.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}

impl AppPaths {
    pub fn from_env() -> Self {
        let config_dir = xdg_home("XDG_CONFIG_HOME", ".config").join(APP_ID);
        let cache_dir = xdg_home("XDG_CACHE_HOME", ".cache").join(APP_ID);
        Self {
            config_file: config_dir.join("config.json"),
            cache_file: cache_dir.join("snapshot.json"),
            config_dir,
            cache_dir,
            upstream_config_file_hint: Some(crate::UPSTREAM_CONFIG_PATH_HINT.to_string()),
            upstream_cli_path: env::var_os("CODEXBAR_CLI")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
        }
    }
}

fn xdg_home(env_key: &str, fallback_child: &str) -> PathBuf {
    if let Some(value) = env::var_os(env_key) {
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(fallback_child)
}
