#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
EXTENSION_UUID="codexbar-linux@codexbar.dev"
USER_SYSTEMD_DIR="$CONFIG_HOME/systemd/user"
DBUS_SERVICE_DIR="$DATA_HOME/dbus-1/services"
EXT_DIR="$DATA_HOME/gnome-shell/extensions/$EXTENSION_UUID"

reload_user_systemd() {
  if ! command -v systemctl >/dev/null 2>&1; then
    return 0
  fi
  if systemctl --user daemon-reload; then
    echo "Reloaded user systemd manager."
  else
    echo "Warning: systemctl --user daemon-reload failed; stale unit metadata may remain until reload." >&2
  fi
}

rm -f "$PREFIX/bin/codexbar-linuxd"
rm -f "$USER_SYSTEMD_DIR/codexbar-linuxd.service"
rm -f "$DBUS_SERVICE_DIR/org.codexbar.Linux1.service"
rm -rf "$EXT_DIR"

LEGACY_SYSTEMD_DIR="$HOME/.config/systemd/user"
LEGACY_DATA_HOME="$PREFIX/share"
if [[ "$LEGACY_SYSTEMD_DIR" != "$USER_SYSTEMD_DIR" ]]; then
  rm -f "$LEGACY_SYSTEMD_DIR/codexbar-linuxd.service"
fi
if [[ "$LEGACY_DATA_HOME" != "$DATA_HOME" ]]; then
  rm -f "$LEGACY_DATA_HOME/dbus-1/services/org.codexbar.Linux1.service"
  rm -rf "$LEGACY_DATA_HOME/gnome-shell/extensions/$EXTENSION_UUID"
fi
reload_user_systemd

echo "Removed CodexBar GNOME files installed by scripts/install-local.sh."
echo "User config/cache is left untouched. Task 08 will define package purge behavior."
