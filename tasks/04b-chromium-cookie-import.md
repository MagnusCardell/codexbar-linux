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

## Task 04B Result

Implemented as a daemon-only synthetic/fake-root slice:

- `daemon/src/browser/` contains Chromium-family discovery, cookie DB temp-copy
  and read-only SQLite querying, fake decryptor states, redaction-safe
  diagnostic-code mapping, and memory-only session material.
- `TestBrowserImport` uses the existing JSON schema and D-Bus method. No D-Bus
  XML or JSON schema changes were required.
- Default runtime does not scan real browser roots. Tests inject
  `BrowserDiscoveryRoots`; development processes may opt into fake/throwaway
  roots with `CODEXBAR_BROWSER_IMPORT_FAKE_HOME`.
- Committed browser fixtures are text metadata/SQL only under
  `daemon/fixtures/browser/chromium/`; tests create throwaway SQLite DBs.
- Provider web fetches, real keyring access, Firefox import, live profile
  scanning by default, Shell changes, and TCP/localhost APIs remain out of
  scope.

## Task 04B.1 Result

Added opt-in live throwaway Chromium-family verification without changing the
Task 04B runtime contract:

- `scripts/chromium-throwaway-smoke.sh` refuses to run unless
  `CODEXBAR_BROWSER_LIVE=1` is set, creates a marked throwaway fake home, and
  launches Chrome/Chromium/Brave only with a throwaway user-data-dir.
- The smoke uses a local `127.0.0.1` test-only server and
  `smoke.example.invalid` host mapping to seed a synthetic cookie; it does not
  contact provider endpoints.
- The ignored `live_throwaway_browser_profile_smoke` integration test validates
  schema-safe `TestBrowserImport` output against the throwaway fake home and is
  excluded from default `cargo test`, `./scripts/check.sh`, and CI.
- `CODEXBAR_BROWSER_IMPORT_FAKE_HOME` now fails closed unless it is an
  absolute, canonical throwaway directory with
  `.codexbar-throwaway-browser-root`; real `$HOME`, real config descendants,
  `/`, empty, relative, and escaping roots are rejected.
- Cookie DB symlink escapes are skipped before temp-copy reads.
- Local Chrome verification observed legacy `Default/Cookies` with no WAL/SHM;
  `Default/Network/Cookies` was not observed in that throwaway run.

## Task 04B.2 Result

Implemented Chromium cookie material classification and safer session material
construction without changing D-Bus XML, JSON schemas, Shell code, or default
provider refresh behavior:

- Browser cookie material summaries report only safe counts/classes:
  discovered profiles, candidate rows, plaintext rows, encrypted rows,
  encrypted prefix counts, expired rows, usable in-memory cookies, decryptor
  backend class, and decryption status.
- Plaintext Chromium rows where `value` is populated and `encrypted_value` is
  empty are usable after provider domain/path validation and remain memory-only.
- Synthetic `v10` and `v11` encrypted rows are covered by fake decryptor tests.
  The production/env backend is `plain`, so encrypted rows fail closed with
  decryption/keyring diagnostics instead of fake-decrypting real browser data.
- Unknown encrypted prefixes map to
  `browser_cookie_decryption_unavailable` without claiming a keyring backend.
- A noninteractive Secret Service probe abstraction maps unavailable, locked,
  and prompt-required states. Real Secret Service secret extraction is deferred.
- If a provider-scoped encrypted row fails while other provider-scoped
  plaintext rows are present, the daemon discards the partial material and the
  Codex web fetch is not attempted.
- Live Codex recon summary now includes the safe cookie material summary in
  addition to normalized provider/refresh classification. It still prints no
  names, values, domains, headers, profile paths, SQL rows, or provider
  payloads.
- Browser fixture validation now rejects unexpected fixture sidecars,
  subdirectories, binary SQLite DB files, WAL/SHM companions, and browser-shaped
  profile/database names.

Observed live provider DB shape remains unknown for Task 04B.2 until the ignored
Codex throwaway recon is rerun against a signed-in throwaway profile. The
summary is designed to record whether `--password-store=basic` produced
plaintext rows or encrypted rows, and which encrypted prefix/backend state is
blocking any provider web fetch.

## Checks To Run

```bash
./scripts/check.sh
cargo fmt --manifest-path daemon/Cargo.toml -- --check
cargo clippy --manifest-path daemon/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path daemon/Cargo.toml
dbus-run-session -- cargo test --manifest-path daemon/Cargo.toml dbus_contract
git diff --check
```
