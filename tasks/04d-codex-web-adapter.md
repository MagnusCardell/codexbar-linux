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

## Task 04D.1C Status

Complete as a safe transport/reporting refinement. The Codex dashboard GET now
uses a static browser-like navigation request header profile, and public output
reports only `requestHeaderProfile="browser_like"` rather than raw request
headers. Response diagnostics include only bounded scalar HTTP metadata:
`httpStatusCode`, `httpStatusClass`, `redirectPresent`, `redirectHostClass`,
`contentTypeClass`, `responseBodyClass`, `responseSizeBucket`, and
`redirectBlocked` where relevant.

Status mapping is explicit for the covered non-2xx cases: 401/403 become
`cookie_rejected`, 429 becomes rate-limited `provider_unavailable`, 5xx becomes
`provider_unavailable`, blocked 3xx redirects become `redirect_blocked`, and
allowed non-login 3xx responses also classify as `redirect_blocked` because
redirect following is disabled. The safe live reconnaissance classification
spelling is `non_200`.

This refinement does not change parsers, does not promote live bodies or
fixtures, does not enable default live provider fetch, does not scan real
browser profiles, does not add keyring work, and does not change Shell, D-Bus
XML, JSON schemas, or TCP/localhost surfaces.

The gated signed-in live rerun was not performed in this workspace because no
marked throwaway home was available through `CODEXBAR_WEB_HOME` or
`CODEXBAR_BROWSER_IMPORT_FAKE_HOME`.

## Task 04D.1D Status

Complete as a safe same-host redirect refinement. The daemon still starts from
only the static Codex dashboard URL. If that first response is a 3xx, it may
perform at most one follow-up GET only when the resolved target is
`https://chatgpt.com`, has no userinfo or fragment, uses the same dashboard path
or trailing-slash dashboard path, and has either no query or a bounded
non-token-like query classified only as metadata. The follow-up request reuses
the daemon's in-memory browser session material only after cookie path/domain
matching for the redirect target.

The adapter does not follow redirects to `openai.com`, attacker-controlled
hosts, private/local hosts, same-host unknown paths, auth/login paths,
userinfo-bearing URLs, fragments, token-like queries, or a second redirect hop.
Same-host auth/login redirects map to `cookie_rejected` rather than parser
work. Public diagnostics and live-recon summaries include only
`redirectTargetClass`, `redirectPathFamily`, `redirectPathDepth`,
`redirectQueryClass`, `redirectCanFollow`, `redirectFollowed`,
`redirectHopCount`, `finalHttpStatusCode`, and `finalHttpStatusClass`. They
still do not include
raw `Location` values, raw response headers, query strings, fragments, bodies,
cookies, Cookie headers, profile paths, or tokens.

This refinement does not change parsers, does not promote live bodies or
fixtures, does not enable default live provider fetch, and does not change
Shell, D-Bus XML, JSON schemas, localhost, or TCP surfaces.

The 2026-05-03 live recon rerun was attempted with
`CODEXBAR_CODEX_WEB_LIVE=1` and
`CODEXBAR_BROWSER_IMPORT_FAKE_HOME="$CODEXBAR_WEB_HOME"`, but
`CODEXBAR_WEB_HOME` was empty in this workspace. The ignored smoke failed
before browser import with "throwaway fake home must exist", so no live HTTP
request, redirect, final status, response body, or parser outcome was observed.

## Task 04D.1E Status

Complete as a same-host redirect path-classification refinement. The daemon
still starts from only the static Codex dashboard URL and still follows at most
one redirect. The policy now classifies same-host Codex redirect targets into
redacted path families, path-depth classes, and query classes, and diagnostics
include `redirectCanFollow` derived from the exact policy gate used before any
follow-up request.

The follow set remains bounded to known safe Codex usage/settings route
families on `https://chatgpt.com` with no userinfo, no fragment, and no query
or an empty query. Same-host query-present redirects are classified but not
followed. Same-host auth/login targets are not followed and still map to
`cookie_rejected`; auth-callback, unknown same-host paths, query-present or
token-like query shapes, `openai.com`, attacker, private/local, and second-hop
redirects fail closed.

This refinement does not output raw `Location`, path, query, fragment, URL,
body, headers, cookies, or profile paths. It does not change parsers, does not
promote live bodies or fixtures, does not enable default live provider fetch,
and does not change Shell, D-Bus XML, JSON schemas, localhost, or TCP surfaces.

The 2026-05-03 Task 04D.1E live recon rerun was attempted with
`CODEXBAR_CODEX_WEB_LIVE=1` and
`CODEXBAR_BROWSER_IMPORT_FAKE_HOME="$CODEXBAR_WEB_HOME"`, but the throwaway
fake home did not exist in this execution environment. The ignored smoke failed
before browser import with "throwaway fake home must exist", so no live HTTP
request, redirect target, path family, final status, response body, or parser
outcome was observed.

## Task 04D.1F Status

Complete as a narrow Codex cloud usage redirect allowance. The daemon still
starts from only the static Codex dashboard URL and still follows at most one
redirect. The policy now classifies exactly
`/codex/cloud/settings/usage` on `chatgpt.com` as
`same_host_usage_path`/`codex_usage` when the target has the same host, uses
HTTPS, has no userinfo, no port, no fragment, and no query except an empty
query. The observed route shape is the static dashboard route redirecting to
the exact cloud usage route; no query or fragment was observed.

The allowance does not generalize the `/codex/cloud/` family:
`/codex/cloud/other`, token-like query variants, query-present variants,
userinfo-bearing URLs, port-bearing URLs, fragments, `openai.com`, attacker,
private/local, unknown same-host, and second-hop redirects still fail closed.
Public output remains limited to redacted redirect classes, path family/depth,
query class, follow booleans, hop count, and final status metadata. It does not
include raw `Location`, path, query, fragment, URL, body, headers, cookies, or
profile paths.

This refinement does not change parsers, does not promote live bodies or
fixtures, does not capture or commit raw response bodies, does not enable
default live provider fetch, and does not change Shell, D-Bus XML, JSON
schemas, localhost, or TCP surfaces.

## Task 04D.1G Status

Complete as a parser reconnaissance and synthetic fixture slice. The daemon now
adds safe parser-structure metadata to Codex web parse outcomes:
`htmlStructureClass`, `embeddedJsonCandidateCount`,
`embeddedJsonSafeKeyClasses`, `parserCandidate`, `parserFailureClass`, and
`parserReached`. These values are classes and counts only. They do not include
raw live HTML, raw script text, raw JSON, raw keys that could carry identity,
account email or IDs, organization/workspace names, cookies, headers, response
snippets, byte offsets, redirect URLs, or profile paths.

The parser remains bounded and fixture-shaped. It supports only hand-authored
synthetic structures under `daemon/fixtures/web/codex/`: synthetic next-data
JSON, generic `application/json` script JSON, allowlisted inline JSON
assignment, app-shell-with-no-data, login-shell, missing-usage, and
redaction-rejected candidates. It does not execute JavaScript, does not use a
browser engine, does not promote live payloads to fixtures, and does not enable
default live provider fetch.

The safe signed-in live baseline for this slice reached authenticated HTML
through one same-host Codex usage redirect (`307` followed to final `200`) and
ended as `parse_error`, proving the blocker was the parser rather than browser
import, Chromium `v10` decryption, Cookie header construction, transport, or
one-hop redirect policy. The local post-change rerun was attempted with the
requested environment command, but no valid marked throwaway fake home existed
at `$CODEXBAR_WEB_HOME`; it failed before browser import with "throwaway fake
home must exist", so no new local live parser result was observed.

This slice does not change Shell code, D-Bus XML, JSON schemas, GSettings,
packaging surfaces, localhost/TCP behavior, or production `linux_web` defaults.

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
- Task 04D.1D follows at most one safe same-host Codex dashboard redirect,
  blocks second-hop redirects, blocks same-host unknown/login/token-like targets
  from follow, maps login/auth redirects to cookie rejection, and emits only
  redacted redirect target classes.
- Task 04D.1E emits redacted redirect path family, path depth, query class, and
  `redirectCanFollow`; follows only known safe same-host Codex usage/settings
  route families; and keeps raw redirect locations, paths, queries, fragments,
  URLs, bodies, headers, cookies, and profile paths out of public output.
- Task 04D.1F classifies exactly the Codex cloud usage redirect as
  `same_host_usage_path`, follows it at most once through the existing redirect
  policy, blocks `/codex/cloud/other`, token-like query variants, userinfo,
  ports, and fragments, and keeps raw redirect locations out of public output.
- Task 04D.1G emits only safe parser structure fields
  (`htmlStructureClass`, `embeddedJsonCandidateCount`,
  `embeddedJsonSafeKeyClasses`, `parserCandidate`, `parserFailureClass`,
  `parserReached`), parses only synthetic next-data, `application/json`, and
  allowlisted inline JSON assignment fixtures, fails closed for app shells,
  missing usage fields, invalid UTF-8, excessive/unknown shapes, and rejected
  candidate redaction, and keeps raw HTML, JSON, scripts, headers, cookies,
  account identity, and profile paths out of public output.
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
