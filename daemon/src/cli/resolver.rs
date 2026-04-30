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

        let Some(path) = env::var_os("PATH") else {
            return CliResolution::Missing {
                diagnostic_code: "upstream_cli_missing",
            };
        };
        for dir in env::split_paths(&path) {
            let candidate = dir.join("codexbar");
            if is_executable_file(&candidate) {
                return CliResolution::Found { path: candidate };
            }
        }

        for candidate in linuxbrew_candidates() {
            if is_executable_file(&candidate) {
                return CliResolution::Found { path: candidate };
            }
        }

        CliResolution::Missing {
            diagnostic_code: "upstream_cli_missing",
        }
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
    if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
        if let Ok(stripped) = path.strip_prefix(&home) {
            return format!("~/{}", stripped.display());
        }
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::io::Write;

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
        }
    }
}
