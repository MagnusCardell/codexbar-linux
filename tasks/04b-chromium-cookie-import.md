# Task 04B - Chromium-Family Cookie Import

## Goal

Implement daemon-side Chromium-family profile discovery and cookie extraction
against fake/test profiles first.

## Scope

Allowed:

- daemon-only browser modules;
- synthetic Chrome/Chromium/Brave profile fixtures;
- SQLite cookie DB parsing in tests;
- private temp DB copy handling;
- fake keyring backend;
- Chromium-family decryption abstraction;
- redaction-safe diagnostics;
- `TestBrowserImport` implementation for browser/cookie capability testing;
- no provider network calls except synthetic local fixtures if needed.

## Forbidden Work

- no Shell browser/profile/cookie/keyring access;
- no raw cookie persistence;
- no raw D-Bus/cache/log/diagnostics/fixture cookie output;
- no real browser profile mutation;
- no provider scraping;
- no Firefox support;
- no live provider endpoints;
- no interactive keyring prompt UX unless separately approved.

## Expected Files/Modules

Suggested future layout:

- `daemon/src/browser/mod.rs`;
- `daemon/src/browser/profile.rs`;
- `daemon/src/browser/chromium.rs`;
- `daemon/src/browser/cookie_store.rs`;
- `daemon/src/browser/keyring.rs`;
- `daemon/src/browser/redact.rs`;
- `daemon/tests/browser_chromium.rs`;
- `daemon/tests/browser_import_redaction.rs`;
- `daemon/fixtures/browser/chromium/*` synthetic only.

The exact layout may differ if it follows established daemon patterns.

## Tests Required

- synthetic DB with plaintext/no-encryption rows where applicable;
- synthetic DB with encrypted-value fixture rows;
- decrypt success through fake keyring;
- keyring locked/unavailable;
- decryption failure;
- cookie absent;
- cookie found and aggregate count only;
- malformed/corrupt DB;
- locked/unreadable DB temp-copy path;
- WAL/SHM companion copy behavior where needed;
- profile ID allowlist enforcement;
- path-like profile ID rejection;
- temp file permissions `0600` and temp directory permissions `0700`;
- no raw cookie serialization in cache, D-Bus, logs, diagnostics, or fixtures.

## Acceptance Criteria

- `TestBrowserImport` no longer returns `not_implemented` for Chromium-family
  test fixtures.
- Real user profile discovery remains disabled or gated until live approval.
- Cookie values never leave daemon memory.
- Browser profile paths never leave daemon internals.
- Diagnostics use the Task 04A code registry.
- Provider statuses distinguish at least missing profile, keyring unavailable,
  cookie missing, decrypt failure, and success using existing schema fields plus
  diagnostic codes.
- No provider web fetches are implemented.

## Checks To Run

```bash
./scripts/check.sh
cargo fmt --manifest-path daemon/Cargo.toml -- --check
cargo clippy --manifest-path daemon/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path daemon/Cargo.toml
dbus-run-session -- cargo test --manifest-path daemon/Cargo.toml dbus_contract
git diff --check
```
