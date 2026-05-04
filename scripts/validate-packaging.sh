#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - "$ROOT" <<'PY'
import re
import subprocess
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
    "packaging/debian/postinst",
    "packaging/debian/prerm",
    "packaging/debian/postrm",
    "packaging/debian/copyright",
    "packaging/debian/source/format",
    "scripts/install-local.sh",
    "scripts/uninstall-local.sh",
    "scripts/build-deb.sh",
    "schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml",
    "extension/metadata.json",
    "daemon/Cargo.toml",
    "daemon/src/main.rs",
]
for rel in required:
    if not (root / rel).is_file():
        raise SystemExit(f"Missing packaging file: {rel}")

dbus = (root / "packaging/dbus/org.codexbar.Linux1.service").read_text(encoding="utf-8")
if "Name=org.codexbar.Linux1" not in dbus:
    raise SystemExit("D-Bus service file must declare org.codexbar.Linux1")
if "Exec=/usr/bin/codexbar-linuxd" not in dbus:
    raise SystemExit("D-Bus service file must execute the packaged daemon path")
if "SystemdService=codexbar-linuxd.service" not in dbus:
    raise SystemExit("D-Bus service file must reference the user service")
if re.search(r"\b(tcp|localhost|ListenStream|Socket)\b", dbus, re.IGNORECASE):
    raise SystemExit("D-Bus service file must not claim TCP/listener behavior")

systemd = (root / "packaging/systemd/codexbar-linuxd.service").read_text(encoding="utf-8")
if "ExecStart=/usr/bin/codexbar-linuxd" not in systemd:
    raise SystemExit("systemd user service must execute /usr/bin/codexbar-linuxd")
if "Type=dbus" not in systemd or "BusName=org.codexbar.Linux1" not in systemd:
    raise SystemExit("systemd user service must be D-Bus activated with BusName=org.codexbar.Linux1")
if "[Socket]" in systemd or "WantedBy=multi-user.target" in systemd or "User=" in systemd:
    raise SystemExit("systemd unit must remain user-scoped and must not be a system daemon/socket")
listener_directives = ("ListenStream=", "ListenDatagram=", "ListenFIFO=", "SocketUser=", "SocketGroup=", "IPAddressAllow=", "IPAddressDeny=")
if any(directive in systemd for directive in listener_directives):
    raise SystemExit("systemd user service must not define listener/socket behavior")
if re.search(r"\b(tcp|localhost|http|listener)\b", systemd, re.IGNORECASE):
    raise SystemExit("systemd user service must not claim TCP/listener behavior")

dbus_xml = (root / "spec/dbus-org.codexbar.Linux1.xml").read_text(encoding="utf-8")
if '<node name="/org/codexbar/Linux1">' not in dbus_xml or '<interface name="org.codexbar.Linux1">' not in dbus_xml:
    raise SystemExit("D-Bus XML must retain org.codexbar.Linux1 object/interface alignment")

lib_rs = (root / "daemon/src/lib.rs").read_text(encoding="utf-8")
if 'pub const DBUS_INTERFACE: &str = "org.codexbar.Linux1";' not in lib_rs:
    raise SystemExit("daemon D-Bus interface constant must match packaged service name")
if 'pub const DBUS_OBJECT_PATH: &str = "/org/codexbar/Linux1";' not in lib_rs:
    raise SystemExit("daemon D-Bus object path constant must match D-Bus XML")
if 'pub const DAEMON_NAME: &str = "codexbar-linuxd";' not in lib_rs:
    raise SystemExit("daemon binary identity must match package daemon name")

install_local = (root / "scripts/install-local.sh").read_text(encoding="utf-8")
if "packaging/dbus/org.codexbar.Linux1.service" not in install_local:
    raise SystemExit("install-local.sh must install the D-Bus activation service")
if "Exec=$PACKAGED_DAEMON_BIN" not in install_local or "Exec=$daemon_bin_sed" not in install_local:
    raise SystemExit("install-local.sh must rewrite D-Bus Exec to the user-local daemon path")
if "ExecStart=$PACKAGED_DAEMON_BIN" not in install_local or "ExecStart=$daemon_bin_sed" not in install_local:
    raise SystemExit("install-local.sh must rewrite systemd ExecStart to the user-local daemon path")
if "systemctl --user daemon-reload" not in install_local:
    raise SystemExit("install-local.sh must reload the user systemd manager after writing the local unit")
if "require_tool glib-compile-schemas" not in install_local:
    raise SystemExit("install-local.sh must require glib-compile-schemas for strict schema compilation")
if "install_runtime_extension" not in install_local or "find src -maxdepth 1 -type f -name '*.js'" not in install_local:
    raise SystemExit("install-local.sh must install only runtime extension files")
if "install-local-manifest.txt" not in install_local or ".codexbar-linux-owned" not in install_local:
    raise SystemExit("install-local.sh must write ownership manifest and extension marker")
if "Refusing to replace extension directory without CodexBar ownership marker" not in install_local:
    raise SystemExit("install-local.sh must not clobber an unowned user extension directory")
local_install_requirements = {
    "XDG_DATA_HOME data root": 'DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"',
    "XDG_CONFIG_HOME config root": 'CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"',
    "GNOME extension under XDG data home": 'EXT_DIR="$DATA_HOME/gnome-shell/extensions/$EXTENSION_UUID"',
    "D-Bus service under XDG data home": 'DBUS_SERVICE_DIR="$DATA_HOME/dbus-1/services"',
    "installed user service permissions": 'install -m 0644 "$user_tmp" "$USER_SERVICE_FILE"',
    "installed D-Bus service permissions": 'install -m 0644 "$dbus_tmp" "$DBUS_SERVICE_FILE"',
    "deterministic extension directory permissions": 'install -d -m 0755 "$EXT_DIR" "$EXT_DIR/src" "$EXT_DIR/schemas"',
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
if "systemctl --user stop codexbar-linuxd.service" not in uninstall_local:
    raise SystemExit("uninstall-local.sh must stop the user service before removing activation files")
if "install-local-manifest.txt" not in uninstall_local or ".codexbar-linux-owned" not in uninstall_local:
    raise SystemExit("uninstall-local.sh must use ownership manifest and extension marker")
if 'grep -F "ExecStart=$PREFIX/bin/codexbar-linuxd"' not in uninstall_local:
    raise SystemExit("uninstall-local.sh fallback must verify owned user service before removal")
if 'grep -F "Exec=$PREFIX/bin/codexbar-linuxd"' not in uninstall_local:
    raise SystemExit("uninstall-local.sh fallback must verify owned D-Bus service before removal")
if "realpath -m --" not in uninstall_local or "is_inside_dir" not in uninstall_local:
    raise SystemExit("uninstall-local.sh must canonicalize manifest paths before removal")
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
    "daemon/target/release/codexbar-linuxd usr/bin/",
    "packaging/dbus/org.codexbar.Linux1.service usr/share/dbus-1/services/",
    "packaging/systemd/codexbar-linuxd.service usr/lib/systemd/user/",
    "extension/metadata.json usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/",
    "extension/extension.js usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/",
    "extension/prefs.js usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/",
    "extension/stylesheet.css usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/",
    "extension/src/*.js usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/src/",
    "schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml usr/share/glib-2.0/schemas/",
    "README.md usr/share/doc/codexbar-linux/",
    "LICENSE usr/share/doc/codexbar-linux/",
    "docs/gnome-smoke-test.md usr/share/doc/codexbar-linux/",
    "docs/release-smoke-test.md usr/share/doc/codexbar-linux/",
]
for entry in expected_install_entries:
    if entry not in debian_install:
        raise SystemExit(f"packaging/debian/install missing required install mapping: {entry}")
if "extension/* " in debian_install or "extension/tests" in debian_install or "task" in debian_install.lower():
    raise SystemExit("packaging/debian/install must not install broad extension globs, tests, or task docs")

release_smoke = (root / "docs/release-smoke-test.md").read_text(encoding="utf-8")
release_smoke_requirements = {
    "apt sandbox warning note": "`apt` may print a non-fatal `_apt` sandbox warning",
    "architecture-neutral /tmp apt install command": "sudo apt install \"/tmp/codexbar-linux_0.1.0-1_${arch}.deb\"",
    "packaged daemon check": "/usr/bin/codexbar-linuxd --check",
    "system extension accepted path": "Path: /usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev",
    "user-local shadowing path": "Path: ~/.local/share/gnome-shell/extensions/codexbar-linux@codexbar.dev",
    "package remove gate": "sudo apt remove codexbar-linux",
    "package purge gate": "sudo apt purge codexbar-linux",
    "CODEXBAR_CLI systemd user environment smoke": "CODEXBAR_CLI` in the systemd user environment",
    "recorded apt install success": "Real `sudo apt install ./dist/codexbar-linux_0.1.0-1_amd64.deb` succeeded",
    "recorded D-Bus activation pass": "D-Bus activation passed from the installed service files",
    "recorded CODEXBAR_CLI refresh pass": "After setting `CODEXBAR_CLI` in the systemd user environment and restarting",
    "non-executable CODEXBAR_CLI degraded state": "`upstream_cli_not_executable` state safely",
}
for description, needle in release_smoke_requirements.items():
    if needle not in release_smoke:
        raise SystemExit(f"docs/release-smoke-test.md missing {description}: {needle}")

gnome_smoke = (root / "docs/gnome-smoke-test.md").read_text(encoding="utf-8")
for needle in (
    "## Package Extension Path Sign-Off",
    "Path: /usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev",
    "Path: ~/.local/share/gnome-shell/extensions/codexbar-linux@codexbar.dev",
):
    if needle not in gnome_smoke:
        raise SystemExit(f"docs/gnome-smoke-test.md missing package extension sign-off marker: {needle}")

control = (root / "packaging/debian/control").read_text(encoding="utf-8")
control_required = {
    "Source: codexbar-linux",
    "Package: codexbar-linux",
    "Architecture: any",
    "Rules-Requires-Root: no",
    "Build-Depends: debhelper-compat (= 13), cargo, rustc, libglib2.0-bin, dbus",
}
for needle in control_required:
    if needle not in control:
        raise SystemExit(f"packaging/debian/control missing required field: {needle}")
if "Bootstrap skeleton" in control or "Task 00" in control or "Task 08" in control:
    raise SystemExit("packaging/debian/control must describe the real development package target")
for dep in ("gnome-shell", "libglib2.0-bin", "dbus-user-session", "gir1.2-gtk-4.0", "gir1.2-adw-1"):
    if dep not in control:
        raise SystemExit(f"packaging/debian/control missing runtime dependency: {dep}")

changelog = (root / "packaging/debian/changelog").read_text(encoding="utf-8")
if not re.search(r"^codexbar-linux \(0\.1\.0-1\) ", changelog):
    raise SystemExit("packaging/debian/changelog must declare the v0.1 development package")
if "Task 08" in changelog or "skeleton" in changelog.lower():
    raise SystemExit("packaging/debian/changelog must not describe packaging as a skeleton")

rules = (root / "packaging/debian/rules").read_text(encoding="utf-8")
if "cargo build --manifest-path daemon/Cargo.toml --release --locked" not in rules:
    raise SystemExit("packaging/debian/rules must build the release daemon")
if "--remap-path-prefix=$(CURDIR)=codexbar-linux" not in rules or "--remap-path-prefix=$(HOME)=home" not in rules:
    raise SystemExit("packaging/debian/rules must remap private build paths in release binaries")
if "cargo test --manifest-path daemon/Cargo.toml --locked" not in rules:
    raise SystemExit("packaging/debian/rules must run daemon tests")

maintainer_scripts = ["packaging/debian/postinst", "packaging/debian/prerm", "packaging/debian/postrm"]
for rel in maintainer_scripts:
    text = (root / rel).read_text(encoding="utf-8")
    if "gnome-extensions enable" in text or "gsettings set org.gnome.shell enabled-extensions" in text:
        raise SystemExit(f"{rel} must not auto-enable the GNOME extension")
    for line_no, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if "systemctl" not in stripped or "command -v systemctl" in stripped:
            continue
        if "--user" not in stripped:
            raise SystemExit(f"{rel}:{line_no} must not operate on the system systemd manager")
    if "systemctl --user enable" in text or "systemctl --user start" in text:
        raise SystemExit(f"{rel} must not enable or start the user daemon automatically")
    for forbidden in ("browser", "cookie", "keyring", "localhost", "TcpListener", "reqwest"):
        if forbidden.lower() in text.lower():
            raise SystemExit(f"{rel} contains forbidden packaging-scope marker: {forbidden}")
if "glib-compile-schemas" not in (root / "packaging/debian/postinst").read_text(encoding="utf-8"):
    raise SystemExit("postinst must compile GSettings schemas when possible")
if "glib-compile-schemas" not in (root / "packaging/debian/postrm").read_text(encoding="utf-8"):
    raise SystemExit("postrm must recompile GSettings schemas when possible")
if "systemctl --user daemon-reload" not in (root / "packaging/debian/postinst").read_text(encoding="utf-8"):
    raise SystemExit("postinst must tolerate user systemd daemon-reload when a user session exists")
if "systemctl --user daemon-reload" not in (root / "packaging/debian/postrm").read_text(encoding="utf-8"):
    raise SystemExit("postrm must tolerate user systemd daemon-reload when a user session exists")
for rel in maintainer_scripts:
    if (root / rel).stat().st_mode & 0o111 == 0:
        raise SystemExit(f"{rel} must be executable in git/package staging")

import json
import xml.etree.ElementTree as ET

metadata = json.loads((root / "extension/metadata.json").read_text(encoding="utf-8"))
if metadata.get("uuid") != "codexbar-linux@codexbar.dev":
    raise SystemExit("extension metadata UUID must match install path")
if metadata.get("settings-schema") != "org.gnome.shell.extensions.codexbar-linux":
    raise SystemExit("extension metadata settings schema must match packaged schema")
if "46" not in metadata.get("shell-version", []):
    raise SystemExit("extension metadata must include GNOME Shell 46 support")
schema = ET.parse(root / "schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml")
schema_ids = {node.attrib.get("id") for node in schema.findall(".//schema")}
if metadata.get("settings-schema") not in schema_ids:
    raise SystemExit("GSettings schema id must match extension metadata settings-schema")

packaging_text = "\n".join(
    [
        control,
        (root / ".github/workflows/check.yml").read_text(encoding="utf-8"),
        (root / "scripts/build-deb.sh").read_text(encoding="utf-8"),
    ]
)
for package in ("pkg-config", "libsqlite3-dev", "sqlite3", "cmake", "ca-certificates", "libsoup", "webkit", "libsecret", "curl", "chromium", "firefox"):
    if re.search(rf"\b{re.escape(package)}\b", packaging_text):
        raise SystemExit(
            f"{package} must not be required while browser-cookie/web-fetch is out of scope"
        )

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
if "Task 08 packaging not implemented" in build_deb or "not implemented" in build_deb:
    raise SystemExit("build-deb.sh must implement the v0.1 development package target")
for needle in [
    "env RUSTFLAGS=\"$RELEASE_RUSTFLAGS\" cargo build --manifest-path \"$ROOT/daemon/Cargo.toml\" --release --locked",
    "--remap-path-prefix=$ROOT=codexbar-linux",
    "--remap-path-prefix=$HOME=home",
    "dpkg-deb --root-owner-group --build",
    "strip --strip-unneeded \"$PKG_ROOT/usr/bin/codexbar-linuxd\"",
    "validate_no_build_path_leaks \"$PKG_ROOT/usr/bin/codexbar-linuxd\"",
    "strings -a \"$binary\"",
    "gzip -cn9 \"$ROOT/packaging/debian/changelog\"",
    "usr/share/gnome-shell/extensions/$EXTENSION_UUID",
    "usr/share/glib-2.0/schemas/$SCHEMA_ID.gschema.xml",
    "usr/share/dbus-1/services/org.codexbar.Linux1.service",
    "usr/lib/systemd/user/codexbar-linuxd.service",
    "--check",
    "libgcc-s1",
]:
    if needle not in build_deb:
        raise SystemExit(f"build-deb.sh missing package build behavior: {needle}")
for forbidden in ("gnome-extensions", "gnome-shell --version", "busctl", "CODEXBAR_CLI", "codexbar cost", "curl", "wget"):
    if forbidden in build_deb:
        raise SystemExit(f"build-deb.sh must not require live GNOME, upstream CLI, or network tools: {forbidden}")

completed = subprocess.run([str(root / "scripts/build-deb.sh"), "--check"], check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
if completed.returncode != 0:
    raise SystemExit(f"build-deb.sh --check failed:\nSTDOUT:\n{completed.stdout}\nSTDERR:\n{completed.stderr}")
if "package inputs valid" not in completed.stdout:
    raise SystemExit("build-deb.sh --check must report valid package inputs")

for rel in ("scripts/install-local.sh", "scripts/uninstall-local.sh", "scripts/build-deb.sh"):
    completed = subprocess.run(["bash", "-n", str(root / rel)], check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if completed.returncode != 0:
        raise SystemExit(f"{rel} failed bash -n:\n{completed.stderr}")
for rel in maintainer_scripts:
    completed = subprocess.run(["sh", "-n", str(root / rel)], check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if completed.returncode != 0:
        raise SystemExit(f"{rel} failed sh -n:\n{completed.stderr}")

print("Packaging development package target structurally valid")
PY
