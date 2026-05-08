# GNOME Shell Smoke Test

Use these steps on a real GNOME Shell 46+ session after installing the
extension, schema, D-Bus service file, and user daemon from the local package or
development install script.

For v0.1 release sign-off, also run the local development and Debian package
paths in `docs/release-smoke-test.md`.

For the Ubuntu 26.04/GNOME 50 package runtime gate, the command-line evidence
helper is:

```bash
scripts/gnome-matrix-smoke.sh --require-shell 50 --require-ubuntu 26.04 --require-package-path --require-wayland --pause-for-ui
```

The helper records Ubuntu `/etc/os-release`, GNOME Shell version, session type,
extension path, installed package metadata, extension metadata, D-Bus
activation, manual refresh, diagnostics redaction scan, and daemon stop/restart
evidence under `target/release-smoke/`.

## Record Environment

Capture these values in the test notes:

```bash
gnome-shell --version
echo "$XDG_SESSION_TYPE"
echo "${XDG_DATA_HOME:-$HOME/.local/share}"
echo "${XDG_CONFIG_HOME:-$HOME/.config}"
pgrep -af gnome-shell
ps -o pid,lstart,cmd -p "$(pgrep -x gnome-shell | tail -n 1)"
gsettings get org.gnome.shell enabled-extensions
gnome-extensions info codexbar-linux@codexbar.dev
```

If a live upstream CLI smoke is being tested, also record whether
`CODEXBAR_CLI=/path/to/codexbar` is configured for the daemon process.

## Preconditions

- GNOME Shell 46 or newer.
- `glib-compile-schemas` has been run for the installed schema directory.
- The user D-Bus service `org.codexbar.Linux1` is installed and can activate
  `codexbar-linuxd`.
- No raw provider credentials, browser/session data, or daemon cache files are
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

## Package Extension Path Sign-Off

For package UI smoke, `gnome-extensions info codexbar-linux@codexbar.dev` must
report the system extension path:

```text
Path: /usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev
```

If it reports the user-local path instead:

```text
Path: ~/.local/share/gnome-shell/extensions/codexbar-linux@codexbar.dev
```

or any expanded path under
`${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/codexbar-linux@codexbar.dev`,
then a development extension is shadowing the package extension. The package UI
smoke is not valid until the user-local shadow is removed or moved aside, the
session is restarted if needed, and `gnome-extensions info` reports the
`/usr/share` path.

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
assert "50" in metadata["shell-version"]
assert metadata["settings-schema"] == "org.gnome.shell.extensions.codexbar-linux"
assert metadata["version"] == 1
PY
stat -c '%A %U:%G %n' "$EXT_DIR" "$EXT_DIR/metadata.json" "$EXT_DIR/extension.js"
journalctl --user -u codexbar-linuxd.service --no-pager -n 80
pgrep -af gnome-shell
ps -o pid,lstart,cmd -p "$(pgrep -x gnome-shell | tail -n 1)"
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
2. Open the popover and confirm the provider strip and selected-provider
   surface render from `GetSnapshot`.
3. Press Refresh and confirm the daemon receives `Refresh` with
   `reason=manual`, `force=true`, and `busyBehavior=return_existing`.
4. If `CODEXBAR_CLI` is configured, verify upstream_cli-normalized Codex data
   appears with `sourceAdapter=upstream_cli` and no raw identity/path fields.
5. Run a missing-CLI degraded pass:
   `systemctl --user unset-environment CODEXBAR_CLI`, ensure `codexbar` is not
   resolvable for the daemon process, restart `codexbar-linuxd.service`, press
   Refresh, and confirm the UI remains recoverable with a
   `missing_dependency` provider state and safe `upstream_cli_missing`
   diagnostics. The copied diagnostics must not include raw PATH contents,
   stdout, stderr, or local filesystem paths.
6. If a non-executable CLI path is being tested, set
   `CODEXBAR_CLI=/path/to/non-executable-codexbar` for the user service,
   restart the daemon, press Refresh, and confirm the UI reports a safe
   not-executable dependency diagnostic without crashing.
7. Press Diagnostics on a provider and copy the payload. Confirm copied text is
   redacted and contains no raw cookies, tokens, emails, browser paths, stdout,
   stderr, provider payloads, or request headers.
8. Stop the daemon:
   `systemctl --user stop codexbar-linuxd.service`.
   Confirm the panel/popover render a daemon-unavailable state instead of going
   blank.
9. Restart the daemon:
   `systemctl --user restart codexbar-linuxd.service`.
   Confirm Retry or Refresh recovers D-Bus data without reloading GNOME Shell.
10. Open preferences and switch `panel-mode` through `merged`, `provider`, and
   `minimal`. Confirm each mode updates the same top-bar item and opens the
   full popover.
11. Change `reset-time-format`, `theme`, and `selected-provider`. Confirm the
   display changes but no daemon config files are edited by preferences.
12. Disable and re-enable:
    `gnome-extensions disable codexbar-linux@codexbar.dev` then
    `gnome-extensions enable codexbar-linux@codexbar.dev`.
    Confirm only one panel item remains and refresh/diagnostics still work.

## Visual Polish Checklist

Use this checklist for Task 03.4 and later visual QA passes. Capture screenshots
only after masking account identity, diagnostics, and provider-specific secrets.
Use `docs/gnome-visual-target.md` as the visual acceptance source of truth.

- Merged mode top-bar item is naked and compact: provider label, two tiny
  meters, and a small state dot with no large pill treatment.
- Provider mode shows at most three compact provider clusters before a `+N`
  cue when there are more providers than can fit comfortably.
- Minimal mode shows a single low-noise icon and still opens the full popover.
- Popover width is stable between loading, refreshed, stale, error, and
  daemon-unavailable states.
- Popover structure is: quiet provider selector, selected-provider title area,
  meter-first Session/Weekly/Credits sections, secondary diagnostics/settings
  actions, and calm footer.
- The provider selector preserves snapshot provider order, marks the selected
  provider subtly, and dims unavailable providers without hiding them.
- The selected-provider surface is scannable at a glance: provider name,
  updated age/state, safe metadata, slim Session and secondary usage meters,
  credits when present, reset text, and working secondary actions are visually
  distinct.
- Refresh remains visible unless a refresh is already in progress.
- Diagnostics are collapsed by default, one click away from the selected
  provider, bounded when loaded, and copied text remains redacted.
- Stale, auth, timeout, parse-error, and daemon-unavailable wording is concise
  and does not repeat the same state twice.
- Provider dashboard/status URL actions are hidden in v0.1; setup guidance and
  diagnostics stay local and D-Bus-backed.
- Footer is one calm line that communicates daemon, CLI, cost, and
  `TestBrowserImport` unsupported/no-op status without showing raw paths or
  debug payloads.
- Disable/re-enable does not leave duplicate top-bar items, timers, signals, or
  stale popover actors behind.

## Visual Sign-Off Screenshot Set

Task 03.6 visual sign-off is blocked until screenshots are captured from a real
GNOME Shell 46+ session. Static checks can support implementation review, but
they cannot approve panel density, popover rhythm, alignment, or visual
hierarchy.

Capture and attach this set after masking account identity, diagnostics,
provider-specific secrets, and private paths:

1. Merged panel plus closed popover: full top-bar crop with adjacent GNOME
   indicators visible.
2. Open popover default state with diagnostics collapsed.
3. Diagnostics expanded for the selected provider.
4. Provider mode, including `+N` overflow when more than three providers exist.
5. Minimal mode closed panel item and the same full popover opened.
6. Daemon unavailable state after stopping `codexbar-linuxd.service`.
7. Stale/error state with stale, timeout, parse-error, or hard error copy
   visible.

Record GNOME Shell version, session type, panel mode, theme setting, fixture vs.
live data source, display scale if not 100 percent, and whether copied
diagnostics were separately checked for redaction.

## Recorded Live Result

Example result from the Task 03.3 live smoke on 2026-05-01:

- GNOME Shell version: 46.0.
- Session type: Wayland.
- The extension became discoverable after a real Shell/session restart.
- The CodexBar top-bar item appeared.
- The popover opened and rendered provider data.
- Manual Refresh worked through the daemon D-Bus `Refresh` method.
- Stopping and starting the daemon reflected in the UI without a blank panel.
- Upstream CLI Codex data was visible through the UI via the daemon.
- Remaining issue: visual polish is still needed in a later UI pass.

No private paths, raw account identifiers, raw diagnostics, screenshots, cookies,
tokens, or browser-profile data are part of this recorded result.

Task 05C.1 package smoke result from 2026-05-04:

- After root-backed package install and logout/login,
  `gnome-extensions info codexbar-linux@codexbar.dev` reported
  `/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev`.
- The extension enabled successfully from the packaged system extension.
- The CodexBar top-bar indicator appeared.
- Popover Refresh worked through the daemon D-Bus API.
- Missing upstream CLI rendered degraded `upstream_cli_missing` safely.
- After `CODEXBAR_CLI` was set in the systemd user environment and
  `codexbar-linuxd.service` restarted, Refresh showed current `upstream_cli`
  data.

No browser-cookie, browser-profile, keyring, provider web-fetch,
browser-extension, localhost/TCP, raw identity, token, cookie, diagnostics, or
provider payload evidence was recorded as part of this result.

## Fixture Mode Development

The production daemon rejects explicit fixture refresh requests. For local UI
development against fixture data, start the daemon with the fixture gate enabled:

```bash
CODEXBAR_LINUX_ALLOW_FIXTURE=1 cargo run --manifest-path daemon/Cargo.toml
```

For the user systemd service in a development session:

```bash
systemctl --user set-environment CODEXBAR_LINUX_ALLOW_FIXTURE=1
systemctl --user restart codexbar-linuxd.service
```

Unset the variable or restart the service from a clean user environment before
production-like smoke runs. Fixture mode is for tests and explicit development
only.

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
