#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXTENSION_UUID="codexbar-linux@codexbar.dev"
PACKAGE_NAME="codexbar-linux"
DBUS_NAME="org.codexbar.Linux1"
DBUS_PATH="/org/codexbar/Linux1"
DBUS_INTERFACE="org.codexbar.Linux1"
EXPECTED_SHELL=""
EXPECTED_UBUNTU=""
REQUIRE_PACKAGE_PATH=0
REQUIRE_WAYLAND=0
PAUSE_FOR_UI=0
EVIDENCE_DIR=""

usage() {
  cat <<'EOF'
Usage: scripts/gnome-matrix-smoke.sh [--require-shell VERSION] [--require-ubuntu VERSION_ID] [--require-package-path] [--require-wayland] [--pause-for-ui] [--evidence-dir DIR]

Capture GNOME runtime matrix evidence for the installed CodexBar extension and
user daemon. For final v0.1 Ubuntu 26.04 validation, run:

  scripts/gnome-matrix-smoke.sh --require-shell 50 --require-ubuntu 26.04 --require-package-path --require-wayland --pause-for-ui

Options:
  --require-shell VERSION  Require gnome-shell --version to report this major version.
  --require-ubuntu VERSION_ID
                           Require /etc/os-release to report ID=ubuntu and this VERSION_ID.
  --require-package-path   Require extension path under /usr/share/gnome-shell/extensions/.
  --require-wayland        Require XDG_SESSION_TYPE=wayland.
  --pause-for-ui           Pause after daemon stop so the operator can inspect UI recovery.
  --evidence-dir DIR       Directory for smoke logs. Defaults under target/release-smoke/.
  -h, --help               Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --require-shell)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "Missing argument for --require-shell" >&2
        exit 2
      fi
      EXPECTED_SHELL="$2"
      shift 2
      ;;
    --require-ubuntu)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "Missing argument for --require-ubuntu" >&2
        exit 2
      fi
      EXPECTED_UBUNTU="$2"
      shift 2
      ;;
    --require-package-path)
      REQUIRE_PACKAGE_PATH=1
      shift
      ;;
    --require-wayland)
      REQUIRE_WAYLAND=1
      shift
      ;;
    --pause-for-ui)
      PAUSE_FOR_UI=1
      shift
      ;;
    --evidence-dir)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "Missing argument for --evidence-dir" >&2
        exit 2
      fi
      EVIDENCE_DIR="$2"
      shift 2
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

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required tool: $1" >&2
    exit 1
  fi
}

for tool in bash busctl date dpkg-query gnome-extensions gnome-shell grep gsettings mkdir pgrep ps python3 sed systemctl tail tee; do
  require_tool "$tool"
done

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
if [[ -z "$EVIDENCE_DIR" ]]; then
  EVIDENCE_DIR="$ROOT/target/release-smoke/gnome-matrix-$timestamp"
fi
umask 077
mkdir -p "$EVIDENCE_DIR"

log_cmd() {
  local log="$1"
  shift
  {
    printf '$'
    printf ' %q' "$@"
    printf '\n'
  } >>"$log"
}

run_captured() {
  local name="$1"
  shift
  local log="$EVIDENCE_DIR/$name.txt"
  : >"$log"
  log_cmd "$log" "$@"
  "$@" >>"$log" 2>&1
}

run_captured "gnome-shell-version" gnome-shell --version
run_captured "os-release" python3 -c 'from pathlib import Path; print(Path("/etc/os-release").read_text(encoding="utf-8"), end="")'
run_captured "session-type" bash -c 'printf "%s\n" "${XDG_SESSION_TYPE:-unknown}"'
run_captured "gnome-shell-processes" pgrep -af gnome-shell
latest_shell_pid="$(pgrep -x gnome-shell | tail -n 1)"
if [[ -z "$latest_shell_pid" ]]; then
  echo "Could not find a running gnome-shell process with pgrep -x gnome-shell" >&2
  exit 1
fi
run_captured "gnome-shell-latest-process" ps -o pid,lstart,cmd -p "$latest_shell_pid"
run_captured "enabled-extensions" gsettings get org.gnome.shell enabled-extensions
run_captured "gnome-extensions-info" gnome-extensions info "$EXTENSION_UUID"
if ! grep -F "$EXTENSION_UUID" "$EVIDENCE_DIR/enabled-extensions.txt" >/dev/null; then
  echo "Expected enabled extension: $EXTENSION_UUID" >&2
  cat "$EVIDENCE_DIR/enabled-extensions.txt" >&2
  exit 1
fi

if [[ -n "$EXPECTED_SHELL" ]]; then
  if ! grep -Eq "GNOME Shell ${EXPECTED_SHELL}([. ]|$)" "$EVIDENCE_DIR/gnome-shell-version.txt"; then
    echo "Expected GNOME Shell major version $EXPECTED_SHELL" >&2
    cat "$EVIDENCE_DIR/gnome-shell-version.txt" >&2
    exit 1
  fi
fi

if [[ -n "$EXPECTED_UBUNTU" ]]; then
  python3 - "$EVIDENCE_DIR/os-release.txt" "$EXPECTED_UBUNTU" <<'PY' >"$EVIDENCE_DIR/os-release-validation.txt"
import sys
from pathlib import Path

path = Path(sys.argv[1])
expected = sys.argv[2]
values = {}
for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
    if line.startswith("$") or "=" not in line:
        continue
    key, value = line.split("=", 1)
    values[key] = value.strip().strip('"')
if values.get("ID") != "ubuntu":
    raise SystemExit(f"Expected ID=ubuntu in {path}")
if values.get("VERSION_ID") != expected:
    raise SystemExit(f"Expected VERSION_ID={expected} in {path}")
print(f"os-release matches Ubuntu {expected}")
PY
fi

if [[ "$REQUIRE_WAYLAND" -eq 1 ]] && ! grep -Fx "wayland" "$EVIDENCE_DIR/session-type.txt" >/dev/null; then
  echo "Expected XDG_SESSION_TYPE=wayland" >&2
  cat "$EVIDENCE_DIR/session-type.txt" >&2
  exit 1
fi

extension_path="$(sed -n 's/^[[:space:]]*Path: //p' "$EVIDENCE_DIR/gnome-extensions-info.txt" | tail -n 1)"
if [[ -z "$extension_path" ]]; then
  echo "Could not parse extension path from gnome-extensions info" >&2
  exit 1
fi

if [[ "$REQUIRE_PACKAGE_PATH" -eq 1 ]]; then
  expected_path="/usr/share/gnome-shell/extensions/$EXTENSION_UUID"
  if [[ "$extension_path" != "$expected_path" ]]; then
    echo "Expected package extension path: $expected_path" >&2
    echo "Actual extension path: $extension_path" >&2
    exit 1
  fi
fi

metadata_path="$extension_path/metadata.json"
if [[ "$extension_path" == "~/"* ]]; then
  metadata_path="$HOME/${extension_path#~/}/metadata.json"
fi
if [[ ! -f "$metadata_path" ]]; then
  echo "Installed extension metadata not found: $metadata_path" >&2
  exit 1
fi

run_captured "installed-extension-metadata" python3 -m json.tool "$metadata_path"
python3 - "$metadata_path" <<'PY' >"$EVIDENCE_DIR/metadata-validation.txt"
import json
import sys
from pathlib import Path

metadata = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
assert metadata["uuid"] == "codexbar-linux@codexbar.dev"
assert metadata["settings-schema"] == "org.gnome.shell.extensions.codexbar-linux"
assert "46" in metadata["shell-version"]
assert "50" in metadata["shell-version"]
assert metadata["version"] == 1
print("metadata includes GNOME Shell 46 support floor, GNOME Shell 50 validation target, and extension version 1")
PY

if [[ "$REQUIRE_PACKAGE_PATH" -eq 1 ]]; then
  run_captured "installed-dpkg-query" \
    dpkg-query -W -f='${binary:Package}\t${Version}\t${Architecture}\n' "$PACKAGE_NAME"
fi

run_captured "daemon-info" busctl --user call "$DBUS_NAME" "$DBUS_PATH" "$DBUS_INTERFACE" GetDaemonInfo
run_captured "snapshot" busctl --user call "$DBUS_NAME" "$DBUS_PATH" "$DBUS_INTERFACE" GetSnapshot
run_captured "manual-refresh" \
  busctl --user call "$DBUS_NAME" "$DBUS_PATH" "$DBUS_INTERFACE" Refresh s \
  '{"schemaVersion":1,"reason":"manual","force":true,"providers":["codex"],"busyBehavior":"return_existing"}'
run_captured "global-diagnostics" \
  busctl --user call "$DBUS_NAME" "$DBUS_PATH" "$DBUS_INTERFACE" GetDiagnostics s global

python3 - "$EVIDENCE_DIR/global-diagnostics.txt" <<'PY' >"$EVIDENCE_DIR/diagnostics-redaction-scan.txt"
import re
import sys
from pathlib import Path

text = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")
patterns = {
    "email": r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}",
    "authorization": r"(?i)\bauthorization\b",
    "bearer": r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]+",
    "cookie": r"(?i)\bcookie\b",
    "token": r"(?i)\b(token|api[_-]?key|secret)\b",
    "home-path": r"/home/[^\"'\\\s]+",
    "mac-user-path": r"/Users/[^\"'\\\s]+",
    "raw-stream-name": r"(?i)\b(rawStdout|rawStderr|rawPayload|rawResponse|stdoutText|stderrText|stdoutJson|stderrJson)\b",
}
matches = [name for name, pattern in patterns.items() if re.search(pattern, text)]
if matches:
    raise SystemExit("diagnostics redaction scan failed: " + ", ".join(matches))
print("diagnostics redaction scan passed")
PY

run_captured "systemd-user-stop" systemctl --user stop codexbar-linuxd.service
if [[ "$PAUSE_FOR_UI" -eq 1 ]]; then
  echo "Daemon stopped. Inspect the panel/popover daemon-unavailable state, then press Enter to restart."
  read -r _
fi
run_captured "systemd-user-restart" systemctl --user restart codexbar-linuxd.service
run_captured "daemon-info-after-restart" busctl --user call "$DBUS_NAME" "$DBUS_PATH" "$DBUS_INTERFACE" GetDaemonInfo

python3 - "$EVIDENCE_DIR/evidence.json" "$metadata_path" "$EVIDENCE_DIR/gnome-shell-version.txt" "$EVIDENCE_DIR/os-release.txt" "$EVIDENCE_DIR/session-type.txt" "$extension_path" "${EXPECTED_SHELL:-}" "${EXPECTED_UBUNTU:-}" "$REQUIRE_PACKAGE_PATH" "$REQUIRE_WAYLAND" "$EVIDENCE_DIR/installed-dpkg-query.txt" <<'PY'
import json
import re
import sys
from pathlib import Path

out, metadata_path, shell_version_path, os_release_path, session_type_path, extension_path, expected_shell, expected_ubuntu, require_package_path, require_wayland, installed_query_path = sys.argv[1:]
metadata = json.loads(Path(metadata_path).read_text(encoding="utf-8"))
shell_version = Path(shell_version_path).read_text(encoding="utf-8", errors="replace").strip().splitlines()[-1]
os_release = {}
for line in Path(os_release_path).read_text(encoding="utf-8", errors="replace").splitlines():
    if line.startswith("$") or "=" not in line:
        continue
    key, value = line.split("=", 1)
    os_release[key] = value.strip().strip('"')
session_type = Path(session_type_path).read_text(encoding="utf-8", errors="replace").strip().splitlines()[-1]
match = re.search(r"GNOME Shell\s+([0-9]+(?:\.[0-9]+)*)", shell_version)
shell_major = match.group(1).split(".")[0] if match else None
os_id = os_release.get("ID")
os_version_id = os_release.get("VERSION_ID")
installed_version = None
installed_architecture = None
installed_query = Path(installed_query_path)
if installed_query.is_file():
    for line in installed_query.read_text(encoding="utf-8", errors="replace").splitlines():
        parts = line.split("\t")
        if len(parts) == 3 and parts[0] == "codexbar-linux":
            installed_version = parts[1]
            installed_architecture = parts[2]
            break
    if installed_version is None:
        raise SystemExit(f"Could not parse installed package metadata from {installed_query}")
evidence = {
    "schemaVersion": 1,
    "smokeType": "gnome-matrix",
    "status": "passed",
    "shellVersion": shell_version,
    "shellMajor": shell_major,
    "osId": os_id,
    "osVersionId": os_version_id,
    "sessionType": session_type,
    "extensionPath": extension_path,
    "installedVersion": installed_version,
    "installedArchitecture": installed_architecture,
    "metadataShellVersions": metadata.get("shell-version", []),
    "expectedShell": expected_shell or None,
    "requireUbuntuVersion": expected_ubuntu or None,
    "requirePackagePath": require_package_path == "1",
    "requireWayland": require_wayland == "1",
    "ubuntuVersionVerified": os_id == "ubuntu" and (not expected_ubuntu or os_version_id == expected_ubuntu),
    "metadataIncludesGnome50": "50" in metadata.get("shell-version", []),
    "enabledExtensionVerified": True,
    "packagePathVerified": extension_path.startswith("/usr/share/gnome-shell/extensions/"),
    "manualRefreshVerified": True,
    "diagnosticsRedactionScanPassed": True,
    "daemonRestartVerified": True,
    "finalReleaseEvidence": (
        expected_shell == "50"
        and expected_ubuntu == "26.04"
        and shell_major == "50"
        and os_id == "ubuntu"
        and os_version_id == "26.04"
        and session_type == "wayland"
        and require_package_path == "1"
        and require_wayland == "1"
        and extension_path == "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
        and installed_version == "0.1.0-1"
        and installed_architecture is not None
    ),
}
Path(out).write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY

cat >"$EVIDENCE_DIR/summary.txt" <<EOF
gnome-matrix-smoke: passed
expected-shell: ${EXPECTED_SHELL:-not-required}
required-ubuntu: ${EXPECTED_UBUNTU:-not-required}
require-package-path: $REQUIRE_PACKAGE_PATH
require-wayland: $REQUIRE_WAYLAND
extension-path: $extension_path
evidence-dir: $EVIDENCE_DIR
final-release-evidence: see evidence.json
EOF

echo "GNOME matrix smoke passed. Evidence written to: $EVIDENCE_DIR"
