# Architecture — CodexBar GNOME

## System shape

```text
┌──────────────────────────────────────────────────────────────┐
│ GNOME Shell process                                           │
│                                                              │
│  extension.js / GJS ESModules                                 │
│  - panel indicator                                            │
│  - popover cards                                              │
│  - D-Bus client                                               │
│  - no Gtk/Gdk/Adw imports                                     │
└───────────────┬──────────────────────────────────────────────┘
                │ D-Bus session bus: org.codexbar.Linux1
┌───────────────▼──────────────────────────────────────────────┐
│ codexbar-linuxd user daemon                                   │
│                                                              │
│  - refresh scheduler                                          │
│  - upstream codexbar CLI runner                               │
│  - local normalized cache                                     │
│  - redacted diagnostics                                       │
└───────┬──────────────┬────────────────┬──────────────────────┘
        │              │
        ▼              ▼
 upstream codexbar   local provider tooling
 CLI                 where available
```

## Why this split

The GNOME Shell process is latency-sensitive, review-sensitive, and not a safe place for subprocess orchestration, cache management, or provider data normalization. The daemon owns local I/O and normalization. The extension owns presentation only and speaks to the daemon over D-Bus.

## Components

### 1. GNOME Shell extension

Responsibilities:

- Render panel indicator modes: merged, provider, minimal.
- Render popover provider cards.
- Render stable loading/stale/error/auth states.
- Call daemon over D-Bus.
- Subscribe to daemon signals.
- Open dashboards in default browser.
- Trigger manual refresh.

Non-responsibilities:

- No provider network calls.
- No browser-cookie reads.
- No browser profile scanning.
- No keyring or session extraction.
- No upstream CLI subprocess calls.
- No cache file reads in production.
- No raw diagnostics construction.

### 2. Preferences

Responsibilities:

- Render native preferences with GTK4/libadwaita.
- Read/write extension UI preferences.
- Configure daemon through D-Bus or documented config file.
- Display daemon health and diagnostics.

### 3. Daemon

Responsibilities:

- D-Bus service `org.codexbar.Linux1`.
- D-Bus object `/org/codexbar/Linux1`.
- Cache file under `${XDG_CACHE_HOME:-~/.cache}/codexbar-linux/snapshot.json`.
- Config file under `${XDG_CONFIG_HOME:-~/.config}/codexbar-linux/config.json` for Linux-specific settings.
- Upstream config read from `~/.codexbar/config.json`.
- Upstream CLI resolver and runner.
- Local provider tooling integration where it is available through upstream CLI semantics.
- Redacted diagnostics.

### 4. Upstream CLI adapter

The adapter invokes:

```bash
codexbar --format json --json-only --provider <provider> --source cli
codexbar --format json --json-only --provider <provider> --source cli --status
codexbar cost --format json --json-only --provider both
codexbar config dump --pretty
codexbar config validate --format json --json-only
```

Usage/status refreshes default to the targeted `codex` provider only when no
provider settings are configured and `RefreshOptions.providers` is empty. A
non-empty daemon provider config is authoritative: providers disabled, set to
source adapter `off`, or without CLI fallback are skipped, and an all-off config
returns a schema-valid `noop` refresh instead of silently probing `codex`.
Explicit `RefreshOptions.providers` remains a manual override. The adapter does
not default usage/status to `--provider all`; all-provider usage/status probes
are explicit only. Cost remains the upstream Codex + Claude local cost command
and deliberately omits `--source`.

All invocations require:

- configurable timeout;
- environment allowlist;
- PATH resolution diagnostics;
- stdout/stderr size limits;
- exit-code mapping;
- redaction before logging;
- parse errors mapped into provider diagnostics.

### 5. Explicitly Unsupported Surfaces

The daemon does not read browser cookies, browser profiles, browser databases,
desktop keyrings, Secret Service, KWallet, provider web dashboards, or provider
session material. It does not run provider web scraping and does not expose a
localhost/TCP API. `TestBrowserImport` is retained only as a stable D-Bus
contract method and returns a schema-valid unsupported/no-op result.

## Contract freeze

See `docs/CONTRACTS.md` and `docs/adr/0005-p0a-contract-freeze.md` for the v1 decisions on settings ownership, identity redaction, source taxonomy, refresh behavior, diagnostics, cache rules, and D-Bus error names. Implementation tasks must treat those files and `spec/*.schema.json` as authoritative.

## Data model

The UI consumes `Snapshot`:

```text
Snapshot
├── schemaVersion
├── generatedAt
├── stale
├── daemon
├── providers[]
│   ├── provider
│   ├── displayName
│   ├── source              # provider semantic source: api/local/web/unknown
│   ├── sourceAdapter       # implementation adapter: upstream_cli/cache/fixture/synthetic/none
│   ├── state
│   ├── usage.primary
│   ├── usage.secondary
│   ├── usage.tertiary
│   ├── credits
│   ├── identity
│   ├── status
│   ├── cost
│   └── diagnosticsSummary
└── selectedProvider
```

Provider states:

- `loading`
- `ok`
- `stale`
- `unauthenticated`
- `cookie_rejected`       # reserved legacy state; not produced by the no-browser daemon
- `missing_dependency`
- `provider_unavailable`
- `parse_error`
- `timeout`
- `error`

## D-Bus API

The D-Bus contract intentionally returns JSON strings rather than a large nested D-Bus variant graph. GJS and Rust both handle JSON well, JSON schemas can be tested, and the contract can evolve with `schemaVersion`.

Methods:

- `GetSnapshot() -> snapshot_json`
- `Refresh(options_json) -> refresh_id`
- `GetDiagnostics(provider_id) -> diagnostics_json`
- `GetDaemonInfo() -> daemon_info_json`
- `SetSettingsPatch(patch_json) -> settings_json`
- `TestBrowserImport(options_json) -> result_json`

Signals:

- `SnapshotChanged(snapshot_json)`
- `RefreshStarted(refresh_id)`
- `RefreshFinished(refresh_id, result_json)`
- `ProviderChanged(provider_id, provider_event_json)`

See `spec/dbus-org.codexbar.Linux1.xml`.

## Cache rules

- Cache stores normalized snapshots only.
- Cache never stores raw provider responses, raw headers, Authorization values, session material, browser paths, or raw identity.
- Cache supports fast daemon startup and stale-state rendering through D-Bus.
- UI must clearly show stale cache when daemon cannot refresh.
- Cache write is atomic: write temp file, fsync, rename.
- Cache file permissions: `0600`; cache directory: `0700`.

## Refresh rules

Refresh sources are attempted by provider preference and recorded as `sourceAdapter`. Snapshot `source` records provider semantics, not implementation policy:

1. upstream CLI/API/local path;
2. stale cache fallback.

Auto mode resolves only to the supported upstream CLI/local path in production. Emitted snapshots must never use `auto`; they must record the actual `source` and `sourceAdapter`.

Manual refresh bypasses normal interval throttling but still respects concurrency limits.

The daemon scheduler starts after the D-Bus object is exported. It runs a
startup refresh when `settings.refresh.startupRefresh` is true, then runs
scheduled refreshes on `settings.refresh.intervalSeconds`. The default interval
is 300 seconds; `intervalSeconds: 0` is manual/off mode for scheduled interval
refresh and does not change `startupRefresh`. `SetSettingsPatch` wakes the
scheduler so interval changes are applied without daemon restart. Repeated
scheduled upstream CLI missing, timeout, parse, and nonzero-exit failures use a
bounded exponential backoff so the daemon does not run a failing CLI every base
interval forever. Manual D-Bus `Refresh` remains available while backoff is in
effect.
Refresh completion failures must clear the active-refresh guard and emit a
schema-valid failed `RefreshFinished` result so a later manual Refresh can
recover.

## GNOME compatibility rules

- Target GNOME 46+ and keep Ubuntu 26.04/GNOME 50 in metadata/runtime
  validation for the release gate. Metadata lists GNOME Shell 46 through 50;
  46 and 50 are validation anchors, while 47-49 are compatibility-declared
  intermediate versions.
- Use GNOME 45+ ESModule style.
- Do not allocate Shell UI objects before `enable()`.
- Destroy UI objects, disconnect signals, and remove timers in `disable()`.
- Keep GTK/libadwaita confined to preferences.

## Packaging rules

Task 05B chooses Option A: a development `.deb` package for local v0.1 release
smoke testing.

Primary package installs:

- `/usr/bin/codexbar-linuxd`
- `/usr/share/dbus-1/services/org.codexbar.Linux1.service`
- `/usr/lib/systemd/user/codexbar-linuxd.service`
- `/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/`
- `/usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml`

Package maintainer scripts compile GSettings schemas when the GLib tool is
available. They must not silently enable the extension, start a system daemon,
create a TCP/listener unit, touch browser/keyring/web files, or assume an active
user session. Active user managers may need `systemctl --user daemon-reload`
after package install or removal.

## Open decisions

1. Whether to seek extensions.gnome.org distribution later, given native daemon dependency.
2. Whether to support KDE via a separate frontend after GNOME MVP.
3. Which upstream CLI/provider polish tasks are needed before release packaging.
