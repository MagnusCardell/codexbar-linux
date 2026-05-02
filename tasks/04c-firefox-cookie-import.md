# Task 04C - Firefox Cookie Import

## Goal

Add daemon-side Firefox profile discovery and cookie extraction after the
Chromium-family path is stable.

## Scope

Allowed:

- daemon-only Firefox profile discovery;
- synthetic Firefox profile fixtures;
- `profiles.ini` and profile metadata parsing;
- SQLite `cookies.sqlite` parsing in tests;
- private temp DB copy handling;
- redaction-safe diagnostics;
- `TestBrowserImport` coverage for Firefox policy.

## Forbidden Work

- no Shell browser/profile/cookie access;
- no raw cookie persistence;
- no provider network calls;
- no web scraping;
- no Chromium refactor beyond shared abstractions needed by Firefox;
- no live user profile reads unless explicitly approved for manual smoke.

## Expected Files/Modules

Suggested future layout:

- `daemon/src/browser/firefox.rs`;
- shared `daemon/src/browser/profile.rs`;
- shared `daemon/src/browser/cookie_store.rs`;
- `daemon/tests/browser_firefox.rs`;
- `daemon/fixtures/browser/firefox/*` synthetic only.

## Tests Required

- synthetic `profiles.ini` with default and named profiles;
- synthetic `cookies.sqlite` success path;
- cookie missing path;
- container/origin attributes safely ignored or handled;
- corrupt DB;
- locked/unreadable DB temp-copy behavior;
- snap-style root fixture if supported by policy;
- profile path redaction;
- no raw cookie serialization.

## Acceptance Criteria

- Firefox import is daemon-only and policy-gated.
- Firefox results use existing `BrowserImportResult` and diagnostics schema.
- Firefox cookie values never leave daemon memory.
- No provider web fetches are implemented in this task.
- Chromium-family behavior remains unchanged.

## Checks To Run

```bash
./scripts/check.sh
cargo fmt --manifest-path daemon/Cargo.toml -- --check
cargo clippy --manifest-path daemon/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path daemon/Cargo.toml
dbus-run-session -- cargo test --manifest-path daemon/Cargo.toml dbus_contract
git diff --check
```
