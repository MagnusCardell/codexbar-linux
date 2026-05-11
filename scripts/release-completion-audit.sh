#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_ROOT_EVIDENCE=""
GNOME_MATRIX_EVIDENCE=""
LOCAL_GATE_LOG=""
CHECK_HOST=0

usage() {
  cat <<'EOF'
Usage: scripts/release-completion-audit.sh --package-root PATH --gnome-matrix PATH --local-gate-log PATH [--check-host]
       scripts/release-completion-audit.sh [--check-host]

Audit whether the 05F-05K v0.1 release objective is complete.

The audit intentionally does not discover the latest evidence automatically.
Pass explicit package-root and GNOME-matrix evidence paths so stale manifests
cannot accidentally satisfy the final release gate. Package-root evidence must
also match the current release candidate in dist/ and its /tmp copy, and the
git working tree must be clean before the audit reports complete. The audit
also requires a saved ./scripts/check.sh log with a success marker for the
current HEAD.

Options:
  --package-root PATH   Final package-root evidence.json.
  --gnome-matrix PATH   Final GNOME matrix evidence.json.
  --local-gate-log PATH Saved ./scripts/check.sh output for the current HEAD.
  --check-host          Also print local host sudo/GNOME capability hints.
  -h, --help            Show this help.
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
    --local-gate-log)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "Missing argument for --local-gate-log" >&2
        exit 2
      fi
      LOCAL_GATE_LOG="$2"
      shift 2
      ;;
    --check-host)
      CHECK_HOST=1
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

TMP="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP"
}
trap cleanup EXIT

print_checklist() {
  cat <<'EOF'
Prompt-to-artifact checklist:
  05F   daemon scheduler startup/interval/reschedule/unwedge: daemon tests and D-Bus contract tests
  05F.1 start-daemon-on-login hidden/reserved: prefs, schema, contracts docs, and GJS lint assertions
  05G   latest .deb root-backed package smoke: package-root evidence required
  05H   release-candidate gate/tag prep: release gate docs, validators, and local check log evidence
  05I   Ubuntu 26.04/GNOME 50 metadata/runtime validation: GNOME matrix evidence required
  05J   upstream CLI/provider cleanup: upstream CLI setup docs and adapter fixtures
  05K   useful prefs/provider UX: prefs daemon info, interval, selector, enable/source writes, and GJS lint assertions
EOF
}

print_host_hints() {
  echo
  echo "Local host hints:"
  if [[ -r /etc/os-release ]]; then
    python3 - <<'PY'
from pathlib import Path

values = {}
for line in Path("/etc/os-release").read_text(encoding="utf-8", errors="replace").splitlines():
    if "=" not in line:
        continue
    key, value = line.split("=", 1)
    values[key] = value.strip().strip('"')
print(f"os-release: ID={values.get('ID', 'unknown')} VERSION_ID={values.get('VERSION_ID', 'unknown')}")
PY
  else
    echo "os-release: unavailable"
  fi
  echo "XDG_SESSION_TYPE: ${XDG_SESSION_TYPE:-unknown}"
  if command -v gnome-shell >/dev/null 2>&1; then
    gnome-shell --version || true
  else
    echo "gnome-shell: not found"
  fi

  if command -v sudo >/dev/null 2>&1; then
    if sudo -n true >/dev/null 2>&1; then
      echo "sudo -n true: available"
    else
      echo "sudo -n true: unavailable"
    fi
  else
    echo "sudo: not found"
  fi
}

print_required_final_commands() {
  cat <<'EOF'
Required final evidence commands:
  ./scripts/build-deb.sh
  cp -f dist/codexbar-linux.deb /tmp/codexbar-linux.deb
  candidate="/tmp/codexbar-linux.deb"
  ./scripts/package-root-smoke.sh --deb "$candidate" --purge
  # On automation hosts with cached sudo credentials:
  # CODEXBAR_LINUX_PACKAGE_SMOKE_SUDO_NONINTERACTIVE=1 ./scripts/package-root-smoke.sh --deb "$candidate" --purge
  ./scripts/gnome-matrix-smoke.sh --require-shell 50 --require-ubuntu 26.04 --require-package-path --require-wayland --pause-for-ui
  ./scripts/check.sh 2>&1 | tee target/release-smoke/check-YYYYMMDDTHHMMSSZ.log
  ./scripts/release-completion-audit.sh --package-root PATH/package-root/evidence.json --gnome-matrix PATH/gnome-matrix/evidence.json --local-gate-log PATH/check.log
EOF
}

release_version() {
  sed -n '1s/^codexbar-linux (\([^)]*\)).*/\1/p' "$ROOT/packaging/debian/changelog"
}

current_candidate_path() {
  if [[ -n "${CODEXBAR_LINUX_RELEASE_CANDIDATE:-}" ]]; then
    printf '%s\n' "$CODEXBAR_LINUX_RELEASE_CANDIDATE"
    return
  fi
  printf '%s/dist/codexbar-linux.deb\n' "$ROOT"
}

current_tmp_candidate_path() {
  if [[ -n "${CODEXBAR_LINUX_RELEASE_TMP_CANDIDATE:-}" ]]; then
    printf '%s\n' "$CODEXBAR_LINUX_RELEASE_TMP_CANDIDATE"
    return
  fi
  local candidate
  candidate="$(current_candidate_path)"
  printf '/tmp/%s\n' "$(basename "$candidate")"
}

validate_current_candidate_matches_evidence() {
  local candidate tmp_candidate
  candidate="$(current_candidate_path)"
  tmp_candidate="$(current_tmp_candidate_path)"
  python3 - "$PACKAGE_ROOT_EVIDENCE" "$candidate" "$tmp_candidate" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

evidence_path = Path(sys.argv[1])
candidate = Path(sys.argv[2])
tmp_candidate = Path(sys.argv[3])

def file_sha256(path, label):
    if not path.is_file():
        raise SystemExit(f"{label} not found: {path}")
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()

data = json.loads(evidence_path.read_text(encoding="utf-8"))
expected_sha = data.get("candidateSha256")
if not isinstance(expected_sha, str) or len(expected_sha) != 64:
    raise SystemExit(f"{evidence_path} must have candidateSha256 before current-candidate audit")
recorded_candidate = data.get("candidate")
recorded_tmp_candidate = data.get("tmpCandidate")
if Path(recorded_candidate or "") != candidate:
    raise SystemExit(
        f"package evidence candidate path does not match current dist candidate {candidate}"
    )
if Path(recorded_tmp_candidate or "") != tmp_candidate:
    raise SystemExit(
        f"package evidence tmpCandidate path does not match current /tmp candidate {tmp_candidate}"
    )
dist_sha = file_sha256(candidate, "current dist candidate")
tmp_sha = file_sha256(tmp_candidate, "current /tmp candidate")
if dist_sha != expected_sha:
    raise SystemExit(
        f"package evidence candidateSha256 does not match current dist candidate {candidate}"
    )
if tmp_sha != expected_sha:
    raise SystemExit(
        f"package evidence candidateSha256 does not match current /tmp candidate {tmp_candidate}"
    )
print("Current release candidate matches package-root evidence:")
print(f"  dist: {candidate}")
print(f"  tmp: {tmp_candidate}")
print(f"  sha256: {expected_sha}")
PY
}

require_clean_worktree() {
  if [[ "${CODEXBAR_LINUX_TEST_ALLOW_DIRTY:-0}" == "1" ]]; then
    echo "Git working tree cleanliness skipped for test fixture"
    return 0
  fi
  local status
  if [[ "${CODEXBAR_LINUX_TEST_FORCE_DIRTY:-0}" == "1" ]]; then
    status=" M synthetic-dirty-test"
  else
    status="$(git -C "$ROOT" status --short --untracked-files=all)"
  fi
  if [[ -n "$status" ]]; then
    echo "git working tree is not clean; commit or remove release changes before completion audit can pass" >&2
    printf '%s\n' "$status" >&2
    return 1
  fi
  echo "Git working tree is clean"
}

validate_local_gate_log() {
  if [[ -z "$LOCAL_GATE_LOG" ]]; then
    echo "missing --local-gate-log final ./scripts/check.sh evidence" >&2
    return 1
  fi
  python3 - "$LOCAL_GATE_LOG" "$ROOT" <<'PY'
import subprocess
import sys
from pathlib import Path

log_path = Path(sys.argv[1])
root = Path(sys.argv[2])
if not log_path.is_file():
    raise SystemExit(f"local gate log not found: {log_path}")

head = subprocess.check_output(
    ["git", "-C", str(root), "rev-parse", "HEAD"],
    text=True,
).strip()
text = log_path.read_text(encoding="utf-8", errors="replace")
required = [
    "Release-candidate gate documentation is explicit about remaining blockers",
    "Release evidence validator tests passed",
    "No browser-cookie/web-fetch surface present",
    "Upstream CLI capture harness tests passed",
    "dbus_scheduler_runs_startup_refresh_when_enabled",
    "dbus_scheduler_runs_interval_refresh_when_enabled",
    "dbus_scheduler_interval_zero_disables_interval_loop_but_allows_startup",
    "dbus_scheduler_backs_off_repeated_upstream_cli_failures",
    "dbus_refresh_all_configured_providers_disabled_returns_noop",
    "settings_patch_advances_scheduler_revision",
    "failed_refresh_can_be_unwedged_without_daemon_restart",
    "app_refresh_uses_configured_provider_targets",
    "app_refresh_all_configured_providers_disabled_noops_without_defaulting_to_codex",
    "app_refresh_explicit_providers_override_settings",
    "upstream_cli_required_live_matrix_is_present",
    "test result: ok.",
    "GJS Shell-process boundary smoke check passed",
    "extension state tests passed",
    f"repository gate passed for HEAD {head}",
]
missing = [marker for marker in required if marker not in text]
if missing:
    raise SystemExit(
        "local gate log does not include required current-HEAD gate marker(s): "
        + ", ".join(missing)
    )
print("Local repository gate evidence matches current HEAD:")
print(f"  log: {log_path}")
print(f"  head: {head}")
PY
}

validate_gnome_final_manifest_markers() {
  if [[ -z "$GNOME_MATRIX_EVIDENCE" ]]; then
    return 0
  fi
  python3 - "$GNOME_MATRIX_EVIDENCE" <<'PY'
import json
import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
errors = []
try:
    data = json.loads(path.read_text(encoding="utf-8"))
except FileNotFoundError:
    raise SystemExit(f"GNOME matrix evidence not found: {path}")
except json.JSONDecodeError as exc:
    raise SystemExit(f"{path} is not valid JSON: {exc}")

def require_equal(key, expected):
    actual = data.get(key)
    if actual != expected:
        errors.append(f"{key}={expected!r} required, found {actual!r}")

def require_true(key):
    actual = data.get(key)
    if actual is not True:
        errors.append(f"{key}=true required, found {actual!r}")

require_equal("schemaVersion", 1)
require_equal("smokeType", "gnome-matrix")
require_equal("status", "passed")
require_equal("expectedShell", "50")
require_equal("shellMajor", "50")
require_equal("osId", "ubuntu")
require_equal("osVersionId", "26.04")
require_equal("requireUbuntuVersion", "26.04")
require_equal("sessionType", "wayland")
require_equal("extensionPath", "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev")
require_true("finalReleaseEvidence")
require_true("metadataIncludesGnome50")
require_true("enabledExtensionVerified")
require_true("manualRefreshVerified")
require_true("diagnosticsRedactionScanPassed")
require_true("daemonRestartVerified")
require_true("ubuntuVersionVerified")
require_true("requirePackagePath")
require_true("requireWayland")
require_true("packagePathVerified")

shell_version = data.get("shellVersion")
if not isinstance(shell_version, str) or not re.search(r"\bGNOME Shell\s+50(?:[.\s]|$)", shell_version):
    errors.append(f"shellVersion must report GNOME Shell 50, found {shell_version!r}")

versions = data.get("metadataShellVersions")
if not isinstance(versions, list) or "50" not in versions:
    errors.append(f"metadataShellVersions must include '50', found {versions!r}")

if errors:
    print(
        f"{path} is not final Ubuntu 26.04/GNOME 50 package-path evidence:",
        file=sys.stderr,
    )
    for error in errors:
        print(f"  {error}", file=sys.stderr)
    raise SystemExit(1)

print("GNOME matrix evidence has final Ubuntu 26.04/GNOME 50 package-path markers:")
print(f"  evidence: {path}")
print(f"  shellVersion: {shell_version}")
print(f"  osVersionId: {data.get('osVersionId')}")
print(f"  extensionPath: {data.get('extensionPath')}")
PY
}

echo "05F-05K release objective audit"
print_checklist

echo
echo "Static gate:"
if "$ROOT/scripts/validate-release-gate.sh" >"$TMP/static-gate.out" 2>"$TMP/static-gate.err"; then
  cat "$TMP/static-gate.out"
else
  cat "$TMP/static-gate.out"
  cat "$TMP/static-gate.err" >&2
  echo "05F-05K release objective audit: not complete" >&2
  exit 1
fi

if [[ "$CHECK_HOST" -eq 1 ]]; then
  print_host_hints
fi

echo
echo "Final release evidence:"
AUDIT_FAILED=0
NEEDS_REQUIRED_COMMANDS=0
FINAL_EVIDENCE_VALID=0
if [[ -z "$PACKAGE_ROOT_EVIDENCE" || -z "$GNOME_MATRIX_EVIDENCE" ]]; then
  if [[ -z "$PACKAGE_ROOT_EVIDENCE" ]]; then
    echo "  missing --package-root final package-root evidence"
  else
    echo "  package-root evidence: $PACKAGE_ROOT_EVIDENCE"
  fi
  if [[ -z "$GNOME_MATRIX_EVIDENCE" ]]; then
    echo "  missing --gnome-matrix final GNOME matrix evidence"
  else
    echo "  GNOME matrix evidence: $GNOME_MATRIX_EVIDENCE"
  fi
  AUDIT_FAILED=1
  NEEDS_REQUIRED_COMMANDS=1
else
  if "$ROOT/scripts/validate-release-evidence.sh" \
    --package-root "$PACKAGE_ROOT_EVIDENCE" \
    --gnome-matrix "$GNOME_MATRIX_EVIDENCE" \
    >"$TMP/final-evidence.out" 2>"$TMP/final-evidence.err"; then
    cat "$TMP/final-evidence.out"
    FINAL_EVIDENCE_VALID=1
  else
    cat "$TMP/final-evidence.out"
    cat "$TMP/final-evidence.err" >&2
    AUDIT_FAILED=1
    NEEDS_REQUIRED_COMMANDS=1
  fi
fi

if [[ "$FINAL_EVIDENCE_VALID" -eq 0 && -n "$GNOME_MATRIX_EVIDENCE" ]]; then
  if validate_gnome_final_manifest_markers >"$TMP/gnome-final-markers.out" 2>"$TMP/gnome-final-markers.err"; then
    cat "$TMP/gnome-final-markers.out"
  else
    cat "$TMP/gnome-final-markers.out"
    cat "$TMP/gnome-final-markers.err" >&2
    AUDIT_FAILED=1
    NEEDS_REQUIRED_COMMANDS=1
  fi
fi

if [[ "$FINAL_EVIDENCE_VALID" -eq 1 ]]; then
  if validate_current_candidate_matches_evidence >"$TMP/current-candidate.out" 2>"$TMP/current-candidate.err"; then
    cat "$TMP/current-candidate.out"
  else
    cat "$TMP/current-candidate.out"
    cat "$TMP/current-candidate.err" >&2
    AUDIT_FAILED=1
  fi
elif [[ -n "$PACKAGE_ROOT_EVIDENCE" ]]; then
  echo "Current release candidate cross-check skipped because package-root evidence is not final-valid"
fi

echo
echo "Repository gate evidence:"
if require_clean_worktree >"$TMP/git-clean.out" 2>"$TMP/git-clean.err"; then
  cat "$TMP/git-clean.out"
else
  cat "$TMP/git-clean.out"
  cat "$TMP/git-clean.err" >&2
  AUDIT_FAILED=1
fi

if validate_local_gate_log >"$TMP/local-gate.out" 2>"$TMP/local-gate.err"; then
  cat "$TMP/local-gate.out"
else
  cat "$TMP/local-gate.out"
  cat "$TMP/local-gate.err" >&2
  AUDIT_FAILED=1
  NEEDS_REQUIRED_COMMANDS=1
fi

if [[ "$AUDIT_FAILED" -eq 1 ]]; then
  if [[ "$NEEDS_REQUIRED_COMMANDS" -eq 1 ]]; then
    echo
    print_required_final_commands
  fi
  echo "05F-05K release objective audit: not complete" >&2
  exit 1
fi

echo "05F-05K release objective audit: complete"
