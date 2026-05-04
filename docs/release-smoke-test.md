# Release Smoke Test

Use this checklist for Task 05B and v0.1 development package candidates on a
real Ubuntu GNOME session. Static validation is required first, but GNOME Shell
extension discovery, D-Bus activation, and panel lifecycle behavior still need
a live desktop session.

## Preconditions

- Ubuntu Desktop 24.04 LTS or newer with GNOME Shell 46+.
- Current checkout passes `./scripts/validate-packaging.sh` and
  `./scripts/validate-no-browser-web-surface.sh`.
- Do not copy raw provider output, diagnostics, cache files, screenshots, local
  paths, tokens, cookies, or browser/session material into test notes.

## A. Local Development Install Smoke

Install user-local files:

```bash
./scripts/install-local.sh
systemctl --user daemon-reload
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
gnome-extensions list --user | grep -Fx codexbar-linux@codexbar.dev
gnome-extensions enable codexbar-linux@codexbar.dev
gnome-extensions info codexbar-linux@codexbar.dev
```

On Wayland, log out and back in if GNOME Shell does not discover the extension
immediately after the copy. Confirm the GNOME Shell PID or start time changed
before recording discovery as a pass.

Exercise the daemon and UI:

```bash
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 Refresh s '{"schemaVersion":1,"reason":"manual","force":true,"providers":["codex"],"busyBehavior":"return_existing"}'
systemctl --user stop codexbar-linuxd.service
systemctl --user restart codexbar-linuxd.service
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
```

Pass conditions:

- The install places the daemon under `${PREFIX:-$HOME/.local}/bin`, the D-Bus
  service under `${XDG_DATA_HOME:-$HOME/.local/share}/dbus-1/services`, the user
  unit under `${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user`, and extension
  files under the canonical user extension directory.
- The installer does not enable the extension or start the daemon.
- D-Bus activation starts `codexbar-linuxd` for `GetDaemonInfo`.
- With no resolvable `codexbar`, Refresh remains available and reports a
  degraded/missing dependency state rather than a blank UI.
- With a configured upstream `codexbar`, Refresh reaches the upstream CLI adapter
  and returned UI/diagnostic data remains normalized and redacted.
- Stopping the daemon produces a recoverable panel/popover state; restarting
  recovers without reloading GNOME Shell.
- Diagnostics copy output contains no raw identities, tokens, cookies, browser
  paths, stdout/stderr payloads, or provider dashboard payloads.

Uninstall:

```bash
gnome-extensions disable codexbar-linux@codexbar.dev
./scripts/uninstall-local.sh
systemctl --user daemon-reload
```

Confirm only files recorded by `scripts/install-local.sh` were removed. User
config under `${XDG_CONFIG_HOME:-$HOME/.config}/codexbar-linux` and cache under
`${XDG_CACHE_HOME:-$HOME/.cache}/codexbar-linux` are intentionally preserved.

## B. Development Debian Package Smoke

Build and inspect the local package:

```bash
./scripts/build-deb.sh
dpkg-deb --field dist/codexbar-linux_0.1.0-1_$(dpkg --print-architecture).deb
dpkg-deb --contents dist/codexbar-linux_0.1.0-1_$(dpkg --print-architecture).deb
```

The package contents must include:

- `/usr/bin/codexbar-linuxd`
- `/usr/share/dbus-1/services/org.codexbar.Linux1.service`
- `/usr/lib/systemd/user/codexbar-linuxd.service`
- `/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/`
- `/usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml`

Install:

```bash
sudo apt install ./dist/codexbar-linux_0.1.0-1_$(dpkg --print-architecture).deb
systemctl --user daemon-reload
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
gnome-extensions list | grep -Fx codexbar-linux@codexbar.dev
gnome-extensions enable codexbar-linux@codexbar.dev
```

If the system-wide extension is not listed immediately on Wayland, log out and
back in, then repeat `gnome-extensions list`. The package must not enable the
extension automatically; the `gnome-extensions enable` command is the explicit
user action.

Exercise the same service and UI cases as the local install path:

```bash
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 Refresh s '{"schemaVersion":1,"reason":"manual","force":true,"providers":["codex"],"busyBehavior":"return_existing"}'
systemctl --user stop codexbar-linuxd.service
systemctl --user restart codexbar-linuxd.service
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
```

Pass conditions:

- D-Bus activation uses the packaged daemon path `/usr/bin/codexbar-linuxd`.
- The installed service remains a user service; no system daemon or socket unit
  is installed or started.
- GSettings schema compilation succeeds during package install/remove.
- Missing upstream CLI, available upstream CLI refresh, manual refresh,
  diagnostics copy, daemon stop/restart, and panel/popover recovery match the
  local install behavior.
- `./scripts/validate-no-browser-web-surface.sh` still passes after the package
  build and before release sign-off.

Remove:

```bash
gnome-extensions disable codexbar-linux@codexbar.dev
sudo apt remove codexbar-linux
systemctl --user daemon-reload
```

After removal, confirm package-owned files under `/usr/bin`, `/usr/share`, and
`/usr/lib/systemd/user` are gone. User config/cache remains preserved unless a
future explicit purge policy documents otherwise.
