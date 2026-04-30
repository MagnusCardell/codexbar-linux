mod common;

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use futures_util::StreamExt;
use tempfile::TempDir;
use tokio::time::timeout;
use zbus::Proxy;

const BUS_NAME: &str = "org.codexbar.Linux1";
const OBJECT_PATH: &str = "/org/codexbar/Linux1";
const INTERFACE: &str = "org.codexbar.Linux1";
const FIXTURE_REFRESH_OPTIONS_JSON: &str = r#"{"schemaVersion":1,"reason":"test","force":true,"sourceAdapterPolicy":{"mode":"only","adapters":["fixture"]}}"#;

struct DaemonChild {
    child: Child,
}

impl Drop for DaemonChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dbus_contract_runtime_methods_signals_errors_and_cache() {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        eprintln!("skipping D-Bus contract test outside dbus-run-session");
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let mut daemon = spawn_daemon(&tmp);
    let connection = zbus::Connection::session().await.expect("session bus");
    let proxy = wait_for_proxy(&connection).await;

    let daemon_info: String = proxy
        .call("GetDaemonInfo", &())
        .await
        .expect("GetDaemonInfo");
    common::assert_schema("daemon-info.schema.json", &daemon_info);

    let snapshot: String = proxy.call("GetSnapshot", &()).await.expect("GetSnapshot");
    common::assert_schema("snapshot.schema.json", &snapshot);

    let diagnostics: String = proxy
        .call("GetDiagnostics", &"global")
        .await
        .expect("GetDiagnostics");
    common::assert_schema("diagnostics.schema.json", &diagnostics);

    let settings: String = proxy
        .call(
            "SetSettingsPatch",
            &r#"{"schemaVersion":1,"refresh":{"intervalSeconds":240}}"#,
        )
        .await
        .expect("SetSettingsPatch");
    common::assert_schema("settings.schema.json", &settings);

    let browser_result: String = proxy
        .call(
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

    let refresh_id: String = proxy
        .call("Refresh", &FIXTURE_REFRESH_OPTIONS_JSON)
        .await
        .expect("Refresh");
    assert!(!refresh_id.is_empty());

    let busy_err = proxy
        .call::<_, _, String>(
            "Refresh",
            &r#"{"schemaVersion":1,"reason":"test","busyBehavior":"reject"}"#,
        )
        .await
        .expect_err("busy refresh should reject");
    assert_eq!(
        method_error_name(&busy_err),
        "org.codexbar.Linux1.Error.RefreshBusy"
    );

    let started_msg = timeout(Duration::from_secs(3), started_stream.next())
        .await
        .expect("RefreshStarted timeout")
        .expect("RefreshStarted message");
    let started_id: String = started_msg.body().deserialize().expect("started body");
    assert_eq!(started_id, refresh_id);

    let provider_msg = timeout(Duration::from_secs(3), provider_stream.next())
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

    let snapshot_msg = timeout(Duration::from_secs(3), snapshot_stream.next())
        .await
        .expect("SnapshotChanged timeout")
        .expect("SnapshotChanged message");
    let changed_snapshot: String = snapshot_msg.body().deserialize().expect("snapshot body");
    common::assert_schema("snapshot.schema.json", &changed_snapshot);

    let finished_msg = timeout(Duration::from_secs(3), finished_stream.next())
        .await
        .expect("RefreshFinished timeout")
        .expect("RefreshFinished message");
    let (finished_id, result_json): (String, String) =
        finished_msg.body().deserialize().expect("finished body");
    assert_eq!(finished_id, refresh_id);
    common::assert_schema("refresh-result.schema.json", &result_json);

    let invalid_err = proxy
        .call::<_, _, String>("Refresh", &"{")
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
    let cached_snapshot: String = proxy
        .call("GetSnapshot", &())
        .await
        .expect("cached GetSnapshot");
    common::assert_schema("snapshot.schema.json", &cached_snapshot);
    let cached_value: serde_json::Value =
        serde_json::from_str(&cached_snapshot).expect("cached snapshot json");
    assert_eq!(cached_value["stale"], true);
    drop(daemon);
}

fn spawn_daemon(tmp: &TempDir) -> DaemonChild {
    let bin = env!("CARGO_BIN_EXE_codexbar-linuxd");
    let child = Command::new(bin)
        .env("XDG_CACHE_HOME", tmp.path().join("cache"))
        .env("XDG_CONFIG_HOME", tmp.path().join("config"))
        .env("HOME", tmp.path().join("home"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    DaemonChild { child }
}

async fn wait_for_proxy<'a>(connection: &'a zbus::Connection) -> Proxy<'a> {
    for _ in 0..50 {
        if let Ok(proxy) = Proxy::new(connection, BUS_NAME, OBJECT_PATH, INTERFACE).await {
            if proxy
                .call::<_, _, String>("GetDaemonInfo", &())
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

fn method_error_name(err: &zbus::Error) -> String {
    match err {
        zbus::Error::MethodError(name, _, _) => name.as_str().to_string(),
        other => panic!("expected D-Bus method error, got {other:?}"),
    }
}
