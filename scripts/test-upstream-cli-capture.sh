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
  "--format json --json-only --provider codex --source cli"|"usage --format json --json-only --provider codex --source cli"|"--format json --json-only --provider codex --source cli --status")
    printf '{"provider":"codex","providerID":"acct_live_raw","accountEmail":"dev@example.com","accountOrganization":"Secret Org","usage":{"identity":{"accountEmail":"nested@example.com","authPath":"~/.local/share/codexbar/auth.json","providerID":"nested_acct_raw"},"primary":{"usedPercent":12}},"diagnosticsPath":"/home/person/.local/share/codexbar/auth.json","rawResponse":{"accountEmail":"raw-response@example.com","token":"raw-response-token"},"rawPayload":"Authorization: Bearer raw-payload-token"}\n'
    ;;
  "--format json --json-only --provider claude --source cli"|"usage --format json --json-only --provider claude --source cli"|"--format json --json-only --provider claude --source cli --status")
    printf '{"provider":"claude","accountEmail":"claude@example.com","usage":{"identity":{"accountEmail":"claude-nested@example.com"},"primary":{"usedPercent":45}}}\n'
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
    printf '{"error":{"code":"invalid_provider","provider":"__codexbar_linux_invalid_provider__","providerID":"acct_text_raw","accountEmail":"opaque_user"}}\n'
    printf '{"identity":{"accountEmail":"opaque_nested","providerID":"nested_text_raw","accountOrganization":"Stream Org"}}\n'
    exit 2
    ;;
  "cost --format json --json-only --provider both")
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
  "cost --format json --json-only --provider both" \
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
for expected_id in \
  '"fixtureId": "usage_all_cli_default"' \
  '"fixtureId": "usage_all_cli_subcommand"' \
  '"fixtureId": "status_all_cli"'
do
  grep -R -F -- "$expected_id" "$TMP/provider" >/dev/null || {
    echo "missing provider/source fixture id in default provider capture: $expected_id" >&2
    exit 1
  }
done

: >"$LOG"
run_capture "$TMP/provider-targeted" --allow-provider-network --providers codex,claude --provider-source cli --usage-timeout 7 --cost-timeout 9 --version-timeout 3
for expected in \
  "--version" \
  "--format json --json-only --provider codex --source cli" \
  "usage --format json --json-only --provider codex --source cli" \
  "--format json --json-only --provider codex --source cli --status" \
  "--format json --json-only --provider claude --source cli" \
  "usage --format json --json-only --provider claude --source cli" \
  "--format json --json-only --provider claude --source cli --status" \
  "cost --format json --json-only --provider both"
do
  grep -Fx -- "$expected" "$LOG" >/dev/null || {
    echo "missing targeted provider capture invocation: $expected" >&2
    cat "$LOG" >&2
    exit 1
  }
done
if grep -E '^cost .*--source' "$LOG" >/dev/null; then
  echo "targeted cost capture should not receive --source" >&2
  cat "$LOG" >&2
  exit 1
fi
for expected_id in \
  '"fixtureId": "usage_codex_cli_default"' \
  '"fixtureId": "usage_codex_cli_subcommand"' \
  '"fixtureId": "status_codex_cli"' \
  '"fixtureId": "usage_claude_cli_default"' \
  '"fixtureId": "usage_claude_cli_subcommand"' \
  '"fixtureId": "status_claude_cli"'
do
  grep -R -F -- "$expected_id" "$TMP/provider-targeted" >/dev/null || {
    echo "missing provider/source fixture id in targeted provider capture: $expected_id" >&2
    exit 1
  }
done
grep -R -F -- '"timeoutSeconds": 3' "$TMP/provider-targeted/status" >/dev/null || {
  echo "version timeout was not recorded in targeted capture metadata" >&2
  exit 1
}
grep -R -F -- '"timeoutSeconds": 7' "$TMP/provider-targeted/usage" >/dev/null || {
  echo "usage timeout was not recorded in targeted capture metadata" >&2
  exit 1
}
grep -R -F -- '"timeoutSeconds": 9' "$TMP/provider-targeted/cost" >/dev/null || {
  echo "cost timeout was not recorded in targeted capture metadata" >&2
  exit 1
}

: >"$LOG"
set +e
TMPDIR="$TMP" CODEXBAR_FAKE_LOG="$LOG" CODEXBAR_CAPTURE_LIVE=1 CODEXBAR_CLI="$FAKE" \
  "$ROOT/scripts/capture-upstream-cli-samples.sh" --output "$TMP/providers-without-network" --providers codex >"$TMP/providers-without-network.out" 2>"$TMP/providers-without-network.err"
status=$?
set -e
if [[ "$status" -ne 2 ]]; then
  echo "expected --providers without --allow-provider-network to exit 2, got $status" >&2
  exit 1
fi
if [[ -s "$LOG" ]]; then
  echo "--providers without network refusal should not invoke fake codexbar" >&2
  cat "$LOG" >&2
  exit 1
fi

: >"$LOG"
set +e
TMPDIR="$TMP" CODEXBAR_FAKE_LOG="$LOG" CODEXBAR_CAPTURE_LIVE=1 CODEXBAR_CLI="$FAKE" \
  "$ROOT/scripts/capture-upstream-cli-samples.sh" --output "$TMP/providers-all-mixed" --allow-provider-network --providers all,codex >"$TMP/providers-all-mixed.out" 2>"$TMP/providers-all-mixed.err"
status=$?
set -e
if [[ "$status" -ne 2 ]]; then
  echo "expected --providers all,codex to exit 2, got $status" >&2
  exit 1
fi
if [[ -s "$LOG" ]]; then
  echo "--providers all,codex refusal should not invoke fake codexbar" >&2
  cat "$LOG" >&2
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
run_capture "$TMP/provider-codex" --allow-provider-network --providers codex --usage-timeout 90
for expected in \
  "--version" \
  "--format json --json-only --provider codex --source cli" \
  "usage --format json --json-only --provider codex --source cli" \
  "--format json --json-only --provider codex --source cli --status" \
  "cost --format json --json-only --provider both"
do
  grep -Fx -- "$expected" "$LOG" >/dev/null || {
    echo "missing targeted provider capture invocation: $expected" >&2
    cat "$LOG" >&2
    exit 1
  }
done
if grep -Fx -- "--format json --json-only --provider all --source cli" "$LOG" >/dev/null || \
   grep -Fx -- "usage --format json --json-only --provider all --source cli" "$LOG" >/dev/null || \
   grep -Fx -- "--format json --json-only --provider all --source cli --status" "$LOG" >/dev/null; then
  echo "targeted provider capture should not run all-provider usage/status probes" >&2
  cat "$LOG" >&2
  exit 1
fi
if grep -E '^cost .*--source' "$LOG" >/dev/null; then
  echo "targeted provider cost capture should not receive --source" >&2
  cat "$LOG" >&2
  exit 1
fi
python3 - "$TMP/provider-codex" <<'PY'
import json
import sys
from pathlib import Path

manifest_path, = Path(sys.argv[1]).glob("manifest.live-*.json")
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
entries = {entry["fixtureId"]: entry for entry in manifest["fixtures"]}
for fixture_id in [
    "usage_codex_cli_default",
    "usage_codex_cli_subcommand",
    "status_codex_cli",
]:
    entry = entries[fixture_id]
    metadata = json.loads((manifest_path.parent / entry["metadataPath"]).read_text(encoding="utf-8"))
    if metadata["timeoutSeconds"] != 90:
        raise SystemExit(f"{fixture_id} timeoutSeconds was {metadata['timeoutSeconds']}, expected 90")
if entries["cost_both"]["argv"] != ["codexbar", "cost", "--format", "json", "--json-only", "--provider", "both"]:
    raise SystemExit("targeted capture cost argv changed unexpectedly")
PY
if grep -R -E 'dev@example.com|nested@example.com|raw-response@example.com|acct_live_raw|nested_acct_raw|Secret Org|rawResponse|rawPayload|raw-response-token|raw-payload-token|/home/person|~/.local/share|auth\.json' "$TMP/provider-codex" >/dev/null; then
  echo "targeted provider capture retained an unredacted fake identity, raw payload, or path" >&2
  exit 1
fi

: >"$LOG"
run_capture "$TMP/provider-codex-claude" --allow-provider-network --providers codex,claude
for expected in \
  "--format json --json-only --provider codex --source cli" \
  "usage --format json --json-only --provider codex --source cli" \
  "--format json --json-only --provider codex --source cli --status" \
  "--format json --json-only --provider claude --source cli" \
  "usage --format json --json-only --provider claude --source cli" \
  "--format json --json-only --provider claude --source cli --status" \
  "cost --format json --json-only --provider both"
do
  grep -Fx -- "$expected" "$LOG" >/dev/null || {
    echo "missing multi-provider targeted invocation: $expected" >&2
    cat "$LOG" >&2
    exit 1
  }
done

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
if grep -R -E 'acct_text_raw|opaque_user|opaque_nested|nested_text_raw|Stream Org' "$TMP/error-probes" >/dev/null; then
  echo "error probe JSON-stream text retained an unredacted fake identity" >&2
  exit 1
fi

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
