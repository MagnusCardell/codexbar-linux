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

This repository is at **Task 04B.4 Codex cookie header/session-material policy plus
Task 04D.1 gated Codex web transport and live reconnaissance** status. The Task
03 GNOME Shell vertical slice and design gate are
complete enough for the browser-cookie implementation phase, with live GNOME 46
Wayland activation smoke proven.

Present:

- Rust crate `daemon/` named `codexbar-linuxd`.
- Task 01 daemon runtime that owns the D-Bus session name `org.codexbar.Linux1`.
- D-Bus methods for snapshots, refresh, diagnostics, daemon info, daemon settings patches, and browser-import testing.
- D-Bus refresh signals for started, provider changed, snapshot changed, and finished events.
- Fixture refresh source for tests and explicit development mode; production
  daemon refresh rejects explicit fixture selection unless
  `CODEXBAR_LINUX_ALLOW_FIXTURE=1` is set.
- Normalized snapshot cache at `${XDG_CACHE_HOME:-~/.cache}/codexbar-linux/snapshot.json`; no raw provider payloads are cached.
- Daemon-owned settings at `${XDG_CONFIG_HOME:-~/.config}/codexbar-linux/config.json`.
- Contract, schema-payload, cache, settings, redaction, browser-import, and D-Bus runtime tests.
- Redacted upstream CLI fixture corpus under `daemon/fixtures/upstream-cli/`.
- Local-only upstream CLI capture harness and fixture validator.
- Production daemon upstream CLI adapter for targeted provider refresh.
- Runtime refresh uses targeted usage/status probes and defaults to `codex` when no provider is configured or requested.
- Cost refresh uses `codexbar cost --format json --json-only --provider all` without `--source`.
- GNOME Shell extension vertical slice under `extension/`, with D-Bus-only data access, merged/provider/minimal panel modes, provider popover cards, manual refresh, diagnostics, and daemon info display.
- Task 03 visual design accepted at baseline commit
  `9a47457c6d776923ada6f24694e444539d852da7`; the Shell remains D-Bus-only.
- Preferences UI that exposes only the five GSettings-owned UI keys from `docs/CONTRACTS.md`.
- GSettings schema under `schemas/`.
- User-scoped systemd/D-Bus and Debian packaging skeleton files.
- Local install/uninstall bootstrap scripts.
- Validation scripts and GitHub Actions check workflow.
- Recorded live GNOME smoke result in `docs/gnome-smoke-test.md`.
- Task 04A architecture freeze docs for daemon-only Linux browser-cookie import
  and provider web-fetch adapters:
  `docs/browser-cookie-architecture.md`,
  `docs/browser-cookie-threat-model.md`,
  `docs/browser-support.md`, `docs/provider-roadmap.md`, and
  `docs/adr/0006-linux-browser-cookie-daemon-layer.md`.
- Task 04B daemon-only Chromium-family browser import infrastructure:
  bounded fake-root discovery for Chrome, Chromium, Chromium snap-shaped roots,
  and Brave; private temp-copy SQLite cookie DB reads; synthetic plaintext and
  fake encrypted cookie-row handling; verified Linux basic/plain `v10`
  decryption for Chromium OSCrypt rows; browser-like Codex static-request
  cookie matching; safe encrypted-prefix, header-eligibility, and failure-class
  summaries; fake keyring/decryptor states; memory-only session material;
  redaction-safe `TestBrowserImport` results; and browser fixture validation
  under `scripts/validate-browser-fixtures.sh`.
- Task 04B.1 opt-in throwaway browser verification:
  `scripts/chromium-throwaway-smoke.sh` creates a private temp home, launches a
  Chromium-family browser only with a throwaway user-data-dir, seeds only a
  synthetic `smoke.example.invalid` cookie through a local test server, and
  runs the ignored live `TestBrowserImport` smoke. Fake-home env roots now
  require canonical throwaway homes with `.codexbar-throwaway-browser-root`,
  reject real home/config roots, and keep public results path-free.
- Task 04D.1 daemon-only Codex web transport and reconnaissance:
  `daemon/src/web/` defines a bounded web request/response abstraction, static
  Codex web policy, redaction-safe web diagnostics, fake HTTP client, a gated
  async Rustls-backed static GET client, and Codex parser/normalizer against
  synthetic fixture shapes only. Production `linux_web` refresh has no live
  provider fetch configured by default and returns schema-valid disabled
  diagnostics instead of contacting provider endpoints. Live Codex web
  reconnaissance is ignored by default and requires `CODEXBAR_CODEX_WEB_LIVE=1`,
  a marked throwaway fake browser home, explicit provider `codex`, and explicit
  `sourceAdapterPolicy.only(["linux_web"])`.
  Web fixtures live under `daemon/fixtures/web/codex/` and are checked by
  `scripts/validate-web-fixtures.sh`.
- Task 04B.3 signed-in Codex throwaway recon result and Task 04B.4 follow-up:
  the browser layer found 19 provider-domain encrypted `v10` rows and no
  plaintext rows, but still produced no usable session material because the
  domain-wide cookie set included material that failed safe Cookie-header
  validation. Task 04B.4 now builds Codex Cookie headers only in memory for the
  fixed `https://chatgpt.com/codex/settings/usage` request, skips only
  syntax-invalid header rows when valid material remains, and records
  counts/classes only. A signed-in live recon rerun still requires a marked
  throwaway fake home and was not run in this workspace because
  `CODEXBAR_WEB_HOME`/`CODEXBAR_BROWSER_IMPORT_FAKE_HOME` was unavailable.

Not implemented after Task 04D.1:

- default production live provider fetch or default `linux_web` refresh;
- real provider scraping;
- real Secret Service/KWallet key extraction or interactive keyring prompts;
- `v20`, encrypted-value-prefix `v24`, app-bound, or unknown Chromium encrypted
  cookie formats;
- real user browser profile scanning by default;
- Firefox browser import;
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
./scripts/validate-browser-fixtures.sh
./scripts/validate-web-fixtures.sh
CODEXBAR_BROWSER_LIVE=1 ./scripts/chromium-throwaway-smoke.sh # optional live smoke
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

Optional Codex web live reconnaissance is also ignored by default and is not
part of `./scripts/check.sh` or CI. It may make one bounded async GET to
`https://chatgpt.com/codex/settings/usage` only when all live gates are set and
the fake home is a marked throwaway browser root:

```bash
CODEXBAR_CODEX_WEB_LIVE=1 \
CODEXBAR_BROWSER_IMPORT_FAKE_HOME=/path/to/throwaway-home \
cargo test --manifest-path daemon/Cargo.toml -- --ignored codex_web_live
```

See [Codex web live reconnaissance](docs/codex-web-live-recon.md) before
signing into ChatGPT/Codex in a throwaway profile. The live test must not print
or commit raw cookies, request headers, response headers, response bodies,
profile paths, or provider identity.

Fixture-backed daemon refresh is disabled in production mode. For explicit local
UI development against fixture snapshots, start the daemon with:

```bash
CODEXBAR_LINUX_ALLOW_FIXTURE=1 cargo run --manifest-path daemon/Cargo.toml
```

Chromium-family browser import tests use synthetic or throwaway roots. Default
daemon startup does not scan real browser profiles. For a development-only live
throwaway browser-import probe, run:

```bash
CODEXBAR_BROWSER_LIVE=1 ./scripts/chromium-throwaway-smoke.sh
```

The smoke script is not part of `./scripts/check.sh` or CI. It deletes its temp
profile unless `KEEP_CODEXBAR_BROWSER_LIVE=1` is set, and its normal output
uses shape labels instead of absolute profile paths. Direct fake-home daemon
runs must use an isolated, canonical temp directory containing the marker file:

```bash
printf 'codexbar throwaway browser smoke\n' \
  > /path/to/throwaway-home/.codexbar-throwaway-browser-root
CODEXBAR_BROWSER_IMPORT_FAKE_HOME=/path/to/throwaway-home \
  cargo run --manifest-path daemon/Cargo.toml
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
