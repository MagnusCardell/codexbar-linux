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
- `/usr/share/man/man1/codexbar-linuxd.1.gz`

Install from `/tmp` to avoid non-fatal `_apt` sandbox permission notes from
project-local paths:

```bash
arch="$(dpkg --print-architecture)"
cp "dist/codexbar-linux_0.1.0-1_${arch}.deb" /tmp/
sudo apt install "/tmp/codexbar-linux_0.1.0-1_${arch}.deb"
systemctl --user daemon-reload
test -x /usr/bin/codexbar-linuxd
/usr/bin/codexbar-linuxd --version
/usr/bin/codexbar-linuxd --check
test -f /usr/share/dbus-1/services/org.codexbar.Linux1.service
test -f /usr/lib/systemd/user/codexbar-linuxd.service
test -d /usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev
test -f /usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml
test -f /usr/share/man/man1/codexbar-linuxd.1.gz
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
gnome-extensions list | grep -Fx codexbar-linux@codexbar.dev
gnome-extensions enable codexbar-linux@codexbar.dev
gnome-extensions info codexbar-linux@codexbar.dev
```

`apt` may print a non-fatal `_apt` sandbox warning when installing a local
`.deb` from a project directory that the `_apt` sandbox user cannot access. If
`sudo apt install` succeeds and the package files are installed, that warning
is not a package failure. The reproducible release-smoke command above copies
the package to `/tmp`; the project-local fallback is:

```bash
sudo apt install ./dist/codexbar-linux_0.1.0-1_$(dpkg --print-architecture).deb
```

If the system-wide extension is not listed immediately on Wayland, log out and
back in, then repeat `gnome-extensions list`. The package must not enable the
extension automatically; the `gnome-extensions enable` command is the explicit
user action.

Package-extension UI smoke is accepted only when:

```text
gnome-extensions info codexbar-linux@codexbar.dev
Path: /usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev
```

If `gnome-extensions info codexbar-linux@codexbar.dev` reports:

```text
Path: ~/.local/share/gnome-shell/extensions/codexbar-linux@codexbar.dev
```

or any expanded path under
`${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/codexbar-linux@codexbar.dev`,
then a user-local development extension is shadowing the package extension. The
package UI smoke is not valid until that shadow is removed or moved aside, the
session is restarted if needed, and `gnome-extensions info` reports the
`/usr/share` path.

Exercise the same service and UI cases as the local install path:

```bash
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 Refresh s '{"schemaVersion":1,"reason":"manual","force":true,"providers":["codex"],"busyBehavior":"return_existing"}'
systemctl --user set-environment CODEXBAR_CLI=/path/to/codexbar
systemctl --user stop codexbar-linuxd.service
systemctl --user restart codexbar-linuxd.service
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 Refresh s '{"schemaVersion":1,"reason":"manual","force":true,"providers":["codex"],"busyBehavior":"return_existing"}'
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
```

Pass conditions:

- `sudo apt install` succeeds. A non-fatal `_apt` sandbox warning from a
  project-local `.deb` path is acceptable only when the install itself succeeds.
- `/usr/bin/codexbar-linuxd --version` reports `codexbar-linuxd 0.1.0`.
- `/usr/bin/codexbar-linuxd --check` succeeds.
- `GetDaemonInfo` returns a schema-valid daemon info JSON string with
  `version` equal to `0.1.0` and `capabilities.browserImport=false`,
  `capabilities.linuxWebAdapters=false`.
- D-Bus activation uses the packaged daemon path `/usr/bin/codexbar-linuxd`.
- The installed service remains a user service; no system daemon or socket unit
  is installed or started.
- GSettings schema compilation succeeds during package install/remove.
- `gnome-extensions info codexbar-linux@codexbar.dev` reports the system
  extension path under `/usr/share/gnome-shell/extensions/`, not a user-local
  path under `~/.local/share/gnome-shell/extensions/`.
- Missing upstream CLI returns a degraded `upstream_cli_missing` state safely.
- A non-executable `CODEXBAR_CLI` path returns a degraded
  `upstream_cli_not_executable` state safely and does not expose the raw path.
- With `CODEXBAR_CLI` set in the systemd user environment and
  `codexbar-linuxd.service` restarted, available upstream CLI refresh returns
  current `upstream_cli` data.
- Manual refresh, diagnostics copy, daemon stop/restart, and panel/popover
  recovery match the local install behavior.
- `./scripts/validate-no-browser-web-surface.sh` still passes after the package
  build and before release sign-off.

Remove:

```bash
gnome-extensions disable codexbar-linux@codexbar.dev
sudo apt remove codexbar-linux
systemctl --user daemon-reload
```

Optional purge gate:

```bash
sudo apt purge codexbar-linux
systemctl --user daemon-reload
```

After removal, confirm package-owned files under `/usr/bin`, `/usr/share`, and
`/usr/lib/systemd/user` are gone. User config/cache remains preserved unless a
future explicit purge policy documents otherwise.

## Recorded Task 05C/05C.1 Candidate Result

Task 05C local release-candidate validation and Task 05C.1 operator package
smoke were run on 2026-05-04 against the v0.1 development package candidate.
Sanitized result:

- Package build passed and produced `codexbar-linux_0.1.0-1_amd64.deb`.
- Package metadata was inspected with `dpkg-deb -I`; package name, version,
  architecture, and GNOME/D-Bus/GSettings/systemd dependencies were correct.
  No browser, cookie, web-fetch, keyring, browser-extension, localhost, or
  provider-dashboard dependency was present.
- Package contents were inspected with `dpkg-deb -c`; the archive contained the
  daemon, session D-Bus service, user systemd unit, system-wide GNOME extension
  files, GSettings schema, manual page, and smoke-test docs under the intended
  paths.
- The package builder now compiles release binaries with path remapping, strips
  the staged daemon, and fails the build if exact private build-root, home,
  Cargo, Rustup, or package-staging paths remain in the packaged daemon.
- `apt-get -s install ./dist/codexbar-linux_0.1.0-1_amd64.deb` resolved
  cleanly as one new local package.
- Real `sudo apt install ./dist/codexbar-linux_0.1.0-1_amd64.deb` succeeded on
  the operator host. `apt` printed a non-fatal `_apt` sandbox permission note
  because the local `.deb` was inside the user project directory; the install
  still completed successfully.
- Installed file layout passed for `/usr/bin/codexbar-linuxd`,
  `/usr/share/dbus-1/services/org.codexbar.Linux1.service`,
  `/usr/lib/systemd/user/codexbar-linuxd.service`,
  `/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev`, and
  `/usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml`.
- `/usr/bin/codexbar-linuxd --check` passed.
- D-Bus activation passed from the installed service files.
- Isolated session-bus activation using the release-built daemon and the package
  D-Bus activation file passed for the missing-upstream-CLI path:
  `GetDaemonInfo` returned `browserImport=false`, `linuxWebAdapters=false`,
  `upstreamCli.available=false`, and `diagnosticCode=upstream_cli_missing`;
  `Refresh` produced a schema-valid `missing_dependency` provider state and
  redacted diagnostics.
- The root-backed package smoke also verified the missing-upstream-CLI degraded
  state: refresh returned degraded `upstream_cli_missing` safely.
- Release-mode live D-Bus upstream-CLI smoke passed with an explicitly
  configured upstream CLI binary. The test validated `GetDaemonInfo`, refresh
  signals, `RefreshFinished`, `GetSnapshot`, diagnostics, cache write, schemas,
  and live secret-marker checks without copying raw identity or diagnostics into
  this document.
- After setting `CODEXBAR_CLI` in the systemd user environment and restarting
  `codexbar-linuxd.service`, refresh worked and showed current `upstream_cli`
  data.
- After logout/login, `gnome-extensions info codexbar-linux@codexbar.dev`
  reported the system package extension path:
  `/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev`.
- The extension enabled successfully from the package install.
- The top-bar indicator appeared.
- Popover refresh worked.
- Browser-cookie, browser-profile, keyring, provider web-fetch,
  browser-extension, and localhost/TCP scope remained removed.
- Real `sudo apt remove` and optional `sudo apt purge` were previously tested
  during package candidate validation, but were not rerun after the final
  successful package-extension smoke. They remain part of the release smoke gate
  and must be rerun before final release sign-off.
- `lintian` exited successfully for the rebuilt package. The remaining
  development package warning was `initial-upload-closes-no-bugs`.

Known limitations before v0.1 release sign-off:

- Re-run `sudo apt remove codexbar-linux` and optional
  `sudo apt purge codexbar-linux` after the final successful package-extension
  smoke, and verify package-owned files are removed.
- Repeat the package smoke on the full target Ubuntu 24.04/26.04 GNOME matrix.
- Continue to reject any package UI smoke where
  `gnome-extensions info codexbar-linux@codexbar.dev` reports a user-local
  `~/.local/share/gnome-shell/extensions/codexbar-linux@codexbar.dev` path.
