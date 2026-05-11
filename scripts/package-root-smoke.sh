#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_NAME="codexbar-linux"
EXTENSION_UUID="codexbar-linux@codexbar.dev"
DBUS_NAME="org.codexbar.Linux1"
DBUS_PATH="/org/codexbar/Linux1"
DBUS_INTERFACE="org.codexbar.Linux1"
DEFAULT_VERSION="0.1.0-1"
DEB_PATH=""
EVIDENCE_DIR=""
KEEP_INSTALLED=0
PURGE_AFTER_REMOVE=0
SUDO_NONINTERACTIVE="${CODEXBAR_LINUX_PACKAGE_SMOKE_SUDO_NONINTERACTIVE:-0}"
STAGE_ONLY="${CODEXBAR_LINUX_TEST_STAGE_ONLY:-0}"

usage() {
  cat <<'EOF'
Usage: scripts/package-root-smoke.sh [--deb PATH] [--evidence-dir DIR] [--stage-only] [--keep-installed] [--purge] [--noninteractive-sudo]

Run the privileged v0.1 Debian package smoke against the exact .deb artifact.
The script copies the candidate to /tmp, reinstalls it through apt, verifies the
installed daemon, D-Bus activation, system GNOME extension path, and then removes
the package unless --keep-installed is passed.

Options:
  --deb PATH          Candidate .deb. Defaults to dist/codexbar-linux.deb.
  --evidence-dir DIR  Directory for smoke logs. Defaults under target/release-smoke/.
  --stage-only        Copy and inspect the candidate package, then stop before sudo.
                      This does not satisfy final release package smoke.
  --keep-installed   Leave the package installed. This does not satisfy final release remove/purge smoke.
  --purge            Run sudo apt purge after sudo apt remove.
  --noninteractive-sudo
                      Use sudo -n for validation and apt actions. This fails
                      instead of prompting when cached sudo credentials are absent.
                      Equivalent environment override:
                      CODEXBAR_LINUX_PACKAGE_SMOKE_SUDO_NONINTERACTIVE=1.
  -h, --help         Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --deb)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "Missing argument for --deb" >&2
        exit 2
      fi
      DEB_PATH="$2"
      shift 2
      ;;
    --evidence-dir)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "Missing argument for --evidence-dir" >&2
        exit 2
      fi
      EVIDENCE_DIR="$2"
      shift 2
      ;;
    --stage-only)
      STAGE_ONLY=1
      shift
      ;;
    --keep-installed)
      KEEP_INSTALLED=1
      shift
      ;;
    --purge)
      PURGE_AFTER_REMOVE=1
      shift
      ;;
    --noninteractive-sudo)
      SUDO_NONINTERACTIVE=1
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

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required tool: $1" >&2
    exit 1
  fi
}

if [[ "$STAGE_ONLY" == "1" ]]; then
  for tool in cmp cp date dpkg dpkg-deb grep mkdir python3 sha256sum test; do
    require_tool "$tool"
  done
else
  for tool in apt busctl cmp cp date dpkg dpkg-deb dpkg-query gnome-extensions grep gsettings mkdir python3 sha256sum sudo systemctl tee test; do
    require_tool "$tool"
  done
fi

if [[ "$STAGE_ONLY" == "1" ]]; then
  arch="${CODEXBAR_LINUX_TEST_ARCH:-$(dpkg --print-architecture)}"
else
  arch="$(dpkg --print-architecture)"
fi
if [[ -z "$DEB_PATH" ]]; then
  DEB_PATH="$ROOT/dist/${PACKAGE_NAME}.deb"
fi
if [[ ! -f "$DEB_PATH" ]]; then
  echo "Candidate package not found: $DEB_PATH" >&2
  echo "Build it first with: ./scripts/build-deb.sh" >&2
  exit 1
fi

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
if [[ -z "$EVIDENCE_DIR" ]]; then
  EVIDENCE_DIR="$ROOT/target/release-smoke/package-root-$timestamp"
fi
umask 077
mkdir -p "$EVIDENCE_DIR"

write_incomplete_marker() {
  local status=$?
  if [[ "$status" -ne 0 && -d "${EVIDENCE_DIR:-}" ]]; then
    local evidence_state="absent"
    if [[ -f "$EVIDENCE_DIR/evidence.json" ]]; then
      evidence_state="present"
    fi
    {
      echo "package-root-smoke: incomplete"
      echo "exit-status: $status"
      echo "evidence-json: $evidence_state"
      echo "final-release-evidence: false"
      echo "reason: command failed or was interrupted before a successful smoke completed"
    } >"$EVIDENCE_DIR/incomplete.txt"
  fi
  return "$status"
}
trap write_incomplete_marker EXIT

log_cmd() {
  local log="$1"
  shift
  {
    printf '$'
    printf ' %q' "$@"
    printf '\n'
  } >>"$log"
}

run_logged() {
  local name="$1"
  shift
  local log="$EVIDENCE_DIR/$name.txt"
  : >"$log"
  log_cmd "$log" "$@"
  "$@" 2>&1 | tee -a "$log"
}

run_captured() {
  local name="$1"
  shift
  local log="$EVIDENCE_DIR/$name.txt"
  : >"$log"
  log_cmd "$log" "$@"
  "$@" >>"$log" 2>&1
}

run_expected_failure() {
  local name="$1"
  shift
  local log="$EVIDENCE_DIR/$name.txt"
  local status
  : >"$log"
  log_cmd "$log" "$@"
  set +e
  "$@" >>"$log" 2>&1
  status=$?
  set -e
  echo "exit-status: $status" >>"$log"
  if [[ "$status" -eq 0 ]]; then
    echo "Expected command to fail but it succeeded: $*" >&2
    cat "$log" >&2
    exit 1
  fi
}

sudo_args=(sudo)
if [[ "$SUDO_NONINTERACTIVE" == "1" ]]; then
  sudo_args=(sudo -n)
fi

assert_file() {
  local path="$1"
  if [[ ! -f "$path" ]]; then
    echo "Expected installed file missing: $path" >&2
    exit 1
  fi
}

assert_dir() {
  local path="$1"
  if [[ ! -d "$path" ]]; then
    echo "Expected installed directory missing: $path" >&2
    exit 1
  fi
}

assert_absent() {
  local path="$1"
  if [[ -e "$path" ]]; then
    echo "Package-owned path still exists after remove: $path" >&2
    exit 1
  fi
}

tmp_deb="/tmp/$(basename "$DEB_PATH")"
candidate_real="$(cd "$(dirname "$DEB_PATH")" && printf "%s/%s" "$(pwd -P)" "$(basename "$DEB_PATH")")"
tmp_real="$(cd "$(dirname "$tmp_deb")" && printf "%s/%s" "$(pwd -P)" "$(basename "$tmp_deb")")"
if [[ "$candidate_real" == "$tmp_real" ]]; then
  copy_log="$EVIDENCE_DIR/copy-candidate-to-tmp.txt"
  : >"$copy_log"
  log_cmd "$copy_log" cp "$DEB_PATH" "$tmp_deb"
  echo "source already matches /tmp candidate; copy skipped" >>"$copy_log"
else
  run_captured "copy-candidate-to-tmp" cp -f "$DEB_PATH" "$tmp_deb"
fi
run_captured "candidate-checksums" sha256sum "$DEB_PATH" "$tmp_deb"
run_captured "candidate-byte-compare" cmp "$DEB_PATH" "$tmp_deb"
run_captured "candidate-fields" dpkg-deb --field "$tmp_deb" Package Version Architecture
run_captured "candidate-contents" dpkg-deb --contents "$tmp_deb"
grep -Fx "Package: $PACKAGE_NAME" "$EVIDENCE_DIR/candidate-fields.txt" >/dev/null
grep -Fx "Version: $DEFAULT_VERSION" "$EVIDENCE_DIR/candidate-fields.txt" >/dev/null
grep -Fx "Architecture: $arch" "$EVIDENCE_DIR/candidate-fields.txt" >/dev/null
for package_path in \
  "usr/bin/codexbar-linuxd" \
  "usr/bin/codexbar-linux-setup" \
  "usr/share/dbus-1/services/org.codexbar.Linux1.service" \
  "usr/lib/systemd/user/codexbar-linuxd.service" \
  "usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml" \
  "usr/share/gnome-shell/extensions/$EXTENSION_UUID/metadata.json" \
  "usr/share/man/man1/codexbar-linuxd.1.gz"
do
  grep -F "$package_path" "$EVIDENCE_DIR/candidate-contents.txt" >/dev/null
done

if [[ "$STAGE_ONLY" == "1" ]]; then
  read -r candidate_sha _ < <(sha256sum "$candidate_real")
  read -r tmp_sha _ < <(sha256sum "$tmp_real")
  python3 - "$EVIDENCE_DIR/evidence.json" "$candidate_real" "$tmp_real" "$candidate_sha" "$tmp_sha" "$arch" <<'PY'
import json
import sys
from pathlib import Path

out, candidate, tmp_candidate, candidate_sha, tmp_sha, arch = sys.argv[1:]
evidence = {
    "schemaVersion": 1,
    "smokeType": "package-stage",
    "status": "passed",
    "candidate": candidate,
    "tmpCandidate": tmp_candidate,
    "candidateSha256": candidate_sha,
    "tmpCandidateSha256": tmp_sha,
    "architecture": arch,
    "installedVersion": "0.1.0-1",
    "stageOnly": True,
    "finalReleaseEvidence": False,
}
Path(out).write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
  cat >"$EVIDENCE_DIR/summary.txt" <<EOF
package-root-smoke-stage-only: passed
candidate: $candidate_real
tmp-candidate: $tmp_real
candidate-sha256: $candidate_sha
final-release-evidence: false
evidence-dir: $EVIDENCE_DIR
EOF
  echo "Package candidate staging passed. Evidence written to: $EVIDENCE_DIR"
  exit 0
fi

if [[ "$SUDO_NONINTERACTIVE" == "1" ]]; then
  echo "Checking non-interactive sudo access."
else
  echo "Checking sudo access. sudo may prompt for a password."
fi
run_logged "sudo-validate" "${sudo_args[@]}" -v
if [[ "$SUDO_NONINTERACTIVE" == "1" ]]; then
  echo "Installing candidate through apt from /tmp with non-interactive sudo."
else
  echo "Installing candidate through apt from /tmp. sudo may prompt for a password."
fi
run_logged "apt-install-reinstall" "${sudo_args[@]}" apt install --reinstall -y "$tmp_deb"
run_captured "installed-dpkg-query" dpkg-query -W -f='${binary:Package}\t${Version}\t${Architecture}\n' "$PACKAGE_NAME"
grep -Fx "$PACKAGE_NAME	$DEFAULT_VERSION	$arch" "$EVIDENCE_DIR/installed-dpkg-query.txt" >/dev/null

run_captured "systemd-user-daemon-reload-after-install" systemctl --user daemon-reload

assert_file /usr/bin/codexbar-linuxd
assert_file /usr/bin/codexbar-linux-setup
assert_file /usr/share/dbus-1/services/org.codexbar.Linux1.service
assert_file /usr/lib/systemd/user/codexbar-linuxd.service
assert_dir /usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev
assert_file /usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml
assert_file /usr/share/man/man1/codexbar-linuxd.1.gz

run_captured "installed-daemon-version" /usr/bin/codexbar-linuxd --version
grep -Fx "codexbar-linuxd 0.1.0" "$EVIDENCE_DIR/installed-daemon-version.txt" >/dev/null
run_captured "installed-daemon-check" /usr/bin/codexbar-linuxd --check
run_captured "installed-setup-helper" /usr/bin/codexbar-linux-setup --dry-run --no-daemon-reload --codexbar-cli /tmp/codexbar
run_captured "installed-dbus-service" grep -Fx "Exec=/usr/bin/codexbar-linuxd" /usr/share/dbus-1/services/org.codexbar.Linux1.service
run_captured "installed-systemd-user-service" grep -Fx "ExecStart=/usr/bin/codexbar-linuxd" /usr/lib/systemd/user/codexbar-linuxd.service
run_captured "daemon-info" busctl --user call "$DBUS_NAME" "$DBUS_PATH" "$DBUS_INTERFACE" GetDaemonInfo

run_captured "gnome-extensions-list" gnome-extensions list
grep -Fx "$EXTENSION_UUID" "$EVIDENCE_DIR/gnome-extensions-list.txt" >/dev/null
run_captured "gnome-extensions-enable" gnome-extensions enable "$EXTENSION_UUID"
run_captured "enabled-extensions-after-enable" gsettings get org.gnome.shell enabled-extensions
grep -F "$EXTENSION_UUID" "$EVIDENCE_DIR/enabled-extensions-after-enable.txt" >/dev/null
run_captured "gnome-extensions-info" gnome-extensions info "$EXTENSION_UUID"
grep -Ex "[[:space:]]*Path: /usr/share/gnome-shell/extensions/$EXTENSION_UUID" "$EVIDENCE_DIR/gnome-extensions-info.txt" >/dev/null

run_captured "manual-refresh" \
  busctl --user call "$DBUS_NAME" "$DBUS_PATH" "$DBUS_INTERFACE" Refresh s \
  '{"schemaVersion":1,"reason":"manual","force":true,"busyBehavior":"return_existing"}'
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
run_captured "systemd-user-restart" systemctl --user restart codexbar-linuxd.service
run_captured "daemon-info-after-restart" busctl --user call "$DBUS_NAME" "$DBUS_PATH" "$DBUS_INTERFACE" GetDaemonInfo

if [[ "$KEEP_INSTALLED" -eq 1 ]]; then
  echo "Package left installed because --keep-installed was passed."
  echo "This does not satisfy the final install/remove/purge release gate." | tee "$EVIDENCE_DIR/keep-installed-warning.txt"
else
  run_captured "gnome-extensions-disable" gnome-extensions disable "$EXTENSION_UUID"
  run_captured "enabled-extensions-after-disable" gsettings get org.gnome.shell enabled-extensions
  if grep -F "$EXTENSION_UUID" "$EVIDENCE_DIR/enabled-extensions-after-disable.txt" >/dev/null; then
    echo "Extension still enabled after disable: $EXTENSION_UUID" >&2
    cat "$EVIDENCE_DIR/enabled-extensions-after-disable.txt" >&2
    exit 1
  fi
  if [[ "$SUDO_NONINTERACTIVE" == "1" ]]; then
    echo "Removing package through apt with non-interactive sudo."
  else
    echo "Removing package through apt. sudo may prompt for a password."
  fi
  run_logged "apt-remove" "${sudo_args[@]}" apt remove -y "$PACKAGE_NAME"
  run_captured "systemd-user-daemon-reload-after-remove" systemctl --user daemon-reload
  run_captured "removed-daemon-absent" test ! -e /usr/bin/codexbar-linuxd
  run_captured "removed-setup-helper-absent" test ! -e /usr/bin/codexbar-linux-setup
  run_captured "removed-dbus-service-absent" test ! -e /usr/share/dbus-1/services/org.codexbar.Linux1.service
  run_captured "removed-systemd-user-service-absent" test ! -e /usr/lib/systemd/user/codexbar-linuxd.service
  run_captured "removed-extension-dir-absent" test ! -e /usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev
  run_captured "removed-gsettings-schema-absent" test ! -e /usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml
  run_captured "removed-manpage-absent" test ! -e /usr/share/man/man1/codexbar-linuxd.1.gz
  assert_absent /usr/bin/codexbar-linuxd
  assert_absent /usr/bin/codexbar-linux-setup
  assert_absent /usr/share/dbus-1/services/org.codexbar.Linux1.service
  assert_absent /usr/lib/systemd/user/codexbar-linuxd.service
  assert_absent /usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev
  assert_absent /usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml
  assert_absent /usr/share/man/man1/codexbar-linuxd.1.gz

  if [[ "$PURGE_AFTER_REMOVE" -eq 1 ]]; then
    if [[ "$SUDO_NONINTERACTIVE" == "1" ]]; then
      echo "Purging package through apt with non-interactive sudo."
    else
      echo "Purging package through apt. sudo may prompt for a password."
    fi
    run_logged "apt-purge" "${sudo_args[@]}" apt purge -y "$PACKAGE_NAME"
    run_captured "systemd-user-daemon-reload-after-purge" systemctl --user daemon-reload
    run_expected_failure "purged-dpkg-query" dpkg-query -W "$PACKAGE_NAME"
  fi
fi

read -r candidate_sha _ < <(sha256sum "$candidate_real")
read -r tmp_sha _ < <(sha256sum "$tmp_real")
final_release_evidence=0
if [[ "$KEEP_INSTALLED" -eq 0 && "$PURGE_AFTER_REMOVE" -eq 1 ]]; then
  final_release_evidence=1
fi
python3 - "$EVIDENCE_DIR/evidence.json" "$candidate_real" "$tmp_real" "$candidate_sha" "$tmp_sha" "$arch" "$KEEP_INSTALLED" "$PURGE_AFTER_REMOVE" "$SUDO_NONINTERACTIVE" <<'PY'
import json
import sys
from pathlib import Path

out, candidate, tmp_candidate, candidate_sha, tmp_sha, arch, keep_installed, purge_after_remove, sudo_noninteractive = sys.argv[1:]
evidence = {
    "schemaVersion": 1,
    "smokeType": "package-root",
    "status": "passed",
    "candidate": candidate,
    "tmpCandidate": tmp_candidate,
    "candidateSha256": candidate_sha,
    "tmpCandidateSha256": tmp_sha,
    "architecture": arch,
    "installedVersion": "0.1.0-1",
    "daemonVersion": "codexbar-linuxd 0.1.0",
    "usedAptReinstallFromTmp": True,
    "sudoValidated": True,
    "sudoNonInteractive": sudo_noninteractive == "1",
    "systemExtensionPathVerified": True,
    "manualRefreshVerified": True,
    "diagnosticsRedactionScanPassed": True,
    "daemonRestartVerified": True,
    "removeVerified": keep_installed == "0",
    "purgeAfterRemove": purge_after_remove == "1",
    "keepInstalled": keep_installed == "1",
    "finalReleaseEvidence": keep_installed == "0" and purge_after_remove == "1",
}
Path(out).write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY

cat >"$EVIDENCE_DIR/summary.txt" <<EOF
package-root-smoke: passed
candidate: $candidate_real
tmp-candidate: $tmp_real
candidate-sha256: $candidate_sha
architecture: $arch
evidence-dir: $EVIDENCE_DIR
keep-installed: $KEEP_INSTALLED
purge-after-remove: $PURGE_AFTER_REMOVE
final-release-evidence: $final_release_evidence
sudo-noninteractive: $SUDO_NONINTERACTIVE
EOF

echo "Root-backed package smoke passed. Evidence written to: $EVIDENCE_DIR"
