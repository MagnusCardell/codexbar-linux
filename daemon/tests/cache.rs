mod common;

use std::fs;

use codexbar_linuxd::app::{App, RefreshStart};
use codexbar_linuxd::cache::{stale_mutated, CacheLoad, SnapshotCache};
use codexbar_linuxd::fixtures;
use codexbar_linuxd::model::{ProviderState, SourceAdapter};

#[test]
fn cache_atomic_write_creates_private_valid_snapshot() {
    let (_tmp, paths) = common::temp_paths();
    let cache = SnapshotCache::new(paths.cache_dir.clone(), paths.cache_file.clone());
    let now = codexbar_linuxd::clock::now_rfc3339();
    let snapshot = fixtures::refreshed_snapshot("cache-test", &now, &now).expect("snapshot");

    cache.store(&snapshot).expect("cache store");

    assert_eq!(common::file_mode(&paths.cache_dir), 0o700);
    assert_eq!(common::file_mode(&paths.cache_file), 0o600);
    let text = fs::read_to_string(&paths.cache_file).expect("cache file");
    common::assert_public_json_safe(&text);
    common::assert_schema("snapshot.schema.json", &text);
    assert!(matches!(cache.load(), CacheLoad::Loaded(_)));
}

#[test]
fn invalid_cache_is_ignored_safely() {
    let (_tmp, paths) = common::temp_paths();
    fs::create_dir_all(&paths.cache_dir).expect("cache dir");
    fs::write(&paths.cache_file, r#"{"schemaVersion":999}"#).expect("invalid cache");
    let cache = SnapshotCache::new(paths.cache_dir, paths.cache_file);
    assert!(matches!(cache.load(), CacheLoad::Invalid));
}

#[tokio::test]
async fn restart_loads_cache_as_stale_without_relabeling_known_adapter() {
    let (_tmp, paths) = common::temp_paths();
    let app = common::fixture_app(paths.clone());
    let refresh = app
        .start_refresh(common::FIXTURE_REFRESH_OPTIONS_JSON)
        .expect("start refresh");
    let RefreshStart::Started { refresh_id } = refresh else {
        panic!("expected started refresh");
    };
    app.finish_refresh(&refresh_id)
        .await
        .expect("finish refresh");

    let restarted = App::new(paths).expect("restarted app");
    let snapshot = restarted.get_snapshot_json().expect("snapshot");
    common::assert_schema("snapshot.schema.json", &snapshot);
    let value: serde_json::Value = serde_json::from_str(&snapshot).expect("snapshot json");
    assert_eq!(value["stale"], true);
    assert_eq!(value["providers"][0]["state"], "stale");
    assert_eq!(value["providers"][0]["sourceAdapter"], "fixture");
    assert_eq!(value["providers"][0]["source"], "api");
}

#[tokio::test]
async fn stale_cache_fallback_reports_partial_refresh() {
    let (_tmp, paths) = common::temp_paths();
    let cache = SnapshotCache::new(paths.cache_dir.clone(), paths.cache_file.clone());
    let now = codexbar_linuxd::clock::now_rfc3339();
    let snapshot = fixtures::refreshed_snapshot("cached-ok", &now, &now).expect("snapshot");
    cache.store(&snapshot).expect("cache store");

    let app = App::new(paths).expect("app");
    let refresh = app
        .start_refresh(
            r#"{"schemaVersion":1,"sourceAdapterPolicy":{"mode":"only","adapters":["linux_web"]}}"#,
        )
        .expect("start refresh");
    let RefreshStart::Started { refresh_id } = refresh else {
        panic!("expected started refresh");
    };
    let completion = app
        .finish_refresh(&refresh_id)
        .await
        .expect("finish refresh");
    let result: serde_json::Value =
        serde_json::from_str(&completion.result_json).expect("result json");
    assert_eq!(result["status"], "partial");
    assert!(result["diagnosticCodes"]
        .as_array()
        .expect("diagnostic codes")
        .iter()
        .any(|code| code == "stale_cache_used"));

    let snapshot: serde_json::Value =
        serde_json::from_str(&completion.snapshot_json).expect("snapshot json");
    assert_eq!(snapshot["stale"], true);
    assert_eq!(snapshot["providers"][0]["state"], "stale");
}

#[tokio::test]
async fn unsupported_linux_web_refresh_uses_generic_diagnostic() {
    let (_tmp, paths) = common::temp_paths();
    let app = App::new(paths).expect("app");
    let refresh = app
        .start_refresh(
            r#"{"schemaVersion":1,"sourceAdapterPolicy":{"mode":"only","adapters":["linux_web"],"allowStaleCacheFallback":false}}"#,
        )
        .expect("start refresh");
    let RefreshStart::Started { refresh_id } = refresh else {
        panic!("expected started refresh");
    };
    let completion = app
        .finish_refresh(&refresh_id)
        .await
        .expect("finish refresh");
    let result: serde_json::Value =
        serde_json::from_str(&completion.result_json).expect("result json");
    assert_eq!(result["status"], "error");
    let codes = result["providers"][0]["diagnosticCodes"]
        .as_array()
        .expect("provider diagnostic codes");
    assert!(codes
        .iter()
        .any(|code| code == "source_adapter_not_implemented"));
    assert!(!codes
        .iter()
        .any(|code| code == "browser_import_not_implemented"));
}

#[test]
fn stale_mutation_preserves_non_usable_provider_states() {
    let now = codexbar_linuxd::clock::now_rfc3339();
    let mut snapshot = fixtures::unsupported_adapter_snapshot(&now).expect("snapshot");
    snapshot.providers[0].state = ProviderState::Unauthenticated;
    snapshot.providers[0].source_adapter = SourceAdapter::LinuxWeb;
    snapshot.providers[0].source = codexbar_linuxd::model::SemanticSource::Web;
    let stale = stale_mutated(snapshot, &now);
    assert!(stale.stale);
    assert_eq!(stale.providers[0].state, ProviderState::Unauthenticated);
    assert_eq!(stale.providers[0].source_adapter, SourceAdapter::LinuxWeb);
}

#[test]
fn cache_rejects_forbidden_public_content() {
    let (_tmp, paths) = common::temp_paths();
    let cache = SnapshotCache::new(paths.cache_dir, paths.cache_file);
    let now = codexbar_linuxd::clock::now_rfc3339();
    let mut snapshot =
        fixtures::refreshed_snapshot("cache-secret-test", &now, &now).expect("snapshot");
    snapshot.providers[0]
        .identity
        .as_mut()
        .expect("identity")
        .account_email_display = Some("raw@example.com".to_string());
    assert!(cache.store(&snapshot).is_err());
}
