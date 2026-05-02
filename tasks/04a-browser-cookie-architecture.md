# Task 04A - Browser-Cookie Architecture Freeze

## Goal

Freeze the architecture, threat model, provider priority, browser support
matrix, diagnostics taxonomy, schema review, dependency review, and follow-up
implementation plan for Linux browser-cookie import and daemon web-fetch
adapters.

## Scope

Allowed:

- architecture docs;
- threat model docs;
- ADRs;
- provider-priority decisions;
- browser support matrix;
- profile discovery design;
- cookie decryption/keyring design;
- network/web-fetch design;
- diagnostics taxonomy;
- test strategy;
- schema review and proposed schema changes when strictly required;
- task breakdown for future implementation;
- small docs/spec consistency checks.

## Forbidden Work

- no browser-cookie reads;
- no SQLite cookie DB access;
- no libsecret/keyring calls;
- no browser profile discovery implementation;
- no provider network calls;
- no HTTP client implementation;
- no web scraping;
- no daemon runtime behavior changes;
- no Shell behavior changes;
- no D-Bus XML changes unless explicitly justified;
- no JSON schema changes unless this task proves a blocker;
- no production dependencies.

## Expected Files

- `docs/browser-cookie-architecture.md`;
- `docs/browser-cookie-threat-model.md`;
- `docs/adr/0006-linux-browser-cookie-daemon-layer.md`;
- `docs/provider-roadmap.md`;
- `docs/browser-support.md`;
- `tasks/04a-browser-cookie-architecture.md`;
- `tasks/04b-chromium-cookie-import.md`;
- `tasks/04c-firefox-cookie-import.md`;
- `tasks/04d-codex-web-adapter.md`;
- `tasks/04e-browser-import-hardening.md`;
- `README.md` status update.

## Tests Required

No new runtime tests are required if only docs/tasks change. The full validation
gate must still run to prove no regression.

## Acceptance Criteria

- Architecture doc freezes daemon-only ownership and Shell non-involvement.
- Threat model covers cookies, decrypted values, tokens, identity, profile
  paths, raw provider responses, D-Bus, cache/config, logs, diagnostics, and
  fixtures.
- ADR rejects Shell cookie reads, browser-extension-first architecture,
  localhost bridge, raw cookie persistence, literal macOS behavior, and
  all-provider implementation first.
- Provider roadmap chooses Codex/OpenAI as pilot and does not promise every
  upstream provider.
- Browser support doc covers Chrome, Brave, Chromium, and Firefox.
- Schema review documents whether changes are required.
- Diagnostic code registry is stable and redaction-safe.
- Follow-up tasks define scope, forbidden work, files/modules, tests,
  acceptance, and checks.
- No runtime behavior changes are present.
- No raw cookies, tokens, browser profile paths, provider payloads, or raw
  identities are added.

## Checks To Run

```bash
./scripts/validate-dbus.sh
./scripts/validate-schemas.sh
./scripts/test-fixtures.sh
./scripts/validate-upstream-cli-fixtures.sh
./scripts/test-upstream-cli-capture.sh
./scripts/validate-gsettings.sh
./scripts/validate-packaging.sh
./scripts/lint-gjs.sh
./scripts/check.sh
cargo fmt --manifest-path daemon/Cargo.toml -- --check
cargo clippy --manifest-path daemon/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path daemon/Cargo.toml
dbus-run-session -- cargo test --manifest-path daemon/Cargo.toml dbus_contract
cargo run --manifest-path daemon/Cargo.toml -- --check
git diff --check
```
