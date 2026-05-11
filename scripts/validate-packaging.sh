#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - "$ROOT" <<'PY'
import os
import re
import subprocess
import sys
import tempfile
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
    "packaging/debian/postrm",
    "packaging/debian/copyright",
    "packaging/debian/source/format",
    "packaging/man/codexbar-linuxd.1",
    "scripts/install-local.sh",
    "scripts/uninstall-local.sh",
    "scripts/codexbar-linux-setup",
    "scripts/build-deb.sh",
    "schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml",
    "extension/metadata.json",
    "daemon/Cargo.toml",
    "daemon/src/main.rs",
    "docs/release-notes-0.1.0.md",
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

setup_helper = (root / "scripts/codexbar-linux-setup").read_text(encoding="utf-8")
setup_requirements = {
    "current-user root guard": "run this helper as the desktop user, not with sudo",
    "user daemon reload": "systemctl --user daemon-reload",
    "explicit CODEXBAR_CLI option": "--codexbar-cli",
    "daemon binary check": "/usr/bin/codexbar-linuxd",
    "D-Bus activation check": "busctl --user call",
    "GNOME enable attempt": 'gnome-extensions enable "$EXTENSION_UUID"',
    "GNOME activation copy": "GNOME activation is explicit user action",
    "user-local shadowing detection": "user-local extension may shadow the package",
    "package extension path": "Expected package extension path:",
}
for description, needle in setup_requirements.items():
    if needle not in setup_helper:
        raise SystemExit(f"codexbar-linux-setup missing {description}: {needle}")
if "gsettings set org.gnome.shell enabled-extensions" in setup_helper:
    raise SystemExit("codexbar-linux-setup must not write enabled-extensions through gsettings")
if "sudo " in setup_helper:
    raise SystemExit("codexbar-linux-setup must not ask the user to run sudo")
if "mktemp" in setup_helper or "config.json" in setup_helper or "SetSettingsPatch" in setup_helper:
    raise SystemExit("codexbar-linux-setup must not write daemon config directly")

debian_install = (root / "packaging/debian/install").read_text(encoding="utf-8")
expected_install_entries = [
    "daemon/target/release/codexbar-linuxd usr/bin/",
    "scripts/codexbar-linux-setup usr/bin/",
    "packaging/dbus/org.codexbar.Linux1.service usr/share/dbus-1/services/",
    "packaging/systemd/codexbar-linuxd.service usr/lib/systemd/user/",
    "extension/metadata.json usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/",
    "extension/extension.js usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/",
    "extension/prefs.js usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/",
    "extension/stylesheet.css usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/",
    "extension/src/*.js usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/src/",
    "schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml usr/share/glib-2.0/schemas/",
    "packaging/man/codexbar-linuxd.1 usr/share/man/man1/",
    "README.md usr/share/doc/codexbar-linux/",
    "LICENSE usr/share/doc/codexbar-linux/",
    "docs/gnome-smoke-test.md usr/share/doc/codexbar-linux/",
    "docs/release-smoke-test.md usr/share/doc/codexbar-linux/",
    "docs/release-notes-0.1.0.md usr/share/doc/codexbar-linux/",
]
for entry in expected_install_entries:
    if entry not in debian_install:
        raise SystemExit(f"packaging/debian/install missing required install mapping: {entry}")
if "extension/* " in debian_install or "extension/tests" in debian_install or "task" in debian_install.lower():
    raise SystemExit("packaging/debian/install must not install broad extension globs, tests, or task docs")

release_smoke = (root / "docs/release-smoke-test.md").read_text(encoding="utf-8")
release_smoke_requirements = {
    "apt sandbox warning note": "`apt` may print a non-fatal `_apt` sandbox warning",
    "stable /tmp apt reinstall command": "sudo apt install --reinstall /tmp/codexbar-linux.deb",
    "packaged daemon check": "/usr/bin/codexbar-linuxd --check",
    "system extension accepted path": "Path: /usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev",
    "user-local shadowing path": "Path: ~/.local/share/gnome-shell/extensions/codexbar-linux@codexbar.dev",
    "package remove gate": "sudo apt remove codexbar-linux",
    "package purge gate": "sudo apt purge codexbar-linux",
    "local repository gate log": "--local-gate-log",
    "saved check log marker": "saved `./scripts/check.sh` log",
    "package setup helper": "codexbar-linux-setup",
    "CODEXBAR_CLI systemd user environment smoke": "CODEXBAR_CLI` in the systemd user environment",
    "recorded apt install success": "Real `sudo apt install ./dist/codexbar-linux.deb` succeeded",
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

cargo_toml = (root / "daemon/Cargo.toml").read_text(encoding="utf-8")
package_match = re.search(r"^codexbar-linux \(([^)-]+)(?:-[^)]+)?\) ", changelog, re.MULTILINE)
cargo_match = re.search(r'^version = "([^"]+)"$', cargo_toml, re.MULTILINE)
if not package_match or not cargo_match or package_match.group(1) != cargo_match.group(1):
    raise SystemExit("daemon/Cargo.toml version must match the upstream version in packaging/debian/changelog")

rules = (root / "packaging/debian/rules").read_text(encoding="utf-8")
if "cargo build --manifest-path daemon/Cargo.toml --release --locked" not in rules:
    raise SystemExit("packaging/debian/rules must build the release daemon")
if "--remap-path-prefix=$(CURDIR)=codexbar-linux" not in rules or "--remap-path-prefix=$(HOME)=home" not in rules:
    raise SystemExit("packaging/debian/rules must remap private build paths in release binaries")
if "cargo test --manifest-path daemon/Cargo.toml --locked" not in rules:
    raise SystemExit("packaging/debian/rules must run daemon tests")

if (root / "packaging/debian/prerm").exists():
    raise SystemExit("packaging/debian/prerm must stay absent unless it performs release-required work")

maintainer_scripts = ["packaging/debian/postinst", "packaging/debian/postrm"]
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
    forbidden_user_home_markers = (
        "$HOME",
        "${HOME",
        "/home/",
        "~/",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_CACHE_HOME",
        "chown",
        "runuser",
        "sudo -u",
        "su -",
        "loginctl",
        "/etc/skel",
    )
    for forbidden in forbidden_user_home_markers:
        if forbidden in text:
            raise SystemExit(f"{rel} must not modify arbitrary user home state: {forbidden}")
    for forbidden in ("browser", "cookie", "keyring", "localhost", "TcpListener", "reqwest"):
        if forbidden.lower() in text.lower():
            raise SystemExit(f"{rel} contains forbidden packaging-scope marker: {forbidden}")
if "glib-compile-schemas" not in (root / "packaging/debian/postinst").read_text(encoding="utf-8"):
    raise SystemExit("postinst must compile GSettings schemas when possible")
if "glib-compile-schemas" not in (root / "packaging/debian/postrm").read_text(encoding="utf-8"):
    raise SystemExit("postrm must recompile GSettings schemas when possible")
if "systemctl --user" in (root / "packaging/debian/postinst").read_text(encoding="utf-8"):
    raise SystemExit("postinst must leave user-session daemon reload to codexbar-linux-setup")
if "systemctl --user" in (root / "packaging/debian/postrm").read_text(encoding="utf-8"):
    raise SystemExit("postrm must leave user-session daemon reload to codexbar-linux-setup")
for rel in maintainer_scripts:
    if (root / rel).stat().st_mode & 0o111 == 0:
        raise SystemExit(f"{rel} must be executable in git/package staging")
if (root / "scripts/codexbar-linux-setup").stat().st_mode & 0o111 == 0:
    raise SystemExit("scripts/codexbar-linux-setup must be executable")

man_page = (root / "packaging/man/codexbar-linuxd.1").read_text(encoding="utf-8")
for needle in (
    ".TH CODEXBAR-LINUXD 1",
    ".SH NAME",
    "codexbar-linuxd \\- user-scoped CodexBar GNOME daemon",
    ".B --version",
    ".B --check",
    ".B --print-snapshot",
    "Credential import, profile discovery, desktop secret-store access, dashboard",
    "collection, browser integration, and local network APIs are outside the v0.1",
):
    if needle not in man_page:
        raise SystemExit(f"packaging/man/codexbar-linuxd.1 missing manual-page content: {needle}")
for forbidden in ("cookie", "keyring", "localhost", "TcpListener", "provider dashboard", "provider web fetch"):
    if forbidden.lower() in man_page.lower():
        raise SystemExit(f"packaging/man/codexbar-linuxd.1 contains forbidden release-scope marker: {forbidden}")

import json
import xml.etree.ElementTree as ET

metadata = json.loads((root / "extension/metadata.json").read_text(encoding="utf-8"))
if metadata.get("uuid") != "codexbar-linux@codexbar.dev":
    raise SystemExit("extension metadata UUID must match install path")
if metadata.get("settings-schema") != "org.gnome.shell.extensions.codexbar-linux":
    raise SystemExit("extension metadata settings schema must match packaged schema")
if metadata.get("version") != 1:
    raise SystemExit("extension metadata version must be 1 for the v0.1 package")
if "46" not in metadata.get("shell-version", []):
    raise SystemExit("extension metadata must include GNOME Shell 46 support")
if "50" not in metadata.get("shell-version", []):
    raise SystemExit("extension metadata must include GNOME Shell 50 validation target")
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
    "require_file \"scripts/codexbar-linux-setup\"",
    "--remap-path-prefix=$ROOT=codexbar-linux",
    "--remap-path-prefix=$HOME=home",
    "dpkg-deb --root-owner-group --build",
    "strip --strip-unneeded \"$PKG_ROOT/usr/bin/codexbar-linuxd\"",
    "validate_no_build_path_leaks \"$PKG_ROOT/usr/bin/codexbar-linuxd\"",
    "strings -a \"$binary\"",
    "gzip -cn9 \"$ROOT/packaging/debian/changelog\"",
    "gzip -cn9 \"$ROOT/packaging/man/codexbar-linuxd.1\"",
    "usr/share/gnome-shell/extensions/$EXTENSION_UUID",
    "usr/share/glib-2.0/schemas/$SCHEMA_ID.gschema.xml",
    "usr/share/man/man1/codexbar-linuxd.1.gz",
    "usr/bin/codexbar-linux-setup",
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

for rel in ("scripts/install-local.sh", "scripts/uninstall-local.sh", "scripts/codexbar-linux-setup", "scripts/build-deb.sh"):
    completed = subprocess.run(["bash", "-n", str(root / rel)], check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if completed.returncode != 0:
        raise SystemExit(f"{rel} failed bash -n:\n{completed.stderr}")
setup_env = os.environ.copy()
with tempfile.TemporaryDirectory(prefix="codexbar-setup-check.") as setup_home:
    setup_home_path = Path(setup_home)
    setup_env["XDG_CONFIG_HOME"] = str(setup_home_path / "config")
    setup_env["XDG_DATA_HOME"] = str(setup_home_path / "data")
    completed = subprocess.run(
        [str(root / "scripts/codexbar-linux-setup"), "--dry-run", "--codexbar-cli", "/tmp/codexbar"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=setup_env,
    )
if completed.returncode != 0:
    raise SystemExit(f"codexbar-linux-setup --dry-run failed:\nSTDOUT:\n{completed.stdout}\nSTDERR:\n{completed.stderr}")
for needle in (
    "DRY RUN: systemctl --user daemon-reload",
    "DRY RUN: /usr/bin/codexbar-linuxd --check",
    "DRY RUN: busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo",
    "Default daemon providers: codex and claude via upstream_cli",
    "DRY RUN: gnome-extensions enable codexbar-linux@codexbar.dev",
):
    if needle not in completed.stdout:
        raise SystemExit(f"codexbar-linux-setup --dry-run missing expected output: {needle}")
for rel in maintainer_scripts:
    completed = subprocess.run(["sh", "-n", str(root / rel)], check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if completed.returncode != 0:
        raise SystemExit(f"{rel} failed sh -n:\n{completed.stderr}")

print("Packaging development package target structurally valid")
PY
