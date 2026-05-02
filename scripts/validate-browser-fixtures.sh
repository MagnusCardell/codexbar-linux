#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_ROOT="$ROOT/daemon/fixtures/browser/chromium"

python3 - "$FIXTURE_ROOT" <<'PY'
import json
import re
import sqlite3
import sys
from pathlib import Path

fixture_root = Path(sys.argv[1])

required = {
    "plaintext-default": "parse",
    "encrypted-fake": "parse",
    "corrupt-db": "corrupt",
    "locked-or-wal": "parse",
    "unsupported-schema": "unsupported_schema",
}

if not fixture_root.is_dir():
    raise SystemExit(f"Missing browser fixture root: {fixture_root}")

raw_email = re.compile(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")
secret_patterns = [
    ("home_path", re.compile(r"/home/[^/\s\"']+")),
    ("bearer_token", re.compile(r"(?i)\bBearer\s+\S+")),
    ("authorization_header", re.compile(r"(?im)^Authorization\s*:")),
    ("cookie_header", re.compile(r"(?im)^(Cookie|Set-Cookie)\s*:")),
    ("openai_api_key", re.compile(r"\bsk-[A-Za-z0-9._-]{8,}\b")),
    ("token_assignment", re.compile(r"(?i)\b(access[_-]?token|refresh[_-]?token|session[_-]?token|session[_-]?key|api[_-]?key|password|secret)\b\s*[:=]")),
]
real_provider_domains = [
    "openai.com",
    "chatgpt.com",
    "claude.ai",
    "anthropic.com",
    "cursor.com",
    "mistral.ai",
    "abacus.ai",
    "ollama.com",
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


for name, expected in required.items():
    directory = fixture_root / name
    if not directory.is_dir():
        raise SystemExit(f"Missing browser fixture directory: {directory}")
    metadata_path = directory / "metadata.json"
    if not metadata_path.is_file():
        raise SystemExit(f"Missing metadata: {metadata_path}")
    metadata_text = metadata_path.read_text(encoding="utf-8")
    check_text(metadata_path, metadata_text)
    metadata = json.loads(metadata_text)
    if metadata.get("schemaVersion") != 1:
        raise SystemExit(f"{metadata_path} schemaVersion must be 1")
    if metadata.get("fixtureId") != name:
        raise SystemExit(f"{metadata_path} fixtureId must match directory name")
    if metadata.get("expected") != expected:
        raise SystemExit(f"{metadata_path} expected must be {expected!r}")

    for path in directory.iterdir():
        if path.is_file():
            check_text(path, path.read_text(encoding="utf-8", errors="replace"))

    sql_path = directory / "schema.sql"
    if expected == "corrupt":
        if not (directory / "corrupt.txt").is_file():
            raise SystemExit(f"Corrupt fixture must include marker file: {directory}")
        if sql_path.exists():
            raise SystemExit(f"Corrupt fixture should not include parseable schema.sql: {directory}")
        continue

    if not sql_path.is_file():
        raise SystemExit(f"Missing SQL fixture: {sql_path}")
    sql = sql_path.read_text(encoding="utf-8")
    try:
        connection = sqlite3.connect(":memory:")
        connection.executescript(sql)
    except sqlite3.Error as exc:
        raise SystemExit(f"SQLite fixture does not parse: {sql_path}: {exc}") from exc
    finally:
        connection.close()

print("browser fixtures validated")
PY
