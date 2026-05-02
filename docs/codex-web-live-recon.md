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

## Safe Classifications

The live test may produce these safe outcomes:

- `dashboard_reachable` when the fixture-shaped parser succeeds and output
  validates;
- `login_required` or `provider_cookie_rejected` when the response indicates an
  authentication flow or rejected session material;
- `account_mismatch` when a future expected-account check proves a mismatch
  without exposing raw identity;
- `redirect_blocked` when `Location` or final URL policy fails;
- `non_200` for non-success HTTP statuses that are not authentication
  rejections or rate limits;
- `response_too_large` when the response body exceeds the cap;
- `parse_error` for unsupported content type, invalid UTF-8, unexpected body
  shape, or redaction guard failure;
- `timeout` when the bounded request times out.

The implementation records stable diagnostic codes for these classes, not raw
provider response data.

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
  cookies. Encrypted sessions usually classify as missing dependency until a
  reviewed noninteractive Secret Service implementation exists.
- Codex cookie names have not been verified. Task 04D.1 permits domain-wide
  `chatgpt.com` cookies only inside marked throwaway reconnaissance. Production
  enablement must narrow cookie names or document why all `chatgpt.com` cookies
  are required.
- Production `linux_web` remains disabled by default.
