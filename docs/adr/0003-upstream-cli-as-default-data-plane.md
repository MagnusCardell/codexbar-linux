# ADR 0003 — Upstream CodexBar CLI as default data plane

## Status

Accepted.

## Context

Upstream CodexBar already owns provider semantics, config shape, local CLI/API provider paths, status output, and cost summaries. A Linux product should not fork provider logic unnecessarily.

## Decision

Use upstream `codexbar` CLI as the default source for providers where Linux CLI support works. Normalize its JSON output into the CodexBar GNOME snapshot schema without changing semantics unless required by Linux constraints.

## Consequences

Positive:

- Less duplicated provider logic.
- Easier upstream compatibility.
- Preserves provider IDs, labels, reset windows, identity fields, and cost output.

Negative:

- We depend on a separately installed or bundled upstream CLI.
- CLI output changes can break parsing unless schema tests catch regressions.
- Linux web/auto gaps remain and need a Linux-native layer.
