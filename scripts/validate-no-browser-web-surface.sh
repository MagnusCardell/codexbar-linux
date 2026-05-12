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
  "extension/browser-extension"
  "extension/manifest.json"
)

for rel in "${absent_paths[@]}"; do
  if [[ -e "$ROOT/$rel" ]]; then
    fail "$rel must not exist while browser-cookie/web-fetch is out of scope"
  fi
done

if find "$ROOT/extension/src" -maxdepth 1 -type f ! -name '*.js' ! -name 'README.md' -print -quit | grep -q .; then
  find "$ROOT/extension/src" -maxdepth 1 -type f ! -name '*.js' ! -name 'README.md' -print >&2
  fail "extension/src must not contain task prompts or non-runtime artifacts"
fi

if find "$ROOT/tasks" -maxdepth 1 -type f \( -iname '*browser*' -o -iname '*cookie*' -o -iname '*web*' \) -print -quit | grep -q .; then
  find "$ROOT/tasks" -maxdepth 1 -type f \( -iname '*browser*' -o -iname '*cookie*' -o -iname '*web*' \) -print >&2
  fail "browser/cookie/web task files are out of scope; use the no-browser ADR for rejected alternatives"
fi

if rg -n '^(reqwest|rusqlite|url|aes|cbc|pbkdf2|sha1|sha2|hyper|axum|warp|ureq|isahc|curl|keyring|secret-service|libsecret|cookie_store|sqlite)\s*=' "$ROOT/daemon/Cargo.toml"; then
  fail "daemon/Cargo.toml contains direct browser/web/keyring dependency"
fi

if rg -n 'package\s*=\s*"(reqwest|rusqlite|url|aes|cbc|pbkdf2|sha1|sha2|hyper|axum|warp|ureq|isahc|curl|keyring|secret-service|libsecret|cookie_store|sqlite)"' "$ROOT/daemon/Cargo.toml"; then
  fail "daemon/Cargo.toml contains renamed browser/web/keyring dependency"
fi

if rg -n '\b(pkg-config|libsqlite3-dev|sqlite3|cmake|ca-certificates|libsoup|webkit|libsecret|gir1\.2-secret|curl|chromium|google-chrome|firefox)\b' \
  "$ROOT/packaging/debian/control" "$ROOT/.github/workflows/check.yml"; then
  fail "packaging or CI contains browser/web/keyring-only system dependency"
fi

if rg -n \
  'CODEXBAR_BROWSER|CODEXBAR_CODEX_WEB_LIVE|CODEXBAR_WEB_HOME|BrowserDiscoveryRoots|collect_session_material|ReqwestStaticGetClient|FakeWebClient|decrypt_cookie|cookie_store|provider web fetch|chatgpt\.com/codex/settings/usage|libsecret|Secret Service|KWallet|TcpListener|TcpStream|std::net|tokio::net|Gio\.SocketService|Gio\.SocketListener|Soup\.Server|axum|hyper|ureq|isahc' \
  "$ROOT/daemon/src" "$ROOT/daemon/tests" "$ROOT/extension/extension.js" "$ROOT/extension/prefs.js" "$ROOT/extension/src/"*.js "$ROOT/packaging" "$ROOT/scripts/install-local.sh" "$ROOT/scripts/uninstall-local.sh" "$ROOT/scripts/build-deb.sh" "$ROOT/.github" \
  "$ROOT/scripts/codexbar-linux-setup" \
  --glob '!daemon/src/model.rs' \
  --glob '!daemon/src/redact.rs' \
  --glob '!daemon/tests/browser_import_stub.rs' \
  --glob '!daemon/tests/redaction.rs' \
  --glob '!packaging/man/**'; then
  fail "runtime code contains browser-cookie/web-fetch/keyring/localhost implementation marker"
fi

if rg -n 'cookies\.sqlite|Network/Cookies|Cookie header|Set-Cookie' \
  "$ROOT/daemon/src" \
  --glob '!daemon/src/model.rs' \
  --glob '!daemon/src/redact.rs'; then
  fail "daemon runtime code contains browser cookie/header marker"
fi

echo "No browser-cookie/web-fetch surface present"
