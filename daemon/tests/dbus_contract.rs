mod common;

use std::fs;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use futures_util::StreamExt;
use tempfile::TempDir;
use tokio::time::timeout;
use zbus::proxy::MethodFlags;
use zbus::Proxy;

const BUS_NAME: &str = "org.codexbar.Linux1";
const OBJECT_PATH: &str = "/org/codexbar/Linux1";
const INTERFACE: &str = "org.codexbar.Linux1";
const ISOLATED_DBUS_ENV: &str = "CODEXBAR_LINUX_TEST_ISOLATED_DBUS";
const FIXTURE_REFRESH_OPTIONS_JSON: &str = r#"{"schemaVersion":1,"reason":"test","force":true,"sourceAdapterPolicy":{"mode":"only","adapters":["fixture"]}}"#;
const LIVE_UPSTREAM_CLI_REFRESH_OPTIONS_JSON: &str = r#"{"schemaVersion":1,"reason":"manual","force":true,"busyBehavior":"return_existing","sourceAdapterPolicy":{"mode":"only","adapters":["upstream_cli"],"allowStaleCacheFallback":false},"providers":["codex"]}"#;
const SIGNAL_TIMEOUT: Duration = Duration::from_secs(10);
static DBUS_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct DaemonChild {
    child: Child,
}

impl Drop for DaemonChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn real_user_session_bus_address_is_not_considered_isolated() {
    assert!(!session_bus_address_is_isolated(
        "unix:path=/run/user/1000/bus"
    ));
}

#[test]
fn dbus_run_session_style_bus_address_is_considered_isolated() {
    assert!(session_bus_address_is_isolated(
        "unix:path=/tmp/dbus-test-bus"
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dbus_contract_runtime_methods_signals_errors_and_cache() {
    if !isolated_session_bus_available("dbus_contract_runtime_methods_signals_errors_and_cache") {
        return;
    }
    let _guard = DBUS_TEST_LOCK.lock().await;

    let tmp = tempfile::tempdir().expect("tempdir");
    let mut daemon = spawn_daemon(&tmp);
    let connection = zbus::Connection::session().await.expect("session bus");
    let proxy = wait_for_proxy(&connection).await;

    let daemon_info = call_string_no_autostart(&proxy, "GetDaemonInfo", &())
        .await
        .expect("GetDaemonInfo");
    common::assert_schema("daemon-info.schema.json", &daemon_info);
    assert_dbus_test_paths_are_isolated(&daemon_info, &tmp);

    let snapshot = call_string_no_autostart(&proxy, "GetSnapshot", &())
        .await
        .expect("GetSnapshot");
    common::assert_schema("snapshot.schema.json", &snapshot);

    let diagnostics = call_string_no_autostart(&proxy, "GetDiagnostics", &"global")
        .await
        .expect("GetDiagnostics");
    common::assert_schema("diagnostics.schema.json", &diagnostics);

    let initial_settings = call_string_no_autostart(&proxy, "GetSettings", &())
        .await
        .expect("GetSettings");
    common::assert_schema("settings.schema.json", &initial_settings);

    let mut settings_stream = proxy
        .receive_signal("SettingsChanged")
        .await
        .expect("settings stream");
    let settings = call_string_no_autostart(
        &proxy,
        "SetSettingsPatch",
        &r#"{"schemaVersion":1,"refresh":{"intervalSeconds":240}}"#,
    )
    .await
    .expect("SetSettingsPatch");
    common::assert_schema("settings.schema.json", &settings);
    let settings_msg = timeout(SIGNAL_TIMEOUT, settings_stream.next())
        .await
        .expect("SettingsChanged timeout")
        .expect("SettingsChanged message");
    let changed_settings: String = settings_msg.body().deserialize().expect("settings body");
    common::assert_schema("settings.schema.json", &changed_settings);

    let browser_result = call_string_no_autostart(
        &proxy,
        "TestBrowserImport",
        &r#"{"schemaVersion":1,"providers":["codex"]}"#,
    )
    .await
    .expect("TestBrowserImport");
    common::assert_schema("browser-import-result.schema.json", &browser_result);
    let browser_value: serde_json::Value =
        serde_json::from_str(&browser_result).expect("browser json");
    assert_eq!(browser_value["status"], "not_implemented");

    let mut started_stream = proxy
        .receive_signal("RefreshStarted")
        .await
        .expect("started stream");
    let mut provider_stream = proxy
        .receive_signal("ProviderChanged")
        .await
        .expect("provider stream");
    let mut snapshot_stream = proxy
        .receive_signal("SnapshotChanged")
        .await
        .expect("snapshot stream");
    let mut finished_stream = proxy
        .receive_signal("RefreshFinished")
        .await
        .expect("finished stream");

    let refresh_id = call_string_no_autostart(&proxy, "Refresh", &FIXTURE_REFRESH_OPTIONS_JSON)
        .await
        .expect("Refresh");
    assert!(!refresh_id.is_empty());

    let busy_err = call_string_no_autostart(
        &proxy,
        "Refresh",
        &r#"{"schemaVersion":1,"reason":"test","busyBehavior":"reject"}"#,
    )
    .await
    .expect_err("busy refresh should reject");
    assert_eq!(
        method_error_name(&busy_err),
        "org.codexbar.Linux1.Error.RefreshBusy"
    );

    let started_msg = timeout(SIGNAL_TIMEOUT, started_stream.next())
        .await
        .expect("RefreshStarted timeout")
        .expect("RefreshStarted message");
    let started_id: String = started_msg.body().deserialize().expect("started body");
    assert_eq!(started_id, refresh_id);

    let provider_msg = timeout(SIGNAL_TIMEOUT, provider_stream.next())
        .await
        .expect("ProviderChanged timeout")
        .expect("ProviderChanged message");
    let (provider_id, provider_event_json): (String, String) =
        provider_msg.body().deserialize().expect("provider body");
    common::assert_schema("provider-event.schema.json", &provider_event_json);
    let provider_event: serde_json::Value =
        serde_json::from_str(&provider_event_json).expect("provider event json");
    assert_eq!(provider_event["providerId"], provider_id);
    assert_eq!(provider_event["provider"]["provider"], provider_id);

    let snapshot_msg = timeout(SIGNAL_TIMEOUT, snapshot_stream.next())
        .await
        .expect("SnapshotChanged timeout")
        .expect("SnapshotChanged message");
    let changed_snapshot: String = snapshot_msg.body().deserialize().expect("snapshot body");
    common::assert_schema("snapshot.schema.json", &changed_snapshot);

    let finished_msg = timeout(SIGNAL_TIMEOUT, finished_stream.next())
        .await
        .expect("RefreshFinished timeout")
        .expect("RefreshFinished message");
    let (finished_id, result_json): (String, String) =
        finished_msg.body().deserialize().expect("finished body");
    assert_eq!(finished_id, refresh_id);
    common::assert_schema("refresh-result.schema.json", &result_json);

    let invalid_err = call_string_no_autostart(&proxy, "Refresh", &"{")
        .await
        .expect_err("invalid JSON should fail");
    assert_eq!(
        method_error_name(&invalid_err),
        "org.codexbar.Linux1.Error.InvalidJson"
    );

    let cache_file = tmp
        .path()
        .join("cache")
        .join("codexbar-linux")
        .join("snapshot.json");
    assert_eq!(
        common::file_mode(cache_file.parent().expect("cache dir")),
        0o700
    );
    assert_eq!(common::file_mode(&cache_file), 0o600);

    drop(daemon);
    daemon = spawn_daemon(&tmp);
    let connection = zbus::Connection::session().await.expect("session bus 2");
    let proxy = wait_for_proxy(&connection).await;
    let cached_snapshot = call_string_no_autostart(&proxy, "GetSnapshot", &())
        .await
        .expect("cached GetSnapshot");
    common::assert_schema("snapshot.schema.json", &cached_snapshot);
    let cached_value: serde_json::Value =
        serde_json::from_str(&cached_snapshot).expect("cached snapshot json");
    assert_eq!(cached_value["stale"], true);
    drop(daemon);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dbus_scheduler_runs_startup_refresh_when_enabled() {
    if !isolated_session_bus_available("dbus_scheduler_runs_startup_refresh_when_enabled") {
        return;
    }
    let _guard = DBUS_TEST_LOCK.lock().await;

    let tmp = tempfile::tempdir().expect("tempdir");
    write_daemon_config(&tmp, true, 86400);
    let daemon = spawn_daemon_with_scheduler_controls(&tmp, None, false, None, Some(2500));
    let connection = zbus::Connection::session().await.expect("session bus");
    let proxy = wait_for_proxy(&connection).await;
    let mut finished_stream = proxy
        .receive_signal("RefreshFinished")
        .await
        .expect("finished stream");

    let finished_msg = timeout(Duration::from_secs(6), finished_stream.next())
        .await
        .expect("startup RefreshFinished timeout")
        .expect("startup RefreshFinished message");
    let (_refresh_id, result_json): (String, String) =
        finished_msg.body().deserialize().expect("finished body");
    common::assert_schema("refresh-result.schema.json", &result_json);
    let result: serde_json::Value = serde_json::from_str(&result_json).expect("result json");
    assert_eq!(result["reason"], "startup");
    drop(daemon);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dbus_scheduler_runs_interval_refresh_when_enabled() {
    if !isolated_session_bus_available("dbus_scheduler_runs_interval_refresh_when_enabled") {
        return;
    }
    let _guard = DBUS_TEST_LOCK.lock().await;

    let tmp = tempfile::tempdir().expect("tempdir");
    write_daemon_config(&tmp, false, 30);
    let daemon = spawn_daemon_with_scheduler_controls(&tmp, None, false, Some(100), None);
    let connection = zbus::Connection::session().await.expect("session bus");
    let proxy = wait_for_proxy(&connection).await;
    let mut finished_stream = proxy
        .receive_signal("RefreshFinished")
        .await
        .expect("finished stream");

    let finished_msg = timeout(SIGNAL_TIMEOUT, finished_stream.next())
        .await
        .expect("scheduled RefreshFinished timeout")
        .expect("scheduled RefreshFinished message");
    let (_refresh_id, result_json): (String, String) =
        finished_msg.body().deserialize().expect("finished body");
    common::assert_schema("refresh-result.schema.json", &result_json);
    let result: serde_json::Value = serde_json::from_str(&result_json).expect("result json");
    assert_eq!(result["reason"], "scheduled");
    drop(daemon);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dbus_scheduler_interval_zero_disables_interval_loop_but_allows_startup() {
    if !isolated_session_bus_available(
        "dbus_scheduler_interval_zero_disables_interval_loop_but_allows_startup",
    ) {
        return;
    }
    let _guard = DBUS_TEST_LOCK.lock().await;

    let tmp = tempfile::tempdir().expect("tempdir");
    write_daemon_config(&tmp, true, 0);
    let daemon = spawn_daemon_with_scheduler_controls(&tmp, None, false, Some(100), Some(1000));
    let connection = zbus::Connection::session().await.expect("session bus");
    let proxy = wait_for_proxy(&connection).await;
    let mut finished_stream = proxy
        .receive_signal("RefreshFinished")
        .await
        .expect("finished stream");

    let finished_msg = timeout(SIGNAL_TIMEOUT, finished_stream.next())
        .await
        .expect("startup RefreshFinished timeout")
        .expect("startup RefreshFinished message");
    let (_refresh_id, result_json): (String, String) =
        finished_msg.body().deserialize().expect("finished body");
    common::assert_schema("refresh-result.schema.json", &result_json);
    let result: serde_json::Value = serde_json::from_str(&result_json).expect("result json");
    assert_eq!(result["reason"], "startup");

    let no_scheduled = timeout(Duration::from_millis(350), finished_stream.next()).await;
    assert!(
        no_scheduled.is_err(),
        "intervalSeconds=0 must not run a scheduled interval refresh after startup"
    );
    drop(daemon);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dbus_scheduler_backs_off_repeated_upstream_cli_failures() {
    if !isolated_session_bus_available("dbus_scheduler_backs_off_repeated_upstream_cli_failures") {
        return;
    }
    let _guard = DBUS_TEST_LOCK.lock().await;

    let tmp = tempfile::tempdir().expect("tempdir");
    write_daemon_config(&tmp, false, 30);
    let daemon = spawn_daemon_with_scheduler_controls(&tmp, None, false, Some(80), Some(10));
    let connection = zbus::Connection::session().await.expect("session bus");
    let proxy = wait_for_proxy(&connection).await;
    let mut finished_stream = proxy
        .receive_signal("RefreshFinished")
        .await
        .expect("finished stream");

    let first_msg = timeout(SIGNAL_TIMEOUT, finished_stream.next())
        .await
        .expect("first scheduled RefreshFinished timeout")
        .expect("first scheduled RefreshFinished message");
    let (_first_id, first_result_json): (String, String) =
        first_msg.body().deserialize().expect("first finished body");
    let first_result: serde_json::Value =
        serde_json::from_str(&first_result_json).expect("first result json");
    assert_eq!(first_result["reason"], "scheduled");
    assert!(first_result["diagnosticCodes"]
        .as_array()
        .expect("diagnostic codes")
        .iter()
        .any(|code| code == "upstream_cli_missing"));

    let after_first = tokio::time::Instant::now();
    let second_msg = timeout(SIGNAL_TIMEOUT, finished_stream.next())
        .await
        .expect("second scheduled RefreshFinished timeout")
        .expect("second scheduled RefreshFinished message");
    let gap = after_first.elapsed();
    assert!(
        gap >= Duration::from_millis(120),
        "second scheduled failure should be delayed by backoff, observed gap {gap:?}"
    );
    let (_second_id, second_result_json): (String, String) = second_msg
        .body()
        .deserialize()
        .expect("second finished body");
    common::assert_schema("refresh-result.schema.json", &second_result_json);

    let no_immediate_third = timeout(Duration::from_millis(180), finished_stream.next()).await;
    assert!(
        no_immediate_third.is_err(),
        "third repeated upstream CLI failure should not run at the base interval"
    );
    drop(daemon);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dbus_refresh_all_configured_providers_disabled_returns_noop() {
    if !isolated_session_bus_available(
        "dbus_refresh_all_configured_providers_disabled_returns_noop",
    ) {
        return;
    }
    let _guard = DBUS_TEST_LOCK.lock().await;

    let tmp = tempfile::tempdir().expect("tempdir");
    write_daemon_config_with_providers(
        &tmp,
        false,
        0,
        r#"{
    "codex": {"enabled": false, "preferredSourceAdapter": "auto", "allowBrowserImport": false, "allowCliFallback": true},
    "claude": {"enabled": true, "preferredSourceAdapter": "off", "allowBrowserImport": false, "allowCliFallback": true},
    "gemini": {"enabled": true, "preferredSourceAdapter": "auto", "allowBrowserImport": false, "allowCliFallback": false}
  }"#,
    );
    let daemon = spawn_daemon_with_scheduler_controls(&tmp, None, false, None, None);
    let connection = zbus::Connection::session().await.expect("session bus");
    let proxy = wait_for_proxy(&connection).await;
    let mut finished_stream = proxy
        .receive_signal("RefreshFinished")
        .await
        .expect("finished stream");

    let refresh_id = call_string_no_autostart(
        &proxy,
        "Refresh",
        &r#"{"schemaVersion":1,"reason":"manual","force":true,"busyBehavior":"return_existing","sourceAdapterPolicy":{"mode":"only","adapters":["upstream_cli"],"allowStaleCacheFallback":false}}"#,
    )
    .await
    .expect("Refresh");
    assert!(!refresh_id.is_empty());
    let finished_msg = timeout(SIGNAL_TIMEOUT, finished_stream.next())
        .await
        .expect("RefreshFinished timeout")
        .expect("RefreshFinished message");
    let (finished_id, result_json): (String, String) =
        finished_msg.body().deserialize().expect("finished body");
    assert_eq!(finished_id, refresh_id);
    common::assert_schema("refresh-result.schema.json", &result_json);
    let result: serde_json::Value = serde_json::from_str(&result_json).expect("result json");
    assert_eq!(result["status"], "noop");
    assert_eq!(result["providers"].as_array().unwrap().len(), 0);
    assert!(result["diagnosticCodes"]
        .as_array()
        .expect("diagnostic codes")
        .iter()
        .any(|code| code == "refresh_no_enabled_providers"));

    let snapshot = call_string_no_autostart(&proxy, "GetSnapshot", &())
        .await
        .expect("GetSnapshot");
    common::assert_schema("snapshot.schema.json", &snapshot);
    let snapshot_value: serde_json::Value = serde_json::from_str(&snapshot).expect("snapshot json");
    assert_eq!(snapshot_value["providers"].as_array().unwrap().len(), 0);
    assert_eq!(snapshot_value["selectedProvider"], serde_json::Value::Null);
    drop(daemon);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires dbus-run-session, CODEXBAR_LIVE=1, and CODEXBAR_CLI=/path/to/codexbar"]
async fn live_dbus_upstream_cli_refresh_smoke_redacts_outputs() {
    if !isolated_session_bus_available("live_dbus_upstream_cli_refresh_smoke_redacts_outputs") {
        return;
    }
    let _guard = DBUS_TEST_LOCK.lock().await;
    let Some(binary) = common::live_codexbar_binary() else {
        return;
    };

    let tmp = tempfile::tempdir().expect("tempdir");
    let daemon = spawn_daemon_with_cli(&tmp, Some(&binary));
    let connection = zbus::Connection::session().await.expect("session bus");
    let proxy = wait_for_proxy(&connection).await;

    let daemon_info = call_string_no_autostart(&proxy, "GetDaemonInfo", &())
        .await
        .expect("GetDaemonInfo");
    common::assert_schema("daemon-info.schema.json", &daemon_info);
    common::assert_public_json_safe(&daemon_info);
    common::assert_no_live_secret_markers("live D-Bus daemon info", &daemon_info);

    let mut snapshot_stream = proxy
        .receive_signal("SnapshotChanged")
        .await
        .expect("snapshot stream");
    let mut finished_stream = proxy
        .receive_signal("RefreshFinished")
        .await
        .expect("finished stream");

    let refresh_id =
        call_string_no_autostart(&proxy, "Refresh", &LIVE_UPSTREAM_CLI_REFRESH_OPTIONS_JSON)
            .await
            .expect("Refresh");
    assert!(!refresh_id.is_empty());

    let snapshot_msg = timeout(Duration::from_secs(120), snapshot_stream.next())
        .await
        .expect("SnapshotChanged timeout")
        .expect("SnapshotChanged message");
    let changed_snapshot: String = snapshot_msg.body().deserialize().expect("snapshot body");
    assert_live_upstream_snapshot("live D-Bus SnapshotChanged", &changed_snapshot);

    let finished_msg = timeout(Duration::from_secs(120), finished_stream.next())
        .await
        .expect("RefreshFinished timeout")
        .expect("RefreshFinished message");
    let (finished_id, result_json): (String, String) =
        finished_msg.body().deserialize().expect("finished body");
    assert_eq!(finished_id, refresh_id);
    common::assert_schema("refresh-result.schema.json", &result_json);
    common::assert_public_json_safe(&result_json);
    common::assert_no_live_secret_markers("live D-Bus RefreshFinished", &result_json);
    let result: serde_json::Value = serde_json::from_str(&result_json).expect("result json");
    assert_eq!(result["cacheWritten"], true);
    let result_providers = result["providers"]
        .as_array()
        .expect("result providers array");
    let result_codex = result_providers
        .iter()
        .find(|provider| provider["provider"] == "codex")
        .expect("codex refresh result present");
    assert_eq!(result_codex["status"], "ok");

    let current_snapshot = call_string_no_autostart(&proxy, "GetSnapshot", &())
        .await
        .expect("GetSnapshot");
    assert_live_upstream_snapshot("live D-Bus GetSnapshot", &current_snapshot);

    let diagnostics_json = call_string_no_autostart(&proxy, "GetDiagnostics", &"global")
        .await
        .expect("GetDiagnostics");
    common::assert_schema("diagnostics.schema.json", &diagnostics_json);
    common::assert_public_json_safe(&diagnostics_json);
    common::assert_no_live_secret_markers("live D-Bus diagnostics", &diagnostics_json);
    let provider_diagnostics_json = call_string_no_autostart(&proxy, "GetDiagnostics", &"codex")
        .await
        .expect("GetDiagnostics codex");
    common::assert_schema("diagnostics.schema.json", &provider_diagnostics_json);
    common::assert_public_json_safe(&provider_diagnostics_json);
    common::assert_no_live_secret_markers(
        "live D-Bus provider diagnostics",
        &provider_diagnostics_json,
    );

    let cache_file = tmp
        .path()
        .join("cache")
        .join("codexbar-linux")
        .join("snapshot.json");
    let cache_json = std::fs::read_to_string(cache_file).expect("live D-Bus cache snapshot");
    common::assert_schema("snapshot.schema.json", &cache_json);
    common::assert_public_json_safe(&cache_json);
    common::assert_no_live_secret_markers("live D-Bus cache", &cache_json);

    drop(daemon);
}

fn spawn_daemon(tmp: &TempDir) -> DaemonChild {
    write_daemon_config(tmp, false, 86400);
    spawn_daemon_with_scheduler_controls(tmp, None, true, None, None)
}

fn spawn_daemon_with_cli(tmp: &TempDir, cli_path: Option<&std::path::Path>) -> DaemonChild {
    write_daemon_config(tmp, false, 86400);
    spawn_daemon_with_scheduler_controls(tmp, cli_path, false, None, None)
}

fn spawn_daemon_with_scheduler_controls(
    tmp: &TempDir,
    cli_path: Option<&std::path::Path>,
    allow_fixture: bool,
    scheduler_interval_ms: Option<u64>,
    refresh_finish_delay_ms: Option<u64>,
) -> DaemonChild {
    let bin = env!("CARGO_BIN_EXE_codexbar-linuxd");
    let home = if cli_path.is_some() {
        std::env::var_os("HOME").unwrap_or_else(|| tmp.path().join("home").into_os_string())
    } else {
        tmp.path().join("home").into_os_string()
    };
    let mut command = Command::new(bin);
    command
        .env("XDG_CACHE_HOME", tmp.path().join("cache"))
        .env("XDG_CONFIG_HOME", tmp.path().join("config"))
        .env("HOME", home)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cli_path) = cli_path {
        command.env("CODEXBAR_CLI", cli_path);
    }
    if allow_fixture {
        command.env("CODEXBAR_LINUX_ALLOW_FIXTURE", "1");
    } else {
        command.env_remove("CODEXBAR_LINUX_ALLOW_FIXTURE");
    }
    if let Some(milliseconds) = scheduler_interval_ms {
        command.env(
            "CODEXBAR_LINUX_TEST_SCHEDULER_INTERVAL_MS",
            milliseconds.to_string(),
        );
    } else {
        command.env_remove("CODEXBAR_LINUX_TEST_SCHEDULER_INTERVAL_MS");
    }
    if let Some(milliseconds) = refresh_finish_delay_ms {
        command.env(
            "CODEXBAR_LINUX_TEST_REFRESH_FINISH_DELAY_MS",
            milliseconds.to_string(),
        );
    } else {
        command.env_remove("CODEXBAR_LINUX_TEST_REFRESH_FINISH_DELAY_MS");
    }
    let child = command.spawn().expect("spawn daemon");
    DaemonChild { child }
}

fn write_daemon_config(tmp: &TempDir, startup_refresh: bool, interval_seconds: u64) {
    write_daemon_config_with_providers(tmp, startup_refresh, interval_seconds, "{}");
}

fn write_daemon_config_with_providers(
    tmp: &TempDir,
    startup_refresh: bool,
    interval_seconds: u64,
    providers_json: &str,
) {
    let config_dir = tmp.path().join("config").join("codexbar-linux");
    fs::create_dir_all(&config_dir).expect("config dir");
    let config = format!(
        r#"{{
  "schemaVersion": 1,
  "refresh": {{
    "intervalSeconds": {interval_seconds},
    "startupRefresh": {startup_refresh},
    "allowStaleCacheFallback": true
  }},
  "providers": {providers_json},
  "browserImport": {{
    "enabled": false,
    "policy": "off",
    "profileIdAllowlist": [],
    "domainAllowlistMode": "provider_required_only"
  }},
  "diagnostics": {{
    "verbosity": "normal",
    "keepRedactedArtifacts": false
  }}
}}"#
    );
    fs::write(config_dir.join("config.json"), config).expect("config file");
}

async fn wait_for_proxy<'a>(connection: &'a zbus::Connection) -> Proxy<'a> {
    for _ in 0..50 {
        if let Ok(proxy) = Proxy::new(connection, BUS_NAME, OBJECT_PATH, INTERFACE).await {
            if call_string_no_autostart(&proxy, "GetDaemonInfo", &())
                .await
                .is_ok()
            {
                return proxy;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("daemon did not become ready on D-Bus");
}

async fn call_string_no_autostart<B>(
    proxy: &Proxy<'_>,
    method_name: &str,
    body: &B,
) -> zbus::Result<String>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
{
    proxy
        .call_with_flags::<_, _, String>(method_name, MethodFlags::NoAutoStart.into(), body)
        .await?
        .ok_or_else(|| zbus::Error::Failure("D-Bus call unexpectedly had no reply".to_string()))
}

fn isolated_session_bus_available(test_name: &str) -> bool {
    let Some(address) = std::env::var_os("DBUS_SESSION_BUS_ADDRESS") else {
        eprintln!("skipping {test_name}: DBUS_SESSION_BUS_ADDRESS is not set");
        return false;
    };
    let address = address.to_string_lossy();
    if std::env::var_os(ISOLATED_DBUS_ENV).is_some()
        || session_bus_address_is_isolated(address.as_ref())
    {
        return true;
    }
    eprintln!(
        "skipping {test_name}: refusing to use ambient user session bus {address:?}; run under dbus-run-session"
    );
    false
}

fn session_bus_address_is_isolated(address: &str) -> bool {
    !address.contains("/run/user/")
}

fn assert_dbus_test_paths_are_isolated(daemon_info_json: &str, tmp: &TempDir) {
    let info: serde_json::Value = serde_json::from_str(daemon_info_json).expect("daemon info json");
    let config_file = info["paths"]["configFile"].as_str().expect("config path");
    let cache_file = info["paths"]["cacheFile"].as_str().expect("cache path");
    let tmp_display = tmp.path().display().to_string();
    assert!(
        !config_file.contains(&tmp_display),
        "D-Bus daemon info must not expose raw temp config path: {config_file}"
    );
    assert!(
        !cache_file.contains(&tmp_display),
        "D-Bus daemon info must not expose raw temp cache path: {cache_file}"
    );
    common::assert_public_json_safe(daemon_info_json);
}

fn method_error_name(err: &zbus::Error) -> String {
    match err {
        zbus::Error::MethodError(name, _, _) => name.as_str().to_string(),
        other => panic!("expected D-Bus method error, got {other:?}"),
    }
}

fn assert_live_upstream_snapshot(label: &str, snapshot_json: &str) {
    common::assert_schema("snapshot.schema.json", snapshot_json);
    common::assert_public_json_safe(snapshot_json);
    common::assert_no_live_secret_markers(label, snapshot_json);
    let snapshot: serde_json::Value = serde_json::from_str(snapshot_json).expect("snapshot json");
    let providers = snapshot["providers"].as_array().expect("providers array");
    let codex = providers
        .iter()
        .find(|provider| provider["provider"] == "codex")
        .expect("codex provider present");
    assert_eq!(codex["state"], "ok");
    assert_eq!(codex["sourceAdapter"], "upstream_cli");
    assert_eq!(codex["source"], "local");
}
