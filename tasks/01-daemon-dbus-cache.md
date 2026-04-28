# Task 01 — Daemon D-Bus and cache vertical slice

## Agent

`daemon_engineer`.

## Goal

Implement a daemon that serves fixture snapshots over D-Bus and persists normalized cache.

## Scope

- Implement D-Bus service `org.codexbar.Linux1` at `/org/codexbar/Linux1`.
- Implement `GetSnapshot`, `Refresh`, `GetDaemonInfo` with fixture data.
- Implement cache read/write with atomic write and permissions.
- Emit `SnapshotChanged`, `RefreshStarted`, `RefreshFinished`.
- Validate snapshots against `spec/snapshot.schema.json` in tests.

## Constraints

- No upstream CLI invocation yet.
- No browser-cookie import.
- D-Bus outputs must be redaction-safe.

## Acceptance

- Test client can call `GetSnapshot` and receive valid JSON.
- Cache survives daemon restart.
- `Refresh` changes timestamp and emits signals.

## Contract references

Read `docs/CONTRACTS.md`, `docs/adr/0005-p0a-contract-freeze.md`, and all relevant `spec/*.schema.json` before changing behavior. Do not contradict the P0A source taxonomy, identity redaction rules, refresh semantics, settings ownership, or Shell/daemon boundary.

## P0A-specific daemon requirements

- Internally use typed Rust structs; serialize JSON strings only at the D-Bus edge.
- `GetSnapshot()` returns current snapshot, cached snapshot, or a minimal valid synthetic/error snapshot.
- `Refresh()` implements P0A busy semantics: return active id by default, reject only when requested.
- `RefreshFinished` returns `spec/refresh-result.schema.json`.
- `ProviderChanged` returns `spec/provider-event.schema.json` with a full provider object.
- Implement `GetDiagnostics`, `GetDaemonInfo`, `SetSettingsPatch`, and `TestBrowserImport` as schema-valid stubs if future behavior is not implemented yet. Validate `SetSettingsPatch` input against `spec/settings-patch.schema.json` and `TestBrowserImport` input against `spec/browser-import-options.schema.json`. Use `CapabilityUnimplemented` only for operations that cannot return a safe schema-valid stub.
- Cache writes use 0700 directory, 0600 file, temp write, flush, rename, and best-effort directory fsync.
- No raw identity, secrets, headers, raw provider payloads, or browser profile paths may enter D-Bus/cache/logs/tests.
