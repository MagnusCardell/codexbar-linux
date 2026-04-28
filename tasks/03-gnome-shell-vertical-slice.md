# Task 03 — GNOME Shell UI vertical slice

## Agent

`gnome_shell_engineer`.

## Goal

Render a working GNOME top-bar item and popover from daemon snapshots.

## Scope

- Implement extension enable/disable lifecycle.
- Connect to daemon over D-Bus.
- Render daemon-provided cached/stale snapshots only when received over D-Bus; if the daemon is unavailable, render a local synthetic unavailable/loading state. Fixtures may be loaded only from dev/test paths.
- Render merged panel mode with two micro-bars.
- Render popover card list.
- Implement manual refresh action.
- Implement `loading`, `ok`, `stale`, `unauthenticated`, `cookie_rejected`, `missing_dependency`, `provider_unavailable`, `parse_error`, `timeout`, and `error` visual states from fixtures.

## Constraints

- No `Gtk`, `Gdk`, or `Adw` imports in Shell process.
- No provider network calls.
- No subprocess calls.
- Destroy all objects and disconnect all signals in `disable()`.

## Acceptance

- Extension loads on GNOME 46+ dev environment.
- Panel item appears in merged mode.
- Popover opens and displays fixture/live daemon providers.
- Manual refresh calls D-Bus.
- Disable/re-enable does not leak panel items or timers.

## Contract references

Read `docs/CONTRACTS.md`, `docs/adr/0005-p0a-contract-freeze.md`, and all relevant `spec/*.schema.json` before changing behavior. Do not contradict the P0A source taxonomy, identity redaction rules, refresh semantics, settings ownership, or Shell/daemon boundary.

## P0A-specific Shell requirements

- Consume daemon data only through D-Bus in production. Production Shell code must not read cache files.
- Fixtures may be loaded only in dev/test paths.
- Accept `SnapshotChanged(snapshot_json)`, `RefreshStarted(refresh_id)`, `RefreshFinished(refresh_id, result_json)`, and `ProviderChanged(provider_id, provider_event_json)`.
- Treat `ProviderChanged` provider payloads as complete provider replacements, not partial patches.
- Read panel mode, reset time format, theme, selected provider, and start-daemon-on-login desired state from GSettings. Do not read daemon config JSON directly from Shell code.
- Do not present `sourceAdapter` values such as `fixture`, `synthetic`, or `cache` as provider semantic source. If rendered, `sourceAdapter` is secondary/diagnostic metadata.
