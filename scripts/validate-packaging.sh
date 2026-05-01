#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - "$ROOT" <<'PY'
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
required = [
    "packaging/dbus/org.codexbar.Linux1.service",
    "packaging/systemd/codexbar-linuxd.service",
    "packaging/debian/control",
    "packaging/debian/changelog",
    "packaging/debian/rules",
    "packaging/debian/install",
    "packaging/debian/source/format",
    "scripts/install-local.sh",
    "scripts/uninstall-local.sh",
    "scripts/build-deb.sh",
]
for rel in required:
    if not (root / rel).is_file():
        raise SystemExit(f"Missing packaging file: {rel}")

dbus = (root / "packaging/dbus/org.codexbar.Linux1.service").read_text(encoding="utf-8")
if "Name=org.codexbar.Linux1" not in dbus:
    raise SystemExit("D-Bus service file must declare org.codexbar.Linux1")
if "SystemdService=codexbar-linuxd.service" not in dbus:
    raise SystemExit("D-Bus service file must reference the user service")

systemd = (root / "packaging/systemd/codexbar-linuxd.service").read_text(encoding="utf-8")
if "ExecStart=/usr/bin/codexbar-linuxd" not in systemd:
    raise SystemExit("systemd user service must execute /usr/bin/codexbar-linuxd")
listener_directives = ("ListenStream=", "ListenDatagram=", "ListenFIFO=", "SocketUser=", "SocketGroup=")
if any(directive in systemd for directive in listener_directives):
    raise SystemExit("Task 00 service file must not define listener/socket behavior")

install_local = (root / "scripts/install-local.sh").read_text(encoding="utf-8")
if "packaging/dbus/org.codexbar.Linux1.service" not in install_local:
    raise SystemExit("install-local.sh must install the D-Bus activation service")
if "Exec=$PACKAGED_DAEMON_BIN" not in install_local or "Exec=$daemon_bin_sed" not in install_local:
    raise SystemExit("install-local.sh must rewrite D-Bus Exec to the user-local daemon path")
if "ExecStart=$PACKAGED_DAEMON_BIN" not in install_local or "ExecStart=$daemon_bin_sed" not in install_local:
    raise SystemExit("install-local.sh must rewrite systemd ExecStart to the user-local daemon path")
if "systemctl --user daemon-reload" not in install_local:
    raise SystemExit("install-local.sh must reload the user systemd manager after writing the local unit")
local_install_requirements = {
    "XDG_DATA_HOME data root": 'DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"',
    "XDG_CONFIG_HOME config root": 'CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"',
    "GNOME extension under XDG data home": 'EXT_DIR="$DATA_HOME/gnome-shell/extensions/$EXTENSION_UUID"',
    "D-Bus service under XDG data home": 'DBUS_SERVICE_DIR="$DATA_HOME/dbus-1/services"',
    "installed service file permissions": 'chmod 0644 "$USER_SERVICE_FILE" "$DBUS_SERVICE_FILE"',
    "deterministic extension directory permissions": 'install -d -m 0755 "$EXT_DIR" "$EXT_DIR/schemas"',
    "deterministic extension file permissions": 'install -m 0644 "$ROOT/extension/$rel_file" "$EXT_DIR/$rel_file"',
    "strict installed schema compilation": 'glib-compile-schemas --strict "$EXT_DIR/schemas"',
    "compiled schema file permissions": 'chmod 0644 "$EXT_DIR/schemas/gschemas.compiled"',
    "installed metadata UUID guard": "Installed metadata.json uuid does not match extension directory",
}
for description, needle in local_install_requirements.items():
    if needle not in install_local:
        raise SystemExit(f"install-local.sh missing {description}: {needle}")

uninstall_local = (root / "scripts/uninstall-local.sh").read_text(encoding="utf-8")
if "systemctl --user daemon-reload" not in uninstall_local:
    raise SystemExit("uninstall-local.sh must reload the user systemd manager after removing the local unit")
local_uninstall_requirements = {
    "XDG_DATA_HOME data root": 'DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"',
    "XDG_CONFIG_HOME config root": 'CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"',
    "GNOME extension removal from XDG data home": 'EXT_DIR="$DATA_HOME/gnome-shell/extensions/$EXTENSION_UUID"',
    "D-Bus service removal from XDG data home": 'DBUS_SERVICE_DIR="$DATA_HOME/dbus-1/services"',
    "legacy PREFIX/share cleanup": 'LEGACY_DATA_HOME="$PREFIX/share"',
}
for description, needle in local_uninstall_requirements.items():
    if needle not in uninstall_local:
        raise SystemExit(f"uninstall-local.sh missing {description}: {needle}")

debian_install = (root / "packaging/debian/install").read_text(encoding="utf-8")
expected_install_entries = [
    "target/release/codexbar-linuxd usr/bin/",
    "packaging/dbus/org.codexbar.Linux1.service usr/share/dbus-1/services/",
    "packaging/systemd/codexbar-linuxd.service usr/lib/systemd/user/",
    "extension/* usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/",
    "schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml usr/share/glib-2.0/schemas/",
]
for entry in expected_install_entries:
    if entry not in debian_install:
        raise SystemExit(f"packaging/debian/install missing required install mapping: {entry}")

auto_enable = re.compile(
    r"(?:\bgnome-extensions\s+enable\b|"
    r"\bgnome-shell-extension-tool\s+-e\b|"
    r"\bgsettings\s+set\s+org\.gnome\.shell\s+enabled-extensions\b)"
)
auto_enable_violations = []
for path in sorted([root / "scripts/install-local.sh", *list((root / "packaging").rglob("*"))]):
    if not path.is_file():
        continue
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or stripped.startswith(("echo ", "printf ")):
            continue
        if auto_enable.search(stripped):
            auto_enable_violations.append(f"{path.relative_to(root)}:{line_no}: {stripped}")
if auto_enable_violations:
    raise SystemExit("Package/local install paths must not auto-enable the extension:\n" + "\n".join(auto_enable_violations))

build_deb = (root / "scripts/build-deb.sh").read_text(encoding="utf-8")
if "Task 08 packaging not implemented" not in build_deb:
    raise SystemExit("build-deb.sh must clearly report Task 08 packaging is not implemented")

print("Packaging skeleton structurally valid")
PY
