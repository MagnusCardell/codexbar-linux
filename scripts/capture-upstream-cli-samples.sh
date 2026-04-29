#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REDACTOR="$ROOT/scripts/redact-upstream-cli-sample.py"
CAPTURE_ID="$(date -u +%Y%m%dT%H%M%SZ)"
OUTPUT_DIR=""
MANIFEST_OUT="${CODEXBAR_CAPTURE_MANIFEST:-}"
LIVE_CAPTURE="${CODEXBAR_CAPTURE_LIVE:-0}"
METADATA_ONLY=0
ALLOW_PROVIDER_NETWORK=0
INCLUDE_ERROR_PROBES=0
INCLUDE_CONFIG_VALIDATE=0
INCLUDE_CONFIG_DUMP=0
UNDERSTAND_CONFIG_DUMP=0
PROVIDER_SOURCE="cli"

usage() {
  cat <<'USAGE'
Usage: scripts/capture-upstream-cli-samples.sh [options]

Live capture is local-only and requires CODEXBAR_CAPTURE_LIVE=1 or --live.

Options:
  --live
      Confirm that live upstream CLI capture is intentional.
  --output DIR
      Write redacted capture artifacts under DIR. Defaults to
      /tmp/codexbar-upstream-cli-live-<timestamp>.
  --manifest PATH
      Write the redacted live manifest to PATH. PATH must be directly under
      --output and its basename must match manifest.live-*.json. Defaults to
      <output>/manifest.live-<timestamp>.json.
  --codexbar PATH
      Use a specific upstream codexbar binary. Same as CODEXBAR_CLI=PATH.
  --metadata-only
      Capture only codexbar --version. This is also the default mode.
  --allow-provider-network
      Also run usage/cost/status commands that may contact provider endpoints
      through the upstream codexbar CLI.
  --provider-source SOURCE
      Source for provider success probes when --allow-provider-network is set.
      Allowed values: cli, auto, web. Defaults to cli, which is the expected
      Linux success source. auto and web are Linux error-probe sources.
  --include-error-probes
      Also run unsupported-source and invalid-provider probes. This requires
      --allow-provider-network, because provider-oriented CLI entry points must
      be treated as potentially network-capable.
  --include-config-validate
      Also run codexbar config validate --format json --json-only.
  --include-config-dump
      Also run codexbar config dump --pretty. This may expose secrets before
      redaction and requires CODEXBAR_CAPTURE_INCLUDE_CONFIG_DUMP=1 or
      --i-understand-config-dump-may-contain-secrets.
  --i-understand-config-dump-may-contain-secrets
      Second confirmation required for --include-config-dump unless the
      CODEXBAR_CAPTURE_INCLUDE_CONFIG_DUMP=1 environment variable is set.
  -h, --help
      Show this help.

Default with CODEXBAR_CAPTURE_LIVE=1: locate codexbar, run only
codexbar --version, write a redacted live manifest sidecar, and print a review
checklist. The script never replaces daemon/fixtures/upstream-cli/manifest.json.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --live)
      LIVE_CAPTURE=1
      shift
      ;;
    --output)
      OUTPUT_DIR="${2:?missing value for --output}"
      shift 2
      ;;
    --manifest)
      MANIFEST_OUT="${2:?missing value for --manifest}"
      shift 2
      ;;
    --codexbar)
      CODEXBAR_CLI="${2:?missing value for --codexbar}"
      shift 2
      ;;
    --metadata-only)
      METADATA_ONLY=1
      shift
      ;;
    --allow-provider-network)
      ALLOW_PROVIDER_NETWORK=1
      shift
      ;;
    --provider-source)
      PROVIDER_SOURCE="${2:?missing value for --provider-source}"
      shift 2
      ;;
    --include-error-probes)
      INCLUDE_ERROR_PROBES=1
      shift
      ;;
    --include-config-validate)
      INCLUDE_CONFIG_VALIDATE=1
      shift
      ;;
    --include-config-dump)
      INCLUDE_CONFIG_DUMP=1
      shift
      ;;
    --i-understand-config-dump-may-contain-secrets)
      UNDERSTAND_CONFIG_DUMP=1
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

if [[ "$LIVE_CAPTURE" != "1" ]]; then
  cat >&2 <<'EOF'
Live upstream CodexBar CLI capture is opt-in because command output may include
account metadata before redaction.

Run:
  ./scripts/capture-upstream-cli-samples.sh --live --metadata-only

Task 02B must not proceed without reviewed redacted Linux samples. CI must use
committed fixtures and fake-codexbar tests, not a live codexbar binary.
EOF
  exit 2
fi

case "$PROVIDER_SOURCE" in
  cli|auto|web) ;;
  *)
    echo "--provider-source must be one of: cli, auto, web" >&2
    exit 2
    ;;
esac

if [[ "$METADATA_ONLY" -eq 1 ]]; then
  if [[ "$ALLOW_PROVIDER_NETWORK" -eq 1 || "$INCLUDE_ERROR_PROBES" -eq 1 || "$INCLUDE_CONFIG_VALIDATE" -eq 1 || "$INCLUDE_CONFIG_DUMP" -eq 1 ]]; then
    echo "--metadata-only cannot be combined with provider, error-probe, or config capture flags" >&2
    exit 2
  fi
fi

if [[ "$INCLUDE_ERROR_PROBES" -eq 1 && "$ALLOW_PROVIDER_NETWORK" -ne 1 ]]; then
  cat >&2 <<'EOF'
--include-error-probes requires --allow-provider-network.

The error probes use provider-oriented upstream CLI entry points. Treat them as
potentially provider-network-capable unless a reviewed upstream version proves
they fail before provider access.
EOF
  exit 2
fi

if [[ "$INCLUDE_CONFIG_DUMP" -eq 1 && "${CODEXBAR_CAPTURE_INCLUDE_CONFIG_DUMP:-}" != "1" && "$UNDERSTAND_CONFIG_DUMP" -ne 1 ]]; then
  cat >&2 <<'EOF'
Refusing config dump capture.

codexbar config dump may contain API keys, OAuth/session material, account
identity, local paths, or other secrets before redaction. Re-run with either:

  CODEXBAR_CAPTURE_INCLUDE_CONFIG_DUMP=1
  --i-understand-config-dump-may-contain-secrets
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
  ./scripts/capture-upstream-cli-samples.sh --live --metadata-only
  ./scripts/capture-upstream-cli-samples.sh --live --codexbar /path/to/codexbar --metadata-only

Task 02B must not proceed without reviewed redacted Linux samples.
EOF
  exit 2
fi

if [[ -z "$OUTPUT_DIR" ]]; then
  OUTPUT_DIR="${TMPDIR:-/tmp}/codexbar-upstream-cli-live-$CAPTURE_ID"
fi

umask 077
RAW_DIR="$(mktemp -d "${TMPDIR:-/tmp}/codexbar-cli-raw.XXXXXX")"
chmod 0700 "$RAW_DIR"
ENTRY_FILE="$RAW_DIR/entries.jsonl"
trap 'rm -rf "$RAW_DIR"' EXIT

canonical_existing_dir() {
  local path="$1"
  (cd "$path" && pwd -P)
}

canonical_path_no_create() {
  local path="$1"
  python3 - "$path" <<'PY'
import sys
from pathlib import Path

print(Path(sys.argv[1]).expanduser().resolve(strict=False))
PY
}

path_is_under() {
  local child="$1"
  local parent="$2"
  [[ "$child" == "$parent" || "$child" == "$parent"/* ]]
}

OUTPUT_DIR="$(canonical_path_no_create "$OUTPUT_DIR")"
COMMITTED_FIXTURE_ROOT="$(canonical_existing_dir "$ROOT/daemon/fixtures/upstream-cli")"
if path_is_under "$OUTPUT_DIR" "$COMMITTED_FIXTURE_ROOT" && [[ "${CODEXBAR_ALLOW_COMMITTED_FIXTURE_OUTPUT:-}" != "1" ]]; then
  cat >&2 <<'EOF'
Refusing to write live capture files into daemon/fixtures/upstream-cli.

Use --output /tmp/codexbar-upstream-cli, review the redacted files, then promote
only selected sidecars into the committed fixture corpus. To intentionally
override this guard, set CODEXBAR_ALLOW_COMMITTED_FIXTURE_OUTPUT=1.
EOF
  exit 2
fi

if [[ -z "$MANIFEST_OUT" ]]; then
  MANIFEST_OUT="$OUTPUT_DIR/manifest.live-$CAPTURE_ID.json"
fi
MANIFEST_OUT="$(canonical_path_no_create "$MANIFEST_OUT")"
MANIFEST_DIR="${MANIFEST_OUT%/*}"
if [[ "$MANIFEST_DIR" == "$MANIFEST_OUT" ]]; then
  MANIFEST_DIR="."
fi
if [[ "$MANIFEST_DIR" != "$OUTPUT_DIR" ]]; then
  cat >&2 <<'EOF'
Refusing to write a live capture manifest outside --output.

Live capture sidecars and their manifest must share one directory root so the
capture can be validated and promoted as a single reviewed package.
EOF
  exit 2
fi
case "${MANIFEST_OUT##*/}" in
  manifest.live-*.json) ;;
  *)
    cat >&2 <<'EOF'
Refusing live capture manifest name.

The manifest basename must match manifest.live-*.json so a capture directory can
be validated directly with scripts/validate-upstream-cli-capture.sh.
EOF
    exit 2
    ;;
esac
if path_is_under "$MANIFEST_OUT" "$COMMITTED_FIXTURE_ROOT" && [[ "${CODEXBAR_ALLOW_COMMITTED_FIXTURE_OUTPUT:-}" != "1" ]]; then
  cat >&2 <<'EOF'
Refusing to write a live capture manifest into daemon/fixtures/upstream-cli.

Live capture must produce a sidecar manifest outside the committed corpus.
Validate and manually promote selected files instead of replacing manifest.json.
EOF
  exit 2
fi
mkdir -p "$OUTPUT_DIR/usage" "$OUTPUT_DIR/cost" "$OUTPUT_DIR/errors" "$OUTPUT_DIR/status"
chmod 0700 "$OUTPUT_DIR" "$OUTPUT_DIR/usage" "$OUTPUT_DIR/cost" "$OUTPUT_DIR/errors" "$OUTPUT_DIR/status"

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

CAPTURE_ENV=(env -i "HOME=${HOME:-}" "PATH=${PATH:-/usr/local/bin:/usr/bin:/bin}")
for env_name in LANG LC_ALL LC_CTYPE NO_COLOR TERM XDG_CONFIG_HOME XDG_CACHE_HOME CODEXBAR_FAKE_LOG; do
  if [[ -n "${!env_name:-}" ]]; then
    CAPTURE_ENV+=("$env_name=${!env_name}")
  fi
done

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
raw_argv = sys.argv[20:]
cli_path = raw_argv[0] if raw_argv else None
argv = ["codexbar", *raw_argv[1:]] if raw_argv else []
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
    "codexbarCliPath": cli_path,
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
metadata_path.chmod(0o600)
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
  timeout "${timeout_seconds}s" "${CAPTURE_ENV[@]}" "$CODEXBAR_BIN" "$@" >"$raw_stdout" 2>"$raw_stderr"
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

  if [[ "$exit_code" -ne 0 ]]; then
    case "$expected_category" in
      usage_success) expected_category="usage_error" ;;
      cost_success) expected_category="cost_error" ;;
    esac
  fi

  "$REDACTOR" --input "$raw_stdout" --output "$redacted_stdout_tmp"
  "$REDACTOR" --input "$raw_stderr" --output "$redacted_stderr_tmp"
  if [[ "$fixture_id" == "version" && "$exit_code" -eq 0 ]]; then
    local version_text
    if IFS= read -r version_text <"$redacted_stdout_tmp" && [[ -n "$version_text" ]]; then
      UPSTREAM_VERSION="$version_text"
    fi
  fi

  stdout_ext="$(json_extension_for "$redacted_stdout_tmp")"
  stderr_ext="$(json_extension_for "$redacted_stderr_tmp")"
  local stdout_rel="$category_dir/live_${CAPTURE_ID}_${fixture_id}_stdout.$stdout_ext"
  local stderr_rel="$category_dir/live_${CAPTURE_ID}_${fixture_id}_stderr.$stderr_ext"
  local metadata_rel="$category_dir/live_${CAPTURE_ID}_${fixture_id}_metadata.json"

  mv "$redacted_stdout_tmp" "$OUTPUT_DIR/$stdout_rel"
  mv "$redacted_stderr_tmp" "$OUTPUT_DIR/$stderr_rel"
  chmod 0600 "$OUTPUT_DIR/$stdout_rel" "$OUTPUT_DIR/$stderr_rel"
  write_metadata_and_entry "$OUTPUT_DIR/$metadata_rel" "$ENTRY_FILE" "$fixture_id" "$command_name" "$expected_category" "$timeout_seconds" "$exit_code" "$timed_out" "$duration_ms" "$stdout_bytes" "$stderr_bytes" "$stdout_rel" "$stderr_rel" "$metadata_rel" "$CODEXBAR_BIN" "$@"
  "$REDACTOR" --input "$OUTPUT_DIR/$metadata_rel" --output "$OUTPUT_DIR/$metadata_rel.redacted"
  mv "$OUTPUT_DIR/$metadata_rel.redacted" "$OUTPUT_DIR/$metadata_rel"
  chmod 0600 "$OUTPUT_DIR/$metadata_rel"
}

CAPTURED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
PLATFORM_OS="$(uname -s)"
PLATFORM_KERNEL="$(uname -r)"
PLATFORM_ARCH="$(uname -m)"

UPSTREAM_VERSION="unknown"

run_capture "version" "status" "version" "version" 5 --version

if [[ "$INCLUDE_CONFIG_VALIDATE" -eq 1 ]]; then
  run_capture "config_validate" "status" "config_validate" "config_validate" 10 config validate --format json --json-only
fi

if [[ "$INCLUDE_CONFIG_DUMP" -eq 1 ]]; then
  cat >&2 <<'EOF'
WARNING: running codexbar config dump. Review redacted output carefully before
promotion; never commit raw config dump output.
EOF
  run_capture "config_dump" "status" "config_dump" "config_dump" 10 config dump --pretty
fi

if [[ "$ALLOW_PROVIDER_NETWORK" -eq 1 ]]; then
  run_capture "usage_default_all" "usage" "usage" "usage_success" 30 --format json --json-only --provider all --source "$PROVIDER_SOURCE"
  run_capture "usage_subcommand_all" "usage" "usage" "usage_success" 30 usage --format json --json-only --provider all --source "$PROVIDER_SOURCE"
  run_capture "cost_all" "cost" "cost" "cost_success" 20 cost --format json --json-only --provider all
  run_capture "status_all" "status" "status" "usage_success" 30 --format json --json-only --provider all --source "$PROVIDER_SOURCE" --status
fi

if [[ "$INCLUDE_ERROR_PROBES" -eq 1 ]]; then
  run_capture "unsupported_web_source" "errors" "usage" "unsupported_source" 30 --format json --json-only --provider all --source web
  run_capture "unsupported_auto_source" "errors" "usage" "unsupported_source" 30 --format json --json-only --provider all --source auto
  run_capture "invalid_provider" "errors" "usage" "invalid_provider" 30 --format json --json-only --provider __codexbar_linux_invalid_provider__
fi

python3 - "$ENTRY_FILE" "$MANIFEST_OUT" "$CAPTURED_AT" "$UPSTREAM_VERSION" "$PLATFORM_OS" "$PLATFORM_KERNEL" "$PLATFORM_ARCH" <<'PY'
import json
import sys
from pathlib import Path

entry_file = Path(sys.argv[1])
manifest_path = Path(sys.argv[2])
entries = [json.loads(line) for line in entry_file.read_text(encoding="utf-8").splitlines() if line.strip()]
categories = sorted({entry["expectedCategory"] for entry in entries})
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
        "notes": ["local live capture; review before promoting selected files into manifest.json"],
    },
    "testExpectations": {
        category: "capture-specific category; validate and review before promotion"
        for category in categories
    },
    "fixtures": entries,
}
manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
manifest_path.chmod(0o600)
PY
"$REDACTOR" --input "$MANIFEST_OUT" --output "$MANIFEST_OUT.redacted"
mv "$MANIFEST_OUT.redacted" "$MANIFEST_OUT"
chmod 0600 "$MANIFEST_OUT"

cat <<EOF
Wrote redacted live capture manifest:
  $MANIFEST_OUT

Raw temporary files were stored only under:
  $RAW_DIR
and will be deleted on exit.

Review checklist before promotion:
  1. Run: ./scripts/validate-upstream-cli-capture.sh "$OUTPUT_DIR"
  2. Manually inspect redacted stdout/stderr/metadata files.
  3. Confirm no raw emails, account IDs, org names, tokens, cookies, headers,
     browser paths, config secrets, raw provider payloads, or stderr secrets.
  4. Copy only reviewed files into daemon/fixtures/upstream-cli/.
  5. Add selected entries to daemon/fixtures/upstream-cli/manifest.json.
  6. Run: ./scripts/validate-upstream-cli-fixtures.sh
EOF
