use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=CODEXBAR_GIT_SHA");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-env-changed=CI_COMMIT_SHA");

    set_sanitized_env("CODEXBAR_BUILD_TARGET", env::var("TARGET").ok());
    set_sanitized_env("CODEXBAR_BUILD_PROFILE", env::var("PROFILE").ok());

    if let Some(sha) = env_git_sha().or_else(repo_git_sha) {
        println!("cargo:rustc-env=CODEXBAR_BUILD_GIT_SHA={sha}");
    }
}

fn set_sanitized_env(key: &str, value: Option<String>) {
    if let Some(value) = value.and_then(safe_build_value) {
        println!("cargo:rustc-env={key}={value}");
    }
}

fn env_git_sha() -> Option<String> {
    ["CODEXBAR_GIT_SHA", "GITHUB_SHA", "CI_COMMIT_SHA"]
        .into_iter()
        .find_map(|key| env::var(key).ok().and_then(safe_git_sha))
}

fn repo_git_sha() -> Option<String> {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?);
    let repo_root = manifest_dir.parent()?;
    let git_dir = git_dir(repo_root)?;
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(sha) = safe_git_sha(head.to_string()) {
        return Some(sha);
    }

    let ref_name = head.strip_prefix("ref: ")?.trim();
    if !safe_git_ref(ref_name) {
        return None;
    }
    fs::read_to_string(git_dir.join(ref_name))
        .ok()
        .and_then(safe_git_sha)
        .or_else(|| packed_ref_sha(&git_dir, ref_name))
}

fn git_dir(repo_root: &Path) -> Option<PathBuf> {
    let dot_git = repo_root.join(".git");
    if dot_git.is_dir() {
        println!("cargo:rerun-if-changed=../.git/HEAD");
        return Some(dot_git);
    }

    let text = fs::read_to_string(&dot_git).ok()?;
    let path = text.trim().strip_prefix("gitdir: ")?.trim();
    let git_dir = PathBuf::from(path);
    if git_dir.is_absolute() {
        Some(git_dir)
    } else {
        Some(repo_root.join(git_dir))
    }
}

fn packed_ref_sha(git_dir: &Path, ref_name: &str) -> Option<String> {
    let text = fs::read_to_string(git_dir.join("packed-refs")).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('^') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(sha) = parts.next() else {
            continue;
        };
        if parts.next() == Some(ref_name) {
            return safe_git_sha(sha.to_string());
        }
    }
    None
}

fn safe_git_ref(value: &str) -> bool {
    value.starts_with("refs/")
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-'))
        && !value.contains("..")
        && !value.starts_with('/')
        && !value.ends_with('/')
}

fn safe_git_sha(value: String) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() == 40 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Some(value)
    } else {
        None
    }
}

fn safe_build_value(value: String) -> Option<String> {
    if !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        Some(value)
    } else {
        None
    }
}
