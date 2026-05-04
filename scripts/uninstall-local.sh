#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
EXTENSION_UUID="codexbar-linux@codexbar.dev"
USER_SYSTEMD_DIR="$CONFIG_HOME/systemd/user"
DBUS_SERVICE_DIR="$DATA_HOME/dbus-1/services"
EXT_DIR="$DATA_HOME/gnome-shell/extensions/$EXTENSION_UUID"
MANIFEST_DIR="$DATA_HOME/codexbar-linux"
MANIFEST_FILE="$MANIFEST_DIR/install-local-manifest.txt"
OWNERSHIP_MARKER=".codexbar-linux-owned"
DAEMON_BIN="$PREFIX/bin/codexbar-linuxd"
USER_SERVICE_FILE="$USER_SYSTEMD_DIR/codexbar-linuxd.service"
DBUS_SERVICE_FILE="$DBUS_SERVICE_DIR/org.codexbar.Linux1.service"

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

canonical_path() {
  realpath -m -- "$1"
}

is_exact_path() {
  [[ "$(canonical_path "$1")" == "$(canonical_path "$2")" ]]
}

is_inside_dir() {
  local path_canon dir_canon
  path_canon="$(canonical_path "$1")"
  dir_canon="$(canonical_path "$2")"
  [[ "$path_canon" == "$dir_canon"/* ]]
}

is_owned_user_service() {
  [[ -f "$USER_SERVICE_FILE" ]] && grep -F "ExecStart=$PREFIX/bin/codexbar-linuxd" "$USER_SERVICE_FILE" >/dev/null 2>&1
}

is_owned_dbus_service() {
  [[ -f "$DBUS_SERVICE_FILE" ]] && grep -F "Exec=$PREFIX/bin/codexbar-linuxd" "$DBUS_SERVICE_FILE" >/dev/null 2>&1
}

safe_remove_file() {
  local path="$1"
  if is_exact_path "$path" "$DAEMON_BIN"; then
    rm -f "$path"
    return 0
  fi
  if is_exact_path "$path" "$USER_SERVICE_FILE"; then
    if is_owned_user_service; then
      rm -f "$path"
    else
      echo "Skipping user service not owned by local install: $path" >&2
    fi
    return 0
  fi
  if is_exact_path "$path" "$DBUS_SERVICE_FILE"; then
    if is_owned_dbus_service; then
      rm -f "$path"
    else
      echo "Skipping D-Bus service not owned by local install: $path" >&2
    fi
    return 0
  fi
  if [[ -f "$EXT_DIR/$OWNERSHIP_MARKER" ]] && is_inside_dir "$path" "$EXT_DIR"; then
    rm -f "$path"
    return 0
  fi
  echo "Skipping non-owned path from manifest: $path" >&2
}

stop_user_service() {
  if ! command -v systemctl >/dev/null 2>&1; then
    return 0
  fi
  if is_owned_user_service || is_owned_dbus_service; then
    systemctl --user stop codexbar-linuxd.service >/dev/null 2>&1 || true
  fi
}

stop_user_service

extension_owned=false
if [[ -f "$EXT_DIR/$OWNERSHIP_MARKER" ]]; then
  extension_owned=true
fi

if [[ -f "$MANIFEST_FILE" ]]; then
  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    safe_remove_file "$path"
  done < "$MANIFEST_FILE"
  rm -f "$MANIFEST_FILE"
else
  if is_owned_user_service; then
    rm -f "$USER_SERVICE_FILE"
  else
    echo "Skipping user service not owned by local install: $USER_SERVICE_FILE" >&2
  fi
  if is_owned_dbus_service; then
    rm -f "$DBUS_SERVICE_FILE"
  else
    echo "Skipping D-Bus service not owned by local install: $DBUS_SERVICE_FILE" >&2
  fi
  if [[ -f "$EXT_DIR/$OWNERSHIP_MARKER" ]]; then
    rm -rf "$EXT_DIR"
  else
    echo "Skipping extension directory without CodexBar ownership marker: $EXT_DIR" >&2
  fi
fi

if [[ "$extension_owned" == "true" && -d "$EXT_DIR" ]]; then
  rm -rf "$EXT_DIR"
fi
rmdir "$MANIFEST_DIR" 2>/dev/null || true

LEGACY_SYSTEMD_DIR="$HOME/.config/systemd/user"
LEGACY_DATA_HOME="$PREFIX/share"
if [[ "$LEGACY_SYSTEMD_DIR" != "$USER_SYSTEMD_DIR" ]]; then
  legacy_unit="$LEGACY_SYSTEMD_DIR/codexbar-linuxd.service"
  if [[ -f "$legacy_unit" ]] && grep -F "ExecStart=$PREFIX/bin/codexbar-linuxd" "$legacy_unit" >/dev/null 2>&1; then
    rm -f "$legacy_unit"
  fi
fi
if [[ "$LEGACY_DATA_HOME" != "$DATA_HOME" ]]; then
  legacy_dbus="$LEGACY_DATA_HOME/dbus-1/services/org.codexbar.Linux1.service"
  legacy_ext="$LEGACY_DATA_HOME/gnome-shell/extensions/$EXTENSION_UUID"
  if [[ -f "$legacy_dbus" ]] && grep -F "Exec=$PREFIX/bin/codexbar-linuxd" "$legacy_dbus" >/dev/null 2>&1; then
    rm -f "$legacy_dbus"
  fi
  if [[ -f "$legacy_ext/$OWNERSHIP_MARKER" ]]; then
    rm -rf "$legacy_ext"
  fi
fi
reload_user_systemd

echo "Removed CodexBar GNOME files installed by scripts/install-local.sh."
echo "User config/cache is left untouched."
