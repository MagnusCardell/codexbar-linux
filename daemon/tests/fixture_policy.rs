mod common;

use std::fs;

use codexbar_linuxd::app::{App, AppRuntime, RefreshCompletion, RefreshStart};
use codexbar_linuxd::model::{RefreshResult, RefreshStatus};

const AUTO_REFRESH_OPTIONS_JSON: &str = r#"{"schemaVersion":1,"reason":"test","force":true}"#;

#[tokio::test]
async fn fixture_policy_allowed_in_test_mode() {
    let (_tmp, paths) = common::temp_paths();
    let app = App::new_with_runtime(paths, AppRuntime::with_fixture_source_for_tests())
        .expect("fixture app starts");

    let completion = run_refresh(&app, common::FIXTURE_REFRESH_OPTIONS_JSON).await;
    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);
    common::assert_schema("refresh-result.schema.json", &completion.result_json);

    let result: RefreshResult =
        serde_json::from_str(&completion.result_json).expect("refresh result");
    assert_eq!(result.status, RefreshStatus::Ok);
    assert!(result.cache_written);
    assert_eq!(
        result.providers[0].source_adapter,
        Some(codexbar_linuxd::model::SourceAdapter::Fixture)
    );
    assert!(app.cache_file_path().is_file());
}

#[tokio::test]
async fn fixture_policy_rejected_in_production_mode() {
    let (_tmp, paths) = common::temp_paths();
    let app = App::new(paths).expect("production app starts");

    let completion = run_refresh(&app, common::FIXTURE_REFRESH_OPTIONS_JSON).await;
    assert_fixture_rejection_payloads(&completion);
    assert!(!app.cache_file_path().exists());
}

#[tokio::test]
async fn auto_mode_without_upstream_cli_does_not_fall_back_to_fixture_in_production() {
    let (tmp, mut paths) = common::temp_paths();
    paths.upstream_cli_path = Some(tmp.path().join("missing-codexbar"));
    let app = App::new(paths).expect("production app starts");

    let completion = run_refresh(&app, AUTO_REFRESH_OPTIONS_JSON).await;
    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);
    common::assert_schema("refresh-result.schema.json", &completion.result_json);
    common::assert_public_json_safe(&completion.snapshot_json);
    common::assert_public_json_safe(&completion.result_json);

    let snapshot: serde_json::Value =
        serde_json::from_str(&completion.snapshot_json).expect("snapshot json");
    let result: serde_json::Value =
        serde_json::from_str(&completion.result_json).expect("result json");
    assert_eq!(snapshot["providers"][0]["state"], "missing_dependency");
    assert_ne!(snapshot["providers"][0]["sourceAdapter"], "fixture");
    assert_ne!(result["providers"][0]["sourceAdapter"], "fixture");
    assert_eq!(result["cacheWritten"], false);
    assert!(result["diagnosticCodes"]
        .as_array()
        .expect("diagnostic codes")
        .iter()
        .any(|code| code == "upstream_cli_missing"));
}

#[tokio::test]
async fn rejected_fixture_refresh_does_not_overwrite_existing_cache() {
    let (_tmp, paths) = common::temp_paths();
    let fixture_app =
        App::new_with_runtime(paths.clone(), AppRuntime::with_fixture_source_for_tests())
            .expect("fixture app starts");
    let allowed = run_refresh(&fixture_app, common::FIXTURE_REFRESH_OPTIONS_JSON).await;
    let allowed_result: RefreshResult =
        serde_json::from_str(&allowed.result_json).expect("allowed refresh result");
    assert!(allowed_result.cache_written);
    let cache_before = fs::read_to_string(&paths.cache_file).expect("cache before rejection");

    let production_app = App::new(paths.clone()).expect("production app starts");
    let rejected = run_refresh(&production_app, common::FIXTURE_REFRESH_OPTIONS_JSON).await;
    assert_fixture_rejection_result(&rejected);

    let cache_after = fs::read_to_string(&paths.cache_file).expect("cache after rejection");
    assert_eq!(cache_after, cache_before);
}

async fn run_refresh(app: &App, options_json: &str) -> RefreshCompletion {
    let start = app.start_refresh(options_json).expect("refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("refresh should start");
    };
    app.finish_refresh(&refresh_id)
        .await
        .expect("refresh finishes")
}

fn assert_fixture_rejection_payloads(completion: &RefreshCompletion) {
    assert_fixture_rejection_result(completion);
    let snapshot: serde_json::Value =
        serde_json::from_str(&completion.snapshot_json).expect("snapshot json");
    assert_ne!(snapshot["providers"][0]["sourceAdapter"], "fixture");
}

fn assert_fixture_rejection_result(completion: &RefreshCompletion) {
    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);
    common::assert_schema("refresh-result.schema.json", &completion.result_json);
    common::assert_public_json_safe(&completion.snapshot_json);
    common::assert_public_json_safe(&completion.result_json);

    let result: serde_json::Value =
        serde_json::from_str(&completion.result_json).expect("result json");
    assert_eq!(result["status"], "error");
    assert_eq!(result["cacheWritten"], false);
    let diagnostic_codes = result["diagnosticCodes"]
        .as_array()
        .expect("diagnostic codes");
    assert!(diagnostic_codes
        .iter()
        .any(|code| code == "fixture_not_allowed"));
    assert!(diagnostic_codes
        .iter()
        .any(|code| code == "capability_unimplemented"));
}
