# CodexBar GNOME / `codexbar-linux`

CodexBar GNOME is a native Ubuntu/GNOME top-bar companion for upstream CodexBar.

It is intentionally split into:

1. a GNOME Shell extension for panel and popover UI;
2. a user-scoped daemon for upstream CLI data collection, cache, refresh scheduling, and diagnostics;
3. a D-Bus session API between the two;
4. upstream `codexbar` CLI and local provider tooling as the production data plane.

The product target is stock Ubuntu Desktop 24.04 LTS+ on GNOME 46+, Wayland-first, with Ubuntu 26.04 LTS compatibility as a release gate.

## Current architectural decision

This project is not an AppIndicator tray app and not a localhost HTTP service.

The MVP is:

- GNOME Shell extension in GJS ESModules.
- Preferences in `prefs.js` using GTK4/libadwaita only in the preferences process.
- Rust daemon `codexbar-linuxd` using D-Bus session service `org.codexbar.Linux1`.
- Local normalized cache for instant UI startup and stale rendering.
- Upstream `codexbar` CLI calls for CLI/API/local providers and cost summaries.
- No browser-cookie import, browser profile scanning, keyring access, provider dashboard scraping, browser extension, or localhost/TCP API.

## Current status

This repository is at **Task 05B packaging and release-install hardening**
status, building on the Task 03 GNOME Shell vertical slice, Task 04R no-browser
cleanup, and Task 05A upstream-CLI-only hardening.
The implemented product surface is GNOME UI + user daemon + upstream CLI adapter.

Present:

- Rust crate `daemon/` named `codexbar-linuxd`.
- Task 01 daemon runtime that owns the D-Bus session name `org.codexbar.Linux1`.
- D-Bus methods for snapshots, refresh, diagnostics, daemon info, daemon settings patches, and a contract-reserved browser-import test method.
- D-Bus refresh signals for started, provider changed, snapshot changed, and finished events.
- Fixture refresh source for tests and explicit development mode; production
  daemon refresh rejects explicit fixture selection unless
  `CODEXBAR_LINUX_ALLOW_FIXTURE=1` is set.
- Normalized snapshot cache at `${XDG_CACHE_HOME:-~/.cache}/codexbar-linux/snapshot.json`; no raw provider payloads are cached.
- Daemon-owned settings at `${XDG_CONFIG_HOME:-~/.config}/codexbar-linux/config.json`.
- Contract, schema-payload, cache, settings, redaction, browser-import no-op, and D-Bus runtime tests.
- Redacted upstream CLI fixture corpus under `daemon/fixtures/upstream-cli/`.
- Local-only upstream CLI capture harness and fixture validator.
- Production daemon upstream CLI adapter for targeted provider refresh.
- Runtime refresh uses targeted usage/status probes and defaults to `codex` when no provider is configured or requested.
- Cost refresh uses `codexbar cost --format json --json-only --provider all` without `--source`.
- GNOME Shell extension vertical slice under `extension/`, with D-Bus-only data access, merged/provider/minimal panel modes, provider popover cards, manual refresh, diagnostics, and daemon info display.
- Preferences UI that exposes only the five GSettings-owned UI keys from `docs/CONTRACTS.md`.
- GSettings schema under `schemas/`.
- User-scoped systemd/D-Bus activation files and a development Debian package
  path for v0.1 local release smoke testing.
- Local install/uninstall bootstrap scripts that copy only runtime extension
  files, compile schemas strictly, and remove owned files while preserving user
  config/cache.
- Validation scripts and GitHub Actions check workflow.
- Recorded live GNOME smoke result in `docs/gnome-smoke-test.md`.

Out of production scope after Task 04R:

- browser-cookie access or import;
- browser profile discovery or cookie database reads;
- keyring, Secret Service, or session extraction;
- provider web fetches or dashboard scraping;
- browser extension or localhost/TCP bridge.

Not implemented after Task 05B:

- Signed repository distribution, package upgrade matrix coverage, and recorded
  package-install GNOME smoke evidence for Ubuntu 24.04/26.04 release sign-off.

`TestBrowserImport` remains in the D-Bus contract for compatibility, but it is
reserved and unsupported. The daemon validates the request JSON and returns a
schema-valid `not_implemented` result without touching browser paths, profile
stores, keyrings, cookies, or provider endpoints.

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
./scripts/validate-no-browser-web-surface.sh
./scripts/build-deb.sh --check
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

Fixture-backed daemon refresh is disabled in production mode. For explicit local
UI development against fixture snapshots, start the daemon with:

```bash
CODEXBAR_LINUX_ALLOW_FIXTURE=1 cargo run --manifest-path daemon/Cargo.toml
```

## Manual GNOME smoke checks

Static checks cannot prove GNOME Shell lifecycle behavior. For Task 03 changes
to extension runtime code, run the detailed [GNOME smoke checklist](docs/gnome-smoke-test.md)
on GNOME 46+. For release packaging, run both paths in
[Release Smoke Test](docs/release-smoke-test.md):

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

## Development Debian package

Task 05B chooses **Option A: development `.deb` package**. The package installs
system-owned files under `/usr/bin`, `/usr/share/dbus-1/services`,
`/usr/lib/systemd/user`, `/usr/share/gnome-shell/extensions`, and
`/usr/share/glib-2.0/schemas`. It does not enable the GNOME extension, start a
system daemon, install any TCP listener, or require a live GNOME session or
upstream `codexbar` CLI during package build.

```bash
./scripts/build-deb.sh
sudo apt install ./dist/codexbar-linux_0.1.0-1_$(dpkg --print-architecture).deb
systemctl --user daemon-reload
gnome-extensions enable codexbar-linux@codexbar.dev
```

The user still controls extension enablement. On Wayland, a logout/login may be
needed before GNOME Shell discovers newly installed system extension files.

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
- **P2 — Packaging and release hardening:** native prefs, systemd user unit, D-Bus activation, `.deb` dev package, smoke test scripts, install/uninstall polish.
- **P3 — Upstream CLI/provider polish:** improve upstream CLI normalization, provider diagnostics, local cost/usage display, and stale/error UX where upstream data is available.
- **P4 — Polish and hardening:** GNOME 46/50 matrix, Wayland validation, accessibility, stale/auth/error UI, threat-model review.

## Development stance

Agents must preserve upstream CodexBar semantics unless a Linux-specific constraint forces a divergence. Divergences must be recorded as ADRs or in `docs/ARCHITECTURE.md`.
