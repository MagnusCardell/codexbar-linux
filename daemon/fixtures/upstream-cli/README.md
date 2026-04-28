# Upstream CLI fixtures

This directory contains redacted fixtures for upstream `codexbar` CLI evidence.
These files are intentionally not normalized snapshots and are not wired into
daemon refresh behavior.

## Layout

- `manifest.json` lists every committed sample and its stdout/stderr/metadata
  sidecars.
- `usage/` contains redacted output for
  `codexbar --format json --json-only --provider all`.
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

The capture harness does not run provider-networking commands by default.
Version/config validation can be captured locally:

```bash
./scripts/capture-upstream-cli-samples.sh --output /tmp/codexbar-upstream-cli
```

Usage, cost, and provider status commands may contact providers through the
upstream CLI. Capture them only with explicit opt-in:

```bash
./scripts/capture-upstream-cli-samples.sh \
  --allow-provider-network \
  --output /tmp/codexbar-upstream-cli
```

Review the redacted output, copy only useful samples into this directory, then
update `manifest.json`. Do not commit raw capture files.
