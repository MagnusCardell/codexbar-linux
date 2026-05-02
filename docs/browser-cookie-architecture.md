# Linux Browser-Cookie And Web-Fetch Architecture

## Status

Frozen for Task 04A and partially implemented by Task 04B/04D. Task 04B adds
daemon-only Chromium-family synthetic/fake-root discovery, private SQLite cookie
DB temp copies, synthetic cookie-row reads, fake decryptor states, in-memory
session material, and schema-valid `TestBrowserImport` results. Task 04D.0 adds
a daemon-only Codex web adapter skeleton with fake HTTP fixtures, static
policy/URL allowlists, response-size/redirect/timeout handling, and
normalization into the existing snapshot provider shape. Task 04D.1 adds a
daemon-only, gated real HTTP transport for opt-in Codex web reconnaissance
against the single static dashboard URL. It still does not implement live
browser-profile scanning by default, default live provider fetch, real keyring
access, real provider scraping, or production web scraping.

## Thesis

CodexBar GNOME needs Linux-native browser-cookie support because upstream
CodexBar CLI documents `web` and `auto` source modes as macOS-only. On Linux,
the upstream CLI remains the production data plane where CLI/API/local parity
exists. Browser-cookie import fills only the web-session gap.

The Linux browser-cookie layer is daemon-owned, memory-only, and redacted. The
Shell remains presentation-only and consumes only schema-backed D-Bus JSON.

## Ownership Boundary

Browser-cookie import and web-backed provider fetches live only in
`codexbar-linuxd`.

The daemon owns:

- browser/profile discovery;
- profile allowlist enforcement;
- temporary browser cookie DB copies;
- SQLite cookie-store reads;
- Secret Service/keyring access;
- cookie decryption;
- in-memory cookie jar construction;
- provider web requests;
- provider response parsing;
- normalization into `spec/snapshot.schema.json`;
- redacted diagnostics.

The GNOME Shell process does not:

- inspect browser profiles;
- read cookie DBs;
- access keyrings;
- build cookie headers;
- call provider endpoints;
- invoke upstream `codexbar`;
- parse daemon cache files;
- persist or construct raw diagnostics.

The preferences process may call daemon D-Bus methods, including
`SetSettingsPatch` and `TestBrowserImport`, but it must not scan browser
profiles or access cookies/keyrings directly.

## Data Flow

Future implementation must keep cookie access separate from provider web
fetching:

1. Provider policy selects enabled providers and allowed adapters.
2. Browser discovery returns safe profile descriptors.
3. Cookie-store access copies the required DB files to a private temp directory
   when the live store may be locked.
4. Cookie queries are restricted to provider-required domains and verified
   cookie names where known.
5. Decryption happens inside the daemon through a keyring abstraction.
6. Decrypted cookie values are placed into an in-memory session material type.
7. Provider web adapters consume the in-memory session material.
8. Provider adapters immediately normalize successful data into provider
   snapshots or return redaction-safe failure states.
9. Durable outputs are limited to normalized snapshots, refresh results, daemon
   settings, daemon info, and diagnostics.

No raw cookie value, decrypted browser secret, full request header, provider raw
response, profile path, raw email, raw organization, or provider account ID may
cross D-Bus, enter cache, logs, diagnostics, fixtures, screenshots, or copied
UI output.

## Public Interfaces

Task 04A does not add or change public interfaces.

Existing surfaces are sufficient for the first implementation slice:

- `Refresh(options_json)` can select `linux_web` through
  `sourceAdapterPolicy`.
- `GetSnapshot()` and `SnapshotChanged` carry normalized providers with
  `source="web"` and `sourceAdapter="linux_web"` when a web adapter succeeds.
- `RefreshFinished` carries provider-level success/failure states.
- `GetDiagnostics(provider_id)` carries redaction-safe diagnostics.
- `GetDaemonInfo()` carries capability booleans.
- `SetSettingsPatch(patch_json)` configures daemon-owned browser import policy.
- `TestBrowserImport(options_json)` carries a safe browser import test result.

Expected runtime failures must be represented as schema-valid payload states
and diagnostics. They should not become D-Bus method errors except for invalid
JSON, invalid settings patches, refresh busy, unimplemented capabilities, or
redacted internal failures. In Task 04D.1, production `linux_web` refreshes
still have no default live provider fetch and return redacted
`linux_web_live_http_disabled` diagnostics unless an explicit reconnaissance
gate is enabled. The live Codex reconnaissance gate requires
`CODEXBAR_CODEX_WEB_LIVE=1`, a safe marked throwaway fake home through
`CODEXBAR_BROWSER_IMPORT_FAKE_HOME`, explicit provider `codex`, and explicit
source adapter `linux_web`.

## Browser Support Sequence

Implementation order is:

1. Chromium-family first:
   - Google Chrome stable;
   - Brave stable;
   - Chromium, including Ubuntu snap behavior after verification.
2. Firefox second.
3. Other Chromium forks, Flatpak browsers, Snap variants beyond Chromium, and
   nonstandard profile roots later only after explicit verification.

Task 04B uses fake or throwaway browser profiles first. Real user profile
discovery remains disabled until reviewed live smoke instructions exist. The
daemon test/runtime gate for this slice is `CODEXBAR_BROWSER_IMPORT_FAKE_HOME`
or an injected `BrowserDiscoveryRoots` in tests; default `App::new()` does not
derive browser roots from the real process `HOME` or `XDG_CONFIG_HOME`.

See `docs/browser-support.md` for the support matrix.

## Provider Support Sequence

Implementation order is:

1. Codex/OpenAI web dashboard as the browser-cookie pilot.
2. Claude web only after the browser-cookie layer and Codex adapter are stable.
3. Cursor, OpenCode, Amp, Ollama, Abacus AI, Mistral, Droid/Factory, MiniMax,
   Kimi, and other browser-cookie providers later, one provider at a time.
4. API-token, OAuth, CLI, local probe, and local-cost providers stay on their
   existing non-browser data planes unless there is a concrete Linux web gap.

See `docs/provider-roadmap.md` for provider classifications and non-promises.

## Profile Discovery Rules

Profile discovery must be bounded known-root enumeration, not recursive home
scanning.

Rules:

- Discover only supported browser families.
- Start from documented or verified browser roots.
- Respect XDG and browser-specific override environment variables only where
  the browser documents them and the daemon can read them safely.
- Never accept arbitrary caller-provided absolute profile paths over D-Bus.
- `profileIds` are opaque safe IDs, never filesystem paths.
- Profile allowlists use only safe opaque IDs matching the existing schema.
- Profile display labels are safe labels such as `Chrome Default` or
  `Firefox default-release`; they must not include path components.
- Symlinks and canonicalization must be handled before file access where
  practical.
- If a candidate escapes the expected browser root after canonicalization,
  skip it with a redaction-safe diagnostic.
- Unknown profile roots are skipped until explicitly supported.

Recommended future profile ID construction:

- stable local hash or HMAC over browser family, install kind, user-data root,
  and profile relative name;
- no raw path material in the resulting ID;
- no cross-machine stable identifier.

## Cookie Access And Decryption Rules

Cookie DB access is read-only and provider-scoped.

Rules:

- Query only provider-required domains.
- Query verified cookie names where known; if a provider requires all cookies
  for a domain, document the reason in that provider adapter.
- Copy locked SQLite DBs and required companion files into a private temp
  directory before reading.
- Private temp directories must be `0700`; temp files must be `0600`.
- Temp copies must be removed after success, failure, timeout, and cancellation.
- Decrypt only selected cookie values.
- Store decrypted values only in memory.
- Do not derive user-visible diagnostics from cookie values or names.
- Do not persist raw cookie headers, cookies, decrypted secrets, bearer tokens,
  session keys, OAuth tokens, or local storage tokens.
- `Debug` for in-memory session material must be redacted or unavailable.

Chromium-family decryption must use a reviewed Secret Service path or a small
reviewed crate. Firefox cookie values are read from the Firefox cookie store
only after profile behavior is verified with synthetic or throwaway profiles.

Scheduled/background refresh must not trigger surprise keyring prompts.
Interactive keyring unlock is out of scope until a later UX and schema task
explicitly accepts it.

## Web-Fetch Adapter Boundary

Provider web adapters are daemon-only and intentionally narrow.

Each adapter must declare:

- provider ID;
- allowed request hosts;
- allowed redirect hosts;
- required cookie domains;
- required cookie names where known;
- timeout;
- response size limit;
- expected response content shape;
- normalized snapshot mapping;
- redaction-safe diagnostic codes.

Rules:

- Build URLs from constants and typed parameters only.
- Do not accept arbitrary URLs from D-Bus, settings, provider responses, or
  diagnostics.
- Block redirects to unexpected hosts.
- Bound response bodies.
- Classify status, redirect, content-type, timeout, and body-size outcomes
  before parser logic.
- Do not execute scripts.
- Treat provider responses as untrusted.
- Fail closed on unexpected shapes.
- Discard raw responses after parsing.
- Persist only normalized snapshots and redaction-safe diagnostics.

The provider adapter consumes in-memory session material. It must not call
browser discovery, read files, or access keyrings directly.

Task 04D.1 Codex reconnaissance is narrower than future production provider
web support:

- the only live request target is
  `https://chatgpt.com/codex/settings/usage`;
- request, redirect, and cookie policy defaults to `chatgpt.com` only;
- `openai.com` redirects and cookies are not used unless a future task verifies
  that they are required and explicitly expands the allowlist;
- Codex cookie names are not yet verified, so the temporary exception for
  all `chatgpt.com` cookies is restricted to a marked throwaway fake home and
  opt-in reconnaissance;
- live responses may be classified, and fixture-shaped parser success remains
  the only asserted normalizer.

## Diagnostics Model

Diagnostics are schema-backed, redaction-safe, and stable enough for support.
Diagnostic events use `spec/diagnostics.schema.json` with `redacted.applied`
set to `true`.

The stable browser/web diagnostic code registry for future implementation is:

| Code | Meaning | Public state mapping |
| --- | --- | --- |
| `browser_import_started` | Browser import test or refresh import began. | none |
| `browser_import_finished` | Browser import test or refresh import finished. | none |
| `browser_not_found` | No supported browser installation was found. | `missing_dependency` |
| `browser_profile_discovered` | A safe profile descriptor was found. | none |
| `browser_profile_skipped` | A profile was skipped by policy or support. | `missing_dependency` when no usable profile remains |
| `browser_profile_not_found` | No matching profile was found for a supported browser. | `missing_dependency` |
| `browser_profile_unreadable` | Profile metadata could not be read safely. | `missing_dependency` |
| `browser_profile_locked` | Profile or store appeared temporarily locked. | `provider_unavailable` or `missing_dependency` |
| `browser_profile_unavailable` | No supported profile was available. | `missing_dependency` |
| `browser_cookie_db_missing` | Cookie DB was absent for a candidate profile. | `missing_dependency` |
| `browser_cookie_db_unreadable` | Cookie DB could not be opened or copied. | `missing_dependency` |
| `browser_cookie_db_locked` | Cookie DB was locked or copy failed due to concurrent browser access. | `provider_unavailable` |
| `browser_cookie_db_schema_unsupported` | Cookie DB schema was not recognized. | `parse_error` |
| `browser_cookie_decryption_unavailable` | Required decryption support was unavailable. | `missing_dependency` |
| `browser_cookie_decryption_failed` | Selected cookie value could not be decrypted. | `missing_dependency` |
| `browser_keyring_locked` | Secret Service/keyring is locked. | `missing_dependency` |
| `browser_keyring_unavailable` | Secret Service/keyring is unavailable. | `missing_dependency` |
| `browser_keyring_prompt_required` | Decryption would require an interactive prompt. | `missing_dependency` |
| `browser_cookie_found` | At least one provider-relevant cookie was found. | none |
| `browser_cookie_decrypted` | Provider-relevant cookie material was decrypted successfully. | none |
| `browser_cookie_missing` | No provider-relevant cookie was found. | `unauthenticated` |
| `provider_cookie_absent` | Provider adapter had no usable cookie material. | `unauthenticated` |
| `provider_cookie_rejected` | Cookie material existed but provider rejected it. | `cookie_rejected` |
| `provider_web_fetch_started` | Provider web request began. | none |
| `provider_web_fetch_finished` | Provider web request completed. | none |
| `provider_web_fetch_timeout` | Provider request timed out. | `timeout` |
| `provider_web_fetch_rate_limited` | Provider returned a rate-limit response. | `provider_unavailable` |
| `provider_web_fetch_nonzero_status` | Provider returned an unsuccessful HTTP status. | `provider_unavailable` or `cookie_rejected` |
| `provider_web_fetch_parse_error` | Provider response shape was unexpected. | `parse_error` |
| `provider_web_fetch_redaction_applied` | Redaction was applied before public output. | none |
| `provider_domain_not_allowed` | Request host was outside the allowlist. | `provider_unavailable` |
| `provider_redirect_blocked` | Redirect host was outside the allowlist. | `provider_unavailable` |
| `provider_response_too_large` | Provider response exceeded the size cap. | `parse_error` |

Allowed diagnostic `details` keys are small redacted scalars only, such as:

- `provider`;
- `browserFamily`;
- `profileId`;
- `profileDisplayName`;
- `keyringState`;
- `durationMs`;
- `httpStatusClass`;
- `redirectBlocked`;
- `responseBytes`;
- `redactionClass`;
- `recoverable`.

Forbidden diagnostic details include paths, cookie names/values, full headers,
raw URLs with query/fragment data, raw emails, account IDs, stdout/stderr, raw
provider payloads, and nested objects containing unbounded data.

Browser-import diagnostics should be available through `TestBrowserImport`
results and through a future reserved diagnostics selector `browser_import`.
Until that selector is implemented, Task 04B/04C must not rely on Shell UI
calling `GetDiagnostics("browser_import")`.

## Settings Model

Existing daemon-owned settings are sufficient for the first implementation.

Global browser settings:

- `browserImport.enabled` disables browser import globally.
- `browserImport.policy` selects `auto`, `chromium_family`, `firefox`, or
  `off`.
- `browserImport.profileIdAllowlist` restricts profiles by opaque ID.
- `browserImport.domainAllowlistMode` remains `provider_required_only`.

Provider settings:

- `enabled` gates the provider.
- `preferredSourceAdapter` selects `auto`, `upstream_cli`, `linux_web`, or
  `off`.
- `allowBrowserImport` gates browser-cookie use for that provider.
- `allowCliFallback` gates fallback to upstream CLI.

Refresh request policy:

- `sourceAdapterPolicy.mode` selects `auto`, `prefer`, `only`, or `exclude`.
- `sourceAdapterPolicy.adapters` can include `linux_web`.
- `allowStaleCacheFallback` controls stale cache fallback for the refresh.

Future runtime selection must honor all three policy layers. It must not create
parallel config files or Shell-owned GSettings keys for provider/browser data.

## Cache Model

The daemon cache stores normalized snapshots only.

Rules:

- Do not cache raw cookies, headers, provider payloads, browser profile paths,
  decrypted secrets, raw identity, or raw errors.
- Do not overwrite a useful cached snapshot with an all-failure web attempt.
- Cache write remains allowed when at least one provider normalizes to `ok`.
- Failed browser/web attempts should produce refresh results and diagnostics.
- Stale fallback preserves semantic `source` and original `sourceAdapter` when
  known.
- Non-usable states such as `unauthenticated`, `cookie_rejected`,
  `missing_dependency`, `provider_unavailable`, `parse_error`, and `timeout`
  remain distinct.

## Schema Review

Task 04A reviewed these schemas:

- `spec/browser-import-options.schema.json`;
- `spec/browser-import-result.schema.json`;
- `spec/settings.schema.json`;
- `spec/settings-patch.schema.json`;
- `spec/diagnostics.schema.json`;
- `spec/snapshot.schema.json`;
- `spec/refresh-options.schema.json`;
- `spec/refresh-result.schema.json`;
- `spec/daemon-info.schema.json`.

Conclusion: no Task 04A schema change is required.

Existing schemas already represent:

- browser import enablement and disablement;
- `auto`, `chromium_family`, `firefox`, and `off` policy;
- opaque profile IDs and profile allowlists;
- high-level keyring states;
- provider statuses including `unauthenticated` and `cookie_rejected`;
- `linux_web` as source adapter;
- `source="web"` as semantic source;
- browser-import diagnostics scope;
- refresh adapter selection;
- daemon browser/web capability booleans.

Detailed states such as `cookie_found`, `cookie_decryption_failed`,
`browser_locked`, `profile_skipped`, and `provider_rejected_cookie` are frozen
as diagnostic codes rather than new snapshot states. `cookiesFound` remains an
aggregate count only; it must never list cookie names or values and may be
`null` when not needed. `browser_keyring_prompt_required` maps to diagnostics
and `keyringState="locked"` or `keyringState="unavailable"` until an explicit
interactive prompt contract exists.

`TestBrowserImport` is a browser/cookie capability test by default. Provider
network validation belongs to future provider web adapter refresh paths, not to
Task 04B cookie import. If a later UX requires an explicit browser-only versus
provider-probe mode inside `TestBrowserImport`, that contract must be reviewed
before schema changes.

## Testing Model

Future implementation must be fixture-first and synthetic by default.

Required tests for Task 04B and later:

- synthetic Chromium-family cookie DB fixture;
- synthetic Firefox cookie DB fixture;
- fake browser profile roots under temp `HOME`/`XDG_*`;
- no real browser profiles in CI;
- no real keyring prompts in CI;
- fake Secret Service/keyring backend;
- keyring locked, unavailable, and decryption-failure paths;
- corrupt DB and unsupported schema handling;
- locked DB/temp-copy behavior;
- provider-domain-only query behavior;
- no raw cookie serialization;
- no profile path serialization;
- fake provider HTTPS/server or fixture-only web client tests;
- redirect-to-wrong-host blocked;
- response-size cap enforced;
- raw response redacted and discarded;
- same-user D-Bus `TestBrowserImport` returns public-copy-safe payloads;
- cache contains normalized snapshots only.

Live tests are ignored by default and gated with explicit environment
variables. Task 04B.1 adds `scripts/chromium-throwaway-smoke.sh`, which requires
`CODEXBAR_BROWSER_LIVE=1`, creates a marked throwaway fake home, launches only a
Chromium-family browser with a throwaway user-data-dir, seeds only a synthetic
`.example.invalid` cookie through a local test-only server, and runs an ignored
`TestBrowserImport` integration test. The script is not a daemon runtime API, is
not part of `./scripts/check.sh` or CI, binds its test server only to
`127.0.0.1`, and must not contact provider endpoints or read default user
profiles. It must not commit live output.

## Dependency And Packaging Review

Task 04A added no dependencies. Task 04B adds `rusqlite = 0.32.1` as a normal
daemon dependency for read-only SQLite cookie DB access against synthetic and
throwaway browser profiles. The dependency intentionally uses system SQLite,
not the bundled SQLite feature, so Ubuntu/Debian security updates continue to
own the SQLite runtime.

Current Rust crates and likely future APIs:

- `rusqlite` for cookie DB reads;
- `reqwest` for the Task 04D.1 daemon-only static Codex GET transport, with
  default features disabled and Rustls-oriented TLS selected;
- `url` for resolving and sanitizing redirect targets instead of ad hoc
  string joining;
- existing `zbus` or a small reviewed Secret Service crate for keyring access;
- narrowly scoped RustCrypto crates only after Chromium behavior is verified;
- an internal cookie/session material type or a small cookie crate;
- `secrecy` or `zeroize` only as defense-in-depth, not as a redaction
  substitute.

Likely Debian/Ubuntu implications:

- `pkg-config`, `libsqlite3-dev`, and runtime `libsqlite3-0` for system SQLite;
- `cmake` for the current Rustls/AWS-LC dependency graph used by `reqwest`;
- `ca-certificates` for HTTPS trust roots;
- optional `gnome-keyring` or Secret Service tooling for live smoke, not as a
  hard browser-import CI dependency;
- avoid OpenSSL/native TLS unless explicitly justified;
- avoid `libsecret` FFI unless it is safer than direct Secret Service D-Bus,
  because it adds GLib/libsecret development packaging.

CI installs `pkg-config`, `libsqlite3-dev`, `cmake`, and `ca-certificates` for
the daemon dependency graph. It must not require installed browsers, real
profiles, real provider endpoints, or an unlocked user keyring. `tempfile`
remains a test dependency only; runtime cookie DB copies use standard-library
Unix mode controls for `0700` private directories and `0600` copied files.

## Non-Goals

Task 04A through Task 04D.1 do not:

- implement real keyring access;
- enable real user browser profile scanning by default;
- enable default provider HTTP fetches;
- implement web scraping;
- add localhost or TCP APIs;
- change Shell behavior;
- change D-Bus XML;
- change JSON schemas;
- persist raw cookies;
- commit real browser profile data;
- support every upstream provider;
- support arbitrary domains or arbitrary profile paths.

## References

- Upstream CodexBar CLI docs: `https://github.com/steipete/CodexBar/blob/main/docs/cli.md`
- Upstream providers overview: `https://github.com/steipete/CodexBar/blob/main/docs/providers.md`
- Upstream provider authoring guide: `https://github.com/steipete/CodexBar/blob/main/docs/provider.md`
- Chromium user data directory: `https://chromium.googlesource.com/chromium/src/+/main/docs/user_data_dir.md`
- Chromium Linux password storage: `https://chromium.googlesource.com/chromium/src/+/main/docs/linux/password_storage.md`
- Mozilla Firefox profiles: `https://support.mozilla.org/en-US/kb/profiles-where-firefox-stores-user-data`
- Secret Service API: `https://specifications.freedesktop.org/secret-service-spec/latest-single/`
- SQLite WAL: `https://www.sqlite.org/wal.html`
