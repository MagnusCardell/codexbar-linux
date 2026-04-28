# ADR 0005 — P0A contract freeze

## Status

Accepted.

## Context

The kickoff review found that the repository had enough product direction to bootstrap, but not enough contract detail for safe daemon/UI implementation. The highest-risk gaps were JSON payload shape, identity redaction, settings ownership, source taxonomy, refresh concurrency, diagnostics, and `ProviderChanged` semantics.

## Decision

Freeze v1 contracts before Task 01 implementation:

- GSettings owns Shell presentation preferences.
- Daemon JSON settings own provider/browser/refresh/diagnostics configuration.
- Production Shell code does not read daemon cache files.
- Snapshots split provider semantic `source` from implementation `sourceAdapter`.
- Raw emails, organizations, provider account IDs, cookies, tokens, headers, browser profile paths, and provider payloads are prohibited from snapshots/cache/D-Bus/logs/fixtures.
- `Refresh` does not queue in v1. Busy refreshes return the active id by default, or raise `RefreshBusy` if the caller requests rejection.
- `ProviderChanged` carries a provider event containing a full normalized provider object, not a partial patch.
- D-Bus error names are stable and documented.
- Diagnostics are schema-backed and redaction-safe.
- `SetSettingsPatch` uses a schema-backed partial update contract, not a free-form patch.
- `TestBrowserImport` has a schema-backed input contract and may return a safe `not_implemented` result before browser import lands.
- Cached stale snapshots are served through the daemon over D-Bus; production Shell code never reads cache files.
- Cost data in snapshots is a bounded redacted summary, not an arbitrary upstream payload.

## Consequences

Task 00 can remain a neutral bootstrap. Task 01 and Task 03 can now implement typed Rust/GJS code against stable payloads. The v1 contracts are stricter than a convenience prototype, but they prevent the Shell and daemon from growing incompatible assumptions or leaking identity/secrets.

Future contract changes require updating the XML/schema/docs/fixtures/tests together.
