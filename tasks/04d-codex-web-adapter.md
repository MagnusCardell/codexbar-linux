# Task 04D - Codex/OpenAI Web Adapter

## Task 04D.0 Status

Complete as a fake-only skeleton slice. The daemon now has a `web` module with
a bounded request/response abstraction, a fake HTTP client, static Codex web
policy, redaction-safe provider web diagnostics, synthetic Codex fixtures, and
parser/normalizer tests against existing snapshot/diagnostics contracts.

Production `linux_web` refresh remains disabled by default: there is no live
HTTP client and no provider endpoint contact. Runtime `Refresh` with
`sourceAdapterPolicy.mode="only"` and `adapters=["linux_web"]` returns a
schema-valid disabled provider state unless a test-only fake fixture is injected.
This task did not add Shell changes, D-Bus XML changes, JSON schema changes,
production HTTP dependencies, real browser profile scanning, keyring access, raw
cookie persistence, or a TCP product API.

## Task 04D.1 Status

Complete as a gated reconnaissance slice. The daemon now has a real bounded
HTTP transport for one static Codex GET to
`https://chatgpt.com/codex/settings/usage`, but production `linux_web` refresh
still performs no default live provider fetch.

Live reconnaissance requires all of:

- `CODEXBAR_CODEX_WEB_LIVE=1`;
- `CODEXBAR_BROWSER_IMPORT_FAKE_HOME=/path/to/marked-throwaway-home`;
- `.codexbar-throwaway-browser-root` inside that fake home;
- explicit refresh provider `codex`;
- explicit `sourceAdapterPolicy.mode="only"` with only `linux_web`.

The transport validates the static URL before network, disables automatic
redirect following, rejects non-allowlisted redirects, enforces timeout and
response-size limits, stores no raw body, exposes no raw headers, and maps live
outcomes to safe provider states. The parser/normalizer remains fixture-shaped:
live responses are classified safely, and parser failure is a safe
`parse_error` rather than evidence of unsupported production behavior.

Task 04D.1 added daemon-only `reqwest` and `url` dependencies. `reqwest` uses
default features disabled with Rustls-oriented TLS for the static outbound GET;
`url` is used to resolve redirects safely instead of joining strings. Packaging
and CI include `cmake` for the current Rustls/AWS-LC graph and
`ca-certificates` for HTTPS trust roots. No OpenSSL/native TLS dependency, Shell
change, D-Bus XML change, JSON schema change, keyring prompt, raw cookie
persistence, browser extension, localhost API, or TCP product API was added.

The live Codex cookie query is intentionally limited to `chatgpt.com` for this
static request. Cookie names are not yet verified, so the temporary domain-wide
cookie exception applies only to marked throwaway reconnaissance and must be
narrowed or justified with live provider evidence before production enablement.

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
- Task 04D.1 transport policy rejects arbitrary hosts, private/local targets,
  userinfo, ports, query/fragment, wrong final paths, and non-allowlisted
  redirects.
- Task 04D.1 live reconnaissance remains ignored by default and is selectable
  with `-- --ignored codex_web_live`.

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

Optional live reconnaissance, never in normal CI:

```bash
CODEXBAR_CODEX_WEB_LIVE=1 \
CODEXBAR_BROWSER_IMPORT_FAKE_HOME=/path/to/marked-throwaway-home \
cargo test --manifest-path daemon/Cargo.toml -- --ignored codex_web_live
```
