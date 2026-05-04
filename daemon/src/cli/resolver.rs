use std::env;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const LINUXBREW_PATHS: &[&str] = &[
    "~/.linuxbrew/bin/codexbar",
    "/home/linuxbrew/.linuxbrew/bin/codexbar",
];

#[derive(Clone, Debug)]
pub struct CliResolver {
    override_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub enum CliResolution {
    Found { path: PathBuf },
    Missing { diagnostic_code: &'static str },
}

impl CliResolution {
    pub fn info_parts(&self) -> (Option<String>, bool, Option<String>) {
        match self {
            Self::Found { path } => (Some(display_safe_path(path)), true, None),
            Self::Missing { diagnostic_code } => {
                (None, false, Some((*diagnostic_code).to_string()))
            }
        }
    }
}

impl CliResolver {
    pub fn new(override_path: Option<PathBuf>) -> Self {
        Self { override_path }
    }

    pub fn resolve(&self) -> CliResolution {
        if let Some(path) = &self.override_path {
            return match path.metadata() {
                Ok(_) if is_executable_file(path) => CliResolution::Found { path: path.clone() },
                Ok(_) => CliResolution::Missing {
                    diagnostic_code: "upstream_cli_not_executable",
                },
                Err(_) => CliResolution::Missing {
                    diagnostic_code: "upstream_cli_missing",
                },
            };
        }

        let mut saw_not_executable = false;
        let Some(path) = env::var_os("PATH") else {
            return resolve_linuxbrew_or_missing(false);
        };
        for dir in env::split_paths(&path) {
            let candidate = dir.join("codexbar");
            if is_executable_file(&candidate) {
                return CliResolution::Found { path: candidate };
            }
            if candidate.exists() {
                saw_not_executable = true;
            }
        }

        resolve_linuxbrew_or_missing(saw_not_executable)
    }
}

fn resolve_linuxbrew_or_missing(mut saw_not_executable: bool) -> CliResolution {
    for candidate in linuxbrew_candidates() {
        if is_executable_file(&candidate) {
            return CliResolution::Found { path: candidate };
        }
        if candidate.exists() {
            saw_not_executable = true;
        }
    }

    if saw_not_executable {
        return CliResolution::Missing {
            diagnostic_code: "upstream_cli_not_executable",
        };
    }

    CliResolution::Missing {
        diagnostic_code: "upstream_cli_missing",
    }
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
}

fn linuxbrew_candidates() -> Vec<PathBuf> {
    LINUXBREW_PATHS
        .iter()
        .map(|path| {
            if let Some(rest) = path.strip_prefix("~/") {
                env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(rest)
            } else {
                PathBuf::from(path)
            }
        })
        .collect()
}

pub fn display_safe_path(path: &Path) -> String {
    let path_text = path.to_string_lossy();
    if path_text == "/usr/bin/codexbar" || path_text == "/usr/local/bin/codexbar" {
        return path_text.to_string();
    }
    if path_text.ends_with("/.linuxbrew/bin/codexbar") {
        return "linuxbrew:codexbar".to_string();
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("[redacted-path]/{name}"))
        .unwrap_or_else(|| "[redacted-path]".to_string())
}

pub fn display_safe_owned_file(path: &Path) -> String {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("config.json") => "[redacted-config]/config.json".to_string(),
        Some("snapshot.json") => "[redacted-cache]/snapshot.json".to_string(),
        Some(name) => format!("[redacted-path]/{name}"),
        None => "[redacted-path]".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn executable(path: &Path) {
        let mut file = fs::File::create(path).expect("file");
        file.write_all(b"#!/usr/bin/env bash\nexit 0\n")
            .expect("write");
        let mut mode = file.metadata().expect("metadata").permissions();
        mode.set_mode(0o700);
        fs::set_permissions(path, mode).expect("chmod");
    }

    #[test]
    fn explicit_path_works_and_non_executable_is_rejected() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("codexbar");
        executable(&path);
        assert!(matches!(
            CliResolver::new(Some(path.clone())).resolve(),
            CliResolution::Found { path: found } if found == path
        ));

        let non_exec = tmp.path().join("not-executable");
        fs::write(&non_exec, b"").expect("write");
        assert!(matches!(
            CliResolver::new(Some(non_exec)).resolve(),
            CliResolution::Missing {
                diagnostic_code: "upstream_cli_not_executable"
            }
        ));
    }

    #[test]
    fn path_lookup_works_and_missing_is_safe() {
        let _guard = env_lock().lock().expect("env lock");
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("codexbar");
        executable(&path);
        let old_path = env::var_os("PATH");
        env::set_var("PATH", tmp.path());
        assert!(matches!(
            CliResolver::new(None).resolve(),
            CliResolution::Found { path: found } if found == path
        ));
        env::set_var("PATH", tmp.path().join("missing"));
        assert!(matches!(
            CliResolver::new(None).resolve(),
            CliResolution::Missing {
                diagnostic_code: "upstream_cli_missing"
            }
        ));
        if let Some(old_path) = old_path {
            env::set_var("PATH", old_path);
        } else {
            env::remove_var("PATH");
        }
    }

    #[test]
    fn path_lookup_reports_existing_non_executable_candidate() {
        let _guard = env_lock().lock().expect("env lock");
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("codexbar");
        fs::write(&path, b"not executable").expect("write");
        let old_path = env::var_os("PATH");
        env::set_var("PATH", tmp.path());

        assert!(matches!(
            CliResolver::new(None).resolve(),
            CliResolution::Missing {
                diagnostic_code: "upstream_cli_not_executable"
            }
        ));

        if let Some(old_path) = old_path {
            env::set_var("PATH", old_path);
        } else {
            env::remove_var("PATH");
        }
    }

    #[test]
    fn linuxbrew_candidate_reports_not_executable_and_path_is_public_safe() {
        let _guard = env_lock().lock().expect("env lock");
        let tmp = tempfile::tempdir().expect("tempdir");
        let candidate = tmp.path().join(".linuxbrew/bin/codexbar");
        fs::create_dir_all(candidate.parent().expect("candidate parent")).expect("linuxbrew dir");
        fs::write(&candidate, b"not executable").expect("write");
        let old_path = env::var_os("PATH");
        let old_home = env::var_os("HOME");
        env::set_var("PATH", tmp.path().join("empty-path"));
        env::set_var("HOME", tmp.path());

        assert!(matches!(
            CliResolver::new(None).resolve(),
            CliResolution::Missing {
                diagnostic_code: "upstream_cli_not_executable"
            }
        ));
        assert_eq!(
            display_safe_path(Path::new("/home/linuxbrew/.linuxbrew/bin/codexbar")),
            "linuxbrew:codexbar"
        );
        assert_eq!(
            display_safe_path(&tmp.path().join("bin/codexbar")),
            "[redacted-path]/codexbar"
        );
        assert_eq!(
            display_safe_owned_file(&tmp.path().join(".config/codexbar-linux/config.json")),
            "[redacted-config]/config.json"
        );

        if let Some(old_path) = old_path {
            env::set_var("PATH", old_path);
        } else {
            env::remove_var("PATH");
        }
        if let Some(old_home) = old_home {
            env::set_var("HOME", old_home);
        } else {
            env::remove_var("HOME");
        }
    }
}
