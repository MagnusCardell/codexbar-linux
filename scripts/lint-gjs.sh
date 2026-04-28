#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

REQUIRED_FILES=(
  "$ROOT/extension/metadata.json"
  "$ROOT/extension/extension.js"
  "$ROOT/extension/prefs.js"
  "$ROOT/extension/stylesheet.css"
  "$ROOT/extension/src/README.md"
)
for file in "${REQUIRED_FILES[@]}"; do
  if [[ ! -f "$file" ]]; then
    echo "Missing required extension skeleton file: ${file#$ROOT/}" >&2
    exit 1
  fi
done

python3 - "$ROOT/extension/metadata.json" "$ROOT/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml" <<'PY'
import json
import sys
from pathlib import Path

metadata_path = Path(sys.argv[1])
schema_path = Path(sys.argv[2])
metadata = json.loads(metadata_path.read_text(encoding="utf-8"))

expected = {
    "uuid": "codexbar-linux@codexbar.dev",
    "settings-schema": "org.gnome.shell.extensions.codexbar-linux",
}
for key, value in expected.items():
    if metadata.get(key) != value:
        raise SystemExit(f"metadata.json {key!r} must be {value!r}")

for key in ("name", "description", "version", "shell-version"):
    if key not in metadata:
        raise SystemExit(f"metadata.json missing {key!r}")
shell_versions = metadata["shell-version"]
if not isinstance(shell_versions, list) or not shell_versions:
    raise SystemExit("metadata.json shell-version must be a non-empty list")
if "46" not in shell_versions:
    raise SystemExit("metadata.json shell-version must include GNOME 46 support floor")
if any(not isinstance(version, str) or not version.isdigit() for version in shell_versions):
    raise SystemExit("metadata.json shell-version entries must be numeric strings")
if not schema_path.is_file():
    raise SystemExit(f"settings schema referenced by metadata is missing: {schema_path}")

print("GNOME extension metadata structurally valid")
PY

if command -v eslint >/dev/null 2>&1 && {
  [[ -f "$ROOT/.eslintrc" ]] || [[ -f "$ROOT/.eslintrc.js" ]] || [[ -f "$ROOT/.eslintrc.cjs" ]] || [[ -f "$ROOT/.eslintrc.json" ]] || [[ -f "$ROOT/eslint.config.js" ]] || [[ -f "$ROOT/eslint.config.mjs" ]] || [[ -f "$ROOT/eslint.config.cjs" ]]
}; then
  eslint "$ROOT/extension"
else
  echo "eslint unavailable or no repo config present; running structural/import-boundary checks only"
fi
python3 - "$ROOT/extension" <<'PY'
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
shell_forbidden = re.compile(r"(gi://(Gtk|Gdk|Adw)(?:\?version=[0-9.]+)?|imports\.gi\.(Gtk|Gdk|Adw))")
prefs_forbidden = re.compile(r"(gi://(St|Clutter|Meta|Shell)(?:\?version=[0-9.]+)?|resource:///org/gnome/shell/|imports\.gi\.(St|Clutter|Meta|Shell))")

violations = []
for path in sorted(root.rglob("*.js")):
    rel = path.relative_to(root)
    text = path.read_text(encoding="utf-8")
    if rel.name == "prefs.js":
        if prefs_forbidden.search(text):
            violations.append(f"{rel}: Shell-only import used in prefs.js")
    elif shell_forbidden.search(text):
        violations.append(f"{rel}: GTK/GDK/Adw import used in Shell-process code")

if violations:
    raise SystemExit("\n".join(violations))
print("GJS import boundary smoke check passed")
PY
