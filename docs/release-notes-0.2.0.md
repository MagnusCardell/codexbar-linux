# CodexBar GNOME v0.2.0 Release Notes

CodexBar GNOME v0.2.0 is a compatibility cleanup release theme for the native
Ubuntu/GNOME companion. The compatibility target is upstream CodexBar CLI
v0.25.1. The supported production data plane remains upstream `codexbar` CLI
and local provider tooling through the user-scoped daemon.

## Compatibility Target

- Upstream CodexBar CLI target: v0.25.1.
- Package/GNOME behavior: keep the v0.1 development package path and GNOME
  Shell behavior intact.
- Runtime usage/status strategy: target providers individually through
  `--source cli`; default daemon providers remain `codex`, then `claude`.
- Runtime cost strategy: use local Codex + Claude cost output through:

```bash
codexbar cost --format json --json-only --provider both
```

The daemon does not add `--source` to the cost command.

## Command Strategy

The daemon continues to invoke usage/status by provider:

```bash
codexbar --format json --json-only --provider codex --source cli
codexbar --format json --json-only --provider codex --source cli --status
codexbar --format json --json-only --provider claude --source cli
codexbar --format json --json-only --provider claude --source cli --status
```

Explicit `RefreshOptions.providers` still override daemon provider settings.
All-provider usage/status probes remain explicit only; the daemon does not
default to `--provider all` for usage or status.

## Source Labels

v0.2.0 normalizes the upstream v0.25.1 source labels used by current CLI
payloads:

- `codex-cli`, `claude`, `cli`, and `local` become semantic `source: "local"`.
- `openai-web` and `web` become semantic `source: "web"`.
- `oauth` and `api` become semantic `source: "api"`.

`sourceAdapter` remains the implementation boundary. Upstream CLI payloads are
still normalized as `sourceAdapter: "upstream_cli"`, including payloads whose
provider semantic source is `web`, `oauth`, or `api`.

## No-Browser Scope

v0.2.0 keeps the no-browser decision intact. The project still does not read
browser cookies, browser profiles, browser cookie databases, desktop keyrings,
provider dashboards, provider session material, or provider web pages. It does
not install a browser extension and does not expose a localhost/TCP API.

The provider-level settings schema now defaults `allowBrowserImport` to `false`,
matching the runtime normalization that already forced browser import off.

## Fixtures And Validation

The committed upstream CLI fixture corpus now includes doc-derived v0.25.1
compatibility fixtures for:

- `cost_both_success`
- `usage_codex_cli_success`
- `usage_claude_cli_success`
- `source_oauth_semantic`
- `source_api_semantic`

The existing reviewed Linux unsupported `web` and `auto` source failures remain
part of the corpus. Live v0.25.1 capture is optional and must stay outside
normal CI.

## Optional Live Smoke

Operators can run these commands against a local upstream v0.25.1 binary:

```bash
/path/to/codexbar --version
/path/to/codexbar --format json --json-only --provider codex --source cli
/path/to/codexbar --format json --json-only --provider claude --source cli
/path/to/codexbar cost --format json --json-only --provider both
/path/to/codexbar config validate --format json --json-only
```

To capture redacted local evidence:

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

Do not commit raw live output or private terminal samples.

## Contract Impact

- D-Bus XML: unchanged.
- Snapshot/settings schema shape: unchanged.
- Settings schema default: `providers.*.allowBrowserImport.default` is now
  `false`.
- Data plane: unchanged; no browser-cookie, keyring, provider-web, localhost,
  Shell subprocess, or Shell cache-read behavior was added.
