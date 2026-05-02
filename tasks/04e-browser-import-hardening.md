# Task 04E - Browser Import Hardening

## Goal

Harden the browser-cookie and web-fetch implementation before broader provider
expansion.

## Scope

Allowed:

- redaction tests;
- diagnostics copy review;
- fixture validators for browser and provider-web artifacts;
- packaging/dependency review updates;
- GNOME live smoke instructions;
- optional live browser/keyring smoke scripts gated by explicit environment
  variables;
- documentation updates for known limitations.

## Forbidden Work

- no new providers;
- no provider scope broadening;
- no Shell browser/keyring/network access;
- no raw live capture promotion;
- no automatic extension enablement;
- no localhost/TCP API.

## Expected Files/Modules

Potential future additions:

- `scripts/test-browser-fixtures.sh`;
- `scripts/validate-provider-web-fixtures.sh`;
- `daemon/tests/browser_import_hardening.rs`;
- `daemon/tests/web_redaction.rs`;
- updates to `docs/gnome-smoke-test.md`;
- updates to `docs/SECURITY.md`;
- updates to `docs/browser-cookie-threat-model.md`;
- updates to packaging docs/scripts only when dependencies have landed.

## Tests Required

- redaction scanner coverage for cookie/header/token/profile path/raw payload
  strings;
- fixture validator rejects raw browser DBs and raw provider bodies;
- cache write validator rejects browser secrets;
- D-Bus return/signal payload safety tests;
- concurrent refresh busy behavior around web adapter;
- temp cleanup on success, failure, timeout, and cancellation;
- live smoke instructions for Ubuntu 24.04/GNOME 46 and Ubuntu 26.04 target
  when available.

## Acceptance Criteria

- No raw secrets in cache, D-Bus, logs, diagnostics, fixtures, screenshots, or
  copied UI output.
- Browser import capabilities are accurately reported.
- Provider failures do not overwrite useful cache.
- Diagnostics are useful and safe.
- Dependency and Debian packaging implications are documented.
- Live smoke is opt-in and does not commit live data.

## Checks To Run

```bash
./scripts/check.sh
./scripts/validate-packaging.sh
./scripts/lint-gjs.sh
cargo fmt --manifest-path daemon/Cargo.toml -- --check
cargo clippy --manifest-path daemon/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path daemon/Cargo.toml
dbus-run-session -- cargo test --manifest-path daemon/Cargo.toml dbus_contract
git diff --check
```
