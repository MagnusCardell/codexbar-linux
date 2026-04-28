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

This repository is at **Task 01 daemon D-Bus/cache vertical slice** status.

Present:

- Rust crate `daemon/` named `codexbar-linuxd`.
- Task 01 daemon runtime that owns the D-Bus session name `org.codexbar.Linux1`.
- D-Bus methods for snapshots, refresh, diagnostics, daemon info, daemon settings patches, and the browser-import test stub.
- D-Bus refresh signals for started, provider changed, snapshot changed, and finished events.
- Fixture-only refresh source that writes normalized snapshot cache data.
- Normalized snapshot cache at `${XDG_CACHE_HOME:-~/.cache}/codexbar-linux/snapshot.json`; no raw provider payloads are cached.
- Daemon-owned settings at `${XDG_CONFIG_HOME:-~/.config}/codexbar-linux/config.json`.
- Contract, schema-payload, cache, settings, redaction, browser-import stub, and D-Bus runtime tests.
- GNOME Shell extension skeleton under `extension/`.
- Preferences skeleton that exposes only the GSettings-owned UI keys from `docs/CONTRACTS.md`.
- GSettings schema under `schemas/`.
- User-scoped systemd/D-Bus and Debian packaging skeleton files.
- Local install/uninstall bootstrap scripts.
- Validation scripts and GitHub Actions check workflow.

Not implemented in Task 01:

- upstream `codexbar` CLI adapter, which remains Task 02;
- browser-cookie import, beyond the schema-valid `not_implemented` test stub;
- provider network calls or Linux web adapters;
- provider scraping;
- keyring access;
- upstream `codexbar` CLI invocation;
- production Shell UI behavior beyond the Task 00 loadable extension skeleton, which remains Task 03;
- Debian package build wiring.

## Local checks

Run the full bootstrap gate:

```bash
./scripts/check.sh
```

Useful narrower checks:

```bash
./scripts/validate-dbus.sh
./scripts/validate-schemas.sh
./scripts/test-fixtures.sh
./scripts/lint-gjs.sh
cargo fmt --manifest-path daemon/Cargo.toml -- --check
cargo clippy --manifest-path daemon/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path daemon/Cargo.toml
```

## Repository layout

```text
.
├── AGENTS.md                         # Repo-wide Codex instructions
├── .codex/
│   ├── config.toml                   # Codex project defaults
│   └── agents/                       # Project-scoped custom Codex agents
├── daemon/                           # Rust daemon crate bootstrap skeleton
├── extension/                        # GNOME Shell extension bootstrap skeleton
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
