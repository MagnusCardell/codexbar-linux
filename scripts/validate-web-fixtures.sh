#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_ROOT="$ROOT/daemon/fixtures/web/codex"

python3 - "$FIXTURE_ROOT" <<'PY'
import json
import re
import sys
from pathlib import Path

fixture_root = Path(sys.argv[1])

required = {
    "README.md",
    "dashboard_success.html",
    "dashboard_login_required.html",
    "dashboard_account_mismatch.html",
    "dashboard_parse_error.html",
    "next_data_usage_success.html",
    "inline_state_usage_success.html",
    "app_shell_no_data.html",
    "login_shell.html",
    "embedded_json_missing_usage.html",
    "embedded_json_redaction_rejected.html",
    "dashboard_too_large.marker",
    "redirect_wrong_host.json",
    "non_200.json",
}

if not fixture_root.is_dir():
    raise SystemExit(f"Missing web fixture root: {fixture_root}")

present = {path.name for path in fixture_root.iterdir() if path.is_file()}
missing = sorted(required - present)
if missing:
    raise SystemExit(f"Missing web fixtures: {', '.join(missing)}")

unexpected = sorted(path.name for path in fixture_root.iterdir() if path.name not in required)
if unexpected:
    raise SystemExit(f"Unexpected web fixture entries: {', '.join(unexpected)}")
not_files = sorted(
    path.name
    for path in fixture_root.iterdir()
    if path.name in required and not path.is_file()
)
if not_files:
    raise SystemExit(f"Web fixture entries must be files: {', '.join(not_files)}")

raw_email = re.compile(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")
domain_like = re.compile(r"\b(?:[a-z0-9-]+\.)+[a-z]{2,}\b", re.I)
secret_patterns = [
    ("home_path", re.compile(r"/home/[^/\s\"']+")),
    ("bearer_token", re.compile(r"(?i)\bBearer\s+\S+")),
    ("authorization_header", re.compile(r"(?im)^Authorization\s*:")),
    ("cookie_header", re.compile(r"(?im)^(Cookie|Set-Cookie)\s*:")),
    ("openai_api_key", re.compile(r"\bsk-[A-Za-z0-9._-]{8,}\b")),
    ("api_key_assignment", re.compile(r"(?i)\bapi[_-]?key\b\s*[:=]")),
    ("token_assignment", re.compile(r"(?i)\b(access[_-]?token|refresh[_-]?token|session[_-]?token|session[_-]?key|password|secret)\b\s*[:=]")),
    ("raw_payload_key", re.compile(r"(?i)\"(raw|rawPayload|rawResponse|rawCookie|rawHeader|headers|cookies?)\"\s*:")),
]
real_provider_domains = {
    "chatgpt.com",
    "openai.com",
    "api.openai.com",
    "auth.openai.com",
    "claude.ai",
    "anthropic.com",
    "cursor.com",
    "mistral.ai",
    "abacus.ai",
    "ollama.com",
}
raw_provider_markers = [
    "client-bootstrap",
    "cf-chl",
    "cloudflare",
    "arkose",
    "intercom",
]


def check_text(path: Path, text: str) -> None:
    for match in raw_email.finditer(text):
        value = match.group(0)
        if value.endswith("@example.invalid"):
            continue
        raise SystemExit(f"Raw email-like value in {path}: {value}")
    for code, pattern in secret_patterns:
        if pattern.search(text):
            raise SystemExit(f"Potential secret marker {code} in {path}")
    lower = text.lower()
    for domain in real_provider_domains:
        if domain in lower:
            raise SystemExit(f"Real provider domain {domain} in {path}")
    for marker in raw_provider_markers:
        if marker.lower() in lower:
            raise SystemExit(f"Raw provider response marker {marker} in {path}")
    for match in domain_like.finditer(text):
        domain = match.group(0).lower()
        if domain.endswith(".example.invalid"):
            continue
        if domain in {"doctype.html"}:
            continue
        if domain == "example.invalid":
            continue
        if any(domain.endswith("." + allowed) for allowed in ["example.invalid"]):
            continue
        # Ignore file-ish Markdown references that are not network domains.
        if domain.endswith((".md", ".json", ".html", ".marker")):
            continue
        raise SystemExit(f"Non-synthetic domain-like value in {path}: {domain}")


def embedded_json(text: str) -> dict:
    marker = '<script id="codexbar-fixture" type="application/json">'
    start = text.find(marker)
    if start < 0:
        raise ValueError("missing fixture script marker")
    start += len(marker)
    rest = text[start:]
    end = rest.find("</script>")
    if end < 0:
        raise ValueError("missing fixture script end")
    return json.loads(rest[:end])


for path in sorted(fixture_root.iterdir()):
    if not path.is_file():
        continue
    text = path.read_text(encoding="utf-8", errors="replace")
    check_text(path, text)
    if path.suffix == ".json":
        data = json.loads(text)
        if data.get("schemaVersion") != 1:
            raise SystemExit(f"{path} schemaVersion must be 1")
        if not str(data.get("fixtureId", "")).startswith("codex-web-"):
            raise SystemExit(f"{path} fixtureId must start with codex-web-")
    elif path.name in {
        "dashboard_success.html",
        "dashboard_login_required.html",
        "dashboard_account_mismatch.html",
    }:
        data = embedded_json(text)
        if data.get("schemaVersion") != 1:
            raise SystemExit(f"{path} embedded schemaVersion must be 1")
        if data.get("state") not in {"ok", "login_required", "account_mismatch"}:
            raise SystemExit(f"{path} embedded state is not expected")
    elif path.name == "dashboard_parse_error.html":
        try:
            embedded_json(text)
        except Exception:
            pass
        else:
            raise SystemExit(f"{path} should not contain parseable embedded fixture JSON")

print("web fixtures validated")
PY
