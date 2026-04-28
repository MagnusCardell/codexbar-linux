use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use codexbar_linuxd::app::{App, RefreshStart};
use codexbar_linuxd::cache::validate_snapshot;
use codexbar_linuxd::error::AppError;
use codexbar_linuxd::model::{
    BrowserImportResult, BrowserImportStatus, DaemonInfo, ProviderEvent, ProviderState,
    RefreshResult, Settings, Snapshot,
};
use codexbar_linuxd::paths::AppPaths;
use codexbar_linuxd::{DBUS_INTERFACE, DBUS_OBJECT_PATH};
use zbus::DBusError;

const REQUIRED_SCHEMAS: &[&str] = &[
    "browser-import-options.schema.json",
    "browser-import-result.schema.json",
    "daemon-info.schema.json",
    "diagnostics.schema.json",
    "provider-event.schema.json",
    "refresh-options.schema.json",
    "refresh-result.schema.json",
    "settings-patch.schema.json",
    "settings.schema.json",
    "snapshot.schema.json",
];

const REQUIRED_METHODS: &[&str] = &[
    "GetSnapshot",
    "Refresh",
    "GetDiagnostics",
    "GetDaemonInfo",
    "SetSettingsPatch",
    "TestBrowserImport",
];

const REQUIRED_SIGNALS: &[&str] = &[
    "SnapshotChanged",
    "RefreshStarted",
    "RefreshFinished",
    "ProviderChanged",
];

#[test]
fn dbus_contract_has_expected_interface_object_methods_and_signals() {
    let xml = read_repo_file("spec/dbus-org.codexbar.Linux1.xml");

    assert_contains(&xml, &format!("<node name=\"{DBUS_OBJECT_PATH}\">"));
    assert_contains(&xml, &format!("<interface name=\"{DBUS_INTERFACE}\">"));

    for method in REQUIRED_METHODS {
        assert_contains(&xml, &format!("<method name=\"{method}\">"));
    }

    for signal in REQUIRED_SIGNALS {
        assert_contains(&xml, &format!("<signal name=\"{signal}\">"));
    }

    assert_contains(&xml, "<arg name=\"provider_event_json\" type=\"s\"/>");
    assert_contains(&xml, "<arg name=\"provider_id\" type=\"s\"/>");
}

#[test]
fn required_json_schema_files_are_present_and_parse() {
    for schema in REQUIRED_SCHEMAS {
        let path = repo_path("spec").join(schema);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        JsonParser::new(&text)
            .parse()
            .unwrap_or_else(|err| panic!("failed to parse {} as JSON: {err}", path.display()));
    }
}

#[test]
fn dbus_error_names_match_frozen_contract() {
    let cases = [
        (
            AppError::invalid_json(),
            "org.codexbar.Linux1.Error.InvalidJson",
        ),
        (
            AppError::invalid_settings_patch("rejected"),
            "org.codexbar.Linux1.Error.InvalidSettingsPatch",
        ),
        (
            AppError::refresh_busy("refresh-test"),
            "org.codexbar.Linux1.Error.RefreshBusy",
        ),
        (
            AppError::DependencyUnavailable("missing".to_string()),
            "org.codexbar.Linux1.Error.DependencyUnavailable",
        ),
        (
            AppError::CapabilityUnimplemented("not implemented".to_string()),
            "org.codexbar.Linux1.Error.CapabilityUnimplemented",
        ),
        (
            AppError::internal_redacted(),
            "org.codexbar.Linux1.Error.Internal",
        ),
    ];
    for (err, name) in cases {
        assert_eq!(err.name().to_string(), name);
    }
}

#[test]
fn snapshot_fixtures_parse_validate_and_are_redaction_safe() {
    let snapshot_dir = repo_path("fixtures/snapshots");
    for entry in fs::read_dir(&snapshot_dir).expect("fixture snapshot dir") {
        let path = entry.expect("fixture entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        let snapshot: Snapshot = serde_json::from_str(&text)
            .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()));
        validate_snapshot(&snapshot)
            .unwrap_or_else(|err| panic!("snapshot fixture {} invalid: {err}", path.display()));

        let expected_state = path.file_stem().and_then(|stem| stem.to_str()).unwrap();
        let states: Vec<String> = snapshot
            .providers
            .iter()
            .map(|provider| {
                serde_json::to_string(&provider.state)
                    .unwrap()
                    .trim_matches('"')
                    .to_string()
            })
            .collect();
        assert!(
            states.iter().any(|state| state == expected_state),
            "snapshot fixture {} should include provider state {expected_state}",
            path.display()
        );
    }
}

#[test]
fn app_getters_return_schema_shaped_redacted_json() {
    let (_tmp, paths) = temp_paths();
    let app = App::new(paths).expect("app starts");

    let snapshot: Snapshot =
        serde_json::from_str(&app.get_snapshot_json().expect("snapshot json")).unwrap();
    validate_snapshot(&snapshot).expect("snapshot validates");

    let info: DaemonInfo =
        serde_json::from_str(&app.get_daemon_info_json().expect("daemon info json")).unwrap();
    assert_eq!(info.schema_version, 1);
    assert_eq!(info.dbus.interface, DBUS_INTERFACE);
    assert!(!info.capabilities.upstream_cli);
    assert!(info.capabilities.settings_patch);

    let diagnostics: serde_json::Value =
        serde_json::from_str(&app.get_diagnostics_json("").expect("diagnostics json")).unwrap();
    assert_eq!(diagnostics["schemaVersion"], 1);
    assert_eq!(diagnostics["redaction"]["applied"], true);
}

#[test]
fn refresh_writes_cache_and_restart_serves_stale_snapshot() {
    let (_tmp, paths) = temp_paths();
    let app = App::new(paths.clone()).expect("app starts");
    let start = app
        .start_refresh(r#"{"schemaVersion":1,"reason":"test","force":true}"#)
        .expect("refresh starts");
    let refresh_id = match start {
        RefreshStart::Started { refresh_id } => refresh_id,
        RefreshStart::Existing { .. } => panic!("first refresh should start"),
    };
    std::thread::sleep(std::time::Duration::from_millis(2));
    let completion = app.finish_refresh(&refresh_id).expect("refresh finishes");

    let result: RefreshResult = serde_json::from_str(&completion.result_json).unwrap();
    assert_eq!(result.schema_version, 1);
    assert_eq!(result.refresh_id, refresh_id);
    assert!(result.cache_written);

    for (_provider_id, event_json) in completion.provider_events {
        let event: ProviderEvent = serde_json::from_str(&event_json).unwrap();
        assert_eq!(event.provider_id, event.provider.provider);
    }

    assert!(app.cache_file_path().is_file());
    assert_mode(app.cache_file_path(), 0o600);
    assert_mode(app.cache_file_path().parent().unwrap(), 0o700);

    let restarted = App::new(paths).expect("app restarts");
    let snapshot: Snapshot = serde_json::from_str(
        &restarted
            .get_snapshot_json()
            .expect("snapshot after restart"),
    )
    .unwrap();
    assert!(snapshot.stale);
    assert_eq!(
        snapshot.daemon.state,
        codexbar_linuxd::model::DaemonState::Degraded
    );
    assert_eq!(snapshot.providers[0].state, ProviderState::Stale);
}

#[test]
fn refresh_busy_semantics_return_existing_or_reject() {
    let (_tmp, paths) = temp_paths();
    let app = App::new(paths).expect("app starts");
    let first = app
        .start_refresh(r#"{"schemaVersion":1,"reason":"manual"}"#)
        .expect("first refresh starts");
    let refresh_id = match first {
        RefreshStart::Started { refresh_id } => refresh_id,
        RefreshStart::Existing { .. } => panic!("first refresh should start"),
    };

    let existing = app
        .start_refresh(r#"{"schemaVersion":1,"busyBehavior":"return_existing"}"#)
        .expect("return_existing succeeds");
    match existing {
        RefreshStart::Existing {
            refresh_id: existing,
        } => assert_eq!(existing, refresh_id),
        RefreshStart::Started { .. } => panic!("busy refresh should not start another refresh"),
    }

    let rejected = app
        .start_refresh(r#"{"schemaVersion":1,"busyBehavior":"reject"}"#)
        .expect_err("reject returns RefreshBusy");
    assert!(matches!(rejected, AppError::RefreshBusy(_)));

    app.finish_refresh(&refresh_id).expect("cleanup refresh");
}

#[test]
fn settings_patch_validates_persists_and_rejects_invalid_json() {
    let (_tmp, paths) = temp_paths();
    let app = App::new(paths.clone()).expect("app starts");
    let settings_json = app
        .set_settings_patch_json(
            r#"{"schemaVersion":1,"refresh":{"intervalSeconds":300},"providers":{"codex":{"enabled":true,"preferredSourceAdapter":"auto"}}}"#,
        )
        .expect("settings patch applies");
    let settings: Settings = serde_json::from_str(&settings_json).unwrap();
    assert_eq!(settings.refresh.interval_seconds, 300);
    assert!(app.settings_file_path().is_file());
    assert_mode(app.settings_file_path(), 0o600);
    assert_mode(app.settings_file_path().parent().unwrap(), 0o700);

    let restarted = App::new(paths).expect("app restarts");
    let persisted_json = restarted
        .set_settings_patch_json(r#"{"schemaVersion":1}"#)
        .expect("empty patch returns persisted settings");
    let persisted: Settings = serde_json::from_str(&persisted_json).unwrap();
    assert_eq!(persisted.refresh.interval_seconds, 300);

    let invalid = restarted
        .set_settings_patch_json(
            r#"{"schemaVersion":1,"browserImport":{"profileIdAllowlist":["/home/maca/profile"]}}"#,
        )
        .expect_err("absolute profile path rejected");
    assert!(matches!(invalid, AppError::InvalidJson(_)));
}

#[test]
fn browser_import_stub_is_schema_valid_and_does_not_probe_browser_state() {
    let (_tmp, paths) = temp_paths();
    let app = App::new(paths).expect("app starts");
    let result_json = app
        .test_browser_import_json(
            r#"{"schemaVersion":1,"providers":["codex"],"profileIds":["Profile-1"]}"#,
        )
        .expect("browser import stub");
    let result: BrowserImportResult = serde_json::from_str(&result_json).unwrap();
    assert_eq!(result.schema_version, 1);
    assert_eq!(result.status, BrowserImportStatus::NotImplemented);
    assert!(result.profiles.is_empty());
    assert_eq!(result.providers[0].provider, "codex");

    let invalid = app
        .test_browser_import_json(r#"{"schemaVersion":1,"profileIds":["/home/maca/profile"]}"#)
        .expect_err("absolute profile path rejected");
    assert!(matches!(invalid, AppError::InvalidJson(_)));
}

fn read_repo_file(relative_path: &str) -> String {
    let path = repo_path(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

fn repo_path(relative_path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative_path)
}

fn temp_paths() -> (tempfile::TempDir, AppPaths) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config_dir = tmp.path().join("config").join("codexbar-linux");
    let cache_dir = tmp.path().join("cache").join("codexbar-linux");
    let paths = AppPaths {
        config_file: config_dir.join("config.json"),
        cache_file: cache_dir.join("snapshot.json"),
        config_dir,
        cache_dir,
        upstream_config_file_hint: Some("~/.codexbar/config.json".to_string()),
    };
    (tmp, paths)
}

fn assert_mode(path: &Path, expected: u32) {
    let mode = fs::metadata(path)
        .unwrap_or_else(|err| panic!("metadata {}: {err}", path.display()))
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, expected, "unexpected mode for {}", path.display());
}

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "expected contract to contain {needle:?}"
    );
}

struct JsonParser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    fn parse(&mut self) -> Result<(), String> {
        self.skip_ws();
        self.parse_value()?;
        self.skip_ws();
        if self.pos == self.input.len() {
            Ok(())
        } else {
            Err(format!("trailing data at byte {}", self.pos))
        }
    }

    fn parse_value(&mut self) -> Result<(), String> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => self.parse_string(),
            Some(b't') => self.expect_literal(b"true"),
            Some(b'f') => self.expect_literal(b"false"),
            Some(b'n') => self.expect_literal(b"null"),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(byte) => Err(format!(
                "unexpected byte {:?} at byte {}",
                byte as char, self.pos
            )),
            None => Err("unexpected end of input".to_string()),
        }
    }

    fn parse_object(&mut self) -> Result<(), String> {
        self.expect_byte(b'{')?;
        self.skip_ws();
        if self.consume_byte(b'}') {
            return Ok(());
        }
        loop {
            self.skip_ws();
            self.parse_string()?;
            self.skip_ws();
            self.expect_byte(b':')?;
            self.parse_value()?;
            self.skip_ws();
            if self.consume_byte(b'}') {
                return Ok(());
            }
            self.expect_byte(b',')?;
        }
    }

    fn parse_array(&mut self) -> Result<(), String> {
        self.expect_byte(b'[')?;
        self.skip_ws();
        if self.consume_byte(b']') {
            return Ok(());
        }
        loop {
            self.parse_value()?;
            self.skip_ws();
            if self.consume_byte(b']') {
                return Ok(());
            }
            self.expect_byte(b',')?;
        }
    }

    fn parse_string(&mut self) -> Result<(), String> {
        self.expect_byte(b'"')?;
        while let Some(byte) = self.next() {
            match byte {
                b'"' => return Ok(()),
                b'\\' => self.parse_escape()?,
                0x00..=0x1f => {
                    return Err(format!("control character in string at byte {}", self.pos));
                }
                _ => {}
            }
        }
        Err("unterminated string".to_string())
    }

    fn parse_escape(&mut self) -> Result<(), String> {
        match self.next() {
            Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => Ok(()),
            Some(b'u') => {
                for _ in 0..4 {
                    match self.next() {
                        Some(byte) if byte.is_ascii_hexdigit() => {}
                        _ => return Err(format!("invalid unicode escape at byte {}", self.pos)),
                    }
                }
                Ok(())
            }
            _ => Err(format!("invalid string escape at byte {}", self.pos)),
        }
    }

    fn parse_number(&mut self) -> Result<(), String> {
        self.consume_byte(b'-');
        match self.peek() {
            Some(b'0') => {
                self.pos += 1;
            }
            Some(b'1'..=b'9') => {
                self.consume_digits();
            }
            _ => return Err(format!("invalid number at byte {}", self.pos)),
        }
        if self.consume_byte(b'.') && !self.consume_digits() {
            return Err(format!("invalid fraction at byte {}", self.pos));
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !self.consume_digits() {
                return Err(format!("invalid exponent at byte {}", self.pos));
            }
        }
        Ok(())
    }

    fn consume_digits(&mut self) -> bool {
        let start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        self.pos > start
    }

    fn expect_literal(&mut self, literal: &[u8]) -> Result<(), String> {
        if self.input.get(self.pos..self.pos + literal.len()) == Some(literal) {
            self.pos += literal.len();
            Ok(())
        } else {
            Err(format!("expected literal at byte {}", self.pos))
        }
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), String> {
        match self.next() {
            Some(byte) if byte == expected => Ok(()),
            _ => Err(format!(
                "expected {:?} at byte {}",
                expected as char, self.pos
            )),
        }
    }

    fn consume_byte(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.pos += 1;
        Some(byte)
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }
}
