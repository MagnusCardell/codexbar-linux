mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use codexbar_linuxd::model::Snapshot;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u8,
    test_expectations: BTreeMap<String, String>,
    fixtures: Vec<FixtureEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureEntry {
    fixture_id: String,
    command: String,
    argv: Vec<String>,
    synthetic: bool,
    doc_derived: bool,
    expected_category: String,
    stdout_path: String,
    stderr_path: String,
    metadata_path: String,
    redaction: Redaction,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Redaction {
    applied: bool,
    policy_version: u8,
}

#[test]
fn upstream_cli_manifest_and_json_fixtures_parse() {
    let (manifest, root) = load_manifest();
    assert_eq!(manifest.schema_version, 1);
    assert!(!manifest.fixtures.is_empty());

    let mut ids = BTreeSet::new();
    for fixture in &manifest.fixtures {
        assert!(
            ids.insert(fixture.fixture_id.clone()),
            "duplicate fixtureId {}",
            fixture.fixture_id
        );
        assert!(
            fixture.redaction.applied && fixture.redaction.policy_version == 1,
            "fixture {} must be redacted with policy v1",
            fixture.fixture_id
        );

        for path in fixture_paths(&root, fixture) {
            assert!(path.is_file(), "missing fixture path {}", path.display());
            let text = fs::read_to_string(&path).expect("fixture text");
            assert_upstream_fixture_text_safe(&text, &path);
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                serde_json::from_str::<Value>(&text).unwrap_or_else(|err| {
                    panic!("JSON fixture {} failed to parse: {err}", path.display())
                });
            }
        }
    }
}

#[test]
fn upstream_cli_live_provider_fixture_ids_are_targeted() {
    let (manifest, _root) = load_manifest();
    for fixture in &manifest.fixtures {
        if fixture.synthetic || fixture.doc_derived {
            continue;
        }
        if fixture.command == "cost" {
            assert_eq!(
                option_value(&fixture.argv, "--provider"),
                Some("all"),
                "cost fixture {} must stay --provider all",
                fixture.fixture_id
            );
            assert!(
                option_value(&fixture.argv, "--source").is_none(),
                "cost fixture {} must not include --source",
                fixture.fixture_id
            );
            continue;
        }
        if !matches!(
            fixture.expected_category.as_str(),
            "usage_success" | "usage_error"
        ) || !matches!(fixture.command.as_str(), "usage" | "status")
        {
            continue;
        }
        let provider = match option_value(&fixture.argv, "--provider") {
            Some(provider) => provider,
            None => continue,
        };
        let source = match option_value(&fixture.argv, "--source") {
            Some(source) => source,
            None => continue,
        };
        let expected_id = if fixture.command == "status" {
            format!("status_{provider}_{source}")
        } else if matches!(fixture.argv.get(1).map(String::as_str), Some("usage")) {
            format!("usage_{provider}_{source}_subcommand")
        } else {
            format!("usage_{provider}_{source}_default")
        };
        assert_eq!(
            fixture.fixture_id, expected_id,
            "live usage/status fixture ids must include provider and source"
        );
    }
}

#[test]
fn upstream_cli_required_live_matrix_is_present() {
    let (manifest, _root) = load_manifest();
    for (fixture_id, argv, expected_category) in [
        ("version", &["codexbar", "--version"][..], "version"),
        (
            "config_validate",
            &[
                "codexbar",
                "config",
                "validate",
                "--format",
                "json",
                "--json-only",
            ],
            "config_validate",
        ),
        (
            "usage_all_cli_default",
            &[
                "codexbar",
                "--format",
                "json",
                "--json-only",
                "--provider",
                "all",
                "--source",
                "cli",
            ],
            "usage_error",
        ),
        (
            "usage_all_cli_subcommand",
            &[
                "codexbar",
                "usage",
                "--format",
                "json",
                "--json-only",
                "--provider",
                "all",
                "--source",
                "cli",
            ],
            "usage_error",
        ),
        (
            "cost_all",
            &[
                "codexbar",
                "cost",
                "--format",
                "json",
                "--json-only",
                "--provider",
                "all",
            ],
            "cost_success",
        ),
        (
            "status_all_cli",
            &[
                "codexbar",
                "--format",
                "json",
                "--json-only",
                "--provider",
                "all",
                "--source",
                "cli",
                "--status",
            ],
            "usage_error",
        ),
        (
            "usage_codex_cli_default",
            &[
                "codexbar",
                "--format",
                "json",
                "--json-only",
                "--provider",
                "codex",
                "--source",
                "cli",
            ],
            "usage_success",
        ),
        (
            "usage_codex_cli_subcommand",
            &[
                "codexbar",
                "usage",
                "--format",
                "json",
                "--json-only",
                "--provider",
                "codex",
                "--source",
                "cli",
            ],
            "usage_success",
        ),
        (
            "status_codex_cli",
            &[
                "codexbar",
                "--format",
                "json",
                "--json-only",
                "--provider",
                "codex",
                "--source",
                "cli",
                "--status",
            ],
            "usage_success",
        ),
        (
            "unsupported_web_source",
            &[
                "codexbar",
                "--format",
                "json",
                "--json-only",
                "--provider",
                "all",
                "--source",
                "web",
            ],
            "unsupported_source",
        ),
        (
            "unsupported_auto_source",
            &[
                "codexbar",
                "--format",
                "json",
                "--json-only",
                "--provider",
                "all",
                "--source",
                "auto",
            ],
            "unsupported_source",
        ),
        (
            "invalid_provider",
            &[
                "codexbar",
                "--format",
                "json",
                "--json-only",
                "--provider",
                "__codexbar_linux_invalid_provider__",
            ],
            "invalid_provider",
        ),
    ] {
        let fixture = manifest
            .fixtures
            .iter()
            .find(|candidate| candidate.fixture_id == fixture_id)
            .unwrap_or_else(|| panic!("missing required live fixture {fixture_id}"));
        assert_eq!(fixture.argv, argv, "argv mismatch for {fixture_id}");
        assert_eq!(
            fixture.expected_category, expected_category,
            "expectedCategory mismatch for {fixture_id}"
        );
    }
}

#[test]
fn upstream_cli_categories_have_expectations_and_required_coverage() {
    let (manifest, _root) = load_manifest();
    let categories = manifest
        .fixtures
        .iter()
        .map(|fixture| fixture.expected_category.as_str())
        .collect::<BTreeSet<_>>();

    for category in &categories {
        assert!(
            manifest.test_expectations.contains_key(*category),
            "manifest category {category} must have a test expectation"
        );
    }

    for required in [
        "unsupported_source",
        "invalid_provider",
        "missing_binary",
        "timeout_synthetic",
        "parse_error_synthetic",
    ] {
        assert!(
            categories.contains(required),
            "missing required upstream CLI fixture category {required}"
        );
    }
    assert!(
        categories.contains("usage_success") || categories.contains("usage_error"),
        "must include usage_success or usage_error"
    );
    assert!(
        categories.contains("cost_success") || categories.contains("cost_error"),
        "must include cost_success or cost_error"
    );
}

#[test]
fn raw_upstream_identity_keys_are_confined_to_upstream_fixtures() {
    let (manifest, root) = load_manifest();
    let mut upstream_identity_keys = 0;
    for fixture in &manifest.fixtures {
        for path in fixture_paths(&root, fixture) {
            let text = fs::read_to_string(&path).expect("fixture text");
            upstream_identity_keys += count_raw_identity_keys(&text);
        }
    }
    assert!(
        upstream_identity_keys > 0,
        "upstream fixture corpus should preserve redacted raw identity field names"
    );

    let snapshot_dir = common::repo_path("fixtures/snapshots");
    for entry in fs::read_dir(&snapshot_dir).expect("snapshot dir") {
        let path = entry.expect("snapshot entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("snapshot text");
        assert_eq!(
            count_raw_identity_keys(&text),
            0,
            "normalized snapshot fixture must not contain raw upstream identity keys: {}",
            path.display()
        );
    }
}

#[test]
fn upstream_cli_fixtures_are_not_normalized_snapshots() {
    let (manifest, root) = load_manifest();
    for fixture in &manifest.fixtures {
        for path in fixture_paths(&root, fixture) {
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let value: Value =
                serde_json::from_str(&fs::read_to_string(&path).expect("fixture text"))
                    .expect("fixture json");
            assert!(
                serde_json::from_value::<Snapshot>(value).is_err(),
                "upstream CLI fixture is accidentally a normalized snapshot: {}",
                path.display()
            );
        }
    }
}

fn load_manifest() -> (Manifest, PathBuf) {
    let root = common::repo_path("daemon/fixtures/upstream-cli");
    let manifest_path = root.join("manifest.json");
    let text = fs::read_to_string(&manifest_path).expect("manifest text");
    common::assert_public_json_safe(&text);
    let manifest = serde_json::from_str::<Manifest>(&text).expect("manifest json");
    (manifest, root)
}

fn fixture_paths(root: &Path, fixture: &FixtureEntry) -> [PathBuf; 3] {
    [
        root.join(&fixture.stdout_path),
        root.join(&fixture.stderr_path),
        root.join(&fixture.metadata_path),
    ]
}

fn count_raw_identity_keys(text: &str) -> usize {
    [
        "\"accountEmail\":",
        "\"accountOrganization\":",
        "\"providerID\":",
        "\"signedInEmail\":",
    ]
    .iter()
    .map(|needle| text.matches(needle).count())
    .sum()
}

fn option_value<'a>(argv: &'a [String], option: &str) -> Option<&'a str> {
    let index = argv.iter().position(|arg| arg == option)?;
    argv.get(index + 1).map(String::as_str)
}

fn assert_upstream_fixture_text_safe(text: &str, path: &Path) {
    let lower = text.to_ascii_lowercase();
    for forbidden in [
        "authorization:",
        "set-cookie",
        "cookie:",
        "bearer ",
        "sk-",
        "access_token",
        "refresh_token",
        "session_token",
        "/home/",
        "/users/",
        ".config/chrom",
        ".mozilla/firefox",
        "network/cookies",
        "login data",
        "\"rawpayload\"",
        "\"rawresponse\"",
    ] {
        assert!(
            !lower.contains(forbidden),
            "{} contains forbidden upstream CLI fixture content: {forbidden}",
            path.display()
        );
    }
    assert_no_raw_json_identity_values(text, path);
}

fn assert_no_raw_json_identity_values(text: &str, path: &Path) {
    for key in ["accountEmail", "signedInEmail"] {
        for value in json_string_values(text, key) {
            assert!(
                value == "[REDACTED_EMAIL]" || value.contains("***@"),
                "{} contains unredacted identity email field {key}",
                path.display()
            );
        }
    }

    for key in [
        "providerID",
        "providerId",
        "accountID",
        "accountId",
        "userID",
        "userId",
        "customerID",
        "customerId",
        "teamID",
        "teamId",
        "workspaceID",
        "workspaceId",
    ] {
        for value in json_string_values(text, key) {
            assert!(
                value.starts_with("[REDACTED_") && value.ends_with(']'),
                "{} contains unredacted account id field {key}",
                path.display()
            );
        }
    }

    for key in [
        "accountOrganization",
        "organization",
        "org",
        "workspace",
        "team",
        "teamName",
    ] {
        for value in json_string_values(text, key) {
            assert!(
                value.starts_with("[REDACTED_") && value.ends_with(']'),
                "{} contains unredacted organization field {key}",
                path.display()
            );
        }
    }
}

fn json_string_values(text: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\"");
    let mut values = Vec::new();
    let mut offset = 0;
    while let Some(relative_index) = text[offset..].find(&needle) {
        let key_start = offset + relative_index;
        let after_key = key_start + needle.len();
        let Some(colon_relative) = text[after_key..].find(':') else {
            break;
        };
        let mut value_start = after_key + colon_relative + 1;
        while let Some(ch) = text[value_start..].chars().next() {
            if !ch.is_whitespace() {
                break;
            }
            value_start += ch.len_utf8();
        }
        if text[value_start..].starts_with('"') {
            let content_start = value_start + 1;
            if let Some(end_relative) = text[content_start..].find('"') {
                let content_end = content_start + end_relative;
                values.push(text[content_start..content_end].to_string());
                offset = content_end + 1;
                continue;
            }
        }
        offset = after_key;
    }
    values
}
