# Upstream CLI Observations

Task 02A recorded upstream `codexbar` CLI evidence without implementing the
production adapter. Task 06A updated the compatibility target to upstream
CodexBar CLI v0.26.1. A 2026-06-12 upstream review retargeted this project to
the latest published upstream release only: GitHub Releases `latest` redirected
to v0.33.0, and the v0.33.0 tag has `version.env`
`MARKETING_VERSION=0.33.0`. Upstream `main` already contains later unreleased
source (`0.34.1` in `version.env`), but that is not the Linux toolbar target
until it is published as a release. The committed corpus still includes older
safe samples as historical evidence, not as a dual-version support promise.

## Sources Inspected

- `steipete/CodexBar` README
- `steipete/CodexBar` `docs/cli.md`
- `steipete/CodexBar` `docs/providers.md`
- `steipete/CodexBar` `docs/provider.md`
- `steipete/CodexBar` `CHANGELOG.md`
- `steipete/CodexBar` `version.env` at tag `v0.33.0`
- `steipete/CodexBar` `Sources/CodexBarCLI/CLIOptions.swift`
- `steipete/CodexBar` `Sources/CodexBarCLI/CLIHelpers.swift`
- `steipete/CodexBar` `Sources/CodexBarCore/Providers/Providers.swift` at tag
  `v0.33.0`

## Version Observed

- Local upstream CLI path: captured only as `[REDACTED_PATH]` in committed
  metadata.
- Local upstream CLI version: `codexbar --version` returned `CodexBar` with no
  semantic version string in the promoted live capture.
- Documentation sample version fields include provider-level examples such as
  `0.6.0`; that is not a verified Linux binary version.
- v0.33.0 is the current supported upstream target. v0.26.1 observations remain
  historical evidence only.
- The 2026-06-12 source review did not promote private live terminal output and
  did not replace the fixture corpus with unreviewed v0.33.0 samples.
- Upstream v0.33.0 public docs describe Linux CLI tarballs, Homebrew formula
  install, and an AUR package. The v0.33.0 provider enum contains 48 provider
  IDs and a descriptor-driven provider registry.
- Upstream `main` uses `version.env` `MARKETING_VERSION=0.34.1`, while the
  latest release page opened during review redirected to v0.33.0. Treat later
  `main` changes as out of scope until published.

## 2026-06-12 Upstream Delta Summary

The current upstream command surface still preserves the Linux-safe commands
used by the daemon:

- `codexbar --format json --json-only --provider <provider> --source cli`
- `codexbar --format json --json-only --provider <provider> --source cli --status`
- `codexbar cost --format json --json-only --provider both`
- `codexbar config validate --format json --json-only`

Notable v0.33.0 additions or clarified behaviors relative to the old local
notes:

- upstream docs now register 48 provider IDs;
- `--provider` defaults to enabled providers in upstream config, with `all` for
  every registered provider and `both` for the primary Codex/Claude set;
- `--account`, `--account-index`, and `--all-accounts` select token accounts or
  all visible Codex accounts for single-provider queries;
- `codexbar cache clear` can clear browser-cookie caches and local cost caches;
- `codexbar config set-api-key`, `config enable`, and `config disable` manage
  upstream provider settings directly;
- `codexbar serve` has a richer localhost cache model, request timeout, config
  reloads, and loopback/Host restrictions, but remains outside this product's
  D-Bus-only data plane;
- upstream help text is descriptor-driven: `Provider to query:
  codex|...|both|all`.

Reflection decision:

- Import: provider inventory parsing should target the v0.33.0
  descriptor-driven help text and continue filtering pseudo-providers `all` and
  `both`.
- Adapt: docs should describe v0.33.0 release evidence while keeping live
  fixture promotion opt-in and reviewed.
- Defer: account-selection flags need a snapshot/settings contract before the
  daemon can expose multi-account selection safely.
- Reject: `serve`, browser cookie cache clearing, keyring/browser import,
  provider web fetching, and upstream config writes remain outside the Linux
  bar runtime.

Task 02B implements the production daemon adapter from the reviewed live
evidence for config validation, cost output, unsupported-source errors,
invalid-provider errors, all-provider timeout behavior, and targeted Codex
usage/status success. The all-provider `--source cli` usage and status probes
timed out with empty stdout/stderr, so the runtime adapter uses targeted
provider probes instead of relying on one monolithic all-provider call.

## Capture Harness Scope

Live capture is local-only and opt-in. The default live path, with or without
explicit `--metadata-only`, captures only:

- `codexbar --version`

Additional read-only probes are individually gated:

- `--include-config-validate` adds
  `codexbar config validate --format json --json-only`.
- `--allow-provider-network` adds usage, cost, and status probes that may
  contact providers through upstream CLI behavior. On Linux these success probes
  default to `--provider-source cli`.
- `--providers LIST` limits usage/default/status success probes to a
  comma-separated provider list such as `codex,claude`; when omitted, the
  target remains `all`. Targeted fixture ids include provider and source, such
  as `usage_codex_cli_default`, `usage_claude_cli_subcommand`, and
  `status_codex_cli`.
- `--provider-source SOURCE` selects the source for usage/default/status
  success probes. Allowed values are `cli`, `auto`, `web`, `oauth`, and `api`;
  `cli` is the expected Linux success path, while `auto` and `web` are expected
  Linux unsupported-source paths when they require browser/WebKit access.
  `oauth` and `api` are capture-only source options for upstream CLI evidence;
  they are not default daemon command paths.
- `--usage-timeout`, `--cost-timeout`, and `--version-timeout` tune the bounded
  command timeouts recorded in live metadata.
- `--include-error-probes` adds unsupported-source and invalid-provider probes
  and requires `--allow-provider-network`.
- `--include-config-dump` adds `codexbar config dump --pretty` and requires a
  second explicit confirmation because config dumps may contain secrets before
  redaction.

The full opted-in matrix is:

- `codexbar config validate --format json --json-only`
- `codexbar --format json --json-only --provider <provider> --source cli`
- `codexbar usage --format json --json-only --provider <provider> --source cli`
- `codexbar cost --format json --json-only --provider both`
- `codexbar --format json --json-only --provider <provider> --source cli --status`
- `codexbar --format json --json-only --provider all --source web`
- `codexbar --format json --json-only --provider all --source auto`
- `codexbar --format json --json-only --provider __codexbar_linux_invalid_provider__`
- `codexbar config dump --pretty`

The script does not mutate `~/.codexbar/config.json`. `config dump` capture is
behind `--include-config-dump` plus a second confirmation because config dumps
can contain secrets and need manual review even after redaction. Live capture
writes `manifest.live-<timestamp>.json` sidecars by default and refuses to
write any live capture manifest under `daemon/fixtures/upstream-cli/` unless
the same explicit committed-fixture override is set for manual promotion. Raw
terminal output must never be promoted.

## Linux Source Behavior

Upstream CLI documentation states:

- `--source auto` and `--source web` are macOS-only for web/browser-cookie
  flows.
- On Linux, `web` and `auto` are not supported and the CLI exits non-zero.
- `--json-only` suppresses non-JSON output and reports errors as JSON payloads.

Manual Linux testing for Task 02A.2 additionally showed:

- plain/default source attempts `auto` and fails on Linux with the macOS-only
  web-support error;
- `usage --format json --pretty --provider all --source cli` succeeds, making
  `cli` the expected Linux success source;
- `--source auto` and `--source web` produce JSON runtime errors on Linux;
- cost capture must use `--json-only` because pretty mode may emit human warning
  text before or around the machine-readable payload;
- raw output can contain `accountEmail`, nested `identity.accountEmail`,
  `/home/...`, `~/.local/share/...`, and `auth.json` paths before redaction.

This repository has a synthetic `unsupported_source` fixture for `--source web`.
The 2026-04-29 live capture also promoted reviewed fixtures for both
`--source web` and `--source auto`; both exited 1 and emitted a single JSON
array on stdout with the macOS-only web-support runtime error.

The 2026-04-29 live capture used the older cost probe shape and showed:

- `codexbar config validate --format json --json-only` exits 0 and emits `[]`.
- `codexbar cost --format json --json-only --provider all` exits 0 and emits a
  JSON array of provider cost payloads with `source: "local"`.
- default usage, `usage` subcommand, and status probes with `--source cli`
  timed out after 30 seconds with empty stdout/stderr.
- invalid provider probing exited 1 and emitted `.txt` stdout containing two
  newline-separated JSON arrays, not one parseable JSON document.

A later targeted Codex capture used these exact command shapes:

- `codexbar --format json --json-only --provider codex --source cli`
- `codexbar usage --format json --json-only --provider codex --source cli`
- `codexbar --format json --json-only --provider codex --source cli --status`

All three targeted Codex probes exited 0 with zero stderr and one valid JSON
document on stdout. The promoted stdout sidecars therefore use `.json`.
The successful targeted payloads were JSON arrays containing one provider
object. The status probe includes a `status` object; the two usage probes carry
usage and credit fields without status. The Task 02B runtime adapter therefore
selects provider targets from refresh options, then enabled daemon settings.
The v0.1 built-in defaults target `codex` first, then `claude`; all-provider
usage/status remains an explicit requested probe or future optimization, not
the default production path.

The runtime cost command remains on the current upstream-supported local cost
shape:

- `codexbar cost --format json --json-only --provider both`

The committed `cost_both_success` fixture is doc-derived and does not replace
the reviewed 2026-04-29 live `cost_all` evidence. It exists to pin the current
daemon command strategy and normalizer coverage without committing private
output.

## Source Labels

Task 06A treats source labels as provider semantic metadata reported by
upstream CLI, not as local daemon implementation adapters:

- `codex-cli`, `claude`, `cli`, and `local` normalize to semantic `local`.
- `openai-web` and `web` normalize to semantic `web`.
- `oauth`, `oauth-api`, and `api` normalize to semantic `api`.
- Current upstream docs also describe provider-specific CLI labels. The daemon
  maps labels containing `cli`, labels beginning with `local-`, and labels
  ending in `-cli` to semantic `local`; labels containing `web` or `browser` to
  semantic `web`; and labels containing `api` or `oauth` to semantic `api`.

The implementation adapter remains `sourceAdapter: "upstream_cli"` for payloads
produced by upstream CLI. A semantic `web` source label does not mean this
daemon read browser cookies, browser profiles, keyrings, provider dashboards,
or web endpoints.

The upstream CLI includes `codexbar serve`, a foreground localhost-only HTTP
adapter for usage and cost JSON. This project deliberately does not use it;
D-Bus remains the daemon interface and no localhost/TCP provider data plane is
added.

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
The implemented adapter keeps bounded cost amounts in provider `cost` summaries
and drops daily chronology, model breakdowns, model lists, raw file paths, and
raw upstream cost payloads before cache or D-Bus output.

## Error Shape Summary

The committed error corpus now combines synthetic dependency/parser coverage
with reviewed live Linux upstream failures:

- `missing_binary`: capture harness cannot locate `codexbar`.
- `timeout_synthetic`: command exceeded a bounded timeout.
- `parse_error_synthetic`: stdout was not parseable JSON.
- `unsupported_source`: synthetic web-source failure plus live web/auto
  failures, both with exit code 1 and JSON-array stdout.
- `invalid_provider`: synthetic invalid provider failure plus live failure with
  exit code 1 and `.txt` stdout containing multiple JSON documents.
- `usage_error`: synthetic stderr redaction stress sample plus live usage/status
  timeout metadata with empty stdout/stderr.
- `usage_success`: doc-derived usage/status shapes plus live targeted Codex
  usage/default, usage subcommand, and status payloads with parseable JSON
  stdout.

The live invalid-provider stdout is intentionally stored as `.txt`: each line is
JSON-looking, but the file as a whole is not valid single-document JSON.

## Harness Validation

`scripts/validate-upstream-cli-fixtures.sh` validates the committed corpus.
`scripts/validate-upstream-cli-capture.sh` validates an unpromoted live capture
directory before manual review. `scripts/test-upstream-cli-capture.sh` runs a
fake `codexbar` binary through the capture harness; it exercises default and
explicit metadata-only capture, config validation, acknowledged config dump,
provider-network probes, targeted `--providers` capture, per-probe timeout
metadata, unsupported `web`/`auto`, invalid provider behavior, committed-corpus
output guards, and redaction of emails, session keys, tokens, headers, and
raw payload fields, multi-document JSON-stream text, account/org identifiers,
and home/profile paths. This test is not production adapter coverage; it only
protects the evidence capture tooling.

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
  enabled on Linux for successful usage/status: one object, an array, or an
  envelope? Targeted Codex success emitted a JSON array containing one provider
  object.
- Does `--provider all --source cli` usage/status complete under a longer
  timeout or different upstream configuration? The promoted run timed out at 30
  seconds with no stdout/stderr.
- Which Linux `--json-only` failures emit single JSON, multiple JSON documents,
  or no output? Unsupported web/auto emitted single JSON arrays, invalid
  provider emitted multiple JSON documents, and timeouts emitted no output.
- Which additional upstream `source` labels, beyond the labels already covered
  by fixtures, should map to semantic `api`, `local`, `web`, or `unknown`?
- Which provider-specific extras are safe and useful enough to normalize, and
  which must become diagnostics or be discarded?
- Can cost output be absent or partial per provider while usage succeeds?
- What stdout/stderr byte limits are appropriate for the production runner?
- Which future published Linux release asset should become the next
  live-capture target after manual review?
- Should multi-account flags become a daemon setting, a manual refresh option,
  or stay upstream-CLI-only until the snapshot identity contract grows an
  account-selection model?
