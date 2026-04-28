#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
EXTENSION_UUID="codexbar-linux@codexbar.dev"

cargo build --manifest-path "$ROOT/daemon/Cargo.toml"

install -Dm755 "$ROOT/daemon/target/debug/codexbar-linuxd" "$PREFIX/bin/codexbar-linuxd"
install -Dm644 "$ROOT/packaging/systemd/codexbar-linuxd.service" "$HOME/.config/systemd/user/codexbar-linuxd.service"
install -Dm644 "$ROOT/packaging/dbus/org.codexbar.Linux1.service" "$PREFIX/share/dbus-1/services/org.codexbar.Linux1.service"

EXT_DIR="$PREFIX/share/gnome-shell/extensions/$EXTENSION_UUID"
mkdir -p "$EXT_DIR" "$EXT_DIR/schemas"
cp -R "$ROOT/extension/." "$EXT_DIR/"
install -Dm644 "$ROOT/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml" \
  "$EXT_DIR/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml"
if command -v glib-compile-schemas >/dev/null 2>&1; then
  glib-compile-schemas "$EXT_DIR/schemas"
fi

echo "Installed Task 00 bootstrap files under user-local paths."
echo "Task 00 does not enable the extension or start the daemon automatically."
echo "Manual extension enable command, after GNOME Shell reload if needed:"
echo "  gnome-extensions enable $EXTENSION_UUID"
