#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REQUIRED_SCHEMAS=(
  browser-import-options.schema.json
  browser-import-result.schema.json
  daemon-info.schema.json
  diagnostics.schema.json
  provider-event.schema.json
  refresh-options.schema.json
  refresh-result.schema.json
  settings-patch.schema.json
  settings.schema.json
  snapshot.schema.json
)
for schema in "${REQUIRED_SCHEMAS[@]}"; do
  if [[ ! -f "$ROOT/spec/$schema" ]]; then
    echo "Missing required schema: spec/$schema" >&2
    exit 1
  fi
done
mapfile -t SCHEMAS < <(find "$ROOT/spec" -maxdepth 1 -type f -name '*.schema.json' | sort)
if [[ ${#SCHEMAS[@]} -eq 0 ]]; then
  echo "No JSON schemas found under spec/" >&2
  exit 1
fi
python3 - "${SCHEMAS[@]}" <<'PY'
import json
import sys
try:
    from jsonschema import Draft202012Validator
except Exception as exc:
    raise SystemExit(f"python jsonschema package is required for schema validation: {exc}")

for path in sys.argv[1:]:
    with open(path, 'r', encoding='utf-8') as f:
        schema = json.load(f)
    Draft202012Validator.check_schema(schema)
    print(f"JSON schema valid: {path}")
PY
