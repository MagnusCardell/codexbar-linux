# Roadmap and work breakdown

## P0 — Research and contract freeze

Goal: make ambiguity explicit before agents start coding large surfaces.

Deliverables:

- Verify upstream Linux CLI commands and sample JSON on Ubuntu.
- Freeze initial `Snapshot` schema.
- Freeze initial D-Bus XML.
- Freeze settings schema.
- Complete Task 00A contract-freeze addendum for refresh payloads, diagnostics, daemon info, reserved browser-import no-op result, provider events, identity redaction, and source taxonomy.
- Threat model v1.
- UI fixture states.

Exit criteria:

- `spec/*.json` validates.
- `spec/dbus-org.codexbar.Linux1.xml` validates.
- `docs/CONTRACTS.md` is current and cited by implementation tasks.
- Agents agree no Shell code will perform provider I/O.

## P1 — Vertical slice

Goal: a working top-bar item backed by a real daemon using upstream CLI only.

Deliverables:

- Rust daemon skeleton.
- D-Bus service with `GetSnapshot`, `Refresh`, `GetDaemonInfo`.
- CLI runner for `codexbar --format json --json-only --provider all`.
- Cost runner for `codexbar cost --format json --json-only --provider all`.
- Cache read/write.
- GJS Shell extension with merged mode and popover cards.
- Manual refresh.

Exit criteria:

- Top-bar indicator renders fixture data and live daemon data.
- Killing daemon leaves stale cache UI.
- Restarting daemon updates snapshot signal.

## P2 — Packaging and release hardening

Goal: users can install, configure, inspect, and uninstall cleanly.

Deliverables:

- Preferences pages.
- GSettings schema.
- systemd user service.
- D-Bus service activation.
- Debian packaging.
- Local dev install/uninstall scripts.
- Smoke test script for Ubuntu 24.04/26.04 VMs.
- Static no-browser/web guard in CI.

Exit criteria:

- `.deb` installs daemon, service, extension, schema.
- Extension can be enabled manually and connects to daemon.
- Uninstall removes installed files and leaves user config/cache unless purge is explicit.
- Browser-cookie/web-fetch files, dependencies, fixtures, validators, and agents remain absent.

## P3 — Upstream CLI/provider polish

Goal: improve the supported upstream-CLI-only data path and provider UX.

Deliverables:

- Upstream CLI output drift tests.
- Provider state and diagnostics polish.
- Local cost/usage display improvements where upstream data is available.
- `TestBrowserImport` remains schema-valid `not_implemented` and side-effect free while retained in v1 D-Bus.

Exit criteria:

- Targeted upstream CLI provider refresh is reliable on supported Ubuntu releases.
- Diagnostics identify missing CLI, timeout, parse error, unavailable provider, stale cache, and success without exposing secrets.

## P4 — Polish and hardening

Goal: high-confidence daily-driver UX.

Deliverables:

- Accessibility review.
- High contrast and compact modes.
- GNOME 46 and current 26.04 GNOME smoke matrix.
- Error-state polish.
- Diagnostics export.
- Security review.
- Release checklist.

Exit criteria:

- No known raw secret paths in logs/diagnostics/cache.
- UI does not layout-shift between skeleton and loaded states.
- All supported provider states have visual fixtures.
