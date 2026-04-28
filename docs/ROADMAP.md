# Roadmap and work breakdown

## P0 — Research and contract freeze

Goal: make ambiguity explicit before agents start coding large surfaces.

Deliverables:

- Verify upstream Linux CLI commands and sample JSON on Ubuntu.
- Freeze initial `Snapshot` schema.
- Freeze initial D-Bus XML.
- Freeze settings schema.
- Complete Task 00A contract-freeze addendum for refresh payloads, diagnostics, daemon info, browser import result, provider events, identity redaction, and source taxonomy.
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

## P2 — Linux browser-cookie path

Goal: signed-in browsers work for first web-backed providers.

Deliverables:

- Browser/profile discovery.
- Safe cookie DB copying.
- Chromium-family cookie read/decrypt path after current-behavior verification.
- Firefox cookie read path after current-behavior verification.
- In-memory cookie jar.
- Codex/OpenAI web adapter.
- Claude web adapter.
- Browser import diagnostics.
- No raw persistence tests.

Exit criteria:

- Import test distinguishes absent profile, keyring locked, cookie absent, cookie rejected, and success.
- Provider cards show `cookie_rejected` distinctly from `unauthenticated`.

## P3 — Preferences and packaging

Goal: users can install, configure, inspect, and uninstall cleanly.

Deliverables:

- Preferences pages.
- GSettings schema.
- systemd user service.
- D-Bus service activation.
- Debian packaging.
- Local dev install/uninstall scripts.
- Smoke test script for Ubuntu 24.04/26.04 VMs.

Exit criteria:

- `.deb` installs daemon, service, extension, schema.
- Extension can be enabled manually and connects to daemon.
- Uninstall removes installed files and leaves user config/cache unless purge is explicit.

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
