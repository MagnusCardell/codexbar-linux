# Upstream CLI Adapter

Task 02B added the production daemon adapter for upstream `codexbar` CLI
refresh. Task 06A keeps that adapter compatible with upstream CodexBar CLI
v0.25.1 while preserving the upstream-CLI-only Linux scope. The adapter is
owned by the Rust daemon and is not called from GNOME Shell, preferences,
reserved browser-import compatibility code, or any localhost API.

## Resolver

The resolver checks, in order:

1. `CODEXBAR_CLI`, used by development and tests;
2. `codexbar` on `PATH`;
3. common Linuxbrew locations:
   - `~/.linuxbrew/bin/codexbar`
   - `/home/linuxbrew/.linuxbrew/bin/codexbar`

The resolved target must be executable. Missing or non-executable binaries
become redaction-safe dependency diagnostics and degraded provider states, not
panics. `GetDaemonInfo()` reports `capabilities.upstreamCli=true` only when an
executable is resolvable. Paths shown through daemon info are display-safe and
do not expose a raw `/home/<user>` prefix.

## Command Matrix

Production usage/status refresh is targeted by provider:

```bash
codexbar --format json --json-only --provider <provider> --source cli
codexbar --format json --json-only --provider <provider> --source cli --status
```

The usage subcommand shape remains available for fixtures and fallback testing:

```bash
codexbar usage --format json --json-only --provider <provider> --source cli
```

Cost uses the upstream Codex + Claude local cost command and deliberately omits
`--source`:

```bash
codexbar cost --format json --json-only --provider both
```

This matches the v0.25.1 compatibility target: upstream `--provider
<id|both|all>` defaults are owned by upstream config when the CLI is called
directly, but the daemon continues to pass explicit provider targets selected
from daemon settings. The built-in daemon defaults are `codex`, then `claude`.
Explicit `RefreshOptions.providers` still override daemon settings. The daemon
does not default usage/status refresh to `--provider all`.

Provider inventory is discovered from bounded upstream help output:

```bash
codexbar --help
```

The daemon parses only the safe `--provider` enum IDs from help text, filters
the pseudo-provider values `all` and `both`, and exposes the result as
`upstreamCli.providerInventory` in normalized daemon/snapshot JSON. This is an
inventory hint for Preferences; it is not provider I/O and it does not fetch
provider data.

The adapter does not default usage/status to `--provider all` because the
promoted live Linux evidence timed out for all-provider usage/status with empty
stdout/stderr. If refresh options request `providers:["all"]`, the adapter may
run that explicit probe and report timeout/error diagnostics without erasing
useful cached data.

## Provider Selection

Provider targets are selected in this order:

1. non-empty `RefreshOptions.providers`;
2. enabled daemon settings with CLI fallback allowed and source adapter not `off`;
3. built-in defaults `codex`, then `claude`, only when the daemon provider
   settings map is empty.

`codex` is the first proven Linux provider in the promoted live usage/status
fixtures. Browser import and Linux web adapters are unsupported compatibility
surface and do not run. Web/auto upstream source paths remain upstream Linux
limitations unless the upstream CLI itself supports them later.

Preferences can show a broader provider catalog from the built-in compatibility
list plus upstream `providerInventory`, daemon settings, and the latest
snapshot. Enabling one of those providers writes a normal provider setting; the
daemon then targets that provider directly with `--source cli`. The Shell
popover remains bounded to configured/default providers and providers already
present in snapshots.

An empty legacy provider settings map is migrated to the v0.1 built-in default
set and targets `codex`, then `claude`. A non-empty provider settings map is
deliberate user configuration. If
every configured provider is disabled, set to source `off`, or has CLI fallback
disabled, the daemon returns a no-op refresh with
`refresh_no_enabled_providers`; it does not silently re-enable `codex`.

## Normalization

The adapter normalizes upstream JSON into the frozen
`spec/snapshot.schema.json` provider contract:

- upstream CLI sources such as `codex-cli`, `claude`, `cli`, and `local`
  become semantic `source: "local"`;
- upstream CLI sources such as `oauth` and `api` become semantic
  `source: "api"`;
- upstream CLI sources such as `openai-web` and `web` become semantic
  `source: "web"`;
- the implementation boundary is always `sourceAdapter: "upstream_cli"`;
- usage meters preserve `usedPercent`, reset timestamps, and window minutes;
- status output is optional and merged only into schema-supported status fields;
- cost output is reduced to bounded provider cost summaries;
- raw daily cost chronology, model breakdowns, model lists, stdout/stderr, raw
  provider payloads, raw identity fields, and local file paths are not cached or
  exposed.

Raw `accountEmail`, organization, provider account id, token, cookie, header,
`rawPayload`, and `rawResponse` fields are transformed or discarded before
diagnostics, cache writes, D-Bus payloads, and test public-payload assertions.

An upstream semantic `web` label can appear in normalized provider metadata
when upstream CLI generated that payload. It does not mean the Linux daemon
performed a local browser-cookie import, provider dashboard fetch, WebKit flow,
or Linux web adapter refresh. Local Linux browser/web adapters remain
unsupported and disabled.

## Refresh And Cache

`sourceAdapterPolicy` keeps fixture refresh explicit for tests and local
development. Auto production refresh tries `upstream_cli` when the binary is
available. If upstream refresh fails and stale fallback is allowed, the daemon
serves the existing normalized cache as stale rather than overwriting it with an
error-only snapshot. Error-only or synthetic snapshots do not replace a useful
cache.

## Opt-In Live Smoke Tests

Normal validation and CI do not require a live upstream binary. The live smoke
tests are ignored Rust tests and run only when explicitly requested with
`CODEXBAR_LIVE=1` and `CODEXBAR_CLI=/path/to/codexbar`.

Adapter refresh smoke:

```bash
CODEXBAR_LIVE=1 CODEXBAR_CLI=/path/to/codexbar \
  cargo test --manifest-path daemon/Cargo.toml \
  live_upstream_cli_refresh_codex_smoke_redacts_outputs \
  -- --ignored --nocapture --test-threads=1
```

D-Bus refresh smoke:

```bash
CODEXBAR_LIVE=1 CODEXBAR_CLI=/path/to/codexbar \
  dbus-run-session -- cargo test --manifest-path daemon/Cargo.toml \
  live_dbus_upstream_cli_refresh_smoke_redacts_outputs \
  -- --ignored --nocapture --test-threads=1
```

The D-Bus smoke uses the frozen refresh-options object form:

```json
{
  "schemaVersion": 1,
  "reason": "manual",
  "force": true,
  "busyBehavior": "return_existing",
  "sourceAdapterPolicy": {
    "mode": "only",
    "adapters": ["upstream_cli"],
    "allowStaleCacheFallback": false
  },
  "providers": ["codex"]
}
```

Both live tests validate schema-shaped public payloads and scan normalized
snapshots, refresh results, diagnostics, provider events, daemon info, and cache
strings for raw emails, home paths, token/cookie/auth markers, `rawResponse`,
and `rawPayload`. They do not commit live output and do not expose raw
stdout/stderr.

For manual upstream v0.25.1 smoke outside CI, run:

```bash
/path/to/codexbar --version
/path/to/codexbar --format json --json-only --provider codex --source cli
/path/to/codexbar --format json --json-only --provider claude --source cli
/path/to/codexbar cost --format json --json-only --provider both
/path/to/codexbar config validate --format json --json-only
```

To capture reviewed redacted evidence without committing private output:

```bash
CODEXBAR_CAPTURE_LIVE=1 CODEXBAR_CLI=/path/to/codexbar \
  ./scripts/capture-upstream-cli-samples.sh \
  --output /tmp/codexbar-upstream-cli-v0251 \
  --allow-provider-network \
  --providers codex,claude \
  --provider-source cli \
  --include-config-validate

./scripts/validate-upstream-cli-capture.sh /tmp/codexbar-upstream-cli-v0251
```

Live v0.25.1 capture is optional and must not be required for normal CI.
