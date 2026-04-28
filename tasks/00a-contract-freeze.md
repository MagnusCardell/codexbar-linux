# Task 00A — Contract freeze addendum

## Agent

`architecture_guardian` with review by `daemon_engineer`, `gnome_shell_engineer`, and `qa_security_reviewer`.

## Goal

Land the missing P0 contracts before Task 01 or Task 03 implementation starts.

## Scope

- Review `docs/CONTRACTS.md` and `docs/adr/0005-p0a-contract-freeze.md`.
- Confirm `spec/dbus-org.codexbar.Linux1.xml` uses `provider_event_json` for `ProviderChanged`.
- Confirm these schemas parse and are internally coherent:
  - `spec/snapshot.schema.json`
  - `spec/settings.schema.json`
  - `spec/settings-patch.schema.json`
  - `spec/refresh-options.schema.json`
  - `spec/refresh-result.schema.json`
  - `spec/daemon-info.schema.json`
  - `spec/diagnostics.schema.json`
  - `spec/browser-import-options.schema.json`
  - `spec/browser-import-result.schema.json`
  - `spec/provider-event.schema.json`
- Align Task 00 through Task 03 language with the freeze.
- Keep the work read/write limited to contracts, docs, tasks, scripts, and fixtures. Do not implement daemon/provider/UI logic here.

## Frozen decisions

- GSettings owns Shell UI preferences.
- Daemon settings own provider/browser/refresh/diagnostics configuration.
- Production Shell code must not read daemon cache files.
- Snapshots use `source` plus `sourceAdapter`.
- Raw identity and secrets are prohibited from D-Bus/cache/logs/fixtures.
- Refresh busy behavior defaults to `return_existing`; no v1 queue.
- `ProviderChanged` carries a full provider event, not a partial provider patch.
- Diagnostics are schema-backed and redaction-safe.
- `SetSettingsPatch` and `TestBrowserImport` inputs are schema-backed.
- Cost summaries are bounded and redaction-safe.

## Acceptance

- `./scripts/validate-dbus.sh` passes.
- `./scripts/validate-schemas.sh` parses every `spec/*.json` schema.
- `./scripts/test-fixtures.sh` validates every shared snapshot fixture against `spec/snapshot.schema.json`.
- `docs/CONTRACTS.md` is cited by Task 01 and Task 03.
- No file under `fixtures/` contains obvious secret-shaped strings, raw cookies, `Authorization`, `Set-Cookie`, or unmasked email addresses.
- Final agent response lists files changed, checks run, and any residual contract risks.
