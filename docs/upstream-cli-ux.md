# Upstream CLI UX States

Task 05D uses this file as the implementation-facing copy table for the
upstream-CLI-only v0.1 experience. Keep daemon `safeMessage` strings short,
redaction-safe, and stable enough for Shell UI tests. Do not include raw
stdout, stderr, provider payloads, raw identity, or raw filesystem paths in
diagnostics or normal UI.

## State Table

| Condition | Stable code | Provider state | User-facing copy | Primary actions |
| --- | --- | --- | --- | --- |
| Upstream CLI missing | `upstream_cli_missing` | `missing_dependency` | CodexBar CLI was not found. Install upstream CodexBar CLI or set CODEXBAR_CLI. | Refresh, Copy diagnostics, read setup docs |
| `CODEXBAR_CLI` path not executable | `upstream_cli_not_executable` | `missing_dependency` | Configured CodexBar CLI path is not executable. | Fix executable permissions or path, Refresh, Copy diagnostics |
| Upstream CLI version detected | `upstream_cli_version_detected` | unchanged | CodexBar CLI available. | Refresh |
| Upstream CLI timeout | `upstream_cli_timeout` | `timeout` | CodexBar CLI timed out. | Refresh, Copy diagnostics |
| Upstream CLI malformed JSON | `upstream_cli_parse_error` | `parse_error` | CodexBar CLI returned output that could not be parsed. | Refresh, Copy diagnostics |
| Upstream CLI nonzero exit | `upstream_cli_nonzero_exit` or a mapped provider code | `provider_unavailable`, `unauthenticated`, `missing_dependency`, or `error` | Provider data was unavailable from CodexBar CLI. | Refresh, Copy diagnostics |
| Provider CLI missing | `upstream_cli_provider_cli_missing` | `missing_dependency` | Provider CLI dependency was not found. | Refresh, Copy diagnostics |
| Provider unauthenticated | `upstream_cli_unauthenticated` | `unauthenticated` | Provider sign-in is required in the upstream CLI. | Refresh, Copy diagnostics |
| Provider rate limited | `upstream_cli_provider_rate_limited` | `provider_unavailable` | Provider is rate limited. Try again later. | Refresh, Copy diagnostics |
| Provider unavailable | `upstream_cli_provider_unavailable` | `provider_unavailable` | Provider data was unavailable from CodexBar CLI. | Refresh, Copy diagnostics |
| Unsupported CLI source/capability | `upstream_cli_capability_unimplemented` or `upstream_cli_unsupported_source` | `provider_unavailable` or `missing_dependency` | Requested provider source is not available on Linux through CodexBar CLI. | Refresh, Copy diagnostics |
| Local cost unavailable | `upstream_cli_cost_unavailable` | provider usage state unchanged | Local cost data was unavailable. | Refresh, Copy diagnostics |
| Stale cache used | `stale_cache_used` | `stale` | Showing cached usage data. | Refresh, Copy diagnostics |
| Refresh succeeded | none required | `ok` | Usage data is current. | Refresh |
| Partial refresh succeeded | warning codes for failed providers or cost | `ok` for refreshed providers, degraded states for failures | Some usage data refreshed. Unavailable providers are shown separately. | Refresh, Copy diagnostics |

## Daemon Safe Messages

Use these exact messages for the core daemon diagnostic events:

- `upstream_cli_missing`: `CodexBar CLI was not found. Install upstream CodexBar CLI or set CODEXBAR_CLI.`
- `upstream_cli_not_executable`: `Configured CodexBar CLI path is not executable.`
- `upstream_cli_timeout`: `CodexBar CLI timed out.`
- `upstream_cli_parse_error`: `CodexBar CLI returned output that could not be parsed.`
- `upstream_cli_provider_error`: `Provider data was unavailable from CodexBar CLI.`
- `upstream_cli_cost_unavailable`: `Local cost data was unavailable.`
- `stale_cache_used`: `Showing cached usage data.`

Provider-specific mapping may use a more specific code when the raw upstream
message clearly fits one of the buckets below, but the raw upstream message
must remain hidden:

- Linux/macOS unsupported-source messages -> `upstream_cli_unsupported_source`
  or `upstream_cli_capability_unimplemented`.
- Missing provider executable or CLI session messages -> `upstream_cli_provider_cli_missing`
  or `upstream_cli_unauthenticated`.
- Login/auth prompts -> `upstream_cli_unauthenticated`.
- Rate-limit messages -> `upstream_cli_provider_rate_limited`.
- Timeout metadata -> `upstream_cli_timeout`.
- Malformed, empty, multiple-document, truncated, or binary stdout -> parse or
  output-limit diagnostics.

## Shell Presentation

- Missing upstream CLI is a first-run setup state, not a crash state.
- Stale snapshots should say `Stale data` or `Showing cached data.`
- Auth and provider dependency states should be concise and setup-oriented.
- Diagnostics remain collapsed until requested.
- Footer copy must communicate upstream CLI availability and that browser import
  and web adapters are unsupported.
- Normal UI must not show raw JSON, stdout, stderr, raw paths, tokens, raw
  identity, or provider payloads.

## Scope Guard

Task 05D must not add browser-cookie access, browser profile discovery, cookie
database reads, keyring access, provider web fetches, browser extension behavior,
localhost/TCP API, Shell subprocess provider reads, or Shell cache reads.
