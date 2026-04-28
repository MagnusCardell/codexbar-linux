# Task 07 — Preferences UI

## Agent

`gnome_shell_engineer`.

## Goal

Implement native preferences pages for general, providers, browser sessions, and diagnostics.

## Scope

- GTK4/libadwaita prefs window.
- General settings.
- Provider enable/source controls.
- Browser session detection and import test action.
- Diagnostics page with copy redacted diagnostics.
- Connect preferences to GSettings and daemon D-Bus/settings patch API.

## Constraints

- `prefs.js` may use GTK4/libadwaita.
- `prefs.js` must not import `St`, `Clutter`, `Meta`, or `Shell`.
- Preferences must work when daemon is unavailable, showing actionable state.

## Acceptance

- Preferences window opens via GNOME Extensions app.
- Settings persist.
- Import test calls daemon and displays structured result.
- Copy diagnostics is redacted.
