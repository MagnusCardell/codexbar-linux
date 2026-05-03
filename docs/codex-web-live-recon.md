# Codex Web Live Reconnaissance

## Status

Task 04D.1 adds an opt-in daemon-only reconnaissance path for the Codex web
adapter. Task 04D.1C refines Codex web request parity and non-2xx response
classification. Task 04D.1D adds one safe same-host redirect hop for the static
Codex dashboard URL. Task 04D.1E adds redacted same-host Codex redirect
path-family classification and derives `redirectCanFollow` from that policy.
This is not default production `linux_web` support and it must not be run
against a real default browser profile.

## What It Does

When every live gate is present, the ignored test may:

- discover Chromium-family profiles under a marked throwaway fake home;
- read `chatgpt.com` cookies from that throwaway profile into daemon memory;
- build an internal Cookie header for the static Codex dashboard URL;
- make one bounded async GET to `https://chatgpt.com/codex/settings/usage`;
- make at most one follow-up GET when that first response redirects to a safe
  same-host Codex usage/settings target on `https://chatgpt.com`;
- send static browser-like navigation headers for that dashboard GET and report
  only the safe profile name `requestHeaderProfile="browser_like"`;
- classify status, redirect, timeout, response-size, content-type, and parser
  outcomes into schema-valid provider states and diagnostics;
- attach safe HTTP response metadata fields to diagnostics without exposing raw
  response headers, redirect locations, or bodies.

The live path does not make Shell, preferences, D-Bus XML, schema, localhost,
TCP API, browser extension, or keyring-prompt changes.

## Gates

The live Codex web smoke requires all of:

- `CODEXBAR_CODEX_WEB_LIVE=1`;
- `CODEXBAR_BROWSER_IMPORT_FAKE_HOME=/path/to/throwaway-home`;
- a `.codexbar-throwaway-browser-root` marker file inside that fake home;
- refresh options with `providers=["codex"]`;
- refresh options with `sourceAdapterPolicy.mode="only"` and
  `adapters=["linux_web"]`.

The daemon refuses the fake home if it is `/`, the real `$HOME`, under the real
`~/.config`, under real `XDG_CONFIG_HOME`, relative, missing, missing the
marker, or canonicalizes through an unsafe root.

## Preparing A Throwaway Profile

Use an isolated temp home. Do not reuse a normal browser profile.

```bash
THROWAWAY_HOME="$(mktemp -d /tmp/codexbar-codex-web.XXXXXX)"
mkdir -p "$THROWAWAY_HOME/.config" "$THROWAWAY_HOME/.cache" "$THROWAWAY_HOME/.local/share"
printf 'codexbar throwaway browser root\n' \
  > "$THROWAWAY_HOME/.codexbar-throwaway-browser-root"
```

Launch a Chromium-family browser with only that throwaway user data directory,
then sign into ChatGPT/Codex manually if live authentication needs to be
observed:

```bash
HOME="$THROWAWAY_HOME" \
XDG_CONFIG_HOME="$THROWAWAY_HOME/.config" \
XDG_CACHE_HOME="$THROWAWAY_HOME/.cache" \
XDG_DATA_HOME="$THROWAWAY_HOME/.local/share" \
google-chrome \
  --user-data-dir="$THROWAWAY_HOME/.config/google-chrome" \
  --no-first-run \
  --no-default-browser-check \
  https://chatgpt.com/codex/settings/usage
```

Close the browser before running the test when possible, so the cookie DB is
less likely to be locked.

## Running The Recon Test

```bash
CODEXBAR_CODEX_WEB_LIVE=1 \
CODEXBAR_BROWSER_IMPORT_FAKE_HOME="$THROWAWAY_HOME" \
cargo test --manifest-path daemon/Cargo.toml -- --ignored codex_web_live
```

Normal CI and `./scripts/check.sh` do not set these variables and do not run
ignored live tests.

## Safe Recon Summary

After the ignored live smoke completes its existing snapshot, refresh-result,
diagnostics, provider-event, and cache redaction assertions, it prints one
compact JSON summary line. The summary is derived from normalized provider
state, refresh-result state, and a daemon browser-cookie material summary that
contains only counts/classes. It does not print raw browser rows, cookie names
or values, request headers, response headers, redirect locations, response
bodies, or diagnostics details.

A passing live test by itself only proves the safety assertions held. The
summary `classification` is the useful reconnaissance result, because it tells
the operator whether the run reached browser session material, attempted the
bounded web fetch, and reached a parser outcome.

Allowed summary fields are:

- `provider`: always `codex`;
- `providerState`: normalized provider state;
- `refreshStatus`: normalized refresh status;
- `cacheWritten`: boolean refresh-result cache-write flag;
- `source`: always `web`;
- `sourceAdapter`: always `linux_web`;
- `requestHeaderProfile`: always `browser_like`;
- `httpStatusCode`: numeric HTTP status when an HTTP response was observed;
- `httpStatusClass`: one of `informational`, `success`, `redirect`,
  `client_error`, `server_error`, or `unknown`;
- `redirectPresent`: boolean;
- `redirectHostClass`: one of `none`, `allowed`, `blocked`, `missing`, or
  `invalid`;
- `redirectTargetClass`: one of `none`, `same_host_canonical`,
  `same_host_usage_path`, `same_host_login_path`, `same_host_other`,
  `allowed_host_other`, `blocked_host`, or `invalid`;
- `redirectPathFamily`: one of `none`, `codex_usage`, `codex_settings`,
  `codex_other`, `auth_login`, `auth_callback`, `root`, `static_asset`, `api`,
  `unknown`, or `invalid`;
- `redirectPathDepth`: one of `zero`, `one`, `two`, `three`, `many`, or
  `unknown`;
- `redirectQueryClass`: one of `none`, `safe_empty`, `present`,
  `token_like`, or `unknown`;
- `redirectCanFollow`: boolean;
- `redirectFollowed`: boolean;
- `redirectHopCount`: `0` or `1`;
- `finalHttpStatusCode`: numeric HTTP status from the one allowed follow-up
  response when a redirect was followed;
- `finalHttpStatusClass`: one of `none`, `informational`, `success`,
  `redirect`, `client_error`, `server_error`, or `unknown`;
- `contentTypeClass`: one of `html`, `json`, `text`, `other`, or `missing`;
- `responseBodyClass`: one of `not_read`, `empty`, `within_cap`, `too_large`,
  or `invalid_encoding`;
- `responseSizeBucket`: one of `zero`, `small`, `medium`, `large`, or `capped`;
- `classification`: one value from the fixed safe classification set below;
- `diagnosticCodes`: de-duplicated browser/web diagnostic codes filtered
  through a stable allowlist;
- `cookieMaterial`: safe browser-cookie material summary with only:
  `profilesDiscovered`, `candidateCookieRows`, `plaintextValueRows`,
  `encryptedValueRows`, `encryptedPrefixes`, `expiredRows`,
  `domainMatchedRows`, `pathMatchedRows`, `secureMatchedRows`,
  `decryptedRows`, `headerEligibleRows`, `headerRejectedRows`,
  `headerRejectedByClass`, `cookieHeaderStatus`, `usableSessionCookies`,
  `decryptorBackend`, `decryptionStatus`, and `decryptionFailureClass`.
  `headerRejectedByClass` may contain only `invalid_name`, `invalid_value`,
  `empty_name`, `too_long`, `expired`, `domain_mismatch`, `path_mismatch`,
  `secure_mismatch`, `unsupported_prefix`, and `decrypt_failed`.
  `cookieHeaderStatus` may be only `not_attempted`, `built`, `empty`,
  `header_too_large`, `too_many_cookies`, or `invalid_material`;
- `cookiePresence`: one of `none`, `found`, `decrypted`, `unavailable`, or
  `unknown`;
- `webFetch`: one of `not_attempted`, `attempted`, `finished`, `blocked`,
  `timeout`, or `parse_error`;
- `redactionApplied`: always `true`.

The summary must not include cookie names or values, encrypted values, Cookie
or Authorization headers, Set-Cookie headers, raw response bodies, full URLs
with query or fragment data, redirect locations, exact domains, profile or home
paths, account email or ID values, SQL rows, raw diagnostic details, or raw
provider payloads.

## Safe Classifications

The live test may produce these safe outcomes:

- `dashboard_reachable` when the fixture-shaped parser succeeds and output
  validates without a more specific parser-success diagnostic;
- `parser_succeeded` when the fixture-shaped parser and redaction path
  succeeded;
- `login_required` or `provider_cookie_rejected` when the response indicates an
  authentication flow or rejected session material;
- `redirect_blocked` when `Location` or final URL policy fails, when the target
  is outside the safe same-host Codex usage policy, or when a second redirect
  hop appears after the one allowed follow;
- `non_200` for non-success HTTP statuses that are not mapped to a more specific
  authentication, redirect, timeout, response-size, or parse classification.
  Rate limits retain `classification="non_200"` but carry
  `provider_web_fetch_rate_limited` and the safe HTTP status fields;
- `parse_error` for unsupported content type, invalid UTF-8, unexpected body
  shape, or redaction guard failure;
- `timeout` when the bounded request times out;
- `response_too_large` when the response body exceeds the cap;
- `browser_cookie_missing` when no provider-relevant browser session material is
  available;
- `browser_cookie_found` when provider-relevant cookie material was found but no
  more specific terminal fetch outcome is available;
- `browser_keyring_unavailable` when cookie decryption would require
  unavailable, locked, or prompt-required keyring access;
- `browser_profile_not_found` when no supported marked throwaway profile is
  available;
- `linux_web_live_http_disabled` when the live HTTP gate did not permit the
  provider fetch;
- `unknown_safe_failure` when the redacted codes prove a failure but not one of
  the more specific safe classes.

The implementation records stable diagnostic codes for these classes, not raw
provider response data.

Safe HTTP response summary and diagnostic fields are limited to scalar metadata:

- `httpStatusCode`: exact numeric HTTP status when an HTTP response was observed;
- `httpStatusClass`: `informational`, `success`, `redirect`, `client_error`,
  `server_error`, or `unknown`;
- `redirectPresent`: boolean;
- `redirectHostClass`: `none`, `allowed`, `blocked`, `missing`, or `invalid`;
- `redirectTargetClass`: `none`, `same_host_canonical`,
  `same_host_usage_path`, `same_host_login_path`, `same_host_other`,
  `allowed_host_other`, `blocked_host`, or `invalid`;
- `redirectPathFamily`: `none`, `codex_usage`, `codex_settings`,
  `codex_other`, `auth_login`, `auth_callback`, `root`, `static_asset`, `api`,
  `unknown`, or `invalid`;
- `redirectPathDepth`: `zero`, `one`, `two`, `three`, `many`, or `unknown`;
- `redirectQueryClass`: `none`, `safe_empty`, `present`, `token_like`, or
  `unknown`;
- `redirectCanFollow`: boolean derived from the same policy gate used before
  issuing the one allowed follow-up request;
- `redirectFollowed`: boolean;
- `redirectHopCount`: `0` or `1`;
- `finalHttpStatusCode`: exact numeric HTTP status for the one allowed follow-up
  response when present;
- `finalHttpStatusClass`: `none`, `informational`, `success`, `redirect`,
  `client_error`, `server_error`, or `unknown`;
- `contentTypeClass`: `missing`, `html`, `json`, `text`, or `other`;
- `responseBodyClass`: `not_read`, `empty`, `within_cap`, `too_large`, or
  `invalid_encoding`;
- `responseSizeBucket`: `zero`, `small`, `medium`, `large`, or `capped`;
- `redirectBlocked`: boolean on redirect-policy failures;
- `requestHeaderProfile`: `browser_like` on request-start diagnostics.

These fields intentionally do not include raw request headers, raw response
headers, `Location`, final URLs, query strings, fragments, response bodies, or
cookie material.

The only redirect follow permitted in Task 04D.1E starts from the static Codex
dashboard URL and follows one same-host `https://chatgpt.com` target whose path
is classified as a known safe Codex usage/settings family. The safe set is the
static dashboard usage route, its trailing-slash form, and the bounded Codex
usage/settings family routes needed to classify same-host provider movement.
The target must have no userinfo or fragment and must have no query or an empty
query. Query-present redirects are classified with `redirectQueryClass` but are
not followed. Public output reports only the redacted family, depth, query
class, and follow booleans; it never reports the raw `Location`, path, query,
fragment, or URL. Same-host auth/login paths are not followed and map to
`provider_cookie_rejected`. Auth callback, `openai.com`, attacker,
private/local, userinfo-bearing, fragment-bearing, query-present, token-like
query, same-host unknown-path, and second-hop redirects fail closed.

`cookieMaterial.decryptionFailureClass` is safe class metadata, not secret
material. It may be `none`, `keyring_needed`, `unsupported_format`,
`malformed_ciphertext`, `wrong_key`, `invalid_material`, `header_too_large`,
`too_many_cookies`, `unavailable`, or `failed`. For Task 04B.3, `none` with
`decryptorBackend="plain"` and `decryptionStatus="succeeded"` can indicate that
Chromium Linux basic/plain `v10` cookie material decrypted successfully.
`keyring_needed` still means Secret Service/KWallet/newer keyring-backed work
is outside this task. `unsupported_format`, `malformed_ciphertext`,
`wrong_key`, `invalid_material`, `header_too_large`, and `too_many_cookies` are
redaction-safe failure classes and must not be expanded with raw encrypted
bytes, cookie names, host keys, profile paths, Cookie headers, or decrypted
values.

## Next Decision

Use the summary classification to choose the next task:

| Classification | Decision |
| --- | --- |
| `parser_succeeded` or `dashboard_reachable` | Consider Task 04D.2 production-shape parser work, still behind explicit review. |
| `parse_error` with `cookiePresence="found"` or `cookiePresence="decrypted"` and `webFetch="finished"` | Update synthetic parser fixtures from hand-authored observations only. Do not copy raw live body. |
| `login_required` or `provider_cookie_rejected` | Investigate cookie/session material validity in the throwaway profile. |
| `browser_cookie_missing` | Investigate browser import and cookie-domain selection. |
| `browser_cookie_missing` with `cookieMaterial.plaintextValueRows > 0` and `usableSessionCookies=0` | Inspect validation/filtering policy with synthetic fixtures; do not print raw row data. |
| `browser_keyring_unavailable` with encrypted prefix counts | Secret Service or unsupported encrypted-prefix work is the blocker; do not work on the Codex parser yet. |
| `unknown_safe_failure` with `decryptionFailureClass="unsupported_format"` | Do not broaden decryption in this task. Split Secret Service/KWallet/newer-prefix work into a reviewed follow-up. |
| `unknown_safe_failure` with `decryptionFailureClass="malformed_ciphertext"` or `"wrong_key"` | Treat the cookie material as unusable; inspect browser version/profile setup with safe counts only. |
| `unknown_safe_failure` with `decryptionFailureClass="invalid_material"`, `"header_too_large"`, or `"too_many_cookies"` | Browser cookie decryption is past the prefix/key step, but domain-wide cookie material cannot safely form a header. Verify required cookie names or header material policy with synthetic fixtures before parser work. |
| `redirect_blocked` | Review the redirect target policy with safe target/follow/final-status evidence only. |
| `timeout`, `non_200`, or `response_too_large` | Follow up on transport/classification behavior before parser work. |
| `browser_keyring_unavailable` or `browser_profile_not_found` | Fix the throwaway browser setup or decryption prerequisite before retrying live recon. |
| `linux_web_live_http_disabled` or `unknown_safe_failure` | Confirm gates and stable diagnostic coverage before broadening implementation. |

## Never Print Or Commit

Do not print, copy into issues, or commit:

- raw cookies or Cookie headers;
- Authorization or Set-Cookie headers;
- raw request or response headers;
- raw response bodies or HTML dumps;
- browser profile paths;
- full provider account identity;
- emails, organization IDs, account IDs, session keys, tokens, or API keys;
- fixtures captured from live provider traffic.

Only schema-valid normalized snapshots, refresh results, provider events, and
redacted diagnostics may be retained.

## Known Limits

- Task 04B.4 adds synthetic session-material policy support for the static
  Codex dashboard URL. It can classify target-domain/path eligibility with
  counts/classes only and preserves host-only versus domain-cookie matching,
  skips only syntax-invalid header rows when valid material remains, and fails
  closed for decrypt/unsupported-prefix/header cap failures. It does not verify
  required live Codex cookie names or enable production web refresh.
- The Task 04B.4 live recon rerun is pending in this workspace because no
  marked throwaway fake home was available through `CODEXBAR_WEB_HOME` or
  `CODEXBAR_BROWSER_IMPORT_FAKE_HOME`.
- The Task 04D.1C live recon rerun is also pending in this workspace for the
  same reason. The implementation now emits `classification="non_200"` instead
  of the earlier spelling drift `non200`, and adds safe HTTP response summary
  fields, but no new signed-in live provider response was observed here.
- The Task 04D.1D live recon rerun was attempted on 2026-05-03 with
  `CODEXBAR_CODEX_WEB_LIVE=1` and
  `CODEXBAR_BROWSER_IMPORT_FAKE_HOME="$CODEXBAR_WEB_HOME"`, but
  `CODEXBAR_WEB_HOME` was empty in this workspace. The ignored smoke failed
  before browser import with "throwaway fake home must exist", so no HTTP
  request, redirect target, final status, response body, or parser outcome was
  observed.
- The Task 04D.1E live recon rerun was attempted on 2026-05-03 with
  `CODEXBAR_CODEX_WEB_LIVE=1` and
  `CODEXBAR_BROWSER_IMPORT_FAKE_HOME="$CODEXBAR_WEB_HOME"`, but the throwaway
  fake home did not exist in this execution environment. The ignored smoke
  failed before browser import with "throwaway fake home must exist", so no
  HTTP request, redirect target, `redirectPathFamily`, `redirectCanFollow`,
  final status, response body, or parser outcome was observed.
- Task 04B.3 live recon against the signed-in throwaway profile reached 19
  candidate `v10` rows and the plain decryptor, but produced
  `usableSessionCookies=0`, `cookiePresence="unavailable"`, and
  `webFetch="not_attempted"` because the domain-wide `chatgpt.com` cookie set
  included material that failed safe Cookie-header validation. A local
  aggregate-only check, with no cookie names/values/encrypted bytes printed,
  showed all 19 rows decrypt to UTF-8 and 2 rows fail value-character
  validation. The next blocker is Codex cookie-name/header-material policy, not
  v10 prefix support, HTTP transport, or parser work.
- The live parser is not a real ChatGPT page parser. The only asserted
  normalizer is the synthetic fixture parser.
- The current live decryptor path supports only plaintext rows and the verified
  Chromium Linux basic/plain `v10` encrypted-value path. Secret Service,
  KWallet, app-bound encryption, `v20`, encrypted-value-prefix `v24`, and
  unknown encrypted formats still classify as unavailable or failed without
  unlocking keyrings or persisting material.
- A noninteractive Secret Service probe abstraction exists only to classify
  unavailable, locked, and prompt-required states. It does not extract or
  persist the Chromium secret.
- Codex cookie names have not been verified. Task 04D.1 permits domain-wide
  `chatgpt.com` cookies only inside marked throwaway reconnaissance. Production
  enablement must narrow cookie names or document why all `chatgpt.com` cookies
  are required.
- Production `linux_web` remains disabled by default.
