# Task 04D - Codex/OpenAI Web Adapter

## Goal

Implement the first daemon-only Linux web provider adapter using in-memory
browser-cookie session material from the browser import layer.

## Scope

Allowed:

- daemon-only bounded HTTP client wrapper;
- Codex/OpenAI provider-specific web adapter;
- provider-required domain and redirect allowlists;
- response-size limits and timeouts;
- redacted provider fixtures;
- parser tests;
- normalization into `spec/snapshot.schema.json`;
- stale cache fallback tests.

## Forbidden Work

- no Shell provider network calls;
- no raw HTTP body/cache exposure;
- no raw cookies, headers, tokens, profile paths, or provider payloads in public
  output;
- no broad provider framework beyond the minimal adapter boundary;
- no Claude/Cursor/OpenCode/Amp/Ollama/Abacus/Mistral work;
- no all-provider browser-cookie sweep;
- no live provider tests in default CI.

## Expected Files/Modules

Suggested future layout:

- `daemon/src/web/mod.rs`;
- `daemon/src/web/client.rs`;
- `daemon/src/web/policy.rs`;
- `daemon/src/web/normalize.rs`;
- `daemon/src/web/providers/codex.rs`;
- `daemon/tests/web_codex.rs`;
- `daemon/fixtures/web/codex/*` redacted or synthetic only.

## Tests Required

- success fixture normalizes to provider `ok`, `source="web"`,
  `sourceAdapter="linux_web"`;
- absent cookie maps to `unauthenticated`;
- cookie exists but provider rejects maps to `cookie_rejected`;
- provider unavailable maps to `provider_unavailable`;
- timeout maps to `timeout`;
- unexpected response maps to `parse_error`;
- redirect to wrong host is blocked;
- response-size cap is enforced;
- raw response body is not persisted or exposed;
- stale cache fallback preserves previous usable data;
- D-Bus payloads validate and pass redaction scans.

## Acceptance Criteria

- Codex/OpenAI web adapter returns normalized snapshots only.
- Cookie-found/provider-rejected/auth-expired states are distinguishable.
- No raw HTTP/cookie data is persisted or exposed.
- Stale cache fallback works.
- D-Bus payloads validate.
- Upstream CLI fallback policy respects settings and refresh options.

## Checks To Run

```bash
./scripts/check.sh
cargo fmt --manifest-path daemon/Cargo.toml -- --check
cargo clippy --manifest-path daemon/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path daemon/Cargo.toml
dbus-run-session -- cargo test --manifest-path daemon/Cargo.toml dbus_contract
cargo run --manifest-path daemon/Cargo.toml -- --check
git diff --check
```
