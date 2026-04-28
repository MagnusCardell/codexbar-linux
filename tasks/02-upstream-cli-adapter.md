# Task 02 — Upstream CodexBar CLI adapter

## Agent

`daemon_engineer`; `qa_security_reviewer` review.

## Goal

Fetch live usage and cost data from upstream `codexbar` CLI and normalize it into the daemon snapshot.

## Scope

- Resolve `codexbar` path.
- Capture `codexbar --version`.
- Invoke `codexbar --format json --json-only --provider all`.
- Invoke `codexbar cost --format json --json-only --provider all`.
- Map exit codes and stderr into diagnostics.
- Normalize upstream JSON to `Snapshot.providers[]`.
- Preserve provider IDs, source/sourceAdapter semantics, usage windows, credits, status, and redacted cost summaries where present.
- Normalize identity only into the allowed masked display and hash fields from `docs/CONTRACTS.md`; never preserve raw identity fields.

## Constraints

- No provider-specific inference in UI.
- Timeout every CLI call.
- Redact all stdout/stderr before logs.
- Do not mutate `~/.codexbar/config.json`.

## Acceptance

- Fixture tests cover success, missing binary, timeout, parse error, non-zero exit.
- Live call works when upstream CLI is installed.
- D-Bus snapshot is valid with live CLI data.

## Contract references

Read `docs/CONTRACTS.md`, `docs/adr/0005-p0a-contract-freeze.md`, and all relevant `spec/*.schema.json` before changing behavior. Do not contradict the P0A source taxonomy, identity redaction rules, refresh semantics, settings ownership, or Shell/daemon boundary.
