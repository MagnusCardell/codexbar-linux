#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

REQUIRED_FILES=(
  "$ROOT/extension/metadata.json"
  "$ROOT/extension/extension.js"
  "$ROOT/extension/prefs.js"
  "$ROOT/extension/stylesheet.css"
  "$ROOT/extension/src/README.md"
  "$ROOT/extension/src/actions.js"
  "$ROOT/extension/src/constants.js"
  "$ROOT/extension/src/dbusClient.js"
  "$ROOT/extension/src/diagnosticsView.js"
  "$ROOT/extension/src/indicator.js"
  "$ROOT/extension/src/logger.js"
  "$ROOT/extension/src/meterBars.js"
  "$ROOT/extension/src/popover.js"
  "$ROOT/extension/src/providerCard.js"
  "$ROOT/extension/src/snapshotStore.js"
  "$ROOT/extension/src/state.js"
  "$ROOT/extension/src/time.js"
  "$ROOT/extension/tests/state.test.js"
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
shell_network_forbidden = re.compile(
    r"(gi://Soup(?:\?version=[0-9.]+)?|imports\.gi\.Soup|\bSoup\.|\bXMLHttpRequest\b|\bfetch\s*\(|"
    r"\bGio\.SocketClient\b|\bGio\.NetworkAddress\b|\bGio\.TcpConnection\b|"
    r"[\"']https?://)"
)
shell_subprocess_forbidden = re.compile(
    r"(\bGio\.Subprocess\b|\bGio\.SubprocessLauncher\b|\bGLib\.spawn_(?:async|sync|command_line_async|command_line_sync)\b|"
    r"\bGLib\.spawn_async_with_pipes\b|\bimports\.misc\.util\.spawn\b|\bUtil\.spawn\b|\bShell\.util_spawn\b)"
)
shell_cache_or_fixture_forbidden = re.compile(
    r"(XDG_CACHE_HOME|get_user_cache_dir|~/\.cache|/\.cache/|codexbar-linux/snapshot\.json|"
    r"daemon/fixtures|fixtures/snapshots|Network/Cookies|Login Data|\bcookies\.sqlite\b|\bCookies\b)"
)
shell_keyring_forbidden = re.compile(
    r"(gi://(Secret|Gcr|GnomeKeyring)(?:\?version=[0-9.]+)?|imports\.gi\.(Secret|Gcr|GnomeKeyring)|"
    r"\bSecret\.|\bGcr\.|\bGnomeKeyring\.|\bkeyring\b)"
)
shell_file_read_forbidden = re.compile(
    r"(\bGio\.File\.new_for_path\b|\bnew_for_path\s*\(|\bload_contents\s*\(|\bread\s*\(|\bread_async\s*\()"
)

violations = []
for path in sorted(root.rglob("*.js")):
    rel = path.relative_to(root)
    text = path.read_text(encoding="utf-8")
    is_test = len(rel.parts) > 1 and rel.parts[0] == "tests"
    if is_test:
        continue
    if rel.name == "prefs.js":
        if prefs_forbidden.search(text):
            violations.append(f"{rel}: Shell-only import used in prefs.js")
    else:
        if shell_forbidden.search(text):
            violations.append(f"{rel}: GTK/GDK/Adw import used in Shell-process code")
        if not is_test and shell_network_forbidden.search(text):
            violations.append(f"{rel}: provider network API used in Shell-process code")
        if shell_subprocess_forbidden.search(text):
            violations.append(f"{rel}: subprocess API used in Shell-process code")
        if not is_test and shell_cache_or_fixture_forbidden.search(text):
            violations.append(f"{rel}: cache, browser-profile, or fixture file path used in Shell-process code")
        if shell_keyring_forbidden.search(text):
            violations.append(f"{rel}: keyring API used in Shell-process code")
        if not is_test and shell_file_read_forbidden.search(text):
            violations.append(f"{rel}: filesystem read API used in Shell-process code")

dbus_client = (root / "src/dbusClient.js").read_text(encoding="utf-8")
for needle, reason in {
    "Gio.bus_watch_name": "watch daemon bus-name lifecycle",
    "Gio.bus_unwatch_name": "remove daemon bus-name watcher in destroy()",
    "_destroyed": "guard async D-Bus callbacks after destroy()",
    "GLib.Source.remove": "remove retry timers in destroy()",
}.items():
    if needle not in dbus_client:
        violations.append(f"src/dbusClient.js: missing lifecycle guard for {reason}")

extension_main = (root / "extension.js").read_text(encoding="utf-8")
if "const store = this._store;" not in extension_main or "this._store !== store || this._client !== client" not in extension_main:
    violations.append("extension.js: async D-Bus callbacks must be tied to the current enable() lifecycle")

state_js = (root / "src/state.js").read_text(encoding="utf-8")
for needle, reason in {
    "function isLocalhostHost": "reject localhost dashboard URLs",
    "authority.includes('@')": "reject dashboard URLs with credentials",
    "X-API-Key": "redact API key headers",
    "session[_-]?(?:token|key)": "redact session keys",
}.items():
    if needle not in state_js:
        violations.append(f"src/state.js: missing validation for {reason}")

if violations:
    raise SystemExit("\n".join(violations))
print("GJS Shell-process boundary smoke check passed")
PY

if command -v gjs >/dev/null 2>&1; then
  gjs -m "$ROOT/extension/tests/state.test.js"
else
  echo "gjs unavailable; extension pure JS tests not run"
fi
