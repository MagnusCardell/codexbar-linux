# GNOME Shell Smoke Test

Use these steps on a real GNOME Shell 46+ session after installing the
extension, schema, D-Bus service file, and user daemon from the local package or
development install script.

## Record Environment

Capture these values in the test notes:

```bash
gnome-shell --version
echo "$XDG_SESSION_TYPE"
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

## Install And Start

```bash
./scripts/install-local.sh
systemctl --user daemon-reload
systemctl --user restart codexbar-linuxd.service
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
gnome-extensions enable codexbar-linux@codexbar.dev
gnome-extensions info codexbar-linux@codexbar.dev
```

On Wayland, log out and back in after installing extension files if GNOME Shell
does not discover the extension immediately.

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
