# Upstream CLI fixtures

This directory contains redacted fixtures for upstream `codexbar` CLI evidence.
These files are intentionally not normalized snapshots and are not wired into
daemon refresh behavior.

## Layout

- `manifest.json` lists every committed sample and its stdout/stderr/metadata
  sidecars.
- `usage/` contains redacted output for usage/default probes such as
  `codexbar --format json --json-only --provider all --source cli`.
- `cost/` contains redacted output for
  `codexbar cost --format json --json-only --provider all`.
- `status/` contains status-bearing CLI output.
- `errors/` contains missing-binary, non-zero-exit, timeout, and parse-oriented
  fixtures.

Committed samples use a sidecar triplet format:

- `*_metadata.json`
- `*_stdout.json` or `*_stdout.txt`
- `*_stderr.txt`

The fixture contract deliberately preserves upstream-looking stdout separately
from capture metadata instead of defining the upstream CLI schema. The future
normalizer should read these files as evidence and add typed normalization tests
without treating them as daemon output.

## Redaction rules

Committed upstream CLI fixtures must not contain raw emails, organization names,
provider account IDs, cookies, Authorization headers, Set-Cookie headers, bearer
tokens, API keys, browser profile paths, upstream config secrets, or raw provider
payload dumps.

Use `scripts/redact-upstream-cli-sample.py` for capture-time redaction of new
live samples. The capture helper writes command envelopes under the requested
output directory; review and convert useful captures into the sidecar corpus
before committing. Run:

```bash
./scripts/validate-upstream-cli-fixtures.sh
```

before committing new samples.

## Capturing new samples

The capture harness is always opt-in (`--live` or `CODEXBAR_CAPTURE_LIVE=1`)
and writes to `/tmp/codexbar-upstream-cli-live-<timestamp>` unless `--output`
is provided. It refuses to write directly into this committed fixture directory
or write a live capture manifest there through `--manifest` unless
`CODEXBAR_ALLOW_COMMITTED_FIXTURE_OUTPUT=1` is set. If `--manifest` is
provided, it must point directly under the capture output directory so the
manifest and sidecars remain one validated package, and its basename must match
`manifest.live-*.json`.

The default live mode, with or without explicit `--metadata-only`, captures only
`codexbar --version`:

```bash
./scripts/capture-upstream-cli-samples.sh \
  --live \
  --metadata-only \
  --output /tmp/codexbar-upstream-cli
```

Config validation is separate from metadata-only mode and may be captured
without provider-network probes:

```bash
./scripts/capture-upstream-cli-samples.sh \
  --live \
  --include-config-validate \
  --output /tmp/codexbar-upstream-cli
```

Usage, cost, and provider status commands may contact providers through the
upstream CLI. Capture them only with explicit opt-in. Linux provider success
captures default to `--provider-source cli`; this adds `--source cli` to usage
and status probes. `auto` and `web` are Linux unsupported-source probe values,
not expected success paths. The cost probe intentionally uses `--json-only` but
does not receive `--source` unless upstream support for that flag is verified.

```bash
./scripts/capture-upstream-cli-samples.sh \
  --live \
  --include-config-validate \
  --allow-provider-network \
  --provider-source cli \
  --include-error-probes \
  --output /tmp/codexbar-upstream-cli
```

`--include-config-dump` requires a second confirmation via
`CODEXBAR_CAPTURE_INCLUDE_CONFIG_DUMP=1` or
`--i-understand-config-dump-may-contain-secrets`. Use it only when the redacted
dump is needed for upstream evidence and manually review it before promotion.

Promotion is deliberate:

1. Capture into `/tmp` or another non-repository directory.
2. Run `./scripts/validate-upstream-cli-capture.sh /path/to/capture`.
3. Manually inspect every redacted stdout, stderr, metadata, and manifest file.
4. Copy only useful reviewed sidecars into this directory.
5. Update `manifest.json` with selected entries.
6. Run `./scripts/validate-upstream-cli-fixtures.sh`.

Do not commit raw capture files or raw terminal output. Live upstream output may
contain raw `accountEmail` values, nested `identity.accountEmail` values, home
paths, `~/.local/share/...` paths, or `auth.json` paths before redaction.
