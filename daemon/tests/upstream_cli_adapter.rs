mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use codexbar_linuxd::app::{App, RefreshStart};
use codexbar_linuxd::cache::SnapshotCache;
use codexbar_linuxd::cli::{CliRefreshRequest, CliTimeouts, UpstreamCliAdapter};
use codexbar_linuxd::fixtures;
use codexbar_linuxd::model::{ProviderState, SemanticSource, SourceAdapter};

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
  "cost --format json --json-only --provider both")
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
async fn upstream_web_semantic_source_does_not_change_daemon_adapter_boundary() {
    let (tmp, binary) = fake_codexbar(
        r#"
case "$*" in
  "--version")
    printf '%s\n' 'CodexBar v0.25.1'
    exit 0
    ;;
  "cost --format json --json-only --provider both")
    printf '%s\n' '[]'
    exit 0
    ;;
  *)
    cat <<'JSON'
[{"provider":"codex","version":"0.25.1","source":"openai-web","usage":{"primary":{"usedPercent":12,"windowMinutes":300,"resetsAt":"2026-05-11T12:00:00Z"},"updatedAt":"2026-05-11T10:00:00Z"}}]
JSON
    exit 0
    ;;
esac
"#,
    );

    let refresh = run_adapter(binary, vec!["codex".to_string()], short_timeouts()).await;
    assert!(tmp.path().is_dir());
    let provider = &refresh.snapshot.providers[0];
    assert_eq!(provider.state, ProviderState::Ok);
    assert_eq!(provider.source, SemanticSource::Web);
    assert_eq!(provider.source_adapter, SourceAdapter::UpstreamCli);
    let snapshot_json = serde_json::to_string(&refresh.snapshot).expect("snapshot json");
    common::assert_schema("snapshot.schema.json", &snapshot_json);
    common::assert_public_json_safe(&snapshot_json);
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
  "cost --format json --json-only --provider both")
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
  "cost --format json --json-only --provider both")
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
  "cost --format json --json-only --provider both")
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
    assert_eq!(
        refresh.snapshot.providers[0].state,
        ProviderState::ProviderUnavailable
    );
    assert!(refresh.snapshot.providers[0]
        .diagnostic_codes
        .contains(&"upstream_cli_provider_error".to_string()));
}

#[tokio::test]
async fn provider_error_payload_maps_to_provider_unavailable_without_raw_payload() {
    let (tmp, binary) = fake_codexbar(
        r#"
case "$*" in
  "--version")
    printf '%s\n' 'CodexBar test-1'
    exit 0
    ;;
  "cost --format json --json-only --provider both")
    printf '%s\n' '[]'
    exit 0
    ;;
  *)
    printf '%s\n' '[{"provider":"codex","error":{"kind":"network_error","message":"provider temporarily unavailable for raw.user@example.com"},"source":"cli"}]'
    exit 0
    ;;
esac
"#,
    );

    let refresh = run_adapter(binary, vec!["codex".to_string()], short_timeouts()).await;
    assert!(tmp.path().is_dir());
    let provider = &refresh.snapshot.providers[0];
    assert_eq!(provider.state, ProviderState::ProviderUnavailable);
    assert_eq!(
        provider.diagnostic_codes,
        vec!["upstream_cli_provider_unavailable".to_string()]
    );
    assert_eq!(
        provider.diagnostics_summary.as_deref(),
        Some("Provider data was unavailable from CodexBar CLI.")
    );
    let snapshot_json = serde_json::to_string(&refresh.snapshot).expect("snapshot json");
    common::assert_schema("snapshot.schema.json", &snapshot_json);
    common::assert_public_json_safe(&snapshot_json);
    assert!(!snapshot_json.contains("raw.user@example.com"));
}

#[tokio::test]
async fn provider_error_payload_maps_common_upstream_messages_to_safe_states() {
    let cases = [
        (
            "CLI session not found for raw.user@example.com. Run login/auth.",
            ProviderState::Unauthenticated,
            "upstream_cli_unauthenticated",
            "Provider sign-in is required in the upstream CLI.",
        ),
        (
            "provider CLI command not found",
            ProviderState::MissingDependency,
            "upstream_cli_provider_cli_missing",
            "Provider CLI dependency was not found.",
        ),
        (
            "provider is rate limited",
            ProviderState::ProviderUnavailable,
            "upstream_cli_provider_rate_limited",
            "Provider is rate limited. Try again later.",
        ),
        (
            "Source 'cli' is not supported",
            ProviderState::ProviderUnavailable,
            "upstream_cli_capability_unimplemented",
            "Requested provider source is not available through CodexBar CLI.",
        ),
        (
            "not supported on Linux; macOS only",
            ProviderState::ProviderUnavailable,
            "upstream_cli_unsupported_source",
            "Requested provider source is not available on Linux through CodexBar CLI.",
        ),
    ];

    for (message, expected_state, expected_code, expected_summary) in cases {
        let payload = serde_json::json!([
            {
                "provider": "codex",
                "error": {
                    "kind": "provider",
                    "message": message
                },
                "source": "cli"
            }
        ]);
        let script = format!(
            r#"
case "$*" in
  "--version")
    printf '%s\n' 'CodexBar test-1'
    exit 0
    ;;
  "cost --format json --json-only --provider both")
    printf '%s\n' '[]'
    exit 0
    ;;
  *)
    cat <<'JSON'
{payload}
JSON
    exit 0
    ;;
esac
"#,
        );
        let (tmp, binary) = fake_codexbar(&script);
        let refresh = run_adapter(binary, vec!["codex".to_string()], short_timeouts()).await;
        assert!(tmp.path().is_dir());
        let provider = &refresh.snapshot.providers[0];
        assert_eq!(provider.state, expected_state, "message: {message}");
        assert_eq!(
            provider.diagnostic_codes,
            vec![expected_code.to_string()],
            "message: {message}"
        );
        assert_eq!(
            provider.diagnostics_summary.as_deref(),
            Some(expected_summary),
            "message: {message}"
        );
        let snapshot_json = serde_json::to_string(&refresh.snapshot).expect("snapshot json");
        common::assert_public_json_safe(&snapshot_json);
        assert!(!snapshot_json.contains("raw.user@example.com"));
        assert!(!snapshot_json.contains(message));
    }
}

#[tokio::test]
async fn app_refresh_with_upstream_cli_uses_v0251_command_strategy() {
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
    let result: serde_json::Value =
        serde_json::from_str(&completion.result_json).expect("result json");
    assert_eq!(result["status"], "ok");
    assert_eq!(
        result["diagnosticCodes"]
            .as_array()
            .expect("diagnostic codes")
            .len(),
        0
    );
    let snapshot_providers = snapshot["providers"].as_array().expect("providers");
    assert_eq!(snapshot_providers.len(), 2);
    assert_eq!(snapshot_providers[0]["provider"], "codex");
    assert_eq!(snapshot_providers[0]["sourceAdapter"], "upstream_cli");
    assert_eq!(snapshot_providers[0]["state"], "ok");
    assert_eq!(snapshot_providers[1]["provider"], "claude");
    assert_eq!(snapshot_providers[1]["sourceAdapter"], "upstream_cli");
    assert_eq!(snapshot_providers[1]["state"], "ok");
    let diagnostics_json = app.get_diagnostics_json("global").expect("diagnostics");
    common::assert_schema("diagnostics.schema.json", &diagnostics_json);
    common::assert_public_json_safe(&diagnostics_json);
    assert_no_warning_or_error_diagnostics(&diagnostics_json);
    let provider_diagnostics_json = app
        .get_diagnostics_json("codex")
        .expect("provider diagnostics");
    common::assert_schema("diagnostics.schema.json", &provider_diagnostics_json);
    common::assert_public_json_safe(&provider_diagnostics_json);
    assert_no_warning_or_error_diagnostics(&provider_diagnostics_json);

    let log = fs::read_to_string(&log_path).unwrap_or_default();
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
            .any(|line| line == "--format json --json-only --provider claude --source cli"),
        "usage command should target claude: {log}"
    );
    assert!(
        log.lines()
            .any(|line| line == "--format json --json-only --provider claude --source cli --status"),
        "status command should target claude: {log}"
    );
    assert!(
        log.lines()
            .any(|line| line == "cost --format json --json-only --provider both"),
        "cost command should use provider both without source: {log}"
    );
    assert!(
        !log.lines()
            .any(|line| line.starts_with("cost ") && line.contains("--source")),
        "cost command must not include --source: {log}"
    );
    assert!(
        !log.lines()
            .any(|line| line.contains("--provider all --source cli")),
        "usage/status must not default to all-provider cli probes: {log}"
    );
    assert!(fake_tmp.path().is_dir());
}

#[tokio::test]
async fn app_refresh_default_codex_success_and_claude_failure_is_partial() {
    let (fake_tmp, binary) = fake_codexbar(
        r#"
case "$*" in
  "--version")
    printf '%s\n' 'CodexBar test-1'
    exit 0
    ;;
  "cost --format json --json-only --provider both")
    printf '%s\n' '[]'
    exit 0
    ;;
  *"--provider claude"*)
    printf '%s\n' '{ "provider": "claude", "usage":'
    exit 0
    ;;
  *)
    cat <<'JSON'
[{"provider":"codex","version":"0.125.0","source":"codex-cli","usage":{"primary":{"usedPercent":34,"windowMinutes":300,"resetsAt":"2026-04-29T22:36:14Z"},"secondary":{"usedPercent":19,"windowMinutes":10080,"resetsAt":"2026-05-05T06:15:41Z"},"updatedAt":"2026-04-29T19:32:11Z"}}]
JSON
    exit 0
    ;;
esac
"#,
    );
    let (_app_tmp, mut paths) = common::temp_paths();
    paths.upstream_cli_path = Some(binary);
    let app = App::new(paths).expect("app starts");

    let completion = run_app_refresh(&app, UPSTREAM_CLI_REFRESH_OPTIONS_JSON).await;
    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);
    common::assert_schema("refresh-result.schema.json", &completion.result_json);

    let snapshot: serde_json::Value =
        serde_json::from_str(&completion.snapshot_json).expect("snapshot json");
    let result: serde_json::Value =
        serde_json::from_str(&completion.result_json).expect("result json");
    assert_eq!(snapshot["daemon"]["state"], "degraded");
    assert_eq!(result["status"], "partial");
    let providers = snapshot["providers"].as_array().expect("providers");
    let codex = providers
        .iter()
        .find(|provider| provider["provider"] == "codex")
        .expect("codex provider");
    let claude = providers
        .iter()
        .find(|provider| provider["provider"] == "claude")
        .expect("claude provider");
    assert_eq!(codex["state"], "ok");
    assert_eq!(claude["state"], "parse_error");
    assert_eq!(result["cacheWritten"], true);
    let result_providers = result["providers"].as_array().expect("result providers");
    assert!(result_providers
        .iter()
        .any(|provider| provider["provider"] == "codex" && provider["status"] == "ok"));
    assert!(result_providers
        .iter()
        .any(|provider| provider["provider"] == "claude" && provider["status"] == "parse_error"));
    assert!(fake_tmp.path().is_dir());
}

#[tokio::test]
async fn app_refresh_uses_configured_provider_targets() {
    let (fake_tmp, binary, log_path) = fake_codexbar_recording();
    let (_app_tmp, mut paths) = common::temp_paths();
    paths.upstream_cli_path = Some(binary);
    let app = App::new(paths).expect("app starts");
    app.set_settings_patch_json(
        r#"{"schemaVersion":1,"providers":{"codex":{"enabled":false},"claude":{"enabled":true}}}"#,
    )
    .expect("settings patch");

    let completion = run_app_refresh(&app, UPSTREAM_CLI_REFRESH_OPTIONS_JSON).await;
    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);

    let log = fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        log.lines()
            .any(|line| line == "--format json --json-only --provider claude --source cli"),
        "configured provider should be targeted: {log}"
    );
    assert!(
        !log.lines()
            .any(|line| line == "--format json --json-only --provider codex --source cli"),
        "disabled default provider should not be targeted: {log}"
    );
    assert!(fake_tmp.path().is_dir());
}

#[tokio::test]
async fn app_refresh_all_configured_providers_disabled_noops_without_defaulting_to_codex() {
    let (fake_tmp, binary, log_path) = fake_codexbar_recording();
    let (_app_tmp, mut paths) = common::temp_paths();
    paths.upstream_cli_path = Some(binary);
    let app = App::new(paths).expect("app starts");
    app.set_settings_patch_json(
        r#"{"schemaVersion":1,"providers":{"codex":{"enabled":false},"claude":{"enabled":true,"preferredSourceAdapter":"off"},"gemini":{"enabled":true,"allowCliFallback":false}}}"#,
    )
    .expect("settings patch");

    let completion = run_app_refresh(&app, UPSTREAM_CLI_REFRESH_OPTIONS_JSON).await;
    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);
    common::assert_schema("refresh-result.schema.json", &completion.result_json);
    let snapshot: serde_json::Value =
        serde_json::from_str(&completion.snapshot_json).expect("snapshot json");
    let result: serde_json::Value =
        serde_json::from_str(&completion.result_json).expect("result json");
    assert_eq!(snapshot["providers"].as_array().unwrap().len(), 0);
    assert_eq!(snapshot["selectedProvider"], serde_json::Value::Null);
    assert_eq!(result["status"], "noop");
    assert_eq!(result["providers"].as_array().unwrap().len(), 0);
    assert!(result["diagnosticCodes"]
        .as_array()
        .expect("diagnostic codes")
        .iter()
        .any(|code| code == "refresh_no_enabled_providers"));

    let log = fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        log.trim().is_empty(),
        "all configured providers are off, so no upstream CLI command should run: {log}"
    );
    assert!(fake_tmp.path().is_dir());
}

#[tokio::test]
async fn app_refresh_explicit_providers_override_settings() {
    let (fake_tmp, binary, log_path) = fake_codexbar_recording();
    let (_app_tmp, mut paths) = common::temp_paths();
    paths.upstream_cli_path = Some(binary);
    let app = App::new(paths).expect("app starts");
    app.set_settings_patch_json(r#"{"schemaVersion":1,"providers":{"claude":{"enabled":true}}}"#)
        .expect("settings patch");

    let completion = run_app_refresh(
        &app,
        r#"{"schemaVersion":1,"reason":"test","force":true,"sourceAdapterPolicy":{"mode":"only","adapters":["upstream_cli"]},"providers":["gemini"]}"#,
    )
    .await;
    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);

    let log = fs::read_to_string(&log_path).expect("command log");
    assert!(
        log.lines()
            .any(|line| line == "--format json --json-only --provider gemini --source cli"),
        "explicit provider should be targeted: {log}"
    );
    assert!(
        !log.lines()
            .any(|line| line == "--format json --json-only --provider claude --source cli"),
        "configured provider should not override explicit provider: {log}"
    );
    assert!(fake_tmp.path().is_dir());
}

#[tokio::test]
async fn app_refresh_explicit_all_provider_is_allowed_only_when_requested() {
    let (fake_tmp, binary, log_path) = fake_codexbar_recording();
    let (_app_tmp, mut paths) = common::temp_paths();
    paths.upstream_cli_path = Some(binary);
    let app = App::new(paths).expect("app starts");

    let completion = run_app_refresh(
        &app,
        r#"{"schemaVersion":1,"reason":"test","force":true,"sourceAdapterPolicy":{"mode":"only","adapters":["upstream_cli"]},"providers":["all"]}"#,
    )
    .await;
    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);

    let log = fs::read_to_string(&log_path).expect("command log");
    assert!(
        log.lines()
            .any(|line| line == "--format json --json-only --provider all --source cli"),
        "all-provider usage should only appear for explicit all requests: {log}"
    );
    assert!(
        log.lines()
            .any(|line| line == "--format json --json-only --provider all --source cli --status"),
        "all-provider status should only appear for explicit all requests: {log}"
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

    let diagnostics_json = app.get_diagnostics_json("global").expect("diagnostics");
    common::assert_schema("diagnostics.schema.json", &diagnostics_json);
    common::assert_public_json_safe(&diagnostics_json);
    assert_diagnostic_message_contains(
        &diagnostics_json,
        "upstream_cli_missing",
        "Install upstream CodexBar CLI",
    );
}

#[tokio::test]
async fn app_refresh_not_executable_upstream_cli_returns_clear_missing_dependency() {
    let (tmp, mut paths) = common::temp_paths();
    let binary = tmp.path().join("codexbar");
    fs::write(&binary, b"not executable").expect("write fake cli");
    chmod(&binary, 0o600);
    paths.upstream_cli_path = Some(binary);
    let app = App::new(paths).expect("app starts");
    let completion = run_app_refresh(&app, UPSTREAM_CLI_REFRESH_OPTIONS_JSON).await;
    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);
    common::assert_schema("refresh-result.schema.json", &completion.result_json);

    let snapshot: serde_json::Value =
        serde_json::from_str(&completion.snapshot_json).expect("snapshot json");
    let result: serde_json::Value =
        serde_json::from_str(&completion.result_json).expect("result json");
    assert_eq!(snapshot["providers"][0]["state"], "missing_dependency");
    assert_eq!(
        snapshot["providers"][0]["diagnosticsSummary"],
        "Configured CodexBar CLI path is not executable."
    );
    assert_eq!(
        snapshot["daemon"]["upstreamCli"]["diagnosticCode"],
        "upstream_cli_not_executable"
    );
    assert!(result["diagnosticCodes"]
        .as_array()
        .expect("diagnostic codes")
        .iter()
        .any(|code| code == "upstream_cli_not_executable"));

    let diagnostics_json = app.get_diagnostics_json("global").expect("diagnostics");
    common::assert_schema("diagnostics.schema.json", &diagnostics_json);
    common::assert_public_json_safe(&diagnostics_json);
    assert_diagnostic_message_contains(
        &diagnostics_json,
        "upstream_cli_not_executable",
        "path is not executable",
    );
}

#[tokio::test]
async fn stale_cache_fallback_after_cli_disappears_reports_current_cli_state() {
    let (tmp, mut paths) = common::temp_paths();
    let cache = SnapshotCache::new(paths.cache_dir.clone(), paths.cache_file.clone());
    let now = codexbar_linuxd::clock::now_rfc3339();
    let cached = fixtures::refreshed_snapshot("cached-ok", &now, &now).expect("snapshot");
    cache.store(&cached).expect("cache store");
    paths.upstream_cli_path = Some(tmp.path().join("missing-codexbar"));

    let app = App::new(paths).expect("app starts");
    let completion = run_app_refresh(
        &app,
        r#"{"schemaVersion":1,"reason":"test","force":true,"sourceAdapterPolicy":{"mode":"only","adapters":["upstream_cli"],"allowStaleCacheFallback":true}}"#,
    )
    .await;
    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);
    common::assert_schema("refresh-result.schema.json", &completion.result_json);

    let snapshot: serde_json::Value =
        serde_json::from_str(&completion.snapshot_json).expect("snapshot json");
    let result: serde_json::Value =
        serde_json::from_str(&completion.result_json).expect("result json");
    assert_eq!(snapshot["stale"], true);
    assert_eq!(snapshot["providers"][0]["state"], "stale");
    assert_eq!(snapshot["daemon"]["upstreamCli"]["available"], false);
    assert_eq!(
        snapshot["daemon"]["upstreamCli"]["diagnosticCode"],
        "upstream_cli_missing"
    );
    let diagnostic_codes = result["diagnosticCodes"]
        .as_array()
        .expect("diagnostic codes");
    assert!(diagnostic_codes
        .iter()
        .any(|code| code == "upstream_cli_missing"));
    assert!(diagnostic_codes
        .iter()
        .any(|code| code == "stale_cache_used"));
    let diagnostics_json = app.get_diagnostics_json("global").expect("diagnostics");
    common::assert_schema("diagnostics.schema.json", &diagnostics_json);
    common::assert_public_json_safe(&diagnostics_json);
    assert_diagnostic_message_contains(
        &diagnostics_json,
        "stale_cache_used",
        "Showing cached usage data.",
    );
}

#[tokio::test]
async fn stale_cache_fallback_after_cli_timeout_preserves_cached_snapshot() {
    let (tmp, mut paths) = common::temp_paths();
    let cache = SnapshotCache::new(paths.cache_dir.clone(), paths.cache_file.clone());
    let now = codexbar_linuxd::clock::now_rfc3339();
    let cached = fixtures::refreshed_snapshot("cached-ok", &now, &now).expect("snapshot");
    cache.store(&cached).expect("cache store");
    let (fake_tmp, binary) = fake_codexbar(
        r#"
case "$*" in
  "--version")
    printf '%s\n' 'CodexBar test-1'
    exit 0
    ;;
  "cost --format json --json-only --provider both")
    printf '%s\n' '[]'
    exit 0
    ;;
  *)
    printf '%s\n' 'provider request timed out' >&2
    exit 1
    ;;
esac
"#,
    );
    paths.upstream_cli_path = Some(binary);

    let app = App::new(paths).expect("app starts");
    let completion = run_app_refresh(
        &app,
        r#"{"schemaVersion":1,"reason":"test","force":true,"sourceAdapterPolicy":{"mode":"only","adapters":["upstream_cli"],"allowStaleCacheFallback":true}}"#,
    )
    .await;
    assert!(tmp.path().is_dir());
    assert!(fake_tmp.path().is_dir());
    common::assert_schema("snapshot.schema.json", &completion.snapshot_json);
    common::assert_schema("refresh-result.schema.json", &completion.result_json);
    common::assert_public_json_safe(&completion.snapshot_json);

    let snapshot: serde_json::Value =
        serde_json::from_str(&completion.snapshot_json).expect("snapshot json");
    let result: serde_json::Value =
        serde_json::from_str(&completion.result_json).expect("result json");
    assert_eq!(snapshot["stale"], true);
    assert_eq!(snapshot["providers"][0]["state"], "stale");
    assert_eq!(snapshot["daemon"]["upstreamCli"]["available"], true);
    let diagnostic_codes = result["diagnosticCodes"]
        .as_array()
        .expect("diagnostic codes");
    assert!(diagnostic_codes
        .iter()
        .any(|code| code == "upstream_cli_timeout"));
    assert!(diagnostic_codes
        .iter()
        .any(|code| code == "stale_cache_used"));
    let diagnostics_json = app.get_diagnostics_json("global").expect("diagnostics");
    common::assert_schema("diagnostics.schema.json", &diagnostics_json);
    common::assert_public_json_safe(&diagnostics_json);
    assert_diagnostic_message_contains(
        &diagnostics_json,
        "stale_cache_used",
        "Showing cached usage data.",
    );
    let provider_diagnostics_json = app
        .get_diagnostics_json("codex")
        .expect("provider diagnostics");
    common::assert_schema("diagnostics.schema.json", &provider_diagnostics_json);
    common::assert_public_json_safe(&provider_diagnostics_json);
    assert_diagnostic_message_contains(
        &provider_diagnostics_json,
        "upstream_cli_timeout",
        "timed out",
    );
}

#[tokio::test]
async fn usage_success_survives_status_and_cost_failures_with_diagnostics() {
    let (tmp, binary) = fake_codexbar(
        r#"
case "$*" in
  "--version")
    printf '%s\n' 'CodexBar not-semver'
    exit 0
    ;;
  "cost --format json --json-only --provider both")
    sleep 1
    exit 0
    ;;
  *"--status")
    printf '%s\n' '[{"provider":"codex","error":{"message":"provider status failed"}}]'
    exit 1
    ;;
  *)
    printf '%s\n' '[{"provider":"codex","version":"not-semver","source":"codex-cli","usage":{"primary":{"usedPercent":21,"windowMinutes":300,"resetsAt":"2026-04-29T22:36:14Z"},"updatedAt":"2026-04-29T19:32:11Z"}}]'
    exit 0
    ;;
esac
"#,
    );

    let refresh = run_adapter(
        binary,
        vec!["codex".to_string()],
        CliTimeouts {
            version: Duration::from_secs(1),
            usage: Duration::from_secs(1),
            status: Duration::from_secs(1),
            cost: Duration::from_millis(50),
        },
    )
    .await;
    assert!(tmp.path().is_dir());
    let provider = &refresh.snapshot.providers[0];
    assert_eq!(provider.state, ProviderState::Ok);
    assert_eq!(
        refresh
            .snapshot
            .daemon
            .upstream_cli
            .as_ref()
            .unwrap()
            .version,
        Some("CodexBar not-semver".to_string())
    );
    assert!(provider
        .diagnostic_codes
        .contains(&"upstream_cli_provider_error".to_string()));
    assert!(provider
        .diagnostic_codes
        .contains(&"upstream_cli_cost_unavailable".to_string()));
    assert_eq!(
        provider.diagnostics_summary.as_deref(),
        Some("Upstream CLI returned partial diagnostics")
    );
    let snapshot_json = serde_json::to_string(&refresh.snapshot).expect("snapshot json");
    common::assert_schema("snapshot.schema.json", &snapshot_json);
    common::assert_public_json_safe(&snapshot_json);
    assert!(refresh
        .diagnostics
        .iter()
        .any(|event| event.code == "upstream_cli_timeout"));
    let cost_diagnostic = refresh
        .diagnostics
        .iter()
        .find(|event| event.code == "upstream_cli_cost_unavailable")
        .expect("cost unavailable diagnostic");
    assert_eq!(
        cost_diagnostic.safe_message,
        "Local cost data was unavailable."
    );
}

#[tokio::test]
async fn cost_failure_does_not_fail_successful_usage_provider() {
    let (tmp, binary) = fake_codexbar(
        r#"
case "$*" in
  "--version")
    printf '%s\n' 'CodexBar local-build'
    exit 0
    ;;
  "cost --format json --json-only --provider both")
    printf '%s\n' '{"providers":[{"provider":"codex","error":{"kind":"local_cost","message":"cost command unavailable for raw.user@example.com"}}]}'
    exit 0
    ;;
  *"--status")
    printf '%s\n' '[{"provider":"codex","source":"codex-cli","usage":{"primary":{"usedPercent":21,"windowMinutes":300,"resetsAt":"2026-04-29T22:36:14Z"},"updatedAt":"2026-04-29T19:32:12Z"},"status":{"updatedAt":"2026-04-29T19:32:12Z","indicator":"none","description":"OK"}}]'
    exit 0
    ;;
  *)
    printf '%s\n' '[{"provider":"codex","version":"local-build","source":"codex-cli","usage":{"primary":{"usedPercent":21,"windowMinutes":300,"resetsAt":"2026-04-29T22:36:14Z"},"updatedAt":"2026-04-29T19:32:11Z"}}]'
    exit 0
    ;;
esac
"#,
    );

    let refresh = run_adapter(binary, vec!["codex".to_string()], short_timeouts()).await;
    assert!(tmp.path().is_dir());
    let provider = &refresh.snapshot.providers[0];
    assert_eq!(provider.state, ProviderState::Ok);
    assert!(provider.cost.is_none());
    assert_eq!(
        provider.diagnostic_codes,
        vec!["upstream_cli_cost_unavailable".to_string()]
    );
    assert_eq!(
        provider.diagnostics_summary.as_deref(),
        Some("Local cost data was unavailable.")
    );
    assert_eq!(
        refresh
            .snapshot
            .daemon
            .upstream_cli
            .as_ref()
            .unwrap()
            .version,
        Some("CodexBar local-build".to_string())
    );
    let snapshot_json = serde_json::to_string(&refresh.snapshot).expect("snapshot json");
    common::assert_schema("snapshot.schema.json", &snapshot_json);
    common::assert_public_json_safe(&snapshot_json);
    assert!(!snapshot_json.contains("raw.user@example.com"));
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

fn assert_diagnostic_message_contains(diagnostics_json: &str, code: &str, expected: &str) {
    let diagnostics: serde_json::Value =
        serde_json::from_str(diagnostics_json).expect("diagnostics json");
    let event = diagnostics["events"]
        .as_array()
        .expect("diagnostics events")
        .iter()
        .find(|event| event["code"] == code)
        .unwrap_or_else(|| panic!("missing diagnostic event {code} in {diagnostics_json}"));
    let safe_message = event["safeMessage"]
        .as_str()
        .expect("diagnostic safeMessage");
    assert!(
        safe_message.contains(expected),
        "diagnostic {code} safeMessage {safe_message:?} did not contain {expected:?}"
    );
    assert!(
        event["details"].get("stdout").is_none(),
        "diagnostic {code} must not expose stdout"
    );
    assert!(
        event["details"].get("stderr").is_none(),
        "diagnostic {code} must not expose stderr"
    );
}

fn assert_no_warning_or_error_diagnostics(diagnostics_json: &str) {
    let diagnostics: serde_json::Value =
        serde_json::from_str(diagnostics_json).expect("diagnostics json");
    for event in diagnostics["events"]
        .as_array()
        .expect("diagnostics events")
    {
        assert_eq!(
            event["severity"], "info",
            "expected only info diagnostics in clean refresh, got {event}"
        );
    }
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

async fn run_app_refresh(app: &App, options_json: &str) -> codexbar_linuxd::app::RefreshCompletion {
    let start = app.start_refresh(options_json).expect("refresh starts");
    let RefreshStart::Started { refresh_id } = start else {
        panic!("refresh should start");
    };
    app.finish_refresh(&refresh_id)
        .await
        .expect("refresh finishes")
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
provider=codex
case "$*" in
  *"--provider claude"*) provider=claude ;;
  *"--provider gemini"*) provider=gemini ;;
  *"--provider all"*) provider=all ;;
esac
case "$*" in
  "--version")
    printf '%s\n' 'CodexBar test-1'
    exit 0
    ;;
  "cost --format json --json-only --provider both")
    printf '%s\n' '[{{"provider":"codex","source":"local","updatedAt":"2026-04-29T19:32:13Z","sessionCostUSD":1.25,"last30DaysCostUSD":12.34,"totals":{{"totalCost":12.34}}}}]'
    exit 0
    ;;
  *"--status")
    cat <<JSON
[{{"provider":"$provider","version":"0.125.0","source":"codex-cli","usage":{{"primary":{{"usedPercent":34,"windowMinutes":300,"resetsAt":"2026-04-29T22:36:14Z"}},"secondary":{{"usedPercent":19,"windowMinutes":10080,"resetsAt":"2026-05-05T06:15:41Z"}},"updatedAt":"2026-04-29T19:32:19Z"}},"credits":{{"updatedAt":"2026-04-29T19:32:20Z","remaining":0}},"status":{{"updatedAt":"2026-04-27T15:52:49Z","indicator":"none","description":"All Systems Operational","url":"https://status.openai.com/"}}}}]
JSON
    exit 0
    ;;
  *)
    cat <<JSON
[{{"provider":"$provider","version":"0.125.0","source":"codex-cli","usage":{{"primary":{{"usedPercent":34,"windowMinutes":300,"resetsAt":"2026-04-29T22:36:14Z"}},"secondary":{{"usedPercent":19,"windowMinutes":10080,"resetsAt":"2026-05-05T06:15:41Z"}},"updatedAt":"2026-04-29T19:32:11Z"}},"credits":{{"events":[],"updatedAt":"2026-04-29T19:32:13Z","remaining":0}}}}]
JSON
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
