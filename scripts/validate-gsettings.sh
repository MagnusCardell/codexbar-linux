#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCHEMA="$ROOT/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml"

python3 - "$SCHEMA" <<'PY'
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

path = Path(sys.argv[1])
if not path.is_file():
    raise SystemExit(f"Missing GSettings schema: {path}")

root = ET.parse(path).getroot()
schema = root.find("schema")
if schema is None:
    raise SystemExit("Missing <schema> in GSettings XML")
if schema.attrib.get("id") != "org.gnome.shell.extensions.codexbar-linux":
    raise SystemExit(f"Unexpected schema id: {schema.attrib.get('id')!r}")

expected = {
    "start-daemon-on-login": {"type": "b", "default": "true", "choices": None},
    "panel-mode": {"type": "s", "default": "'merged'", "choices": {"merged", "provider", "minimal"}},
    "reset-time-format": {"type": "s", "default": "'countdown'", "choices": {"countdown", "absolute", "both"}},
    "theme": {"type": "s", "default": "'system'", "choices": {"system", "compact", "high_contrast"}},
    "selected-provider": {"type": "s", "default": "''", "choices": None},
}
forbidden = {
    "refresh-interval",
    "refresh-interval-seconds",
    "provider-enablement",
    "browser-import-policy",
    "diagnostics-verbosity",
    "source-adapter",
    "preferred-source-adapter",
}

keys = {key.attrib.get("name"): key for key in schema.findall("key")}
if set(keys) != set(expected):
    raise SystemExit(f"Unexpected GSettings keys: got {sorted(keys)}, expected {sorted(expected)}")
if forbidden.intersection(keys):
    raise SystemExit(f"Daemon-owned keys must not be in GSettings: {sorted(forbidden.intersection(keys))}")

for name, spec in expected.items():
    key = keys[name]
    if key.attrib.get("type") != spec["type"]:
        raise SystemExit(f"{name}: unexpected type {key.attrib.get('type')!r}")
    default = key.findtext("default")
    if default != spec["default"]:
        raise SystemExit(f"{name}: unexpected default {default!r}")
    choices = key.find("choices")
    if spec["choices"] is None:
        if choices is not None:
            raise SystemExit(f"{name}: unexpected choices")
    else:
        got = {choice.attrib.get("value") for choice in choices.findall("choice")} if choices is not None else set()
        if got != spec["choices"]:
            raise SystemExit(f"{name}: unexpected choices {sorted(got)}")

print(f"GSettings schema structurally valid: {path}")
PY

if command -v glib-compile-schemas >/dev/null 2>&1; then
  glib-compile-schemas --strict --dry-run "$ROOT/schemas"
  echo "glib-compile-schemas strict dry-run passed"
else
  echo "glib-compile-schemas unavailable; structural GSettings validation only"
fi
