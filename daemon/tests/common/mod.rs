#![allow(dead_code)]

use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::{NamedTempFile, TempDir};

use codexbar_linuxd::app::{App, AppRuntime};
use codexbar_linuxd::paths::AppPaths;

pub const FIXTURE_REFRESH_OPTIONS_JSON: &str = r#"{"schemaVersion":1,"reason":"test","force":true,"sourceAdapterPolicy":{"mode":"only","adapters":["fixture"]}}"#;

pub fn repo_path(relative_path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative_path)
}

pub fn temp_paths() -> (TempDir, AppPaths) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config_dir = tmp.path().join("config").join("codexbar-linux");
    let cache_dir = tmp.path().join("cache").join("codexbar-linux");
    let paths = AppPaths {
        config_file: config_dir.join("config.json"),
        cache_file: cache_dir.join("snapshot.json"),
        config_dir,
        cache_dir,
        upstream_config_file_hint: Some("~/.codexbar/config.json".to_string()),
        upstream_cli_path: None,
    };
    (tmp, paths)
}

pub fn fixture_app(paths: AppPaths) -> App {
    App::new_with_runtime(paths, AppRuntime::with_fixture_source_for_tests())
        .expect("fixture-enabled app")
}

pub fn assert_schema(schema_name: &str, json_text: &str) {
    let mut payload = NamedTempFile::new().expect("payload temp file");
    payload
        .write_all(json_text.as_bytes())
        .expect("write payload");
    let script = r#"
import json
import sys
import warnings
from pathlib import Path
from jsonschema import Draft202012Validator, FormatChecker, RefResolver

warnings.filterwarnings("ignore", category=DeprecationWarning)
schema_dir = Path(sys.argv[1])
schema_name = sys.argv[2]
payload_path = Path(sys.argv[3])
schemas = {}
for path in schema_dir.glob("*.schema.json"):
    schema = json.loads(path.read_text(encoding="utf-8"))
    schemas[path.name] = schema
    if "$id" in schema:
        schemas[schema["$id"]] = schema
schema = schemas[schema_name]
payload = json.loads(payload_path.read_text(encoding="utf-8"))
resolver = RefResolver.from_schema(schema, store=schemas)
validator = Draft202012Validator(schema, resolver=resolver, format_checker=FormatChecker())
errors = sorted(validator.iter_errors(payload), key=lambda err: list(err.path))
if errors:
    first = errors[0]
    where = ".".join(str(part) for part in first.path) or "<root>"
    raise SystemExit(f"{schema_name} validation failed at {where}: {first.message}")
"#;
    let output = Command::new("python3")
        .arg("-")
        .arg(repo_path("spec"))
        .arg(schema_name)
        .arg(payload.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .as_mut()
                .expect("python stdin")
                .write_all(script.as_bytes())?;
            child.wait_with_output()
        })
        .expect("run python jsonschema validator");
    assert!(
        output.status.success(),
        "schema validation failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn assert_public_json_safe(text: &str) {
    codexbar_linuxd::redact::validate_public_json_text(text)
        .unwrap_or_else(|finding| panic!("public JSON failed redaction scan: {:?}", finding));
}

pub fn assert_no_live_secret_markers(label: &str, text: &str) {
    let lower = text.to_ascii_lowercase();
    for needle in [
        "/home/",
        "~/.local/share",
        "auth.json",
        "authorization",
        "bearer ",
        "cookie",
        "set-cookie",
        "access_token",
        "accesstoken",
        "refresh_token",
        "refreshtoken",
        "session_token",
        "sessionkey",
        "sessiontoken",
        "apikey",
        "api_key",
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "github_pat",
        "xoxb-",
        "xoxp-",
        "xoxa-",
        "rawresponse",
        "raw_response",
        "rawpayload",
        "raw_payload",
    ] {
        assert!(
            !lower.contains(needle),
            "{label} contains forbidden live secret marker {needle:?}"
        );
    }
    assert!(
        !contains_raw_email(text),
        "{label} contains a raw email-like value"
    );
}

pub fn live_codexbar_binary() -> Option<PathBuf> {
    if std::env::var("CODEXBAR_LIVE").ok().as_deref() != Some("1") {
        eprintln!("skipping live upstream CLI smoke: CODEXBAR_LIVE=1 is not set");
        return None;
    }
    let Some(value) = std::env::var_os("CODEXBAR_CLI") else {
        eprintln!("skipping live upstream CLI smoke: CODEXBAR_CLI is not set");
        return None;
    };
    let path = PathBuf::from(value);
    if path.as_os_str().is_empty() {
        eprintln!("skipping live upstream CLI smoke: CODEXBAR_CLI is empty");
        return None;
    }
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(err) => {
            eprintln!(
                "skipping live upstream CLI smoke: CODEXBAR_CLI {} is unavailable: {err}",
                display_safe_path_for_test(&path)
            );
            return None;
        }
    };
    #[cfg(unix)]
    let is_executable = metadata.permissions().mode() & 0o111 != 0;
    #[cfg(not(unix))]
    let is_executable = true;
    if !metadata.is_file() || !is_executable {
        eprintln!(
            "skipping live upstream CLI smoke: CODEXBAR_CLI {} is not executable",
            display_safe_path_for_test(&path)
        );
        return None;
    }
    Some(path)
}

fn display_safe_path_for_test(path: &Path) -> String {
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        if let Ok(stripped) = path.strip_prefix(home) {
            return format!("~/{}", stripped.display());
        }
    }
    path.display().to_string()
}

fn contains_raw_email(text: &str) -> bool {
    text.split(|ch: char| {
        !(ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-' | '@' | '*'))
    })
    .any(|token| {
        let Some((local, domain)) = token.split_once('@') else {
            return false;
        };
        !token.contains('*')
            && !local.is_empty()
            && domain.contains('.')
            && domain
                .rsplit('.')
                .next()
                .is_some_and(|suffix| suffix.len() >= 2)
    })
}

#[cfg(unix)]
#[allow(dead_code)]
pub fn file_mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .unwrap_or_else(|err| panic!("metadata for {}: {err}", path.display()))
        .permissions()
        .mode()
        & 0o777
}
