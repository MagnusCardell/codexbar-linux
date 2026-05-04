#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
EXTENSION_UUID="codexbar-linux@codexbar.dev"
PACKAGED_DAEMON_BIN="/usr/bin/codexbar-linuxd"
DAEMON_BIN="$PREFIX/bin/codexbar-linuxd"
USER_SYSTEMD_DIR="$CONFIG_HOME/systemd/user"
USER_SERVICE_FILE="$USER_SYSTEMD_DIR/codexbar-linuxd.service"
DBUS_SERVICE_DIR="$DATA_HOME/dbus-1/services"
DBUS_SERVICE_FILE="$DBUS_SERVICE_DIR/org.codexbar.Linux1.service"
MANIFEST_DIR="$DATA_HOME/codexbar-linux"
MANIFEST_FILE="$MANIFEST_DIR/install-local-manifest.txt"
OWNERSHIP_MARKER=".codexbar-linux-owned"

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required tool: $1" >&2
    exit 1
  fi
}

escape_sed_replacement() {
  printf '%s' "$1" | sed 's/[#&\]/\\&/g'
}

reload_user_systemd() {
  if ! command -v systemctl >/dev/null 2>&1; then
    return 0
  fi
  if systemctl --user daemon-reload; then
    echo "Reloaded user systemd manager."
  else
    echo "Warning: systemctl --user daemon-reload failed; run it before D-Bus activation." >&2
  fi
}

record_manifest_path() {
  printf '%s\n' "$1" >> "$MANIFEST_FILE"
}

install_runtime_extension() {
  EXT_DIR="$DATA_HOME/gnome-shell/extensions/$EXTENSION_UUID"
  rm -rf "$EXT_DIR"
  install -d -m 0755 "$EXT_DIR" "$EXT_DIR/src" "$EXT_DIR/schemas"
  for rel_file in metadata.json extension.js prefs.js stylesheet.css; do
    install -m 0644 "$ROOT/extension/$rel_file" "$EXT_DIR/$rel_file"
    record_manifest_path "$EXT_DIR/$rel_file"
  done
  while IFS= read -r -d '' rel_file; do
    install -m 0644 "$ROOT/extension/$rel_file" "$EXT_DIR/$rel_file"
    record_manifest_path "$EXT_DIR/$rel_file"
  done < <(cd "$ROOT/extension" && find src -maxdepth 1 -type f -name '*.js' -printf '%P\0' | sort -z | sed -z 's#^#src/#')
  install -Dm644 "$ROOT/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml" \
    "$EXT_DIR/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml"
  record_manifest_path "$EXT_DIR/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml"
  glib-compile-schemas --strict "$EXT_DIR/schemas"
  chmod 0644 "$EXT_DIR/schemas/gschemas.compiled"
  record_manifest_path "$EXT_DIR/schemas/gschemas.compiled"
  printf '%s\n' "installed-by=scripts/install-local.sh" > "$EXT_DIR/$OWNERSHIP_MARKER"
  chmod 0644 "$EXT_DIR/$OWNERSHIP_MARKER"
  record_manifest_path "$EXT_DIR/$OWNERSHIP_MARKER"
}

write_and_validate_service_files() {
  install -d -m 0755 "$USER_SYSTEMD_DIR" "$DBUS_SERVICE_DIR"
  daemon_bin_sed="$(escape_sed_replacement "$DAEMON_BIN")"
  local user_tmp dbus_tmp
  user_tmp="$(mktemp "${TMPDIR:-/tmp}/codexbar-user-service.XXXXXX")"
  dbus_tmp="$(mktemp "${TMPDIR:-/tmp}/codexbar-dbus-service.XXXXXX")"
  sed "s#ExecStart=$PACKAGED_DAEMON_BIN#ExecStart=$daemon_bin_sed#" \
    "$ROOT/packaging/systemd/codexbar-linuxd.service" > "$user_tmp"
  sed "s#Exec=$PACKAGED_DAEMON_BIN#Exec=$daemon_bin_sed#" \
    "$ROOT/packaging/dbus/org.codexbar.Linux1.service" > "$dbus_tmp"
  grep -Fx "ExecStart=$DAEMON_BIN" "$user_tmp" >/dev/null
  grep -Fx "Type=dbus" "$user_tmp" >/dev/null
  grep -Fx "BusName=org.codexbar.Linux1" "$user_tmp" >/dev/null
  grep -Fx "Name=org.codexbar.Linux1" "$dbus_tmp" >/dev/null
  grep -Fx "Exec=$DAEMON_BIN" "$dbus_tmp" >/dev/null
  grep -Fx "SystemdService=codexbar-linuxd.service" "$dbus_tmp" >/dev/null
  install -m 0644 "$user_tmp" "$USER_SERVICE_FILE"
  install -m 0644 "$dbus_tmp" "$DBUS_SERVICE_FILE"
  rm -f "$user_tmp" "$dbus_tmp"
  record_manifest_path "$USER_SERVICE_FILE"
  record_manifest_path "$DBUS_SERVICE_FILE"
}

require_tool cargo
require_tool glib-compile-schemas

cargo build --manifest-path "$ROOT/daemon/Cargo.toml"

install -Dm755 "$ROOT/daemon/target/debug/codexbar-linuxd" "$DAEMON_BIN"
install -d -m 0755 "$MANIFEST_DIR"
: > "$MANIFEST_FILE"
chmod 0644 "$MANIFEST_FILE"
record_manifest_path "$DAEMON_BIN"
write_and_validate_service_files
install_runtime_extension

python3 - "$EXT_DIR/metadata.json" "$EXTENSION_UUID" <<'PY'
import json
import sys
from pathlib import Path

metadata_path = Path(sys.argv[1])
expected_uuid = sys.argv[2]
metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
if metadata.get("uuid") != expected_uuid:
    raise SystemExit(
        "Installed metadata.json uuid does not match extension directory: "
        f"{metadata.get('uuid')!r} != {expected_uuid!r}"
    )
if "46" not in metadata.get("shell-version", []):
    raise SystemExit("Installed metadata.json must include GNOME Shell 46 support")
PY
reload_user_systemd

echo "Installed CodexBar GNOME files under user-local paths."
echo "The installer does not enable the extension or start the daemon automatically."
echo "Extension path: $EXT_DIR"
echo "D-Bus service path: $DBUS_SERVICE_FILE"
echo "D-Bus activation and the user service point to: $DAEMON_BIN"
echo "On Wayland, log out and back in or restart the full user session if GNOME Shell does not list the extension yet."
echo "Discovery check:"
echo "  gnome-extensions list --user | grep -Fx $EXTENSION_UUID"
echo "Manual extension enable command, after GNOME Shell discovery succeeds:"
echo "  gnome-extensions enable $EXTENSION_UUID"
