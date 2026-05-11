# CodexBar GNOME v0.1.0 Release Notes

CodexBar GNOME v0.1.0 is a development release-candidate package for a native
Ubuntu/GNOME top-bar companion backed by a user-scoped daemon. The supported
production data plane is upstream `codexbar` CLI and local provider tooling
through the daemon.

## Supported Environment

- Primary verified target: Ubuntu Desktop 24.04 LTS with GNOME Shell 46 on
  Wayland.
- Ubuntu 26.04 LTS/GNOME 50 compatibility remains a release gate before final
  sign-off.
- The release artifact is the stable `codexbar-linux.deb` file produced by
  `./scripts/build-deb.sh`.

## What Works

- Installs `codexbar-linuxd`, `codexbar-linux-setup`, the D-Bus session
  activation file, the systemd user unit, the GNOME Shell extension, the
  GSettings schema, documentation, and the `codexbar-linuxd(1)` manual page.
- D-Bus activation starts the user daemon for `org.codexbar.Linux1`.
- `GetDaemonInfo` reports daemon version `0.1.0`, safe build metadata where
  available, upstream CLI availability, redacted paths, and disabled browser/web
  capabilities.
- Manual Refresh remains available from the Shell UI and travels through the
  daemon D-Bus API.
- The daemon can refresh on startup, repeat on the configured 300-second
  default interval, treat `intervalSeconds: 0` as manual/off for scheduled
  interval refresh, back off repeated upstream CLI missing/timeout/parse/nonzero
  failures, and reschedule after daemon settings change without restarting.
- Default provider settings enable Codex and Claude, with refresh targeting
  ordered as `codex`, then `claude`; non-empty settings with every provider
  disabled, source `off`, or CLI fallback disabled return a no-op refresh
  instead of silently probing `codex`.
- Preferences show daemon info, refresh interval, panel provider selection, and
  provider enable/source controls backed by `SetSettingsPatch`. The reserved
  start-on-login control is hidden in v0.1 because daemon startup is D-Bus
  activation based.
- Missing or broken upstream CLI states degrade safely with setup-oriented copy
  and redacted diagnostics.
- A configured upstream `codexbar` CLI can refresh targeted Codex usage/status
  through `sourceAdapter=upstream_cli`; cost is attempted with upstream
  `codexbar cost --format json --json-only --provider both`.
- Normalized snapshots are cached locally for stale rendering. Raw provider
  payloads are not cached.
- The package does not auto-enable the GNOME extension, start a system daemon,
  install a TCP listener, or require a live GNOME session during build.

## Install Path

Build the development package:

```bash
./scripts/build-deb.sh
```

Install from `/tmp` to avoid the non-fatal `_apt` sandbox warning that can
appear for project-local paths:

```bash
cp -f dist/codexbar-linux.deb /tmp/codexbar-linux.deb
sudo apt install --reinstall /tmp/codexbar-linux.deb
codexbar-linux-setup
```

`--reinstall` keeps final smoke tied to the copied candidate artifact when the
same package version is already installed.

Enable the extension explicitly:

```bash
gnome-extensions enable codexbar-linux@codexbar.dev
gnome-extensions info codexbar-linux@codexbar.dev
```

`codexbar-linux-setup` is intentionally user-run. It reloads the user systemd
manager, verifies the daemon and D-Bus activation, detects user-local extension
shadowing, and enables the extension only when GNOME Shell already discovers the
packaged system extension.

Package UI smoke is valid only when `gnome-extensions info` reports:

```text
Path: /usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev
```

## Upstream CLI Setup

Install upstream CodexBar CLI, then verify:

```bash
codexbar --version
codexbar --format json --json-only --provider codex --source cli
codexbar cost --format json --json-only --provider both
```

If `codexbar` is not on the packaged daemon's `PATH`, configure the systemd user
manager environment and restart the user service:

```bash
systemctl --user set-environment CODEXBAR_CLI=/path/to/codexbar
systemctl --user restart codexbar-linuxd.service
```

## No-Browser Scope

v0.1.0 intentionally does not read browser cookies, browser profiles, browser
cookie databases, desktop keyrings, provider dashboards, provider session
material, or provider web pages. It does not install a browser extension and
does not expose a localhost/TCP API. The retained `TestBrowserImport` D-Bus
method is compatibility-only and returns a schema-valid `not_implemented`
result.

## Package Smoke Status

Recorded Task 05C/05C.1 evidence includes successful development package build,
metadata/content inspection, root-backed package install, `codexbar-linuxd
--check`, D-Bus activation, missing-upstream-CLI degraded state, configured
`CODEXBAR_CLI` refresh, system extension discovery under `/usr/share`, explicit
extension enablement, visible top-bar indicator, and popover refresh.

Task 05E release-candidate cleanup aligns daemon/package version metadata,
adds `codexbar-linuxd --version`, installs the daemon man page, removes the
empty `prerm` maintainer script, and keeps no-browser/package guards active.

## Known Limitations

- This is a development `.deb`, not a signed apt repository distribution.
- Package upgrade testing is not complete.
- Full Ubuntu 24.04/26.04 GNOME matrix coverage is not complete; the final
  matrix must explicitly record Ubuntu 26.04/GNOME 50 metadata/runtime
  validation.
- Real `sudo apt remove codexbar-linux` and `sudo apt purge codexbar-linux`
  were previously tested, but both must be rerun after the final successful
  package-extension smoke before final release sign-off.
- Upstream CLI usage/status defaults to targeted `codex`, then `claude`;
  all-provider usage/status probes remain explicit because promoted Linux
  evidence timed out for all-provider usage/status.
- Browser/web-backed providers remain unsupported unless upstream CLI or local
  provider tooling provides normalized data.

## Troubleshooting

Missing upstream CLI:

- The UI should show a recoverable setup state and keep Refresh available.
- Install upstream `codexbar` or set `CODEXBAR_CLI` for the systemd user
  manager, then restart `codexbar-linuxd.service`.

Systemd user environment:

- `CODEXBAR_CLI=/path/to/codexbar` must be set with `systemctl --user
  set-environment` for the packaged daemon, not only in an interactive shell.
- Restart the service after changing the environment.

GNOME extension path shadowing:

- Package UI smoke is invalid if `gnome-extensions info
  codexbar-linux@codexbar.dev` reports a path under
  `${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/`.
- Remove or move the user-local development extension, log out and back in if
  needed, and confirm the `/usr/share/...` path.

Wayland discovery:

- If the extension is not discovered immediately after install, log out and
  back in.
- Record that the GNOME Shell PID or start time changed before accepting the
  package extension discovery result.

Private package path install note:

- `sudo apt install ./dist/codexbar-linux.deb` may produce a non-fatal `_apt` sandbox warning
  if the project path is not readable by `_apt`.
- Copy the package to `/tmp` and install from there for the clean release-smoke
  path.
