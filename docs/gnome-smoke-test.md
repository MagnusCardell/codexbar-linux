# GNOME Shell Smoke Test

Use these steps on a real GNOME Shell 46+ session after installing the
extension, schema, D-Bus service file, and user daemon from the local package or
development install script.

## Record Environment

Capture these values in the test notes:

```bash
gnome-shell --version
echo "$XDG_SESSION_TYPE"
echo "${XDG_DATA_HOME:-$HOME/.local/share}"
echo "${XDG_CONFIG_HOME:-$HOME/.config}"
pgrep -af gnome-shell
ps -o pid,lstart,cmd -p "$(pgrep -n gnome-shell)"
gsettings get org.gnome.shell enabled-extensions
```

If a live upstream CLI smoke is being tested, also record whether
`CODEXBAR_CLI=/path/to/codexbar` is configured for the daemon process.

## Preconditions

- GNOME Shell 46 or newer.
- `glib-compile-schemas` has been run for the installed schema directory.
- The user D-Bus service `org.codexbar.Linux1` is installed and can activate
  `codexbar-linuxd`.
- No raw provider credentials, browser-cookie data, or daemon cache files are
  copied into the extension directory.
- Local development installs place GNOME extension and D-Bus service files
  under `${XDG_DATA_HOME:-$HOME/.local/share}`. If `PREFIX` is set, it only
  changes where the debug daemon binary is installed.

## Install And Start

```bash
./scripts/install-local.sh
systemctl --user daemon-reload
systemctl --user restart codexbar-linuxd.service
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
gnome-extensions list --user | grep -Fx codexbar-linux@codexbar.dev
gnome-extensions enable codexbar-linux@codexbar.dev
gnome-extensions info codexbar-linux@codexbar.dev
```

On Wayland, log out and back in after first installing or replacing extension
files if GNOME Shell does not discover the extension immediately. A copied
extension directory can be correct on disk while the running Shell process still
has not rescanned user extensions. After logging back in, confirm the
`gnome-shell` PID or start time changed before treating discovery as a live
post-restart result.

## Discovery Diagnostics

If `gnome-extensions list --user` does not show
`codexbar-linux@codexbar.dev`, capture these checks before changing files:

```bash
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
EXT_DIR="$DATA_HOME/gnome-shell/extensions/codexbar-linux@codexbar.dev"
test -f "$EXT_DIR/metadata.json"
test -f "$EXT_DIR/extension.js"
test -f "$EXT_DIR/schemas/gschemas.compiled"
python3 -m json.tool "$EXT_DIR/metadata.json" >/dev/null
python3 - "$EXT_DIR/metadata.json" <<'PY'
import json
import sys
from pathlib import Path

metadata = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
assert metadata["uuid"] == "codexbar-linux@codexbar.dev"
assert "46" in metadata["shell-version"]
assert metadata["settings-schema"] == "org.gnome.shell.extensions.codexbar-linux"
PY
stat -c '%A %U:%G %n' "$EXT_DIR" "$EXT_DIR/metadata.json" "$EXT_DIR/extension.js"
journalctl --user -u codexbar-linuxd.service --no-pager -n 80
pgrep -af gnome-shell
ps -o pid,lstart,cmd -p "$(pgrep -n gnome-shell)"
```

Common discovery failures are a nested directory such as
`.../codexbar-linux@codexbar.dev/extension/metadata.json`, installing under
`$PREFIX/share` when `XDG_DATA_HOME` points elsewhere, missing
`schemas/gschemas.compiled`, metadata UUID mismatch, or a running Wayland Shell
session that has not been restarted since the files were copied. If the Shell
PID/start time predates the install, discovery cannot prove Task 03.2 runtime
activation yet; restart the full user session and rerun the discovery checks.

## Functional Checks

1. Verify the merged-mode CodexBar item appears in the top bar.
2. Open the popover and confirm provider cards render from `GetSnapshot`.
3. Press Refresh and confirm the daemon receives `Refresh` with
   `reason=manual`, `force=true`, and `busyBehavior=return_existing`.
4. If `CODEXBAR_CLI` is configured, verify upstream_cli-normalized Codex data
   appears with `sourceAdapter=upstream_cli` and no raw identity/path fields.
5. Press Diagnostics on a provider and copy the payload. Confirm copied text is
   redacted and contains no raw cookies, tokens, emails, browser paths, stdout,
   stderr, provider payloads, or request headers.
6. Stop the daemon:
   `systemctl --user stop codexbar-linuxd.service`.
   Confirm the panel/popover render a daemon-unavailable state instead of going
   blank.
7. Restart the daemon:
   `systemctl --user restart codexbar-linuxd.service`.
   Confirm Retry or Refresh recovers D-Bus data without reloading GNOME Shell.
8. Open preferences and switch `panel-mode` through `merged`, `provider`, and
   `minimal`. Confirm each mode updates the same top-bar item and opens the
   full popover.
9. Change `reset-time-format`, `theme`, and `selected-provider`. Confirm the
   display changes but no daemon config files are edited by preferences.
10. Disable and re-enable:
    `gnome-extensions disable codexbar-linux@codexbar.dev` then
    `gnome-extensions enable codexbar-linux@codexbar.dev`.
    Confirm only one panel item remains and refresh/diagnostics still work.

## Cleanup

```bash
gnome-extensions disable codexbar-linux@codexbar.dev
./scripts/uninstall-local.sh
```

## Boundaries

The Shell extension must not invoke upstream `codexbar`, read browser profiles,
read daemon cache files, call provider network endpoints, write daemon config,
or open a localhost API. All provider data in this smoke test comes from
`org.codexbar.Linux1` D-Bus methods and signals.
