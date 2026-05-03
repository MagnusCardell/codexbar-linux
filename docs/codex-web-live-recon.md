# Codex Web Live Reconnaissance

## Status

Task 04D.1 adds an opt-in daemon-only reconnaissance path for the Codex web
adapter. It is not default production `linux_web` support and it must not be run
against a real default browser profile.

## What It Does

When every live gate is present, the ignored test may:

- discover Chromium-family profiles under a marked throwaway fake home;
- read `chatgpt.com` cookies from that throwaway profile into daemon memory;
- build an internal Cookie header for the static Codex dashboard URL;
- make one bounded GET to `https://chatgpt.com/codex/settings/usage`;
- classify status, redirect, timeout, response-size, content-type, and parser
  outcomes into schema-valid provider states and diagnostics.

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
- `classification`: one value from the fixed safe classification set below;
- `diagnosticCodes`: de-duplicated browser/web diagnostic codes filtered
  through a stable allowlist;
- `cookieMaterial`: safe browser-cookie material summary with only:
  `profilesDiscovered`, `candidateCookieRows`, `plaintextValueRows`,
  `encryptedValueRows`, `encryptedPrefixes`, `expiredRows`,
  `usableSessionCookies`, `decryptorBackend`, and `decryptionStatus`;
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
- `redirect_blocked` when `Location` or final URL policy fails;
- `non_200` for non-success HTTP statuses that are not authentication
  rejections or rate limits;
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
| `redirect_blocked` | Review the redirect host/path policy with safe host/path-class evidence only. |
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

- The live parser is not a real ChatGPT page parser. The only asserted
  normalizer is the synthetic fixture parser.
- The current live decryptor path does not unlock real encrypted Chromium
  cookies. The production/env backend is `plain`: plaintext rows can produce
  in-memory session material, while encrypted rows usually classify as missing
  dependency until a reviewed noninteractive Secret Service decryptor exists.
- A noninteractive Secret Service probe abstraction exists only to classify
  unavailable, locked, and prompt-required states. It does not extract or
  persist the Chromium secret.
- Codex cookie names have not been verified. Task 04D.1 permits domain-wide
  `chatgpt.com` cookies only inside marked throwaway reconnaissance. Production
  enablement must narrow cookie names or document why all `chatgpt.com` cookies
  are required.
- Production `linux_web` remains disabled by default.
