# Acceptance criteria

## Product acceptance

### A. Install and first launch

- On Ubuntu Desktop 24.04 LTS and 26.04 LTS, installing the Debian package places the daemon, D-Bus service, systemd user unit, GSettings schema, and GNOME Shell extension files in expected locations.
- The package does not silently enable the extension.
- The package-installed `codexbar-linux-setup` helper runs as the desktop user,
  reloads the user systemd manager, verifies the daemon and D-Bus activation,
  detects user-local extension shadowing, and attempts user extension enablement
  only when GNOME Shell already discovers the packaged system extension.
- After explicit user enablement, a top-bar item appears without requiring X11.
- If daemon is not running, D-Bus activation starts it or the UI shows a clear recoverable state.
- On Wayland, a logout/login remains the reliable path for first discovery of a
  newly installed system-wide extension when the running Shell does not list it.

### B. Upstream CLI path

- If `codexbar` is on PATH and configured, daemon can fetch `usage` JSON.
- If `codexbar cost` is available, cost summaries appear in diagnostics/card secondary detail where appropriate.
- CLI missing, non-executable configured CLI path, timeout, parse error, and
  non-zero exit states are distinct.
- Provider CLI missing, unauthenticated, rate limited, unavailable, local cost
  unavailable, stale-cache fallback, success, and partial success states have
  setup-oriented copy and redacted diagnostics.
- Upstream provider IDs/order are preserved where possible.

### C. No-browser data path

- The daemon does not read browser cookies, browser profiles, browser cookie databases, keyrings, or provider web dashboards.
- `TestBrowserImport` returns a schema-valid `not_implemented` result and has no browser/cache/settings side effects.
- Browser-cookie/web-fetch implementation modules, fixtures, validators, direct dependencies, and project agent are absent.
- Static validation fails if the removed browser-cookie/web-fetch surface is reintroduced.
- Raw tokens, cookie/header strings, local paths, and raw provider payloads never appear in cache, logs, D-Bus output, fixtures, or copied diagnostics.

### D. Panel indicator

- Merged mode shows one item with two micro-bars where data exists.
- Provider mode shows one compact item per enabled provider without overflow for two to four providers.
- Minimal mode shows a low-noise icon/percent.
- Stale/error/unauthenticated states are visible but not visually noisy.

### E. Popover

- Popover renders cards for enabled providers.
- Loading and loaded states have stable dimensions.
- Manual refresh is always reachable.
- Provider dashboard/status URL actions are hidden for v0.1.
- Diagnostics copy action redacts secrets.

### F. Preferences

- General preferences save and apply.
- Panel mode, reset time format, theme, and selected provider preferences save
  and apply.
- The reserved start-on-login preference is not shown as an active v0.1 control.
- Daemon-owned refresh interval and provider enablement/source configuration
  save through `SetSettingsPatch`; they are not stored in Shell-owned GSettings.
- Reserved browser import test reports unsupported/no-op behavior.
- Diagnostics page shows daemon status, redacted CLI/config/cache path labels,
  CLI version when available, and D-Bus service.

## Engineering acceptance

- `./scripts/check.sh` passes.
- Unit tests cover redaction, schema normalization, CLI error mapping, cache read/write, and provider state mapping.
- UI fixture tests cover success, stale, unauthenticated, cookie rejected, missing dependency, timeout, and parse error.
- ADRs are updated for architectural changes.

## v0.1 Release Acceptance

These are the explicit pass/fail gates for a release-quality upstream-CLI-only
v0.1 candidate. A candidate is not releasable until each item is recorded with
the command or smoke evidence used.

- No-browser/web guard passes: `./scripts/validate-no-browser-web-surface.sh`
  reports no browser-cookie, browser-profile, keyring, provider web-fetch,
  browser-extension, localhost/TCP, forbidden dependency, fixture, validator, or
  runtime marker surface.
- Package install smoke passed on the recorded operator host: real
  `sudo apt install` succeeded, the local `_apt` sandbox warning was non-fatal,
  `/usr/bin/codexbar-linuxd --check` passed, D-Bus activation passed, and the
  packaged extension path was verified under `/usr/share/...`.
- Release smoke documentation recommends copying the local `.deb` to `/tmp`
  before `sudo apt install` so the `_apt` sandbox user can read the package
  without a project-directory permission note.
- Daemon/package version metadata passes: the daemon crate version is `0.1.0`,
  the Debian package version is `0.1.0-1`, `/usr/bin/codexbar-linuxd --version`
  reports `codexbar-linuxd 0.1.0`, `/usr/bin/codexbar-linuxd --check` validates
  version metadata quietly, and `GetDaemonInfo.version` reports `0.1.0` rather
  than `0.0.0`.
- Package metadata/content inspection passes: `dpkg-deb -I` and `dpkg-deb -c`
  show the expected package name, version, dependencies, daemon, D-Bus service,
  systemd user unit, system GNOME extension path, GSettings schema,
  `codexbar-linuxd(1)` manual page, and release smoke docs.
- Full repository gate passes: `./scripts/check.sh` completes successfully on
  Ubuntu 24.04 or newer and the final completion audit is given the saved log
  whose `repository gate passed for HEAD ...` marker matches the audited
  commit.
- Live GNOME 46+ Wayland smoke passes after `./scripts/install-local.sh`, an
  explicit user enablement, and a session restart when Wayland discovery
  requires it.
- GNOME metadata/runtime matrix includes GNOME 50: `metadata.json` lists Shell
  versions `46` through `50`; `46` and `50` are the required validation
  anchors, while `47`, `48`, and `49` are compatibility-declared intermediate
  Shell versions. Static validators assert the GNOME 46 support floor and GNOME
  50 validation target, and Ubuntu 26.04/GNOME 50 live smoke is recorded before
  final release sign-off.
- Daemon auto-refresh passes: startup refresh runs when daemon settings allow
  it, scheduled refresh repeats on `refresh.intervalSeconds`, `intervalSeconds:
  0` disables scheduled interval refresh without changing startup refresh,
  settings patches reschedule the interval without daemon restart, repeated
  upstream CLI missing/timeout/parse/nonzero failures back off instead of
  running every interval forever, and refresh failure clears the active-refresh
  guard so manual Refresh can recover.
- Provider off semantics pass: an empty provider config defaults empty-provider
  refreshes to the built-in Codex + Claude provider defaults, but a non-empty
  config with every provider disabled, set to source `off`, or without CLI
  fallback returns a schema-valid `noop` refresh instead of silently probing
  `codex`; explicit `RefreshOptions.providers` remains a manual override.
- Default provider settings pass: fresh daemon settings and legacy empty
  provider maps expose Codex and Claude enabled by default, with browser import
  disabled, CLI fallback enabled, and refresh targeting ordered as `codex`,
  then `claude`.
- Preferences UX passes: v0.1 does not show inert login-start controls;
  preferences display daemon info, refresh interval, panel provider selection,
  and provider enable/source controls, and daemon-owned writes go through
  `SetSettingsPatch`.
- Upstream CLI missing degraded UI passes: with no resolvable `codexbar`, manual
  refresh remains available, provider state is `missing_dependency`, and
  diagnostics are copyable and redacted.
- Upstream CLI broken-path degraded UI passes: with `CODEXBAR_CLI` pointing at a
  non-executable file, manual refresh remains available, provider state is
  `missing_dependency`, and diagnostics report
  `upstream_cli_not_executable` without exposing the raw path.
- Upstream CLI available Codex refresh passes: with a configured upstream
  `codexbar`, targeted Codex usage/status refresh succeeds through
  `sourceAdapter=upstream_cli`, cost is attempted through `provider both`, and no
  raw stdout/stderr, identity, token, cookie, or local path crosses D-Bus/cache.
- Daemon stop/restart recovery passes: stopping `codexbar-linuxd.service`
  renders a recoverable daemon-unavailable UI, and restarting the service lets
  Retry or Refresh recover without reloading GNOME Shell.
- Install/uninstall smoke passes: local install places only owned runtime files
  in user-local paths, does not auto-enable the extension, compiles schemas
  strictly, reloads the user systemd manager, and uninstall removes owned files
  while preserving user config/cache.
- Development package build passes: `./scripts/build-deb.sh` produces
  `dist/codexbar-linux.deb` from source without requiring live
  GNOME Shell or upstream `codexbar`, and the package contains the daemon,
  session D-Bus service, systemd user unit, system-wide GNOME extension files,
  GSettings schema, `codexbar-linuxd(1)` manual page, and release smoke docs.
- Package install/uninstall smoke passes: installing the development `.deb`
  compiles schemas, does not auto-enable the extension, does not start a system
  daemon or install a TCP listener, supports D-Bus activation through the
  user-run `codexbar-linux-setup` helper or an equivalent
  `systemctl --user daemon-reload`, and package removal removes only
  package-owned system files while preserving user config/cache.
- Package UI smoke is accepted only when
  `gnome-extensions info codexbar-linux@codexbar.dev` reports
  `Path: /usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev`. If it
  reports any path under
  `${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/codexbar-linux@codexbar.dev`
  or an expanded user-local equivalent, a development extension is shadowing the
  package extension and the package UI smoke is invalid.
- Task 05C package candidate validation records whether the package was tested
  through real `sudo apt install/remove/purge` or only through non-mutating
  package inspection and `apt-get -s install`. A candidate remains blocked from
  release sign-off until the real root-backed install/remove/purge path is
  recorded on the target Ubuntu GNOME host. If remove/purge was not rerun after
  the final successful package-extension smoke, it remains a required
  release-smoke gate even when package install and UI evidence have passed.
- A non-fatal `_apt` sandbox warning during `sudo apt install ./dist/codexbar-linux.deb` is
  not a package failure when the install succeeds; it indicates the local `.deb`
  path was inaccessible to the `_apt` sandbox user. The reproducible package
  smoke path copies the `.deb` to `/tmp` before installing.
- Packaged release binaries must not leak exact private build-root, home, Cargo,
  Rustup, or package-staging paths. The development package builder must compile
  with path remapping, strip the staged daemon, and fail the package build if
  those exact paths remain in the packaged daemon. Path-leak scanner failure
  output must report only redacted marker classes/counts, not the raw matching
  private paths.
- Diagnostics/cache/log-like output redaction passes: copied diagnostics,
  normalized cache, D-Bus payloads, fixture outputs, and command summaries
  contain no raw secrets, raw identity, raw provider payloads, raw paths,
  stdout/stderr snippets, cookie/header strings, or browser/session material.
- Missing/broken CLI diagnostics are user-facing and redacted according to
  `docs/upstream-cli-ux.md`.
