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
import shutil
import subprocess
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

named_import = re.compile(
    r"import\s*\{(?P<names>[^}]*)\}\s*from\s*[\"'](?P<spec>\.{1,2}/[^\"']+)[\"']",
    re.S,
)
local_import = re.compile(
    r"import\s+(?:[^;]*?)\s+from\s*[\"'](?P<spec>\.{1,2}/[^\"']+)[\"']",
    re.S,
)
export_decl = re.compile(r"\bexport\s+(?:async\s+)?(?:function|class|const|let|var)\s+([A-Za-z_$][\w$]*)")
export_list = re.compile(r"\bexport\s*\{(?P<names>.*?)\}", re.S)
css_class_token = re.compile(r"\bcodexbar-[A-Za-z0-9_-]+")

def resolve_local_module(path, spec):
    target = (path.parent / spec).resolve()
    if target.suffix == "":
        target = target.with_suffix(".js")
    if target.is_dir():
        target = target / "index.js"
    try:
        target.relative_to(root.resolve())
    except ValueError:
        return None
    return target

def exported_names(text):
    names = set(export_decl.findall(text))
    for match in export_list.finditer(text):
        for part in match.group("names").split(","):
            item = part.strip()
            if not item:
                continue
            pieces = re.split(r"\s+as\s+", item)
            names.add(pieces[-1].strip())
    return names

def imported_names(block):
    names = []
    for part in block.split(","):
        item = part.strip()
        if not item:
            continue
        pieces = re.split(r"\s+as\s+", item)
        names.append(pieces[0].strip())
    return names

violations = []
js_files = sorted(root.rglob("*.js"))
js_texts = {path: path.read_text(encoding="utf-8") for path in js_files}
exports_by_path = {path.resolve(): exported_names(text) for path, text in js_texts.items()}
node = shutil.which("node")
for path in js_files:
    rel = path.relative_to(root)
    text = js_texts[path]
    if node:
        check = subprocess.run(
            [node, "--input-type=module", "--check"],
            input=text,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if check.returncode != 0:
            violations.append(f"{rel}: ES module parse check failed: {check.stderr.strip()}")
    for match in local_import.finditer(text):
        spec = match.group("spec")
        target = resolve_local_module(path, spec)
        if not target or not target.is_file():
            violations.append(f"{rel}: local import target is missing: {spec}")
    for match in named_import.finditer(text):
        spec = match.group("spec")
        target = resolve_local_module(path, spec)
        if not target or not target.is_file():
            continue
        available = exports_by_path.get(target.resolve(), set())
        for name in imported_names(match.group("names")):
            if name not in available:
                violations.append(f"{rel}: imports missing export {name} from {spec}")
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
    "looksUnsafePublicString(decodedValue)": "reject unsafe dashboard URL query/fragment values",
    "hasUnsafeUrlPath(text)": "reject unsafe dashboard URL path values",
    "hasExactKeys(result, REFRESH_RESULT_REQUIRED_KEYS": "validate refresh-result contract shape exactly",
}.items():
    if needle not in state_js:
        violations.append(f"src/state.js: missing validation for {reason}")

stylesheet = (root / "stylesheet.css").read_text(encoding="utf-8")
lower_stylesheet = stylesheet.lower()
for forbidden_color in ("#00ff00", "#0f0", "#00ff41", "#39ff14", " lime"):
    if forbidden_color in lower_stylesheet:
        violations.append(f"stylesheet.css: forbidden terminal/neon color remains: {forbidden_color.strip()}")
stale_selector_families = (
    "codexbar-panel-state-",
    "codexbar-diagnostics-line",
    "codexbar-meter-segment",
    "codexbar-provider-card",
    "codexbar-provider-header",
    "codexbar-provider-list",
    "codexbar-meter-list",
    "codexbar-meter-row-compact",
    "codexbar-secondary-section",
    "codexbar-credit-section",
    "codexbar-section-kicker",
    "codexbar-panel-detail",
    "codexbar-icon-button",
    "codexbar-button-content",
    "codexbar-button-icon",
    "codexbar-provider-meta",
    "codexbar-provider-status-box",
    "codexbar-state-pill",
    "codexbar-provider-glyph",
    "codexbar-terminal",
    "font-family: monospace",
)
for forbidden in stale_selector_families:
    if forbidden in stylesheet:
        violations.append(f"stylesheet.css: stale selector family remains: {forbidden}")
    for path, text in js_texts.items():
        rel = path.relative_to(root)
        if len(rel.parts) > 1 and rel.parts[0] == "tests":
            continue
        if forbidden in text:
            violations.append(f"{rel}: stale selector family remains: {forbidden}")
for path, text in js_texts.items():
    rel = path.relative_to(root)
    if len(rel.parts) > 1 and rel.parts[0] == "tests":
        continue
    for token in sorted(set(css_class_token.findall(text))):
        if token == "codexbar-linux" or token.endswith("-"):
            continue
        if f".{token}" not in stylesheet:
            violations.append(f"{rel}: emitted class lacks stylesheet selector: {token}")
for selector in (
    ".codexbar-panel-content",
    ".codexbar-panel-status-dot",
    ".codexbar-state-ok .codexbar-panel-icon",
    ".codexbar-provider-strip",
    ".codexbar-provider-strip-item",
    ".codexbar-provider-strip-item-selected",
    ".codexbar-provider-strip-item-dimmed",
    ".codexbar-provider-strip-overflow",
    ".codexbar-provider-strip-empty",
    ".codexbar-divider",
    ".codexbar-selected-provider",
    ".codexbar-selected-provider-title",
    ".codexbar-selected-provider-heading",
    ".codexbar-provider-name",
    ".codexbar-provider-plan",
    ".codexbar-provider-state-note",
    ".codexbar-stale",
    ".codexbar-severity-ok",
    ".codexbar-severity-warning",
    ".codexbar-severity-loading",
    ".codexbar-severity-error",
    ".codexbar-usage-sections",
    ".codexbar-usage-section",
    ".codexbar-action-section",
    ".codexbar-cost-section",
    ".codexbar-cost-row",
    ".codexbar-cost-label",
    ".codexbar-cost-value",
    ".codexbar-meter-fill",
    ".codexbar-meter-fill-ok",
    ".codexbar-meter-fill-warning",
    ".codexbar-meter-fill-danger",
    ".codexbar-meter-fill-unknown",
    ".codexbar-meter-ok",
    ".codexbar-meter-detail-row",
    ".codexbar-meter-detail-left",
    ".codexbar-meter-detail-right",
    ".codexbar-diagnostics-row",
    ".codexbar-diagnostics-row-loaded",
    ".codexbar-diagnostics-row-collapsed",
    ".codexbar-diagnostics-detail",
    ".codexbar-diagnostics-title",
    ".codexbar-diagnostic-detail-list",
    ".codexbar-diagnostic-detail",
    ".codexbar-diagnostic-line",
    ".codexbar-section-header",
    ".codexbar-action-row",
    ".codexbar-menu-item",
):
    if selector not in stylesheet:
        violations.append(f"stylesheet.css: missing selector for emitted class {selector}")

provider_card_js = (root / "src/providerCard.js").read_text(encoding="utf-8")
if "sectionTitle(" in provider_card_js and not re.search(r"\b(?:function\s+sectionTitle|(?:const|let|var)\s+sectionTitle\s*=)", provider_card_js):
    violations.append("src/providerCard.js: sectionTitle() is called but no local helper is defined")
if "codexbar-state-pill" in provider_card_js:
    violations.append("src/providerCard.js: selected provider surface must not render a status pill")
if "row.shortLabel" in provider_card_js:
    violations.append("src/providerCard.js: popover must not render provider short-code glyphs")
if "'Open'" in provider_card_js or '"Open"' in provider_card_js:
    violations.append("src/providerCard.js: visible dashboard action must not use generic Open wording")
for removed_action_label in ("Usage Dashboard", "Status Page"):
    if removed_action_label in provider_card_js:
        violations.append(f"src/providerCard.js: provider web/dashboard action label remains: {removed_action_label!r}")
for action_label in ("Load diagnostics", "Copy diagnostics"):
    if action_label not in (root / "src/diagnosticsView.js").read_text(encoding="utf-8"):
        violations.append(f"src/diagnosticsView.js: missing diagnostics action label {action_label!r}")
if "createDiagnosticsButton" not in provider_card_js:
    violations.append("src/providerCard.js: missing secondary Load diagnostics action")
if "dashboardUrl: safeUrl(provider?.dashboardUrl || '')" in state_js:
    violations.append("src/state.js: provider dashboard URL must not be exposed in the normal view model")
if "statusPageUrl: safeUrl(provider?.status?.url || '')" in state_js:
    violations.append("src/state.js: provider status URL must not be exposed in the normal view model")

launch_users = []
for path in sorted(root.rglob("*.js")):
    rel = path.relative_to(root)
    if "launch_default_for_uri" in path.read_text(encoding="utf-8"):
        launch_users.append(str(rel))
if any(path != "src/actions.js" for path in launch_users):
    violations.append(f"Gio.AppInfo.launch_default_for_uri must only be used in src/actions.js, got {launch_users}")
actions_js = (root / "src/actions.js").read_text(encoding="utf-8")
if "src/actions.js" in launch_users and ("const safe = safeUrl(url);" not in actions_js or "launch_default_for_uri(safe" not in actions_js):
    violations.append("src/actions.js: URI launches must pass through safeUrl()")

if violations:
    raise SystemExit("\n".join(violations))
print("GJS Shell-process boundary smoke check passed")
PY

if command -v gjs >/dev/null 2>&1; then
  gjs -m "$ROOT/extension/tests/state.test.js"
else
  if [[ "${CI:-}" == "true" ]]; then
    echo "gjs unavailable in CI; extension pure JS tests must run" >&2
    exit 1
  fi
  echo "gjs unavailable; extension pure JS tests not run"
fi
