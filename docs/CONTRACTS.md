# P0A contract freeze addendum

This document closes the ambiguity surfaced by the kickoff review. Treat it as the authoritative v1 contract until a later ADR intentionally changes it.

## Settings ownership

CodexBar GNOME has two settings domains.

### GSettings-owned Shell UI preferences

The GNOME Shell extension and preferences UI own presentation settings through `schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml`.

Required v1 keys:

| Key | Type | Values | Owner |
|---|---:|---|---|
| `start-daemon-on-login` | boolean | `true` / `false` | reserved; hidden in v0.1 because D-Bus activation starts the daemon on demand |
| `panel-mode` | string | `merged`, `provider`, `minimal` | Shell UI |
| `reset-time-format` | string | `countdown`, `absolute`, `both` | Shell UI |
| `theme` | string | `system`, `compact`, `high_contrast` | Shell UI |
| `selected-provider` | string | provider id or empty | Shell UI |

The Shell process may read GSettings and D-Bus only. It must not read daemon cache files in production.

### Daemon-owned provider/refresh settings

The daemon owns `spec/settings.schema.json` and persists it under `${XDG_CONFIG_HOME:-~/.config}/codexbar-linux/config.json` with `0600` file permissions and `0700` directory permissions.

Daemon-owned settings include:

- refresh interval and stale-cache fallback;
- provider enablement;
- provider source adapter preference;
- diagnostics verbosity.

`refresh.intervalSeconds` defaults to 300 seconds. The value `0` means
manual/off mode for scheduled interval refresh only; it does not disable
startup refresh, which remains controlled by `refresh.startupRefresh`.

Provider selection defaults empty-provider refreshes to `codex` only when the
provider settings map is empty. Once the settings map is non-empty, disabled
providers, providers whose preferred source adapter is `off`, and providers
with CLI fallback disabled are excluded. If no configured provider remains, the
daemon returns a schema-valid `noop` refresh with
`refresh_no_enabled_providers` instead of silently refreshing `codex`.

The v1 settings schema still contains browser-import and Linux-web fields for
compatibility with the frozen contract. In the no-browser product scope those
fields are deprecated, normalized off by the daemon, and must not trigger
browser profile scanning, keyring access, cookie reads, or provider web fetches.

The preferences UI configures daemon refresh/provider settings through `SetSettingsPatch(patch_json)` after the daemon exists. It may read the documented config file path to populate controls before the daemon replies, but daemon-owned writes should prefer D-Bus so validation, permissions, scheduler reschedule, and redaction stay centralized.

`SetSettingsPatch(patch_json)` accepts `spec/settings-patch.schema.json`. It is a partial update object, not an RFC 7396 merge patch:

- `schemaVersion` is required and must be `1`.
- Top-level sections are optional.
- Omitted fields are unchanged.
- `null` does not delete or reset fields unless a field explicitly allows `null` in the patch schema.
- After applying a patch, the daemon must validate the full settings object against `spec/settings.schema.json`.
- Invalid JSON or schema-invalid patches raise `org.codexbar.Linux1.Error.InvalidJson`; syntactically valid patches rejected by daemon policy raise `org.codexbar.Linux1.Error.InvalidSettingsPatch`.

## Source taxonomy

Do not conflate the provider semantic source with the implementation data plane.

`source` describes the provider-level semantic source:

- `api` — provider API, OAuth-backed provider result, or upstream CLI API-derived result;
- `local` — local CLI/config/state result;
- `web` — provider web/session result if reported by upstream provider semantics; not a local daemon web fetch;
- `unknown` — source is unknown or unavailable.

`sourceAdapter` describes the implementation adapter that produced the normalized data:

- `upstream_cli` — upstream `codexbar` CLI output;
- `linux_web` — deprecated compatibility value; local Linux web adapters are unsupported and must not run;
- `cache` — cache-only/synthetic fallback where the original adapter is unknown;
- `fixture` — test/dev fixture source;
- `synthetic` — daemon-generated placeholder/error state;
- `none` — no adapter was attempted or available.

`auto` is never emitted in snapshots. It is a settings/input policy only.

Shell UI must not conflate these fields. Provider cards may display `source` as provider semantic metadata. `sourceAdapter` is secondary/diagnostic metadata; adapter values such as `fixture`, `synthetic`, and `cache` must never be presented as provider semantic source.

## Identity contract

Snapshots, cache, D-Bus outputs, diagnostics, logs, and fixtures must not contain raw account emails, raw organization names, raw provider account IDs, cookies, headers, bearer/API/session tokens, OAuth tokens, browser profile absolute paths, or raw provider payloads.

Allowed identity fields are:

- `accountEmailDisplay` — masked display string such as `m***@example.com`;
- `accountEmailHash` — non-reversible local hash/HMAC used only for account disambiguation;
- `accountOrganizationDisplay` — masked or generalized organization display;
- `accountOrganizationHash` — non-reversible local hash/HMAC;
- `providerAccountIdHash` — non-reversible local hash/HMAC;
- `loginMethod` — safe high-level string such as `api_key`, `oauth`, `upstream_cli`, or `unknown`. The legacy `browser_cookie` value is reserved for compatibility only and is not produced by the no-browser daemon.

If upstream returns raw identity fields, the daemon must normalize them immediately and discard raw values before cache/D-Bus/logging. The Shell never receives raw identity.

## Refresh semantics

`Refresh(options_json)` accepts `spec/refresh-options.schema.json` and returns a `refresh_id` immediately.

Default behavior:

- `busyBehavior` defaults to `return_existing`.
- If a refresh is already running and `busyBehavior=return_existing`, `Refresh` returns the active refresh id and does not queue a second refresh.
- If a refresh is already running and `busyBehavior=reject`, `Refresh` raises `org.codexbar.Linux1.Error.RefreshBusy`.
- v1 does not queue refreshes and does not cancel in-flight refreshes.
- `force=true` bypasses interval throttling but does not override busy handling.
- Manual refresh is always accepted unless busy behavior explicitly rejects it or the JSON is invalid.

`RefreshStarted(refresh_id)` is emitted when work starts. `RefreshFinished(refresh_id, result_json)` carries `spec/refresh-result.schema.json`.

`SnapshotChanged(snapshot_json)` carries a complete `spec/snapshot.schema.json` snapshot.

`ProviderChanged(provider_id, provider_event_json)` carries `spec/provider-event.schema.json`. The embedded `provider` must be a full normalized provider object, not a partial patch. This avoids UI merge bugs and makes fixture testing simpler.

For `ProviderChanged`, the D-Bus `provider_id` argument, `provider_event_json.providerId`, and `provider_event_json.provider.provider` must match.

## D-Bus error names

The service must use these stable error names:

| Error name | Meaning |
|---|---|
| `org.codexbar.Linux1.Error.InvalidJson` | Input JSON is invalid or fails the payload schema. |
| `org.codexbar.Linux1.Error.InvalidSettingsPatch` | Settings patch is syntactically valid JSON but invalid or rejected. |
| `org.codexbar.Linux1.Error.RefreshBusy` | Caller requested `busyBehavior=reject` while a refresh is running. |
| `org.codexbar.Linux1.Error.DependencyUnavailable` | Required dependency such as upstream CLI or local provider tooling is unavailable for the requested operation. |
| `org.codexbar.Linux1.Error.CapabilityUnimplemented` | The method/capability is part of v1 surface but not implemented in the current phase. |
| `org.codexbar.Linux1.Error.Internal` | Redacted internal failure. Details must be safe and actionable. |

Future errors may be added only with docs, tests, and UI fallback behavior.

## Diagnostics contract

`GetDiagnostics(provider_id)` returns `spec/diagnostics.schema.json`.

- `provider_id=""` or `provider_id="global"` returns global diagnostics.
- Provider diagnostics use the provider id.
- `scope=browser_import` is retained only for the compatibility no-op method and must not imply active browser access.
- Every event has a stable `code`, `severity`, `safeMessage`, `timestamp`, and `redacted.applied=true`.
- `details` may contain small scalar redacted values only.
- Copy-diagnostics uses this payload after one more redaction pass.

`TestBrowserImport(options_json)` accepts `spec/browser-import-options.schema.json` and returns `spec/browser-import-result.schema.json`. The method is reserved and unsupported in the no-browser product scope. The daemon must validate JSON/schema and return a schema-valid result with `status=not_implemented`, empty `profiles`, provider results with `sourceAdapter=none`, and safe diagnostic codes. It must not inspect browser profiles, keyrings, cookie stores, provider endpoints, daemon cache files, or settings files.

## Cache contract

The daemon cache stores normalized snapshots only.

- Cache directory: `${XDG_CACHE_HOME:-~/.cache}/codexbar-linux`, mode `0700`.
- Cache file: `snapshot.json`, mode `0600`.
- Writes: temporary file in same directory, write, flush, fsync file, rename, best-effort fsync directory.
- Cache must never contain raw cookies, headers, provider payloads, raw identity, raw paths to browser profiles, or unredacted errors.
- Production Shell code must not parse cache files. Cache exists for daemon startup and stale-state rendering through D-Bus.

When the daemon serves cached data because startup refresh has not run or live refresh failed:

- Preserve the cached snapshot `generatedAt`, provider `updatedAt`, provider semantic `source`, and original `sourceAdapter` when known.
- Set top-level `stale=true`.
- For providers with previously usable data, set `state=stale` and set `staleSince` if it was not already set.
- Preserve non-usable provider states such as `unauthenticated`, `cookie_rejected`, `missing_dependency`, `provider_unavailable`, `parse_error`, `timeout`, and `error` unless the refresh result supplies a newer normalized provider state. `cookie_rejected` is a legacy/reserved state and is not produced by local browser-cookie logic.
- Use `sourceAdapter=cache` only for cache-only fallback records where the original adapter is unknown.

## Fixtures

Use shared normalized snapshots under `fixtures/snapshots/` as the source of truth for daemon and Shell tests. Shell-specific visual fixtures may reference these or copy them only when a test requires intentional divergence.

Required fixture states by Task 03:

- `loading`
- `ok`
- `stale`
- `unauthenticated`
- `cookie_rejected`
- `missing_dependency`
- `provider_unavailable`
- `parse_error`
- `timeout`
- `error`

## Upstream CLI samples

The CLI normalizer must not be implemented from memory. Task 02 must add redacted upstream Linux samples under `daemon/fixtures/upstream-cli/` before normalization code lands.
