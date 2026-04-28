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
            common::assert_public_json_safe(&text);
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                serde_json::from_str::<Value>(&text).unwrap_or_else(|err| {
                    panic!("JSON fixture {} failed to parse: {err}", path.display())
                });
            }
        }
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
}
