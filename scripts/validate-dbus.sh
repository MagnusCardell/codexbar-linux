#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
XML="$ROOT/spec/dbus-org.codexbar.Linux1.xml"
python3 -S - "$XML" <<'PY'
import re
import sys
import xml.etree.ElementTree as ET
path = sys.argv[1]
text = open(path, 'r', encoding='utf-8').read()
text = re.sub(r'<!DOCTYPE[^>]*>\s*', '', text, count=1)
root = ET.fromstring(text)
if root.attrib.get("name") != "/org/codexbar/Linux1":
    raise SystemExit(f"Unexpected D-Bus object path: {root.attrib.get('name')!r}")

interfaces = {node.attrib.get("name"): node for node in root.findall("interface")}
interface = interfaces.get("org.codexbar.Linux1")
if interface is None:
    raise SystemExit("Missing org.codexbar.Linux1 interface")

expected_methods = {
    "GetSnapshot": [("snapshot_json", "s", "out")],
    "Refresh": [("options_json", "s", "in"), ("refresh_id", "s", "out")],
    "GetDiagnostics": [("provider_id", "s", "in"), ("diagnostics_json", "s", "out")],
    "GetDaemonInfo": [("daemon_info_json", "s", "out")],
    "GetSettings": [("settings_json", "s", "out")],
    "SetSettingsPatch": [("patch_json", "s", "in"), ("settings_json", "s", "out")],
    "TestBrowserImport": [("options_json", "s", "in"), ("result_json", "s", "out")],
}
expected_signals = {
    "SnapshotChanged": [("snapshot_json", "s")],
    "RefreshStarted": [("refresh_id", "s")],
    "RefreshFinished": [("refresh_id", "s"), ("result_json", "s")],
    "ProviderChanged": [("provider_id", "s"), ("provider_event_json", "s")],
    "SettingsChanged": [("settings_json", "s")],
}

methods = {node.attrib.get("name"): node for node in interface.findall("method")}
signals = {node.attrib.get("name"): node for node in interface.findall("signal")}
if set(methods) != set(expected_methods):
    raise SystemExit(f"Unexpected D-Bus methods: got {sorted(methods)}, expected {sorted(expected_methods)}")
if set(signals) != set(expected_signals):
    raise SystemExit(f"Unexpected D-Bus signals: got {sorted(signals)}, expected {sorted(expected_signals)}")

for name, expected_args in expected_methods.items():
    got = [
        (arg.attrib.get("name"), arg.attrib.get("type"), arg.attrib.get("direction"))
        for arg in methods[name].findall("arg")
    ]
    if got != expected_args:
        raise SystemExit(f"Unexpected args for method {name}: got {got!r}, expected {expected_args!r}")

for name, expected_args in expected_signals.items():
    got = [
        (arg.attrib.get("name"), arg.attrib.get("type"))
        for arg in signals[name].findall("arg")
    ]
    if got != expected_args:
        raise SystemExit(f"Unexpected args for signal {name}: got {got!r}, expected {expected_args!r}")

print(f"D-Bus XML contract valid: {path}")
PY
