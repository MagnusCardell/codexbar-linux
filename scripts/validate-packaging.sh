#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - "$ROOT" <<'PY'
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

build_deb = (root / "scripts/build-deb.sh").read_text(encoding="utf-8")
if "Task 08 packaging not implemented" not in build_deb:
    raise SystemExit("build-deb.sh must clearly report Task 08 packaging is not implemented")

print("Packaging skeleton structurally valid")
PY
