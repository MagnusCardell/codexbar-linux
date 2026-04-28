# ADR 0001 — GNOME Shell extension plus user daemon

## Status

Accepted.

## Context

The product must feel native on Ubuntu/GNOME, render a polished top-bar popover, work on Wayland, and avoid a weak AppIndicator-only experience. At the same time, provider fetching, browser-cookie import, subprocess execution, cache management, and diagnostics are inappropriate for the GNOME Shell process.

## Decision

Build CodexBar GNOME as:

- a GNOME Shell extension for top-bar and popover UI;
- a user-scoped daemon for all data-plane work;
- D-Bus session API between them.

## Consequences

Positive:

- Native GNOME visual integration.
- Shell process remains presentation-only.
- Daemon can be restarted independently.
- D-Bus is a local desktop component interface rather than an integration API.

Negative:

- Packaging is more complex than a single extension zip.
- Extensions.gnome.org distribution may be harder because a native daemon is required.
- Requires strong contract tests between UI and daemon.
