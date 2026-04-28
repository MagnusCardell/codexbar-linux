# Upstream CLI Observations

Task 02A records upstream `codexbar` CLI evidence without implementing the
production adapter. The current committed corpus is a safe baseline only:
doc-derived samples from upstream public docs plus synthetic error samples. No
local live Linux capture was available in this workspace.

## Sources Inspected

- `steipete/CodexBar` README
- `steipete/CodexBar` `docs/cli.md`
- `steipete/CodexBar` `docs/providers.md`
- `steipete/CodexBar` `docs/provider.md`

## Version Observed

- Local upstream CLI path: not available.
- Local upstream CLI version: not available.
- Documentation sample version fields include provider-level examples such as
  `0.6.0`; that is not a verified Linux binary version.

Task 02B remains blocked on reviewed real redacted Linux samples from an
installed upstream `codexbar` binary.

## Commands Attempted Or Modeled

The capture harness models this bounded read-only command matrix:

- `codexbar --version`
- `codexbar config validate --format json --json-only`
- `codexbar --format json --json-only --provider all`
- `codexbar usage --format json --json-only --provider all`
- `codexbar cost --format json --json-only --provider all`
- `codexbar --format json --json-only --provider all --status`
- `codexbar --format json --json-only --provider all --source web`
- `codexbar --format json --json-only --provider __codexbar_linux_invalid_provider__`

The script does not mutate `~/.codexbar/config.json`. `config dump` capture is
behind an explicit `--include-config-dump` flag because config dumps can contain
secrets and need manual review even after redaction.

## Linux Source Behavior

Upstream CLI documentation states:

- `--source auto` and `--source web` are macOS-only for web/browser-cookie
  flows.
- On Linux, `web` and `auto` are not supported and the CLI exits non-zero.
- `--json-only` suppresses non-JSON output and reports errors as JSON payloads.

This repository has a synthetic `unsupported_source` fixture for `--source web`.
`--source auto` was not locally observed because the binary is unavailable; it
must be captured before Task 02B relies on exact exit-code or stderr behavior.

## Usage JSON Shape Summary

The upstream docs show a provider usage payload with these notable fields:

- `provider`, `version`, and upstream `source` label.
- `status` object with `indicator`, `description`, `updatedAt`, and `url`.
- `usage.primary`, `usage.secondary`, and `usage.tertiary` meters.
- `usage.updatedAt`.
- raw upstream identity fields under `usage.identity`, plus duplicated
  top-level usage identity fields such as `accountEmail`,
  `accountOrganization`, and `loginMethod`.
- optional `credits`.
- provider-specific dashboard extras such as `openaiDashboard`.

The committed doc-derived fixture keeps field names but redacts identity values.
Task 02B must not pass raw upstream identity fields into normalized snapshots.

## Cost JSON Shape Summary

The upstream docs describe `codexbar cost --format json` as an array of provider
payloads with:

- `provider`, `source`, `updatedAt`;
- `sessionTokens`, `sessionCostUSD`;
- `last30DaysTokens`, `last30DaysCostUSD`;
- `daily[]` rows with token counts, total cost, models used, and model
  breakdowns;
- `totals` with token and cost aggregates.

Task 02B must map this into the bounded `cost` summary in
`spec/snapshot.schema.json`, not preserve arbitrary upstream cost payloads.

## Error Shape Summary

Current committed error samples are synthetic because no local binary was
available:

- `missing_binary`: capture harness cannot locate `codexbar`.
- `timeout_synthetic`: command exceeded a bounded timeout.
- `parse_error_synthetic`: stdout was not parseable JSON.
- `unsupported_source`: Linux web source request failed.
- `invalid_provider`: invalid provider id failed.
- `usage_error`: stderr redaction stress sample.

Real Linux samples must confirm exact upstream exit codes, stdout/stderr JSON
shape, and whether unsupported `auto` behaves identically to unsupported `web`.

## Fields To Discard Or Redact Before Cache/D-Bus

Task 02B must discard or transform at least:

- raw `accountEmail`, `signedInEmail`, `accountOrganization`, and provider
  account IDs;
- raw provider-specific dashboard payloads such as `openaiDashboard` except for
  explicitly normalized safe summaries;
- raw `raw`, `rawResponse`, `rawPayload`, `headers`, `cookies`, `cookie`,
  `authorization`, `token`, `accessToken`, `refreshToken`, `sessionKey`,
  `apiKey`, `password`, and `secret` fields;
- raw home paths, browser profile paths, and debug dump paths;
- raw stderr/stdout snippets before diagnostics/logging.

Allowed normalized identity is limited to the frozen fields in
`docs/CONTRACTS.md`: masked display values and non-reversible hashes.

## Open Questions For Task 02B

- What exact JSON shape does `--provider all` emit when multiple providers are
  enabled on Linux: one object, an array, or an envelope?
- What exact exit codes and JSON error payloads are emitted for unsupported
  `--source web`, unsupported `--source auto`, and invalid provider ids?
- Does Linux `--json-only` always produce JSON on stderr/stdout for failures?
- Which upstream `source` labels should map to semantic `api`, `local`, `web`,
  or `unknown`?
- Which provider-specific extras are safe and useful enough to normalize, and
  which must become diagnostics or be discarded?
- Can cost output be absent or partial per provider while usage succeeds?
- What stdout/stderr byte limits are appropriate for the production runner?
