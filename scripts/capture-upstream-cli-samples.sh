#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_ROOT="${CODEXBAR_CAPTURE_OUTPUT_DIR:-$ROOT/daemon/fixtures/upstream-cli}"
REDACTOR="$ROOT/scripts/redact-upstream-cli-sample.py"
CAPTURE_ID="$(date -u +%Y%m%dT%H%M%SZ)"
MANIFEST_OUT="${CODEXBAR_CAPTURE_MANIFEST:-$FIXTURE_ROOT/manifest.live-$CAPTURE_ID.json}"

if [[ "${CODEXBAR_CAPTURE_LIVE:-}" != "1" ]]; then
  cat >&2 <<'EOF'
Live upstream CodexBar CLI capture is opt-in because command output may include
account metadata before redaction.

Run:
  CODEXBAR_CAPTURE_LIVE=1 ./scripts/capture-upstream-cli-samples.sh

The production Task 02B adapter must not proceed without reviewed redacted
Linux samples. CI must use daemon/fixtures/upstream-cli/manifest.json and must
not depend on a live codexbar binary.
EOF
  exit 2
fi

if [[ ! -x "$REDACTOR" ]]; then
  echo "Missing executable redactor: $REDACTOR" >&2
  exit 1
fi

locate_codexbar() {
  if [[ -n "${CODEXBAR_CLI:-}" ]]; then
    if [[ -x "$CODEXBAR_CLI" ]]; then
      printf '%s\n' "$CODEXBAR_CLI"
      return 0
    fi
    echo "CODEXBAR_CLI is set but not executable: $CODEXBAR_CLI" >&2
    return 1
  fi

  if command -v codexbar >/dev/null 2>&1; then
    command -v codexbar
    return 0
  fi

  for candidate in \
    "$HOME/.linuxbrew/bin/codexbar" \
    "/home/linuxbrew/.linuxbrew/bin/codexbar"
  do
    if [[ -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  if [[ -n "${CODEXBAR_CLI_DEV_PATH:-}" && -x "$CODEXBAR_CLI_DEV_PATH" ]]; then
    printf '%s\n' "$CODEXBAR_CLI_DEV_PATH"
    return 0
  fi

  return 1
}

if ! CODEXBAR_BIN="$(locate_codexbar)"; then
  cat >&2 <<'EOF'
Unable to locate an upstream codexbar CLI binary.

Install options:
  brew install steipete/tap/codexbar
  or download CodexBarCLI-v<tag>-linux-<arch>.tar.gz from:
  https://github.com/steipete/CodexBar/releases

Then run with either:
  CODEXBAR_CAPTURE_LIVE=1 ./scripts/capture-upstream-cli-samples.sh
  CODEXBAR_CAPTURE_LIVE=1 CODEXBAR_CLI=/path/to/codexbar ./scripts/capture-upstream-cli-samples.sh

Task 02B must not proceed without reviewed redacted Linux samples.
EOF
  exit 2
fi

umask 077
RAW_DIR="$(mktemp -d "${TMPDIR:-/tmp}/codexbar-cli-raw.XXXXXX")"
chmod 0700 "$RAW_DIR"
ENTRY_FILE="$RAW_DIR/entries.jsonl"
trap 'rm -rf "$RAW_DIR"' EXIT

mkdir -p "$FIXTURE_ROOT/usage" "$FIXTURE_ROOT/cost" "$FIXTURE_ROOT/errors" "$FIXTURE_ROOT/status"

json_extension_for() {
  local file="$1"
  if python3 - "$file" <<'PY' >/dev/null 2>&1
import json
import sys
with open(sys.argv[1], "r", encoding="utf-8") as handle:
    json.load(handle)
PY
  then
    printf 'json'
  else
    printf 'txt'
  fi
}

write_metadata_and_entry() {
  local metadata_path="$1"
  local entry_file="$2"
  local fixture_id="$3"
  local command_name="$4"
  local expected_category="$5"
  local timeout_seconds="$6"
  local exit_code="$7"
  local timed_out="$8"
  local duration_ms="$9"
  local stdout_bytes="${10}"
  local stderr_bytes="${11}"
  local stdout_rel="${12}"
  local stderr_rel="${13}"
  local metadata_rel="${14}"
  shift 14
  python3 - "$metadata_path" "$entry_file" "$fixture_id" "$command_name" "$expected_category" "$timeout_seconds" "$exit_code" "$timed_out" "$duration_ms" "$stdout_bytes" "$stderr_bytes" "$stdout_rel" "$stderr_rel" "$metadata_rel" "$UPSTREAM_VERSION" "$CAPTURED_AT" "$PLATFORM_OS" "$PLATFORM_KERNEL" "$PLATFORM_ARCH" "$@" <<'PY'
import json
import sys
from pathlib import Path

metadata_path = Path(sys.argv[1])
entry_file = Path(sys.argv[2])
argv = sys.argv[20:]
redaction = {
    "applied": True,
    "policyVersion": 1,
    "notes": ["live output redacted before fixture write"],
}
platform = {
    "os": sys.argv[17],
    "kernel": sys.argv[18],
    "arch": sys.argv[19],
}
metadata = {
    "fixtureId": sys.argv[3],
    "synthetic": False,
    "docDerived": False,
    "command": sys.argv[4],
    "expectedCategory": sys.argv[5],
    "timeoutSeconds": int(sys.argv[6]),
    "exitCode": int(sys.argv[7]),
    "timedOut": sys.argv[8] == "true",
    "durationMs": int(sys.argv[9]),
    "stdoutBytes": int(sys.argv[10]),
    "stderrBytes": int(sys.argv[11]),
    "stdoutPath": sys.argv[12],
    "stderrPath": sys.argv[13],
    "metadataPath": sys.argv[14],
    "upstreamVersion": sys.argv[15],
    "capturedAt": sys.argv[16],
    "platform": platform,
    "codexbarCliPath": argv[0] if argv else None,
    "argv": argv,
    "redaction": redaction,
}
entry = {
    "fixtureId": sys.argv[3],
    "command": sys.argv[4],
    "argv": argv,
    "upstreamVersion": sys.argv[15],
    "platform": platform,
    "capturedAt": sys.argv[16],
    "exitCode": int(sys.argv[7]),
    "timedOut": sys.argv[8] == "true",
    "stdoutPath": sys.argv[12],
    "stderrPath": sys.argv[13],
    "metadataPath": sys.argv[14],
    "expectedCategory": sys.argv[5],
    "synthetic": False,
    "docDerived": False,
    "redaction": redaction,
}
metadata_path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
with entry_file.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(entry, separators=(",", ":")) + "\n")
PY
}

run_capture() {
  local fixture_id="$1"
  local category_dir="$2"
  local command_name="$3"
  local expected_category="$4"
  local timeout_seconds="$5"
  shift 5
  local raw_stdout="$RAW_DIR/$fixture_id.stdout.raw"
  local raw_stderr="$RAW_DIR/$fixture_id.stderr.raw"
  local redacted_stdout_tmp="$RAW_DIR/$fixture_id.stdout.redacted"
  local redacted_stderr_tmp="$RAW_DIR/$fixture_id.stderr.redacted"
  local started_ns ended_ns duration_ms exit_code timed_out stdout_bytes stderr_bytes stdout_ext stderr_ext

  started_ns="$(date +%s%N)"
  set +e
  timeout "${timeout_seconds}s" "$CODEXBAR_BIN" "$@" >"$raw_stdout" 2>"$raw_stderr"
  exit_code=$?
  set -e
  ended_ns="$(date +%s%N)"
  duration_ms="$(((ended_ns - started_ns) / 1000000))"
  timed_out=false
  if [[ "$exit_code" -eq 124 || "$exit_code" -eq 137 ]]; then
    timed_out=true
  fi
  stdout_bytes="$(wc -c <"$raw_stdout" | awk '{print $1}')"
  stderr_bytes="$(wc -c <"$raw_stderr" | awk '{print $1}')"

  "$REDACTOR" --input "$raw_stdout" --output "$redacted_stdout_tmp"
  "$REDACTOR" --input "$raw_stderr" --output "$redacted_stderr_tmp"

  stdout_ext="$(json_extension_for "$redacted_stdout_tmp")"
  stderr_ext="$(json_extension_for "$redacted_stderr_tmp")"
  local stdout_rel="$category_dir/live_${CAPTURE_ID}_${fixture_id}_stdout.$stdout_ext"
  local stderr_rel="$category_dir/live_${CAPTURE_ID}_${fixture_id}_stderr.$stderr_ext"
  local metadata_rel="$category_dir/live_${CAPTURE_ID}_${fixture_id}_metadata.json"

  mv "$redacted_stdout_tmp" "$FIXTURE_ROOT/$stdout_rel"
  mv "$redacted_stderr_tmp" "$FIXTURE_ROOT/$stderr_rel"
  write_metadata_and_entry "$FIXTURE_ROOT/$metadata_rel" "$ENTRY_FILE" "$fixture_id" "$command_name" "$expected_category" "$timeout_seconds" "$exit_code" "$timed_out" "$duration_ms" "$stdout_bytes" "$stderr_bytes" "$stdout_rel" "$stderr_rel" "$metadata_rel" "$CODEXBAR_BIN" "$@"
  "$REDACTOR" --input "$FIXTURE_ROOT/$metadata_rel" --output "$FIXTURE_ROOT/$metadata_rel.redacted"
  mv "$FIXTURE_ROOT/$metadata_rel.redacted" "$FIXTURE_ROOT/$metadata_rel"
}

CAPTURED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
PLATFORM_OS="$(uname -s)"
PLATFORM_KERNEL="$(uname -r)"
PLATFORM_ARCH="$(uname -m)"

set +e
UPSTREAM_VERSION="$("$CODEXBAR_BIN" --version 2>/dev/null | head -n 1)"
VERSION_STATUS=$?
set -e
if [[ "$VERSION_STATUS" -ne 0 || -z "$UPSTREAM_VERSION" ]]; then
  UPSTREAM_VERSION="unknown"
fi

run_capture "version" "status" "version" "usage_success" 5 --version
run_capture "usage_default_all" "usage" "usage" "usage_success" 30 --format json --json-only --provider all
run_capture "usage_subcommand_all" "usage" "usage" "usage_success" 30 usage --format json --json-only --provider all
run_capture "cost_all" "cost" "cost" "cost_success" 20 cost --format json --json-only --provider all
run_capture "status_all" "status" "status" "usage_success" 30 --format json --json-only --provider all --status
run_capture "unsupported_web_source" "errors" "usage" "unsupported_source" 30 --format json --json-only --provider all --source web
run_capture "invalid_provider" "errors" "usage" "invalid_provider" 30 --format json --json-only --provider __codexbar_linux_invalid_provider__

python3 - "$ENTRY_FILE" "$MANIFEST_OUT" "$CAPTURED_AT" "$UPSTREAM_VERSION" "$PLATFORM_OS" "$PLATFORM_KERNEL" "$PLATFORM_ARCH" <<'PY'
import json
import sys
from pathlib import Path

entry_file = Path(sys.argv[1])
manifest_path = Path(sys.argv[2])
entries = [json.loads(line) for line in entry_file.read_text(encoding="utf-8").splitlines() if line.strip()]
manifest = {
    "schemaVersion": 1,
    "fixtureSet": "upstream-cli-live-capture",
    "generatedAt": sys.argv[3],
    "upstreamVersion": sys.argv[4],
    "platform": {
        "os": sys.argv[5],
        "kernel": sys.argv[6],
        "arch": sys.argv[7],
    },
    "redaction": {
        "applied": True,
        "policyVersion": 1,
        "notes": ["local live capture; review before promoting into manifest.json"],
    },
    "testExpectations": {
        "usage_success": "parseable usage/status JSON or redacted nonzero usage payload",
        "cost_success": "parseable cost JSON array/object",
        "unsupported_source": "Linux unsupported source error is captured and redacted",
        "invalid_provider": "invalid provider error is captured and redacted",
    },
    "fixtures": entries,
}
manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
PY

echo "Wrote redacted live capture manifest: $MANIFEST_OUT"
echo "Raw temporary files were deleted from: $RAW_DIR"
