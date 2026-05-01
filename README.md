# CodexBar GNOME / `codexbar-linux`

CodexBar GNOME is a native Ubuntu/GNOME top-bar companion for upstream CodexBar.

It is intentionally split into:

1. a GNOME Shell extension for panel and popover UI;
2. a user-scoped daemon for data collection, cache, browser-cookie import, refresh scheduling, and diagnostics;
3. a D-Bus session API between the two;
4. upstream `codexbar` CLI as the default data plane wherever Linux parity exists.

The product target is stock Ubuntu Desktop 24.04 LTS+ on GNOME 46+, Wayland-first, with Ubuntu 26.04 LTS compatibility as a release gate.

## Current architectural decision

This project is not an AppIndicator tray app and not a localhost HTTP service.

The MVP is:

- GNOME Shell extension in GJS ESModules.
- Preferences in `prefs.js` using GTK4/libadwaita only in the preferences process.
- Rust daemon `codexbar-linuxd` using D-Bus session service `org.codexbar.Linux1`.
- Local normalized cache for instant UI startup and stale rendering.
- Browser-cookie import in the daemon; cookies are read just-in-time, used in-memory, and never persisted by this project.
- Upstream `codexbar` CLI calls for CLI/API/local providers and cost summaries.

## Current status

This repository is at **Task 03 GNOME Shell vertical slice** status.

Present:

- Rust crate `daemon/` named `codexbar-linuxd`.
- Task 01 daemon runtime that owns the D-Bus session name `org.codexbar.Linux1`.
- D-Bus methods for snapshots, refresh, diagnostics, daemon info, daemon settings patches, and the browser-import test stub.
- D-Bus refresh signals for started, provider changed, snapshot changed, and finished events.
- Fixture-only refresh source that writes normalized snapshot cache data.
- Normalized snapshot cache at `${XDG_CACHE_HOME:-~/.cache}/codexbar-linux/snapshot.json`; no raw provider payloads are cached.
- Daemon-owned settings at `${XDG_CONFIG_HOME:-~/.config}/codexbar-linux/config.json`.
- Contract, schema-payload, cache, settings, redaction, browser-import stub, and D-Bus runtime tests.
- Redacted upstream CLI fixture corpus under `daemon/fixtures/upstream-cli/`.
- Local-only upstream CLI capture harness and fixture validator.
- Production daemon upstream CLI adapter for targeted provider refresh.
- Runtime refresh uses targeted usage/status probes and defaults to `codex` when no provider is configured or requested.
- Cost refresh uses `codexbar cost --format json --json-only --provider all` without `--source`.
- GNOME Shell extension vertical slice under `extension/`, with D-Bus-only data access, merged/provider/minimal panel modes, provider popover cards, manual refresh, diagnostics, and daemon info display.
- Preferences UI that exposes only the five GSettings-owned UI keys from `docs/CONTRACTS.md`.
- GSettings schema under `schemas/`.
- User-scoped systemd/D-Bus and Debian packaging skeleton files.
- Local install/uninstall bootstrap scripts.
- Validation scripts and GitHub Actions check workflow.

Not implemented after Task 03:

- browser-cookie import, beyond the schema-valid `not_implemented` test stub;
- provider network calls or Linux web adapters;
- provider scraping;
- keyring access;
- Debian package build wiring.

The upstream CLI adapter does not default production usage/status refresh to
`--provider all` because the promoted live Linux evidence timed out for those
all-provider usage/status probes. The first proven Linux usage/status provider
is `codex`; `all` remains explicit for usage/status and is used by default only
for cost summaries.

## Local checks

Run the full bootstrap gate:

```bash
./scripts/check.sh
```

Useful narrower checks:

```bash
./scripts/validate-dbus.sh
./scripts/validate-schemas.sh
./scripts/validate-gsettings.sh
./scripts/validate-packaging.sh
./scripts/test-fixtures.sh
./scripts/lint-gjs.sh
cargo fmt --manifest-path daemon/Cargo.toml -- --check
cargo clippy --manifest-path daemon/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path daemon/Cargo.toml
```

Optional upstream CLI live smoke tests are ignored by default and are not part
of `./scripts/check.sh` or CI:

```bash
CODEXBAR_LIVE=1 CODEXBAR_CLI=/path/to/codexbar \
  cargo test --manifest-path daemon/Cargo.toml -- --ignored --test-threads=1
```

## Manual GNOME smoke checks

Static checks cannot prove GNOME Shell lifecycle behavior. For Task 03 changes
to extension runtime code, run the detailed [GNOME smoke checklist](docs/gnome-smoke-test.md)
on GNOME 46+:

```bash
./scripts/install-local.sh
gnome-extensions enable codexbar-linux@codexbar.dev
gnome-extensions info codexbar-linux@codexbar.dev
gnome-extensions disable codexbar-linux@codexbar.dev
gnome-extensions enable codexbar-linux@codexbar.dev
./scripts/uninstall-local.sh
```

On Wayland, log out and back in after installing extension files if GNOME Shell
does not discover the extension immediately. During the smoke, confirm that the
panel item appears in merged mode, the popover opens, manual refresh reaches
D-Bus, disabling removes the panel item, and re-enabling does not create duplicate
panel items or timers. Install scripts and package hooks must not enable the
extension automatically; the `gnome-extensions enable` command above is the
explicit user action.

## Repository layout

```text
.
├── AGENTS.md                         # Repo-wide Codex instructions
├── .codex/
│   ├── config.toml                   # Codex project defaults
│   └── agents/                       # Project-scoped custom Codex agents
├── daemon/                           # Rust daemon crate
├── extension/                        # GNOME Shell extension vertical slice
├── schemas/                          # GSettings schema for Shell UI preferences
├── spec/
│   ├── dbus-org.codexbar.Linux1.xml
│   ├── snapshot.schema.json
│   ├── settings.schema.json
│   └── *.schema.json
├── docs/
│   ├── PRD.md
│   ├── ARCHITECTURE.md
│   ├── SECURITY.md
│   ├── ACCEPTANCE.md
│   ├── ROADMAP.md
│   ├── SOURCES.md
│   └── adr/
├── tasks/                            # Codex-ready implementation tasks
└── prompts/                          # Dispatch/review prompts for agents
```

## Phase gates

- **P0 — Research and contract freeze:** verify upstream CLI behavior on Linux, freeze snapshot schema, D-Bus XML, daemon cache contract, and UI state model.
- **P1 — Vertical slice:** daemon invokes upstream CLI, caches normalized data, D-Bus returns snapshot, Shell extension renders panel + popover from fixture/live snapshot.
- **P2 — Browser-cookie path:** Chromium-family and Firefox cookie import, provider web fetch adapters, no raw cookie persistence, redacted diagnostics.
- **P3 — Preferences and install:** native prefs, systemd user unit, D-Bus activation, `.deb` dev package, smoke test scripts.
- **P4 — Polish and hardening:** GNOME 46/50 matrix, Wayland validation, accessibility, stale/auth/error UI, threat-model review.

## Development stance

Agents must preserve upstream CodexBar semantics unless a Linux-specific constraint forces a divergence. Divergences must be recorded as ADRs or in `docs/ARCHITECTURE.md`.
