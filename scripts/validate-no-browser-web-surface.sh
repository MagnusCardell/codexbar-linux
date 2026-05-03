#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

fail() {
  echo "No-browser/web surface validation failed: $*" >&2
  exit 1
}

absent_paths=(
  ".codex/agents/browser_cookie_engineer.toml"
  "daemon/src/browser"
  "daemon/src/web"
  "daemon/fixtures/browser"
  "daemon/fixtures/web"
  "daemon/tests/browser_chromium.rs"
  "daemon/tests/web_codex.rs"
  "docs/adr/0004-linux-browser-cookie-layer.md"
  "docs/adr/0006-linux-browser-cookie-daemon-layer.md"
  "docs/browser-cookie-architecture.md"
  "docs/browser-cookie-threat-model.md"
  "docs/browser-support.md"
  "docs/codex-web-live-recon.md"
  "docs/provider-roadmap.md"
  "prompts/02-browser-cookie-dispatch.md"
  "scripts/chromium-throwaway-smoke.sh"
  "scripts/validate-browser-fixtures.sh"
  "scripts/validate-web-fixtures.sh"
  "tasks/04-browser-cookie-research.md"
  "tasks/04a-browser-cookie-architecture.md"
  "tasks/04b-chromium-cookie-import.md"
  "tasks/04c-firefox-cookie-import.md"
  "tasks/04d-codex-web-adapter.md"
  "tasks/04e-browser-import-hardening.md"
  "tasks/05-browser-cookie-adapter.md"
  "tasks/06-provider-web-adapters.md"
)

for rel in "${absent_paths[@]}"; do
  if [[ -e "$ROOT/$rel" ]]; then
    fail "$rel must not exist while browser-cookie/web-fetch is out of scope"
  fi
done

if rg -n '^(reqwest|rusqlite|url|aes|cbc|pbkdf2|sha1|sha2)\s*=' "$ROOT/daemon/Cargo.toml"; then
  fail "daemon/Cargo.toml contains direct browser/web dependency"
fi

if rg -n '\b(pkg-config|libsqlite3-dev|cmake|ca-certificates)\b' \
  "$ROOT/packaging/debian/control" "$ROOT/.github/workflows/check.yml"; then
  fail "packaging or CI contains browser/web-only system dependency"
fi

if rg -n \
  'CODEXBAR_BROWSER|CODEXBAR_CODEX_WEB_LIVE|CODEXBAR_WEB_HOME|BrowserDiscoveryRoots|collect_session_material|ReqwestStaticGetClient|FakeWebClient|decrypt_cookie|cookie_store|provider web fetch|chatgpt\.com/codex/settings/usage|libsecret|Secret Service|KWallet' \
  "$ROOT/daemon/src" "$ROOT/daemon/tests" "$ROOT/extension/src" "$ROOT/packaging" "$ROOT/.github" \
  --glob '!daemon/src/model.rs' \
  --glob '!daemon/src/redact.rs' \
  --glob '!daemon/tests/browser_import_stub.rs' \
  --glob '!daemon/tests/redaction.rs'; then
  fail "runtime code contains browser-cookie/web-fetch implementation marker"
fi

if rg -n 'cookies\.sqlite|Network/Cookies|Cookie header|Set-Cookie' \
  "$ROOT/daemon/src" \
  --glob '!daemon/src/model.rs' \
  --glob '!daemon/src/redact.rs'; then
  fail "daemon runtime code contains browser cookie/header marker"
fi

echo "No browser-cookie/web-fetch surface present"
