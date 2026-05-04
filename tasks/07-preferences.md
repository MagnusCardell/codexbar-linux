# Task 07 — Preferences UI

## Agent

`gnome_shell_engineer`.

## Goal

Implement native preferences pages for general, providers, reserved unsupported
browser-import status, and diagnostics.

## Scope

- GTK4/libadwaita prefs window.
- General settings.
- Provider enable/source controls.
- Reserved browser-import status that reports the daemon's
  `TestBrowserImport` compatibility method as unsupported/no-op only.
- Diagnostics page with copy redacted diagnostics.
- Connect preferences to GSettings and daemon D-Bus/settings patch API.

## Constraints

- `prefs.js` may use GTK4/libadwaita.
- `prefs.js` must not import `St`, `Clutter`, `Meta`, or `Shell`.
- Preferences must work when daemon is unavailable, showing actionable state.
- Do not add browser discovery, browser profile detection, cookie database
  access, keyring access, provider web fetch, import flows, browser extensions,
  or localhost/TCP APIs. Browser-import UI is compatibility diagnostics only
  while ADR 0006 is in force.

## Acceptance

- Preferences window opens via GNOME Extensions app.
- Settings persist.
- Reserved import test calls daemon and displays structured `not_implemented`
  result without implying browser access is available.
- Copy diagnostics is redacted.
