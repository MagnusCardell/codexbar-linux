#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
EXTENSION_UUID="codexbar-linux@codexbar.dev"

rm -f "$PREFIX/bin/codexbar-linuxd"
rm -f "$HOME/.config/systemd/user/codexbar-linuxd.service"
rm -f "$PREFIX/share/dbus-1/services/org.codexbar.Linux1.service"
rm -rf "$PREFIX/share/gnome-shell/extensions/$EXTENSION_UUID"

echo "Removed CodexBar GNOME files installed by scripts/install-local.sh."
echo "User config/cache is left untouched. Task 08 will define package purge behavior."
