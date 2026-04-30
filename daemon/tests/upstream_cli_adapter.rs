mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use codexbar_linuxd::app::{App, RefreshStart};
use codexbar_linuxd::cli::{CliRefreshRequest, CliTimeouts, UpstreamCliAdapter};
use codexbar_linuxd::model::{ProviderState, SemanticSource};

const UPSTREAM_CLI_REFRESH_OPTIONS_JSON: &str = r#"{"schemaVersion":1,"reason":"test","force":true,"sourceAdapterPolicy":{"mode":"only","adapters":["upstream_cli"]}}"#;
const LIVE_UPSTREAM_CLI_REFRESH_OPTIONS_JSON: &str = r#"{"schemaVersion":1,"reason":"manual","force":true,"busyBehavior":"return_existing","sourceAdapterPolicy":{"mode":"only","adapters":["upstream_cli"],"allowStaleCacheFallback":false},"providers":["codex"]}"#;

#[tokio::test]
async fn upstream_cli_adapter_normalizes_targeted_success_and_redacts_identity() {
    let (tmp, binary) = fake_codexbar(
        r#"
case "$*" in
  "--version")
    printf '%s\n' 'CodexBar test-1'
    exit 0
    ;;
  "cost --format json --json-only --provider all")
    cat <<'JSON'
[{"provider":"codex","source":"local","updatedAt":"2026-04-29T19:32:13Z","sessionCostUSD":1.25,"last30DaysCostUSD":12.34,"totals":{"totalCost":12.34},"daily":[{"date":"2026-04-28","totalCost":5.0},{"date":"2026-04-29","totalCost":7.34}]}]
JSON
    exit 0
    ;;
  *"--status")
    cat <<'JSON'
[{"provider":"codex","version":"0.125.0","source":"codex-cli","usage":{"primary":{"usedPercent":34,"windowMinutes":300,"resetsAt":"2026-04-29T22:36:14Z"},"secondary":{"usedPercent":19,"windowMinutes":10080,"resetsAt":"2026-05-05T06:15:41Z"},"updatedAt":"2026-04-29T19:32:19Z","accountEmail":"raw.user@example.com","loginMethod":"prolite","identity":{"providerID":"raw-provider-id","accountEmail":"raw.user@example.com","loginMethod":"prolite"}},"credits":{"updatedAt":"2026-04-29T19:32:20Z","remaining":0},"status":{"updatedAt":"2026-04-27T15:52:49Z","indicator":"none","description":"All Systems Operational","url":"https://status.openai.com/"}}]
JSON
    exit 0
    ;;
  *)
    cat <<'JSON'
[{"provider":"codex","version":"0.125.0","source":"codex-cli","usage":{"primary":{"usedPercent":34,"windowMinutes":300,"resetsAt":"2026-04-29T22:36:14Z"},"secondary":{"usedPercent":19,"windowMinutes":10080,"resetsAt":"2026-05-05T06:15:41Z"},"updatedAt":"2026-04-29T19:32:11Z","accountEmail":"raw.user@example.com","loginMethod":"prolite","identity":{"providerID":"raw-provider-id","accountEmail":"raw.user@example.com","loginMethod":"prolite"}},"credits":{"events":[],"updatedAt":"2026-04-29T19:32:13Z","remaining":0}}]
JSON
    exit 0
    ;;
esac
"#,
    );

    let refresh = run_adapter(binary, vec!["codex".to_string()], short_timeouts()).await;
    assert!(tmp.path().is_dir());
    let snapshot = refresh.snapshot;
    let provider = &snapshot.providers[0];

    assert!(snapshot.daemon.upstream_cli.as_ref().unwrap().available);
    assert_eq!(provider.provider, "codex");
    assert_eq!(provider.state, ProviderState::Ok);
    assert_eq!(provider.source, SemanticSource::Local);
    assert_eq!(
        provider.usage.primary.as_ref().unwrap().used_percent,
        Some(34.0)
    );
    assert_eq!(
        provider.identity.as_ref().unwrap().account_email_display,
        Some("r***@example.com".to_string())
    );
    assert_eq!(provider.cost.as_ref().unwrap().total, Some(12.34));

    let snapshot_json = serde_json::to_string(&snapshot).expect("snapshot json");
    common::assert_public_json_safe(&snapshot_json);
    common::assert_schema("snapshot.schema.json", &snapshot_json);
    assert!(!snapshot_json.contains("raw.user@example.com"));
    assert!(!snapshot_json.contains("raw-provider-id"));
}

#[tokio::test]
async fn upstream_cli_adapter_reports_missing_binary_without_running_probe() {
    let temp = tempfile::tempdir().expect("tempdir");
    let missing = temp.path().join("missing-codexbar");
    let refresh = run_adapter(missing, Vec::new(), short_timeouts()).await;
    assert!(temp.path().is_dir());
    let snapshot = refresh.snapshot;

    assert_eq!(
        snapshot
            .daemon
            .upstream_cli
            .as_ref()
            .unwrap()
            .diagnostic_code,
        Some("upstream_cli_missing".to_string())
    );
    assert_eq!(
        snapshot.providers[0].state,
        ProviderState::MissingDependency
    );
    assert_eq!(
        snapshot.providers[0].diagnostic_codes,
        vec!["upstream_cli_missing".to_string()]
    );
    let snapshot_json = serde_json::to_string(&snapshot).expect("snapshot json");
    common::assert_schema("snapshot.schema.json", &snapshot_json);
}

#[tokio::test]
async fn upstream_cli_adapter_classifies_timeouts() {
    let (tmp, binary) = fake_codexbar(
        r#"
case "$*" in
  "--version")
    printf '%s\n' 'CodexBar test-1'
    exit 0
    ;;
  "cost --format json --json-only --provider all")
    printf '%s\n' '[]'
    exit 0
    ;;
  *)
    sleep 1
    exit 0
    ;;
esac
"#,
    );

    let refresh = run_adapter(binary, vec!["codex".to_string()], very_short_timeouts()).await;
    assert!(tmp.path().is_dir());
    assert_eq!(refresh.snapshot.providers[0].state, ProviderState::Timeout);
    assert!(refresh.snapshot.providers[0]
        .diagnostic_codes
        .contains(&"upstream_cli_timeout".to_string()));
}

#[tokio::test]
async fn upstream_cli_adapter_classifies_parse_errors() {
    let (tmp, binary) = fake_codexbar(
        r#"
case "$*" in
  "--version")
    printf '%s\n' 'CodexBar test-1'
    exit 0
    ;;
  "cost --format json --json-only --provider all")
    printf '%s\n' '[]'
    exit 0
    ;;
  *)
    printf '%s\n' '{ "provider": "codex", "usage":'
    exit 0
    ;;
esac
"#,
    );

    let refresh = run_adapter(binary, vec!["codex".to_string()], short_timeouts()).await;
    assert!(tmp.path().is_dir());
    assert_eq!(
        refresh.snapshot.providers[0].state,
        ProviderState::ParseError
    );
    assert!(refresh.snapshot.providers[0]
        .diagnostic_codes
        .contains(&"upstream_cli_parse_error".to_string()));
}

#[tokio::test]
async fn upstream_cli_adapter_classifies_nonzero_exit() {
    let (tmp, binary) = fake_codexbar(
        r#"
case "$*" in
  "--version")
    printf '%s\n' 'CodexBar test-1'
    exit 0
    ;;
  "cost --format json --json-only --provider all")
    printf '%s\n' '[]'
    exit 0
    ;;
  *)
    printf '%s\n' '[{"provider":"codex","error":{"kind":"provider","code":1,"message":"Error"},"source":"cli"}]'
    exit 1
    ;;
esac
"#,
    );

    let refresh = run_adapter(binary, vec!["codex".to_string()], short_timeouts()).await;
    assert!(tmp.path().is_dir());
    assert_eq!(refresh.snapshot.providers[0].state, ProviderState::Error);
    assert!(refresh.snapshot.providers[0]
        .diagnostic_codes
        .contains(&"upstream_cli_provider_error".to_string()));
}

#[tokio::test]
async fn app_refresh_with_upstream_cli_uses_targeted_codex_default() {
    let (fake_tmp, binary, log_path) = fake_codexbar_recording();
    let (_app_tmp, mut paths) = common::temp_paths();
    paths.upstream_cli_path = Some(binary);
    let app = App::new(paths).expect("app starts");
    let start = app
        .start_refresh(UPSTREAM_CLI_REFRESH_OPTIONS_JSON)
        .expect("refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("refresh should start");
    };
    let completion = app
        .finish_refresh(&refresh_id)
        .await
        .expect("refresh finishes");

    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);
    common::assert_schema("refresh-result.schema.json", &completion.result_json);
    common::assert_public_json_safe(&completion.snapshot_json);
    let snapshot: serde_json::Value =
        serde_json::from_str(&completion.snapshot_json).expect("snapshot json");
    assert_eq!(snapshot["providers"][0]["provider"], "codex");
    assert_eq!(snapshot["providers"][0]["sourceAdapter"], "upstream_cli");
    assert_eq!(snapshot["providers"][0]["state"], "ok");

    let log = fs::read_to_string(&log_path).expect("command log");
    assert!(
        log.lines()
            .any(|line| line == "--format json --json-only --provider codex --source cli"),
        "usage command should target codex: {log}"
    );
    assert!(
        log.lines()
            .any(|line| line == "--format json --json-only --provider codex --source cli --status"),
        "status command should target codex: {log}"
    );
    assert!(
        log.lines()
            .any(|line| line == "cost --format json --json-only --provider all"),
        "cost command should use provider all without source: {log}"
    );
    assert!(
        !log.lines()
            .any(|line| line.contains("--provider all --source cli")),
        "usage/status must not default to all-provider cli probes: {log}"
    );
    assert!(fake_tmp.path().is_dir());
}

#[tokio::test]
async fn app_refresh_missing_upstream_cli_returns_schema_valid_missing_dependency() {
    let (tmp, mut paths) = common::temp_paths();
    paths.upstream_cli_path = Some(tmp.path().join("missing-codexbar"));
    let app = App::new(paths).expect("app starts");
    let start = app
        .start_refresh(UPSTREAM_CLI_REFRESH_OPTIONS_JSON)
        .expect("refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("refresh should start");
    };
    let completion = app
        .finish_refresh(&refresh_id)
        .await
        .expect("refresh finishes");
    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);
    common::assert_schema("refresh-result.schema.json", &completion.result_json);
    let snapshot: serde_json::Value =
        serde_json::from_str(&completion.snapshot_json).expect("snapshot json");
    let result: serde_json::Value =
        serde_json::from_str(&completion.result_json).expect("result json");
    assert_eq!(snapshot["providers"][0]["provider"], "codex");
    assert_eq!(snapshot["providers"][0]["state"], "missing_dependency");
    assert_eq!(
        snapshot["daemon"]["upstreamCli"]["diagnosticCode"],
        "upstream_cli_missing"
    );
    assert_eq!(result["status"], "error");
}

#[tokio::test]
#[ignore = "requires CODEXBAR_LIVE=1 and CODEXBAR_CLI=/path/to/codexbar"]
async fn live_upstream_cli_refresh_codex_smoke_redacts_outputs() {
    let Some(binary) = common::live_codexbar_binary() else {
        return;
    };
    let (_tmp, mut paths) = common::temp_paths();
    paths.upstream_cli_path = Some(binary);
    let app = App::new(paths).expect("app starts");
    let start = app
        .start_refresh(LIVE_UPSTREAM_CLI_REFRESH_OPTIONS_JSON)
        .expect("refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("live refresh should start");
    };
    let completion = app
        .finish_refresh(&refresh_id)
        .await
        .expect("refresh finishes");

    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);
    common::assert_schema("refresh-result.schema.json", &completion.result_json);
    common::assert_public_json_safe(&completion.snapshot_json);
    common::assert_public_json_safe(&completion.result_json);
    common::assert_no_live_secret_markers("live snapshot", &completion.snapshot_json);
    common::assert_no_live_secret_markers("live refresh result", &completion.result_json);
    for (_, provider_event_json) in &completion.provider_events {
        common::assert_schema("provider-event.schema.json", provider_event_json);
        common::assert_public_json_safe(provider_event_json);
        common::assert_no_live_secret_markers("live provider event", provider_event_json);
    }

    let snapshot: serde_json::Value =
        serde_json::from_str(&completion.snapshot_json).expect("snapshot json");
    let result: serde_json::Value =
        serde_json::from_str(&completion.result_json).expect("result json");
    let providers = snapshot["providers"].as_array().expect("providers array");
    let codex = providers
        .iter()
        .find(|provider| provider["provider"] == "codex")
        .expect("codex provider present");
    assert_eq!(codex["state"], "ok");
    assert_eq!(codex["sourceAdapter"], "upstream_cli");
    assert_eq!(codex["source"], "local");
    assert_eq!(result["cacheWritten"], true);
    let result_providers = result["providers"]
        .as_array()
        .expect("result providers array");
    let result_codex = result_providers
        .iter()
        .find(|provider| provider["provider"] == "codex")
        .expect("codex refresh result present");
    assert_eq!(result_codex["status"], "ok");

    let daemon_info_json = app.get_daemon_info_json().expect("daemon info json");
    common::assert_schema("daemon-info.schema.json", &daemon_info_json);
    common::assert_public_json_safe(&daemon_info_json);
    common::assert_no_live_secret_markers("live daemon info", &daemon_info_json);

    let diagnostics_json = app
        .get_diagnostics_json("global")
        .expect("diagnostics json");
    common::assert_schema("diagnostics.schema.json", &diagnostics_json);
    common::assert_public_json_safe(&diagnostics_json);
    common::assert_no_live_secret_markers("live diagnostics", &diagnostics_json);
    let provider_diagnostics_json = app
        .get_diagnostics_json("codex")
        .expect("provider diagnostics json");
    common::assert_schema("diagnostics.schema.json", &provider_diagnostics_json);
    common::assert_public_json_safe(&provider_diagnostics_json);
    common::assert_no_live_secret_markers("live provider diagnostics", &provider_diagnostics_json);

    let cache_json = fs::read_to_string(app.cache_file_path()).expect("live cache snapshot");
    common::assert_schema("snapshot.schema.json", &cache_json);
    common::assert_public_json_safe(&cache_json);
    common::assert_no_live_secret_markers("live cache", &cache_json);
}

async fn run_adapter(
    binary: PathBuf,
    providers: Vec<String>,
    timeouts: CliTimeouts,
) -> codexbar_linuxd::cli::CliRefresh {
    let adapter = UpstreamCliAdapter::with_overrides(Some(binary), timeouts);
    adapter
        .refresh(CliRefreshRequest {
            refresh_id: "refresh-test".to_string(),
            started_at: "2026-04-29T19:32:00Z".to_string(),
            finished_at: "2026-04-29T19:32:30Z".to_string(),
            providers,
            selected_provider: None,
        })
        .await
        .expect("adapter refresh")
}

fn short_timeouts() -> CliTimeouts {
    CliTimeouts {
        version: Duration::from_secs(1),
        usage: Duration::from_secs(1),
        status: Duration::from_secs(1),
        cost: Duration::from_secs(1),
    }
}

fn very_short_timeouts() -> CliTimeouts {
    CliTimeouts {
        version: Duration::from_secs(1),
        usage: Duration::from_millis(50),
        status: Duration::from_millis(50),
        cost: Duration::from_secs(1),
    }
}

fn fake_codexbar(script_body: &str) -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("tempdir");
    let binary = temp.path().join("codexbar");
    let script = format!("#!/bin/sh\nset -eu\n{script_body}\n");
    fs::write(&binary, script).expect("write fake codexbar");
    chmod(&binary, 0o700);
    (temp, binary)
}

fn fake_codexbar_recording() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().expect("tempdir");
    let binary = temp.path().join("codexbar");
    let log_path = temp.path().join("argv.log");
    let script = format!(
        r#"#!/bin/sh
set -eu
printf '%s\n' "$*" >> '{}'
case "$*" in
  "--version")
    printf '%s\n' 'CodexBar test-1'
    exit 0
    ;;
  "cost --format json --json-only --provider all")
    printf '%s\n' '[{{"provider":"codex","source":"local","updatedAt":"2026-04-29T19:32:13Z","sessionCostUSD":1.25,"last30DaysCostUSD":12.34,"totals":{{"totalCost":12.34}}}}]'
    exit 0
    ;;
  *"--status")
    printf '%s\n' '[{{"provider":"codex","version":"0.125.0","source":"codex-cli","usage":{{"primary":{{"usedPercent":34,"windowMinutes":300,"resetsAt":"2026-04-29T22:36:14Z"}},"secondary":{{"usedPercent":19,"windowMinutes":10080,"resetsAt":"2026-05-05T06:15:41Z"}},"updatedAt":"2026-04-29T19:32:19Z","accountEmail":"raw.user@example.com","loginMethod":"prolite","identity":{{"providerID":"raw-provider-id","accountEmail":"raw.user@example.com","loginMethod":"prolite"}}}},"credits":{{"updatedAt":"2026-04-29T19:32:20Z","remaining":0}},"status":{{"updatedAt":"2026-04-27T15:52:49Z","indicator":"none","description":"All Systems Operational","url":"https://status.openai.com/"}}}}]'
    exit 0
    ;;
  *)
    printf '%s\n' '[{{"provider":"codex","version":"0.125.0","source":"codex-cli","usage":{{"primary":{{"usedPercent":34,"windowMinutes":300,"resetsAt":"2026-04-29T22:36:14Z"}},"secondary":{{"usedPercent":19,"windowMinutes":10080,"resetsAt":"2026-05-05T06:15:41Z"}},"updatedAt":"2026-04-29T19:32:11Z","accountEmail":"raw.user@example.com","loginMethod":"prolite","identity":{{"providerID":"raw-provider-id","accountEmail":"raw.user@example.com","loginMethod":"prolite"}}}},"credits":{{"events":[],"updatedAt":"2026-04-29T19:32:13Z","remaining":0}}}}]'
    exit 0
    ;;
esac
"#,
        log_path.display()
    );
    fs::write(&binary, script).expect("write fake codexbar");
    chmod(&binary, 0o700);
    (temp, binary, log_path)
}

fn chmod(path: &Path, mode: u32) {
    let mut permissions = fs::metadata(path)
        .unwrap_or_else(|err| panic!("metadata {}: {err}", path.display()))
        .permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions)
        .unwrap_or_else(|err| panic!("chmod {}: {err}", path.display()));
}
