#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ ! -d "$ROOT/fixtures" ]]; then
  echo "fixtures/ not present yet; skipping fixture tests until Task 03"
  exit 0
fi
python3 - "$ROOT/fixtures" "$ROOT/spec/snapshot.schema.json" <<'PY'
import json
import re
import sys
from pathlib import Path
from jsonschema import Draft202012Validator, FormatChecker

root = Path(sys.argv[1])
snapshot_schema_path = Path(sys.argv[2])
snapshot_schema = json.loads(snapshot_schema_path.read_text(encoding='utf-8'))
snapshot_validator = Draft202012Validator(snapshot_schema, format_checker=FormatChecker())

required_snapshot_states = {
    "loading",
    "ok",
    "stale",
    "unauthenticated",
    "cookie_rejected",
    "missing_dependency",
    "provider_unavailable",
    "parse_error",
    "timeout",
    "error",
}
snapshot_dir = root / "snapshots"
if not snapshot_dir.is_dir():
    raise SystemExit("Missing required fixtures/snapshots directory")
missing = sorted(state for state in required_snapshot_states if not (snapshot_dir / f"{state}.json").is_file())
if missing:
    raise SystemExit(f"Missing required snapshot fixture states: {', '.join(missing)}")

secret_patterns = [
    re.compile(r'Authorization', re.I),
    re.compile(r'Set-Cookie', re.I),
    re.compile(r'Cookie:', re.I),
    re.compile(r'Bearer\s+[A-Za-z0-9._~+/=-]+', re.I),
    re.compile(r'\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b'),
    re.compile(r'(^|["\s])/(home|Users)/', re.I),
    re.compile(r'~/(\\.config|Library|AppData)', re.I),
    re.compile(r'(Login Data|Cookies|Network/Cookies)', re.I),
    re.compile(r'(api[_-]?key|access[_-]?token|refresh[_-]?token|session[_-]?token)\s*[:=]', re.I),
]
for path in sorted(root.rglob('*.json')):
    text = path.read_text(encoding='utf-8')
    data = json.loads(text)
    for pat in secret_patterns:
        if pat.search(text):
            raise SystemExit(f"Potential secret or raw email in fixture: {path}")
    if path.parent == snapshot_dir:
        errors = sorted(snapshot_validator.iter_errors(data), key=lambda err: list(err.path))
        if errors:
            first = errors[0]
            where = ".".join(str(part) for part in first.path) or "<root>"
            raise SystemExit(f"Snapshot fixture schema validation failed for {path} at {where}: {first.message}")
        expected_state = path.stem
        if expected_state in required_snapshot_states:
            states = {provider.get("state") for provider in data.get("providers", [])}
            if expected_state not in states:
                raise SystemExit(f"Snapshot fixture {path} must include provider state {expected_state}")
    print(f"Fixture JSON schema/redaction valid: {path}")
PY
