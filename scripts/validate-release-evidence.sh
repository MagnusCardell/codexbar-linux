#!/usr/bin/env bash
set -euo pipefail

ALLOW_DEVELOPMENT_GNOME=0
PACKAGE_ROOT_EVIDENCE=""
GNOME_MATRIX_EVIDENCE=""

usage() {
  cat <<'EOF'
Usage: scripts/validate-release-evidence.sh --package-root PATH --gnome-matrix PATH
       scripts/validate-release-evidence.sh --allow-development-gnome --gnome-matrix PATH

Validate release smoke evidence.json files produced by:
  scripts/package-root-smoke.sh
  scripts/gnome-matrix-smoke.sh

Final v0.1 release evidence requires both package-root and GNOME-matrix
manifests. The GNOME manifest must be from Ubuntu 26.04, GNOME Shell 50,
Wayland, and the system package extension path. --allow-development-gnome
validates the GNOME manifest schema and shared safety assertions without
treating it as final release evidence.

Options:
  --package-root PATH         package-root smoke evidence.json.
  --gnome-matrix PATH         GNOME matrix smoke evidence.json.
  --allow-development-gnome   Allow non-GNOME-50 or user-local GNOME evidence.
  -h, --help                  Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --package-root)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "Missing argument for --package-root" >&2
        exit 2
      fi
      PACKAGE_ROOT_EVIDENCE="$2"
      shift 2
      ;;
    --gnome-matrix)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "Missing argument for --gnome-matrix" >&2
        exit 2
      fi
      GNOME_MATRIX_EVIDENCE="$2"
      shift 2
      ;;
    --allow-development-gnome)
      ALLOW_DEVELOPMENT_GNOME=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$PACKAGE_ROOT_EVIDENCE" && -z "$GNOME_MATRIX_EVIDENCE" ]]; then
  echo "At least one evidence path is required" >&2
  usage >&2
  exit 2
fi

if [[ "$ALLOW_DEVELOPMENT_GNOME" -eq 0 && ( -z "$PACKAGE_ROOT_EVIDENCE" || -z "$GNOME_MATRIX_EVIDENCE" ) ]]; then
  echo "Final release evidence requires both --package-root and --gnome-matrix" >&2
  exit 2
fi

python3 - "$PACKAGE_ROOT_EVIDENCE" "$GNOME_MATRIX_EVIDENCE" "$ALLOW_DEVELOPMENT_GNOME" <<'PY'
import json
import hashlib
import re
import sys
from pathlib import Path

package_path, gnome_path, allow_development_gnome_raw = sys.argv[1:]
allow_development_gnome = allow_development_gnome_raw == "1"

SHA256_RE = re.compile(r"^[0-9a-f]{64}$")

def load(path_value, expected_type):
    path = Path(path_value)
    if not path.is_file():
        raise SystemExit(f"Evidence file not found: {path}")
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SystemExit(f"{path} is not valid JSON: {exc}") from exc
    if data.get("schemaVersion") != 1:
        raise SystemExit(f"{path} must have schemaVersion=1")
    actual_type = data.get("smokeType")
    if actual_type != expected_type:
        if expected_type == "package-root" and actual_type == "package-stage":
            raise SystemExit(
                f"{path} is package-stage preflight evidence, not final root-backed package smoke"
            )
        raise SystemExit(f"{path} must have smokeType={expected_type}")
    if data.get("status") != "passed":
        raise SystemExit(f"{path} must have status=passed")
    return path, data

def require_bool(data, path, key, expected=True):
    if data.get(key) is not expected:
        raise SystemExit(f"{path} must have {key}={str(expected).lower()}")

def require_str(data, path, key):
    value = data.get(key)
    if not isinstance(value, str) or not value:
        raise SystemExit(f"{path} must have non-empty string {key}")
    return value

def require_sibling_files(path, names):
    evidence_dir = path.parent
    for name in names:
        sibling = evidence_dir / name
        if not sibling.is_file():
            raise SystemExit(f"{path} missing evidence sidecar: {name}")

def require_no_sibling_file(path, name):
    sibling = path.parent / name
    if sibling.exists():
        raise SystemExit(f"{path} has incomplete package-smoke marker: {name}")

def read_sibling(path, name):
    sibling = path.parent / name
    if not sibling.is_file():
        raise SystemExit(f"{path} missing evidence sidecar: {name}")
    return sibling.read_text(encoding="utf-8", errors="replace")

def sibling_payload_lines(path, name):
    lines = []
    for line in read_sibling(path, name).splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("$"):
            continue
        lines.append(stripped)
    return lines

def require_last_payload_line(path, name, expected):
    lines = sibling_payload_lines(path, name)
    if not lines:
        raise SystemExit(f"{path} sidecar {name} has no captured payload")
    if lines[-1] != expected:
        raise SystemExit(f"{path} sidecar {name} last payload must be: {expected}")

def require_payload_lines(path, name, expected):
    lines = sibling_payload_lines(path, name)
    if lines != list(expected):
        raise SystemExit(
            f"{path} sidecar {name} payload lines must be: {list(expected)!r}"
        )

def parse_os_release_sidecar(path, name):
    values = {}
    for line in sibling_payload_lines(path, name):
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        values[key] = value.strip().strip('"')
    return values

def gnome_shell_major(value):
    match = re.search(r"\bGNOME Shell\s+([0-9]+)(?:[.\s]|$)", value)
    return match.group(1) if match else None

def require_sibling_contains(path, name, *needles):
    text = read_sibling(path, name)
    for needle in needles:
        if needle not in text:
            raise SystemExit(f"{path} sidecar {name} missing expected content: {needle}")

def require_sibling_contains_any(path, name, *needles):
    text = read_sibling(path, name)
    if not any(needle in text for needle in needles):
        expected = " or ".join(needles)
        raise SystemExit(f"{path} sidecar {name} missing expected content: {expected}")

def require_sibling_not_contains(path, name, *needles):
    text = read_sibling(path, name)
    for needle in needles:
        if needle in text:
            raise SystemExit(f"{path} sidecar {name} contains forbidden content: {needle}")

def file_sha256(path_value, label):
    path = Path(path_value)
    if not path.is_file():
        raise SystemExit(f"{label} file not found: {path}")
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()

def validate_package(path_value):
    path, data = load(path_value, "package-root")
    require_no_sibling_file(path, "incomplete.txt")
    for key in (
        "candidate",
        "tmpCandidate",
        "candidateSha256",
        "tmpCandidateSha256",
        "architecture",
        "installedVersion",
        "daemonVersion",
    ):
        require_str(data, path, key)
    if not SHA256_RE.match(data["candidateSha256"]):
        raise SystemExit(f"{path} candidateSha256 is not a lowercase sha256")
    if not SHA256_RE.match(data["tmpCandidateSha256"]):
        raise SystemExit(f"{path} tmpCandidateSha256 is not a lowercase sha256")
    if data["candidateSha256"] != data["tmpCandidateSha256"]:
        raise SystemExit(f"{path} candidate and /tmp candidate checksums differ")
    if data["installedVersion"] != "0.1.0-1":
        raise SystemExit(f"{path} installedVersion must be 0.1.0-1")
    if data["daemonVersion"] != "codexbar-linuxd 0.1.0":
        raise SystemExit(f"{path} daemonVersion must be codexbar-linuxd 0.1.0")
    if not str(data["tmpCandidate"]).startswith("/tmp/"):
        raise SystemExit(f"{path} tmpCandidate must be under /tmp")
    candidate_actual_sha = file_sha256(data["candidate"], "candidate")
    if candidate_actual_sha != data["candidateSha256"]:
        raise SystemExit(f"{path} candidate file sha256 does not match candidateSha256")
    tmp_actual_sha = file_sha256(data["tmpCandidate"], "tmpCandidate")
    if tmp_actual_sha != data["tmpCandidateSha256"]:
        raise SystemExit(f"{path} tmpCandidate file sha256 does not match tmpCandidateSha256")
    for key in (
        "usedAptReinstallFromTmp",
        "sudoValidated",
        "systemExtensionPathVerified",
        "manualRefreshVerified",
        "diagnosticsRedactionScanPassed",
        "daemonRestartVerified",
        "removeVerified",
    ):
        require_bool(data, path, key, True)
    require_bool(data, path, "keepInstalled", False)
    require_bool(data, path, "purgeAfterRemove", True)
    require_bool(data, path, "finalReleaseEvidence", True)
    require_sibling_files(path, (
        "copy-candidate-to-tmp.txt",
        "candidate-checksums.txt",
        "candidate-byte-compare.txt",
        "candidate-fields.txt",
        "candidate-contents.txt",
        "sudo-validate.txt",
        "apt-install-reinstall.txt",
        "systemd-user-daemon-reload-after-install.txt",
        "installed-dpkg-query.txt",
        "installed-daemon-version.txt",
        "installed-daemon-check.txt",
        "installed-dbus-service.txt",
        "installed-systemd-user-service.txt",
        "daemon-info.txt",
        "gnome-extensions-list.txt",
        "gnome-extensions-enable.txt",
        "enabled-extensions-after-enable.txt",
        "gnome-extensions-info.txt",
        "manual-refresh.txt",
        "global-diagnostics.txt",
        "diagnostics-redaction-scan.txt",
        "systemd-user-stop.txt",
        "systemd-user-restart.txt",
        "daemon-info-after-restart.txt",
        "gnome-extensions-disable.txt",
        "enabled-extensions-after-disable.txt",
        "apt-remove.txt",
        "systemd-user-daemon-reload-after-remove.txt",
        "removed-daemon-absent.txt",
        "removed-dbus-service-absent.txt",
        "removed-systemd-user-service-absent.txt",
        "removed-extension-dir-absent.txt",
        "removed-gsettings-schema-absent.txt",
        "removed-manpage-absent.txt",
        "apt-purge.txt",
        "systemd-user-daemon-reload-after-purge.txt",
        "purged-dpkg-query.txt",
    ))
    require_sibling_contains(path, "copy-candidate-to-tmp.txt", "cp", data["candidate"], data["tmpCandidate"])
    require_sibling_contains(path, "candidate-checksums.txt", data["candidateSha256"], data["tmpCandidateSha256"])
    require_sibling_contains(path, "candidate-byte-compare.txt", "cmp", data["candidate"], data["tmpCandidate"])
    require_sibling_contains(path, "candidate-fields.txt", "Package: codexbar-linux", "Version: 0.1.0-1", f"Architecture: {data['architecture']}")
    require_payload_lines(path, "candidate-fields.txt", (
        "Package: codexbar-linux",
        "Version: 0.1.0-1",
        f"Architecture: {data['architecture']}",
    ))
    require_sibling_contains(
        path,
        "candidate-contents.txt",
        "usr/bin/codexbar-linuxd",
        "usr/share/dbus-1/services/org.codexbar.Linux1.service",
        "usr/lib/systemd/user/codexbar-linuxd.service",
        "usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml",
        "usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/metadata.json",
        "usr/share/man/man1/codexbar-linuxd.1.gz",
    )
    require_sibling_contains_any(path, "sudo-validate.txt", "sudo -v", "sudo -n -v")
    require_sibling_contains(path, "apt-install-reinstall.txt", "apt install --reinstall", data["tmpCandidate"])
    require_sibling_contains(path, "systemd-user-daemon-reload-after-install.txt", "systemctl --user daemon-reload")
    require_sibling_contains(path, "installed-dpkg-query.txt", "codexbar-linux", "0.1.0-1", data["architecture"])
    require_payload_lines(path, "installed-dpkg-query.txt", (
        f"codexbar-linux\t0.1.0-1\t{data['architecture']}",
    ))
    require_sibling_contains(path, "installed-daemon-version.txt", "codexbar-linuxd 0.1.0")
    require_sibling_contains(path, "installed-daemon-check.txt", "codexbar-linuxd --check")
    require_sibling_contains(path, "installed-dbus-service.txt", "Exec=/usr/bin/codexbar-linuxd")
    require_sibling_contains(path, "installed-systemd-user-service.txt", "ExecStart=/usr/bin/codexbar-linuxd")
    require_sibling_contains(path, "daemon-info.txt", "GetDaemonInfo")
    require_sibling_contains(path, "gnome-extensions-list.txt", "codexbar-linux@codexbar.dev")
    require_sibling_contains(path, "gnome-extensions-enable.txt", "gnome-extensions enable codexbar-linux@codexbar.dev")
    require_sibling_contains(path, "enabled-extensions-after-enable.txt", "codexbar-linux@codexbar.dev")
    require_sibling_contains(path, "gnome-extensions-info.txt", "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev")
    require_sibling_contains(path, "manual-refresh.txt", "Refresh")
    require_sibling_contains(path, "global-diagnostics.txt", "GetDiagnostics")
    require_sibling_contains(path, "diagnostics-redaction-scan.txt", "diagnostics redaction scan passed")
    require_sibling_contains(path, "systemd-user-stop.txt", "systemctl --user stop codexbar-linuxd.service")
    require_sibling_contains(path, "systemd-user-restart.txt", "systemctl --user restart codexbar-linuxd.service")
    require_sibling_contains(path, "daemon-info-after-restart.txt", "GetDaemonInfo")
    require_sibling_contains(path, "gnome-extensions-disable.txt", "gnome-extensions disable codexbar-linux@codexbar.dev")
    require_sibling_contains(path, "enabled-extensions-after-disable.txt", "gsettings get org.gnome.shell enabled-extensions")
    require_sibling_not_contains(path, "enabled-extensions-after-disable.txt", "codexbar-linux@codexbar.dev")
    require_sibling_contains(path, "apt-remove.txt", "apt remove -y codexbar-linux")
    require_sibling_contains(path, "systemd-user-daemon-reload-after-remove.txt", "systemctl --user daemon-reload")
    require_sibling_contains(path, "removed-daemon-absent.txt", "test", "/usr/bin/codexbar-linuxd")
    require_sibling_contains(path, "removed-dbus-service-absent.txt", "test", "/usr/share/dbus-1/services/org.codexbar.Linux1.service")
    require_sibling_contains(path, "removed-systemd-user-service-absent.txt", "test", "/usr/lib/systemd/user/codexbar-linuxd.service")
    require_sibling_contains(path, "removed-extension-dir-absent.txt", "test", "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev")
    require_sibling_contains(path, "removed-gsettings-schema-absent.txt", "test", "/usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml")
    require_sibling_contains(path, "removed-manpage-absent.txt", "test", "/usr/share/man/man1/codexbar-linuxd.1.gz")
    require_sibling_contains(path, "apt-purge.txt", "apt purge -y codexbar-linux")
    require_sibling_contains(path, "systemd-user-daemon-reload-after-purge.txt", "systemctl --user daemon-reload")
    require_sibling_contains(path, "purged-dpkg-query.txt", "dpkg-query -W codexbar-linux", "exit-status:")
    require_sibling_not_contains(path, "purged-dpkg-query.txt", "exit-status: 0")
    return path, data

def validate_gnome(path_value, final, package_data=None):
    path, data = load(path_value, "gnome-matrix")
    for key in ("shellVersion", "shellMajor", "osId", "osVersionId", "sessionType", "extensionPath"):
        require_str(data, path, key)
    if final:
        for key in ("installedVersion", "installedArchitecture"):
            require_str(data, path, key)
    versions = data.get("metadataShellVersions")
    if not isinstance(versions, list) or not all(isinstance(item, str) for item in versions):
        raise SystemExit(f"{path} must have string array metadataShellVersions")
    for version in ("46", "50"):
        if version not in versions:
            raise SystemExit(f"{path} metadataShellVersions missing {version}")
    for key in (
        "metadataIncludesGnome50",
        "enabledExtensionVerified",
        "manualRefreshVerified",
        "diagnosticsRedactionScanPassed",
        "daemonRestartVerified",
    ):
        require_bool(data, path, key, True)
    require_sibling_files(path, (
        "gnome-shell-version.txt",
        "os-release.txt",
        "session-type.txt",
        "gnome-shell-processes.txt",
        "gnome-shell-latest-process.txt",
        "enabled-extensions.txt",
        "gnome-extensions-info.txt",
        "installed-extension-metadata.txt",
        "metadata-validation.txt",
        "daemon-info.txt",
        "snapshot.txt",
        "manual-refresh.txt",
        "global-diagnostics.txt",
        "diagnostics-redaction-scan.txt",
        "systemd-user-stop.txt",
        "systemd-user-restart.txt",
        "daemon-info-after-restart.txt",
    ))
    require_sibling_contains(path, "gnome-shell-version.txt", data["shellVersion"])
    require_sibling_contains(path, "os-release.txt", data["osId"], data["osVersionId"])
    require_sibling_contains(path, "session-type.txt", data["sessionType"])
    require_last_payload_line(path, "gnome-shell-version.txt", data["shellVersion"])
    require_last_payload_line(path, "session-type.txt", data["sessionType"])
    os_release_sidecar = parse_os_release_sidecar(path, "os-release.txt")
    if os_release_sidecar.get("ID") != data["osId"]:
        raise SystemExit(f"{path} sidecar os-release.txt ID does not match evidence osId")
    if os_release_sidecar.get("VERSION_ID") != data["osVersionId"]:
        raise SystemExit(f"{path} sidecar os-release.txt VERSION_ID does not match evidence osVersionId")
    require_sibling_contains(path, "gnome-shell-processes.txt", "gnome-shell")
    require_sibling_contains(path, "gnome-shell-latest-process.txt", "ps", "gnome-shell")
    require_sibling_contains(path, "enabled-extensions.txt", "codexbar-linux@codexbar.dev")
    require_sibling_contains(path, "gnome-extensions-info.txt", data["extensionPath"])
    require_sibling_contains(
        path,
        "installed-extension-metadata.txt",
        '"uuid": "codexbar-linux@codexbar.dev"',
        '"settings-schema": "org.gnome.shell.extensions.codexbar-linux"',
        '"version": 1',
        '"46"',
        '"50"',
    )
    require_sibling_contains(path, "metadata-validation.txt", "metadata includes GNOME Shell 46 support floor, GNOME Shell 50 validation target, and extension version 1")
    require_sibling_contains(path, "daemon-info.txt", "GetDaemonInfo")
    require_sibling_contains(path, "snapshot.txt", "GetSnapshot")
    require_sibling_contains(path, "manual-refresh.txt", "Refresh")
    require_sibling_contains(path, "global-diagnostics.txt", "GetDiagnostics")
    require_sibling_contains(path, "diagnostics-redaction-scan.txt", "diagnostics redaction scan passed")
    require_sibling_contains(path, "systemd-user-stop.txt", "systemctl --user stop codexbar-linuxd.service")
    require_sibling_contains(path, "systemd-user-restart.txt", "systemctl --user restart codexbar-linuxd.service")
    require_sibling_contains(path, "daemon-info-after-restart.txt", "GetDaemonInfo")
    if final:
        if package_data is None:
            raise SystemExit(f"{path} final GNOME evidence requires package-root evidence for installed package metadata cross-check")
        if data.get("expectedShell") != "50":
            raise SystemExit(f"{path} final GNOME evidence must have expectedShell=50")
        if data.get("shellMajor") != "50":
            raise SystemExit(f"{path} final GNOME evidence must have shellMajor=50")
        if gnome_shell_major(data["shellVersion"]) != "50":
            raise SystemExit(f"{path} final GNOME evidence shellVersion must report GNOME Shell 50")
        if data.get("requireUbuntuVersion") != "26.04":
            raise SystemExit(f"{path} final GNOME evidence must have requireUbuntuVersion=26.04")
        if data.get("osId") != "ubuntu":
            raise SystemExit(f"{path} final GNOME evidence must have osId=ubuntu")
        if data.get("osVersionId") != "26.04":
            raise SystemExit(f"{path} final GNOME evidence must have osVersionId=26.04")
        if data.get("sessionType") != "wayland":
            raise SystemExit(f"{path} final GNOME evidence must have sessionType=wayland")
        require_bool(data, path, "ubuntuVersionVerified", True)
        require_bool(data, path, "requirePackagePath", True)
        require_bool(data, path, "requireWayland", True)
        require_bool(data, path, "packagePathVerified", True)
        require_bool(data, path, "finalReleaseEvidence", True)
        if data["extensionPath"] != "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev":
            raise SystemExit(f"{path} final GNOME evidence must use the system extension path")
        require_sibling_files(path, (
            "installed-dpkg-query.txt",
            "os-release-validation.txt",
        ))
        require_sibling_contains(path, "os-release-validation.txt", "os-release matches Ubuntu 26.04")
        require_sibling_contains(
            path,
            "installed-dpkg-query.txt",
            "codexbar-linux",
            data["installedVersion"],
            data["installedArchitecture"],
        )
        require_payload_lines(path, "installed-dpkg-query.txt", (
            f"codexbar-linux\t{data['installedVersion']}\t{data['installedArchitecture']}",
        ))
        if data["installedVersion"] != package_data["installedVersion"]:
            raise SystemExit(
                f"{path} GNOME evidence installedVersion does not match package-root installedVersion"
            )
        if data["installedArchitecture"] != package_data["architecture"]:
            raise SystemExit(
                f"{path} GNOME evidence installedArchitecture does not match package-root architecture"
            )
    return path

validated = []
package_result = None
if package_path:
    package_result = validate_package(package_path)
    validated.append(str(package_result[0]))
if gnome_path:
    validated.append(str(validate_gnome(
        gnome_path,
        final=not allow_development_gnome,
        package_data=package_result[1] if package_result else None,
    )))

mode = "development GNOME evidence" if allow_development_gnome else "final release evidence"
print(f"Release evidence valid for {mode}:")
for item in validated:
    print(f"  {item}")
PY
