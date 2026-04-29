#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

bash -n "$ROOT/scripts/capture-upstream-cli-samples.sh"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

FAKE="$TMP/codexbar"
LOG="$TMP/invocations.log"

cat >"$FAKE" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"${CODEXBAR_FAKE_LOG:?}"
case "$*" in
  "--version")
    printf 'codexbar fake 1.2.3\n'
    ;;
  "config validate --format json --json-only")
    printf '{"ok":true,"email":"dev@example.com","sessionKey":"sk-secret-value"}\n'
    ;;
  "config dump --pretty")
    printf '{"apiKey":"sk-config-secret","home":"/home/person/.codexbar/config.json"}\n'
    printf 'sessionKey=plain-session-secret Authorization: Bearer plain-token\n' >&2
    ;;
  "--format json --json-only --provider all --source cli"|"usage --format json --json-only --provider all --source cli"|"--format json --json-only --provider all --source cli --status")
    printf '{"provider":"codex","accountEmail":"dev@example.com","usage":{"identity":{"accountEmail":"nested@example.com","authPath":"~/.local/share/codexbar/auth.json"},"primary":{"usedPercent":12}},"diagnosticsPath":"/home/person/.local/share/codexbar/auth.json"}\n'
    ;;
  *"--source web"*)
    printf '{"error":{"code":"unsupported_source","message":"web unsupported"}}\n'
    exit 1
    ;;
  *"--source auto"*)
    printf '{"error":{"code":"unsupported_source","message":"auto unsupported"}}\n'
    exit 1
    ;;
  *"__codexbar_linux_invalid_provider__"*)
    printf '{"error":{"code":"invalid_provider","provider":"__codexbar_linux_invalid_provider__"}}\n'
    exit 2
    ;;
  "cost --format json --json-only --provider all")
    printf '[{"provider":"codex","totalCost":1.23,"authorization":"Bearer secret"}]\n'
    ;;
  *)
    printf '{"provider":"codex","accountEmail":"dev@example.com","usage":{"primary":{"usedPercent":12}}}\n'
    ;;
esac
SH
chmod +x "$FAKE"

run_capture() {
  local out_dir="$1"
  shift
  TMPDIR="$TMP" CODEXBAR_FAKE_LOG="$LOG" CODEXBAR_CAPTURE_LIVE=1 CODEXBAR_CLI="$FAKE" \
    "$ROOT/scripts/capture-upstream-cli-samples.sh" --output "$out_dir" "$@"
  "$ROOT/scripts/validate-upstream-cli-capture.sh" "$out_dir"
  if find "$TMP" -maxdepth 1 -type d -name 'codexbar-cli-raw.*' | grep -q .; then
    echo "raw temp directory was left behind under $TMP" >&2
    exit 1
  fi
}

assert_only_version_invoked() {
  if [[ "$(wc -l <"$LOG")" -ne 1 ]] || ! grep -Fx -- "--version" "$LOG" >/dev/null; then
    echo "$1 should invoke only --version" >&2
    cat "$LOG" >&2
    exit 1
  fi
}

set +e
TMPDIR="$TMP" CODEXBAR_FAKE_LOG="$LOG" CODEXBAR_CLI="$FAKE" \
  "$ROOT/scripts/capture-upstream-cli-samples.sh" --output "$TMP/unset-live" >"$TMP/unset-live.out" 2>"$TMP/unset-live.err"
status=$?
set -e
if [[ "$status" -ne 2 ]]; then
  echo "expected CODEXBAR_CAPTURE_LIVE unset run to exit 2, got $status" >&2
  exit 1
fi
if [[ -f "$LOG" ]]; then
  echo "fake codexbar was invoked without CODEXBAR_CAPTURE_LIVE=1" >&2
  exit 1
fi

run_capture "$TMP/default-live"
assert_only_version_invoked "default CODEXBAR_CAPTURE_LIVE=1 capture"

: >"$LOG"
run_capture "$TMP/metadata-only" --metadata-only
assert_only_version_invoked "--metadata-only capture"

: >"$LOG"
TMPDIR="$TMP" CODEXBAR_FAKE_LOG="$LOG" CODEXBAR_CAPTURE_LIVE=1 CODEXBAR_CLI="$FAKE" \
  "$ROOT/scripts/capture-upstream-cli-samples.sh" \
    --output "$TMP/custom-manifest" \
    --manifest "$TMP/custom-manifest/manifest.live-custom.json"
"$ROOT/scripts/validate-upstream-cli-capture.sh" "$TMP/custom-manifest/manifest.live-custom.json"
assert_only_version_invoked "custom manifest under --output capture"

: >"$LOG"
set +e
TMPDIR="$TMP" CODEXBAR_FAKE_LOG="$LOG" CODEXBAR_CAPTURE_LIVE=1 CODEXBAR_CLI="$FAKE" \
  "$ROOT/scripts/capture-upstream-cli-samples.sh" \
    --output "$TMP/manifest-root" \
    --manifest "$TMP/manifest.live-outside-output.json" \
    >"$TMP/manifest-outside-output-refused.out" 2>"$TMP/manifest-outside-output-refused.err"
status=$?
set -e
if [[ "$status" -ne 2 ]]; then
  echo "expected manifest outside --output to exit 2, got $status" >&2
  exit 1
fi
if [[ -s "$LOG" ]]; then
  echo "manifest outside --output refusal should not invoke fake codexbar" >&2
  cat "$LOG" >&2
  exit 1
fi

: >"$LOG"
set +e
TMPDIR="$TMP" CODEXBAR_FAKE_LOG="$LOG" CODEXBAR_CAPTURE_LIVE=1 CODEXBAR_CLI="$FAKE" \
  "$ROOT/scripts/capture-upstream-cli-samples.sh" \
    --output "$TMP/invalid-manifest-name" \
    --manifest "$TMP/invalid-manifest-name/live.json" \
    >"$TMP/invalid-manifest-name-refused.out" 2>"$TMP/invalid-manifest-name-refused.err"
status=$?
set -e
if [[ "$status" -ne 2 ]]; then
  echo "expected invalid manifest basename to exit 2, got $status" >&2
  exit 1
fi
if [[ -s "$LOG" ]]; then
  echo "invalid manifest basename refusal should not invoke fake codexbar" >&2
  cat "$LOG" >&2
  exit 1
fi

: >"$LOG"
set +e
TMPDIR="$TMP" CODEXBAR_FAKE_LOG="$LOG" CODEXBAR_CAPTURE_LIVE=1 CODEXBAR_CLI="$FAKE" \
  "$ROOT/scripts/capture-upstream-cli-samples.sh" \
    --output "$TMP/committed-manifest-refused" \
    --manifest "$ROOT/daemon/fixtures/upstream-cli/manifest.json" \
    >"$TMP/committed-manifest-refused.out" 2>"$TMP/committed-manifest-refused.err"
status=$?
set -e
if [[ "$status" -ne 2 ]]; then
  echo "expected committed manifest output to exit 2, got $status" >&2
  exit 1
fi
if [[ -s "$LOG" ]]; then
  echo "committed manifest refusal should not invoke fake codexbar" >&2
  cat "$LOG" >&2
  exit 1
fi
if find "$TMP" -maxdepth 1 -type d -name 'codexbar-cli-raw.*' | grep -q .; then
  echo "raw temp directory was left behind after committed manifest refusal" >&2
  exit 1
fi

: >"$LOG"
run_capture "$TMP/provider" --allow-provider-network
for expected in \
  "--version" \
  "--format json --json-only --provider all --source cli" \
  "usage --format json --json-only --provider all --source cli" \
  "cost --format json --json-only --provider all" \
  "--format json --json-only --provider all --source cli --status"
do
  grep -Fx -- "$expected" "$LOG" >/dev/null || {
    echo "missing provider capture invocation: $expected" >&2
    cat "$LOG" >&2
    exit 1
  }
done
if grep -E '^cost .*--source' "$LOG" >/dev/null; then
  echo "cost capture should not receive --source" >&2
  cat "$LOG" >&2
  exit 1
fi
if grep -R -E 'dev@example.com|nested@example.com|/home/person|~/.local/share|auth\.json' "$TMP/provider" >/dev/null; then
  echo "provider capture retained an unredacted fake identity or path" >&2
  exit 1
fi

: >"$LOG"
run_capture "$TMP/provider-cli-flag" --allow-provider-network --provider-source cli
grep -Fx -- "usage --format json --json-only --provider all --source cli" "$LOG" >/dev/null || {
  echo "explicit --provider-source cli was not passed to usage capture" >&2
  cat "$LOG" >&2
  exit 1
}

: >"$LOG"
set +e
TMPDIR="$TMP" CODEXBAR_FAKE_LOG="$LOG" CODEXBAR_CAPTURE_LIVE=1 CODEXBAR_CLI="$FAKE" \
  "$ROOT/scripts/capture-upstream-cli-samples.sh" --output "$TMP/provider-source-invalid" --allow-provider-network --provider-source local >"$TMP/provider-source-invalid.out" 2>"$TMP/provider-source-invalid.err"
status=$?
set -e
if [[ "$status" -ne 2 ]]; then
  echo "expected invalid --provider-source to exit 2, got $status" >&2
  exit 1
fi
if [[ -s "$LOG" ]]; then
  echo "invalid provider source refusal should not invoke fake codexbar" >&2
  cat "$LOG" >&2
  exit 1
fi

: >"$LOG"
set +e
TMPDIR="$TMP" CODEXBAR_FAKE_LOG="$LOG" CODEXBAR_CAPTURE_LIVE=1 CODEXBAR_CLI="$FAKE" \
  "$ROOT/scripts/capture-upstream-cli-samples.sh" --output "$TMP/error-probes-refused" --include-error-probes >"$TMP/error-probes-refused.out" 2>"$TMP/error-probes-refused.err"
status=$?
set -e
if [[ "$status" -ne 2 ]]; then
  echo "expected --include-error-probes without --allow-provider-network to exit 2, got $status" >&2
  exit 1
fi
if [[ -s "$LOG" ]]; then
  echo "error probe refusal should not invoke fake codexbar" >&2
  cat "$LOG" >&2
  exit 1
fi

run_capture "$TMP/error-probes" --allow-provider-network --include-error-probes
for expected in \
  "--format json --json-only --provider all --source web" \
  "--format json --json-only --provider all --source auto" \
  "--format json --json-only --provider __codexbar_linux_invalid_provider__"
do
  grep -Fx -- "$expected" "$LOG" >/dev/null || {
    echo "missing error probe invocation: $expected" >&2
    cat "$LOG" >&2
    exit 1
  }
done

: >"$LOG"
set +e
TMPDIR="$TMP" CODEXBAR_FAKE_LOG="$LOG" CODEXBAR_CAPTURE_LIVE=1 CODEXBAR_CLI="$FAKE" \
  "$ROOT/scripts/capture-upstream-cli-samples.sh" --output "$TMP/config-dump-refused" --include-config-dump >"$TMP/config-dump-refused.out" 2>"$TMP/config-dump-refused.err"
status=$?
set -e
if [[ "$status" -ne 2 ]]; then
  echo "expected --include-config-dump without confirmation to exit 2, got $status" >&2
  exit 1
fi
if [[ -s "$LOG" ]]; then
  echo "config dump refusal should not invoke fake codexbar" >&2
  cat "$LOG" >&2
  exit 1
fi

: >"$LOG"
run_capture "$TMP/config-dump-flag" --include-config-dump --i-understand-config-dump-may-contain-secrets
grep -Fx -- "config dump --pretty" "$LOG" >/dev/null || {
  echo "missing config dump invocation with acknowledgement flag" >&2
  cat "$LOG" >&2
  exit 1
}
if grep -R -E 'plain-session-secret|plain-token|sk-config-secret|/home/person' "$TMP/config-dump-flag" >/dev/null; then
  echo "config dump capture retained an unredacted fake secret or home path" >&2
  exit 1
fi

: >"$LOG"
TMPDIR="$TMP" CODEXBAR_FAKE_LOG="$LOG" CODEXBAR_CAPTURE_LIVE=1 CODEXBAR_CLI="$FAKE" CODEXBAR_CAPTURE_INCLUDE_CONFIG_DUMP=1 \
  "$ROOT/scripts/capture-upstream-cli-samples.sh" --output "$TMP/config-dump-env" --include-config-dump
"$ROOT/scripts/validate-upstream-cli-capture.sh" "$TMP/config-dump-env"
if find "$TMP" -maxdepth 1 -type d -name 'codexbar-cli-raw.*' | grep -q .; then
  echo "raw temp directory was left behind after config dump env capture" >&2
  exit 1
fi
grep -Fx -- "config dump --pretty" "$LOG" >/dev/null || {
  echo "missing config dump invocation with acknowledgement environment variable" >&2
  cat "$LOG" >&2
  exit 1
}

: >"$LOG"
run_capture "$TMP/config-validate" --include-config-validate
grep -Fx -- "config validate --format json --json-only" "$LOG" >/dev/null || {
  echo "missing config validate invocation" >&2
  cat "$LOG" >&2
  exit 1
}

echo "Upstream CLI capture harness tests passed"
