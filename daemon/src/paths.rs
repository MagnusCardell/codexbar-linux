use std::env;
use std::path::PathBuf;

use crate::APP_ID;

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub cache_dir: PathBuf,
    pub cache_file: PathBuf,
    pub upstream_config_file_hint: Option<String>,
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
