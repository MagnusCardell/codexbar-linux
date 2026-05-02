# Browser-Cookie Threat Model

## Status

Frozen for Task 04A. This document extends `docs/SECURITY.md` for the future
Linux browser-cookie and daemon web-fetch layer.

## Assets

High-sensitivity assets:

- browser cookies;
- decrypted cookie values;
- session tokens;
- OAuth access tokens;
- OAuth refresh tokens;
- bearer tokens;
- API keys;
- provider account identity;
- raw organization/account IDs;
- browser profile paths;
- browsing metadata implied by profile and domain selection;
- provider raw responses;
- request and response headers;
- copied diagnostic payloads;
- test fixtures and captured artifacts.

Lower-sensitivity assets:

- normalized usage snapshots;
- normalized cost summaries;
- provider IDs and labels;
- safe profile display labels;
- opaque profile IDs;
- high-level keyring state;
- stable diagnostic codes;
- provider state;
- reset windows and usage percentages.

## Trust Boundaries

```text
GNOME Shell process
  -> D-Bus JSON
  -> codexbar-linuxd daemon
  -> browser profile files
  -> Secret Service/keyring
  -> provider HTTPS endpoints
  -> normalized cache/config/logs/diagnostics
```

The Shell is trusted to display normalized data, not to handle secrets. The
daemon may transiently handle cookies and decrypted values. Provider HTTPS
endpoints are untrusted inputs. The session bus is not a secret vault: same-user
processes can call the service, so every return value and signal must be safe
to display and copy.

## Threats And Mitigations

### Accidental Raw Cookie Persistence

Threat: raw cookies, cookie headers, decrypted values, session keys, or bearer
tokens are written to cache, config, logs, diagnostics, fixtures, temp files, or
screenshots.

Mitigations:

- keep cookie values in memory only;
- never serialize in-memory session material;
- redacted or unavailable `Debug` for session material;
- validate public JSON before cache writes and D-Bus returns;
- run redaction before tracing, diagnostics, fixtures, and UI copy;
- commit only synthetic browser fixtures;
- reject raw `Cookie`, `Set-Cookie`, `Authorization`, bearer, token, key, and
  secret fields in fixture scanners.

### Exposing Raw Cookies Over D-Bus

Threat: `TestBrowserImport`, `GetDiagnostics`, `RefreshFinished`,
`SnapshotChanged`, or provider events include cookie values or full headers.

Mitigations:

- D-Bus payloads remain JSON strings validated against schemas;
- schemas expose only profile availability, high-level keyring state, aggregate
  counts, provider states, and diagnostic codes;
- details are scalar and redacted;
- expected runtime failures are payload states, not raw error objects;
- same-user clients are assumed able to call the service, so no D-Bus response
  may require secrecy.

### Committing Fixture Captures With Secrets

Threat: live browser cookies, provider responses, emails, profile paths, or
headers are committed as fixtures.

Mitigations:

- CI fixtures use synthetic DBs and fake provider servers only;
- live provider tests are ignored and opt-in;
- captured live data is never promoted without manual redaction review;
- raw provider response fixtures are not committed by default;
- fixture validators reject raw identity, token-like keys, headers, browser
  paths, home paths, and raw payload field names.

### Reading The Wrong Browser Profile

Threat: auto-discovery reads an unintended work/personal profile, causing
privacy confusion or cross-account data mixups.

Mitigations:

- bounded known-root discovery only;
- safe opaque profile IDs and explicit allowlists;
- no arbitrary D-Bus profile paths;
- provider account identity is masked or hashed only after normalization;
- provider adapters must reject mismatched safe account identity when they can
  prove a mismatch without exposing raw identity;
- diagnostics name only browser family and safe display label.

### Profile-Path Injection And Arbitrary File Reads

Threat: caller passes an absolute path or crafted ID that causes daemon to read
arbitrary files.

Mitigations:

- `profileIds` remain safe opaque IDs matching the existing schema pattern;
- implementation resolves IDs through daemon-discovered descriptors only;
- no path-like IDs are accepted;
- canonicalize candidates where practical;
- skip symlink escapes or unsupported roots with redacted diagnostics;
- never expose absolute profile paths.

### Symlink And Race Issues In Browser DB Access

Threat: profile roots or cookie DBs are swapped or symlinked during discovery
or temp-copy, causing unintended reads.

Mitigations:

- check file type and canonical path before copy where practical;
- copy DB and required companion files into private temp storage;
- open copied files read-only;
- avoid following unexpected symlink escapes;
- remove temp copies on all exit paths;
- map races to `browser_cookie_db_unreadable`,
  `browser_cookie_db_locked`, or `browser_profile_unreadable`.

### Keyring Unlock Prompt Confusion

Threat: scheduled refresh triggers unexpected Secret Service prompts or trains
users to approve unknown keyring access.

Mitigations:

- background and scheduled refresh are noninteractive;
- prompt-required states map to `browser_keyring_prompt_required`;
- interactive keyring unlock requires a later explicit UX/schema decision;
- user-facing copy says `Browser session could not be unlocked`, not raw
  keyring internals.

### Provider Rejecting Cookies And Leaking Response

Threat: provider rejects cookies and adapter exposes the raw rejection body,
headers, redirect URL, or account data.

Mitigations:

- map rejected existing cookie material to `cookie_rejected`;
- map absent cookie material to `unauthenticated`;
- discard raw bodies;
- record only status class and stable diagnostic code;
- block redirects to unexpected hosts;
- no raw provider error body in diagnostics.

### SSRF-Like Unsafe Provider URLs

Threat: provider adapter follows arbitrary URLs from settings, redirects, or
payloads.

Mitigations:

- URLs are constants plus typed parameters;
- no arbitrary URL settings;
- strict host allowlist per provider;
- redirect host allowlist;
- no local file, localhost, private network, or tokenized URL launch from
  provider payloads;
- Shell URL opening still uses `safeUrl()`.

### Cross-Provider Cookie Confusion

Threat: cookies for provider A are sent to provider B, or provider B receives
broader cookies than needed.

Mitigations:

- provider-owned required domain lists;
- provider-owned cookie-name filters where verified;
- in-memory jars scoped per provider fetch;
- no global cookie jar persistence;
- adapter code cannot query arbitrary domains.

### Stale Browser Sessions

Threat: expired browser cookies produce misleading usage data or erase good
cache.

Mitigations:

- provider rejected existing cookie material maps to `cookie_rejected`;
- absent cookies map to `unauthenticated`;
- failed all-provider web refresh does not overwrite useful cache;
- stale cache remains marked stale and preserves previous source metadata.

### Corrupted Cookie DB

Threat: malformed or unsupported cookie DB crashes daemon or leaks data through
error reporting.

Mitigations:

- read copied DBs through bounded queries;
- map unsupported schema to `browser_cookie_db_schema_unsupported`;
- map parse failures to `parse_error`;
- never include SQL error strings if they contain paths or raw data;
- fuzz or synthetic malformed DB tests before live enablement.

### Concurrent Browser DB Locks

Threat: browser locks live cookie DB and daemon reads inconsistent data.

Mitigations:

- copy DBs and WAL/SHM companions where needed;
- detect copy/read failure;
- report `browser_cookie_db_locked` or `browser_cookie_db_unreadable`;
- retry policy is bounded and does not block Shell.

### Local Same-User Clients Requesting Sensitive Operations

Threat: another same-user process repeatedly calls `TestBrowserImport` or
`Refresh` to learn browser state.

Mitigations:

- D-Bus outputs contain only high-level availability, counts, and safe codes;
- no cookie names, values, paths, raw identity, or raw response data;
- future implementation should rate-limit expensive probes if needed;
- provider and browser import tests use existing busy refresh semantics.

### Diagnostics As Secret Exfiltration Path

Threat: copy diagnostics includes raw payloads, paths, emails, headers, or
token-like strings.

Mitigations:

- diagnostics schema permits only scalar redacted details;
- copied diagnostics gets an additional Shell-side redaction pass;
- invalid diagnostics copy falls back to a bounded redacted unavailable object;
- diagnostic UI remains secondary and collapsed by default.

### Process And Environment Leakage

Threat: provider fetch or helper process inherits sensitive environment or logs
command output.

Mitigations:

- no helper process for browser import unless a later task justifies it;
- reuse upstream CLI runner discipline when commands are required elsewhere:
  env allowlist, timeout, bounded stdout/stderr, and redaction;
- provider web adapters use a bounded in-process client and do not pass cookie
  material to child processes.

## Test Obligations

Future browser-cookie implementation must add tests for:

- no raw cookie/cache/D-Bus/log/fixture serialization;
- redaction of raw headers, tokens, emails, profile paths, and provider bodies;
- profile ID policy rejection for path-like values;
- synthetic Chromium-family cookie DB read paths;
- synthetic Firefox cookie DB read paths;
- keyring locked/unavailable/decrypt-failure states through a fake backend;
- temp DB copy permissions and cleanup;
- corrupt DB and locked DB behavior;
- provider redirect blocking;
- provider timeout and response-size cap;
- stale cache preservation after failed web refresh;
- Shell diagnostics copy fallback.

## Residual Risks Before Task 04B

- Exact Ubuntu 24.04/26.04 browser storage behavior must be verified with
  synthetic or throwaway profiles.
- Chromium-family decryption details must be verified against current Chrome,
  Chromium, and Brave builds.
- Firefox reliability for target provider sessions remains unknown.
- Provider-specific domains and cookie names must be verified before fetch
  adapters are implemented.
- Interactive keyring prompts are not designed and remain out of scope.
