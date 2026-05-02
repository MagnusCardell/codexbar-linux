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
│  - Linux browser-cookie import                                │
│  - provider web fetch adapters                                │
│  - redacted diagnostics                                       │
└───────┬──────────────┬────────────────┬──────────────────────┘
        │              │                │
        ▼              ▼                ▼
 upstream codexbar   browser DBs      provider web endpoints
 CLI                 + keyring         (cookie-backed, no raw persistence)
```

## Why this split

The GNOME Shell process is latency-sensitive, review-sensitive, and not a safe place for provider fetching, browser-cookie decryption, subprocess orchestration, or cache management. The daemon owns all I/O and normalization. The extension owns presentation only.

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
- No upstream CLI subprocess calls.
- No raw diagnostics construction.

### 2. Preferences

Responsibilities:

- Render native preferences with GTK4/libadwaita.
- Read/write extension UI preferences.
- Configure daemon through D-Bus or documented config file.
- Display daemon health and import tests.

### 3. Daemon

Responsibilities:

- D-Bus service `org.codexbar.Linux1`.
- D-Bus object `/org/codexbar/Linux1`.
- Cache file under `${XDG_CACHE_HOME:-~/.cache}/codexbar-linux/snapshot.json`.
- Config file under `${XDG_CONFIG_HOME:-~/.config}/codexbar-linux/config.json` for Linux-specific settings.
- Upstream config read from `~/.codexbar/config.json`.
- Upstream CLI resolver and runner.
- Browser-cookie import.
- Provider web fetch adapters.
- Redacted diagnostics.

### 4. Upstream CLI adapter

The adapter invokes:

```bash
codexbar --format json --json-only --provider all
codexbar cost --format json --json-only --provider all
codexbar config dump --pretty
codexbar config validate --format json --json-only
```

All invocations require:

- configurable timeout;
- environment allowlist;
- PATH resolution diagnostics;
- stdout/stderr size limits;
- exit-code mapping;
- redaction before logging;
- parse errors mapped into provider diagnostics.

### 5. Browser-cookie adapter

The adapter does four things:

1. discover browser profiles;
2. read a safe temporary copy of cookie stores;
3. decrypt values only when needed using the user’s normal keyring/session facilities;
4. build an in-memory cookie jar for provider web requests.

The adapter must not persist raw cookies or full cookie headers.

Task 04A freezes the detailed Linux browser-cookie and web-fetch architecture
in `docs/browser-cookie-architecture.md`, with the related threat model in
`docs/browser-cookie-threat-model.md`.

### 6. Provider web adapters

Provider web adapters are thin Linux shims, not a new provider framework. They exist only where upstream Linux CLI cannot yet provide web-backed data.

Adapter outputs must normalize to `spec/snapshot.schema.json` and should preserve upstream provider field names where possible.

Initial adapters:

- Codex/OpenAI web dashboard.
- Claude web.

Later candidates:

- Cursor.
- Factory/Droid.
- Other providers only after explicit product decision.

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
│   ├── sourceAdapter       # implementation adapter: upstream_cli/linux_web/cache/fixture/synthetic/none
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
- `cookie_rejected`
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
- Cache never stores raw cookies, raw provider responses, raw headers, Authorization values, Set-Cookie values, or decrypted browser secrets.
- Cache supports fast daemon startup and stale-state rendering through D-Bus.
- UI must clearly show stale cache when daemon cannot refresh.
- Cache write is atomic: write temp file, fsync, rename.
- Cache file permissions: `0600`; cache directory: `0700`.

## Refresh rules

Refresh sources are attempted by provider preference and recorded as `sourceAdapter`. Snapshot `source` records provider semantics, not implementation policy:

1. explicit Linux web source, if enabled and implemented;
2. upstream CLI/API/local path;
3. stale cache fallback.

Auto mode may choose Linux web first for providers where CLI Linux web parity is absent, but emitted snapshots must never use `auto`; they must record the actual `source` and `sourceAdapter`.

Manual refresh bypasses normal interval throttling but still respects concurrency limits.

## GNOME compatibility rules

- Target GNOME 46+.
- Use GNOME 45+ ESModule style.
- Do not allocate Shell UI objects before `enable()`.
- Destroy UI objects, disconnect signals, and remove timers in `disable()`.
- Keep GTK/libadwaita confined to preferences.

## Packaging rules

Primary package installs:

- `/usr/bin/codexbar-linuxd`
- `/usr/share/dbus-1/services/org.codexbar.Linux1.service`
- `/usr/lib/systemd/user/codexbar-linuxd.service`
- `/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/`
- `/usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml`

Package post-install may compile schemas and reload systemd user daemon. It must not silently enable the extension.

## Open decisions

1. Exact provider-web adapter boundaries after upstream CLI parity evolves.
2. Whether Firefox cookie import can be reliable enough without a helper extension for all target provider domains.
3. Whether to seek extensions.gnome.org distribution later, given native daemon dependency.
4. Whether to support KDE via a separate frontend after GNOME MVP.
