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

escape_sed_replacement() {
  printf '%s' "$1" | sed 's/[#&]/\\&/g'
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

cargo build --manifest-path "$ROOT/daemon/Cargo.toml"

install -Dm755 "$ROOT/daemon/target/debug/codexbar-linuxd" "$DAEMON_BIN"
install -d -m 0755 "$USER_SYSTEMD_DIR" "$DBUS_SERVICE_DIR"
daemon_bin_sed="$(escape_sed_replacement "$DAEMON_BIN")"
sed "s#ExecStart=$PACKAGED_DAEMON_BIN#ExecStart=$daemon_bin_sed#" \
  "$ROOT/packaging/systemd/codexbar-linuxd.service" > "$USER_SERVICE_FILE"
sed "s#Exec=$PACKAGED_DAEMON_BIN#Exec=$daemon_bin_sed#" \
  "$ROOT/packaging/dbus/org.codexbar.Linux1.service" > "$DBUS_SERVICE_FILE"
chmod 0644 "$USER_SERVICE_FILE" "$DBUS_SERVICE_FILE"

EXT_DIR="$DATA_HOME/gnome-shell/extensions/$EXTENSION_UUID"
install -d -m 0755 "$EXT_DIR" "$EXT_DIR/schemas"
while IFS= read -r -d '' rel_dir; do
  install -d -m 0755 "$EXT_DIR/$rel_dir"
done < <(cd "$ROOT/extension" && find . -mindepth 1 -type d -printf '%P\0')
while IFS= read -r -d '' rel_file; do
  install -m 0644 "$ROOT/extension/$rel_file" "$EXT_DIR/$rel_file"
done < <(cd "$ROOT/extension" && find . -type f -printf '%P\0')
install -Dm644 "$ROOT/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml" \
  "$EXT_DIR/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml"
if command -v glib-compile-schemas >/dev/null 2>&1; then
  glib-compile-schemas --strict "$EXT_DIR/schemas"
  chmod 0644 "$EXT_DIR/schemas/gschemas.compiled"
fi

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
