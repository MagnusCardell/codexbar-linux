#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - "$ROOT" <<'PY'
import json
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])

def read(rel):
    path = root / rel
    if not path.is_file():
        raise SystemExit(f"Missing release-gate file: {rel}")
    return path.read_text(encoding="utf-8")

def require(rel, needle):
    text = read(rel)
    if needle not in text:
        raise SystemExit(f"{rel} missing release-gate marker: {needle}")

for rel in [
    "scripts/package-root-smoke.sh",
    "scripts/gnome-matrix-smoke.sh",
]:
    text = read(rel)
    for marker in ("set -euo pipefail", "target/release-smoke", "evidence.json"):
        if marker not in text:
            raise SystemExit(f"{rel} missing smoke-helper marker: {marker}")

gnome_matrix_smoke = read("scripts/gnome-matrix-smoke.sh")
for marker in ("--require-ubuntu", "os-release", "ubuntuVersionVerified", "installed-dpkg-query", "installedVersion", "installedArchitecture", "dpkg-query -W"):
    if marker not in gnome_matrix_smoke:
        raise SystemExit(f"scripts/gnome-matrix-smoke.sh missing GNOME package metadata marker: {marker}")
if "extension version 1" not in gnome_matrix_smoke:
    raise SystemExit("scripts/gnome-matrix-smoke.sh missing installed metadata version marker")

for rel in (
    "scripts/build-deb.sh",
    "scripts/install-local.sh",
    "scripts/validate-packaging.sh",
):
    if "version must be 1" not in read(rel):
        raise SystemExit(f"{rel} missing extension metadata version guard")

require("scripts/package-root-smoke.sh", "--stage-only")
require("scripts/package-root-smoke.sh", "--noninteractive-sudo")
require("scripts/package-root-smoke.sh", "CODEXBAR_LINUX_PACKAGE_SMOKE_SUDO_NONINTERACTIVE")
require("scripts/package-root-smoke.sh", "sudo_args=(sudo -n)")
require("scripts/package-root-smoke.sh", "package-root-smoke: incomplete")
require("scripts/package-root-smoke.sh", "final-release-evidence: false")
completion_audit = read("scripts/release-completion-audit.sh")
for marker in (
    "05F-05K release objective audit",
    "--package-root",
    "--gnome-matrix",
    "--local-gate-log",
    "GJS lint assertions",
    "Ubuntu 26.04/GNOME 50 metadata/runtime validation",
    "validate-release-gate.sh",
    "validate-release-evidence.sh",
    "Current release candidate matches package-root evidence",
    "Local repository gate evidence matches current HEAD",
    "missing --local-gate-log final ./scripts/check.sh evidence",
    "dbus_scheduler_runs_startup_refresh_when_enabled",
    "dbus_scheduler_runs_interval_refresh_when_enabled",
    "settings_patch_advances_scheduler_revision",
    "failed_refresh_can_be_unwedged_without_daemon_restart",
    "app_refresh_uses_configured_provider_targets",
    "app_refresh_explicit_providers_override_settings",
    "upstream_cli_required_live_matrix_is_present",
    "package evidence candidateSha256 does not match current dist candidate",
    "package evidence candidate path does not match current dist candidate",
    "git working tree is not clean",
    "not complete",
    "complete",
):
    if marker not in completion_audit:
        raise SystemExit(f"scripts/release-completion-audit.sh missing completion-audit marker: {marker}")

gjs_lint = read("scripts/lint-gjs.sh")
for marker in (
    "reserved login-start key must not be exposed in prefs UI",
    "write daemon-owned settings through D-Bus",
    "display daemon info from D-Bus",
    "load selected provider state from D-Bus snapshot",
    "expose refresh interval control",
    "display upstream CLI availability",
    "expose provider settings group",
    "offer supported provider source choices",
    "write provider source through SetSettingsPatch",
    "write provider enabled state through SetSettingsPatch",
    "write refresh interval through SetSettingsPatch",
):
    if marker not in gjs_lint:
        raise SystemExit(f"scripts/lint-gjs.sh missing prefs UX assertion marker: {marker}")

evidence_validator = read("scripts/validate-release-evidence.sh")
for marker in (
    "set -euo pipefail",
    "--package-root",
    "--gnome-matrix",
    "--allow-development-gnome",
    "final GNOME evidence must have shellMajor=50",
    "Ubuntu 26.04, GNOME Shell 50",
    "final GNOME evidence must have osVersionId=26.04",
    "ubuntuVersionVerified",
    "candidate file sha256 does not match candidateSha256",
    "copy-candidate-to-tmp.txt",
    "candidate-byte-compare.txt",
    "payload lines must be",
    "Architecture:",
    "sudo-validate.txt",
    "sudo -v",
    "sudo -n -v",
    "cmp",
    "installed-dpkg-query.txt",
    "installedArchitecture",
    "GNOME evidence installedVersion does not match package-root installedVersion",
    "candidate-contents.txt",
    "usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml",
    "usr/share/man/man1/codexbar-linuxd.1.gz",
    "gnome-extensions-enable.txt",
    "enabled-extensions-after-enable.txt",
    "systemd-user-daemon-reload-after-install.txt",
    "systemd-user-daemon-reload-after-remove.txt",
    "systemd-user-daemon-reload-after-purge.txt",
    "gnome-extensions-disable.txt",
    "enabled-extensions-after-disable.txt",
    "removed-manpage-absent.txt",
    "purged-dpkg-query.txt",
    "contains forbidden content",
    "package-stage preflight evidence, not final root-backed package smoke",
    "has incomplete package-smoke marker",
    "installed-extension-metadata.txt",
    '"version": 1',
    "extension version 1",
    "enabled-extensions.txt",
    "gnome-shell-processes.txt",
    "gnome-shell-latest-process.txt",
    "last payload must be",
    "os-release.txt VERSION_ID does not match evidence osVersionId",
    "final GNOME evidence shellVersion must report GNOME Shell 50",
    "enabledExtensionVerified",
    "GetDaemonInfo",
    "GetSnapshot",
    "GetDiagnostics",
    "systemd-user-stop.txt",
    "missing evidence sidecar",
    "purgeAfterRemove",
    "removeVerified",
):
    if marker not in evidence_validator:
        raise SystemExit(f"scripts/validate-release-evidence.sh missing release-evidence marker: {marker}")

release_evidence_test = read("scripts/test-release-evidence.sh")
for marker in (
    "Release evidence validator tests passed",
    "Stage-only package evidence must not satisfy final release evidence",
    "GNOME 46 development evidence must not satisfy final release evidence",
    "GNOME evidence with non-26.04 Ubuntu runtime must not satisfy final release evidence",
    "GNOME evidence with mismatched shell major must not satisfy final release evidence",
    "GNOME evidence with mismatched shell-version sidecar must not satisfy final release evidence",
    "GNOME evidence with mismatched os-release sidecar must not satisfy final release evidence",
    "GNOME evidence with mismatched session-type sidecar must not satisfy final release evidence",
    "GNOME evidence without Wayland session must not satisfy final release evidence",
    "GNOME evidence without package path verification must not satisfy final release evidence",
    "GNOME evidence with mismatched metadata sidecar must not satisfy final release evidence",
    "GNOME evidence with mismatched metadata version sidecar must not satisfy final release evidence",
    "GNOME evidence with mismatched runtime sidecar must not satisfy final release evidence",
    "GNOME evidence with mismatched enabled-extension sidecar must not satisfy final release evidence",
    "GNOME evidence with mismatched installed package metadata must not satisfy final release evidence",
    "GNOME evidence with mismatched installed architecture must not satisfy final release evidence",
    "GNOME evidence with finalReleaseEvidence=false must not satisfy final release evidence",
    "GNOME evidence with false release-critical GNOME boolean must not satisfy final release evidence",
    "Package evidence with keepInstalled=true must not satisfy final release evidence",
    "Package evidence without purge must not satisfy final release evidence",
    "Package evidence with finalReleaseEvidence=false must not satisfy final release evidence",
    "Package evidence with incomplete marker must not satisfy final release evidence",
    "Package evidence with false release-critical package boolean must not satisfy final release evidence",
    "Package evidence with stale candidate sha must not satisfy final release evidence",
    "Package evidence with mismatched architecture sidecar must not satisfy final release evidence",
    "Package evidence with mismatched candidate copy sidecar must not satisfy final release evidence",
    "Package evidence with mismatched candidate byte-compare sidecar must not satisfy final release evidence",
    "Package evidence with mismatched sudo validation sidecar must not satisfy final release evidence",
    "Package evidence with mismatched installed package query sidecar must not satisfy final release evidence",
    "Package evidence with mismatched D-Bus sidecar must not satisfy final release evidence",
    "Package evidence with mismatched candidate contents sidecar must not satisfy final release evidence",
    "Package evidence with mismatched systemd daemon-reload sidecar must not satisfy final release evidence",
    "Package evidence with mismatched extension enable sidecar must not satisfy final release evidence",
    "Package evidence with mismatched enabled-extension state sidecar must not satisfy final release evidence",
    "Package evidence with still-enabled post-disable sidecar must not satisfy final release evidence",
    "Package evidence with mismatched remove absence sidecar must not satisfy final release evidence",
    "Package evidence with successful post-purge dpkg query must not satisfy final release evidence",
    "Package evidence with mismatched daemon-version sidecar must not satisfy final release evidence",
    "Failed package smoke must not exit successfully",
    "Failed package smoke must not write release evidence.json",
    "Completion audit without GNOME evidence must not satisfy final release evidence",
    "Stage-only package evidence must not satisfy completion audit",
    "Completion audit with stale package-root evidence must not satisfy latest .deb gate",
    "Completion audit with wrong package-root candidate path must not satisfy latest .deb gate",
    "Completion audit with dirty git worktree must not satisfy tag-prep gate",
    "Completion audit without local check log must not satisfy tag-prep gate",
    "Completion audit with stale local check log must not satisfy tag-prep gate",
    "Completion audit with local check log missing scheduler tests must not satisfy tag-prep gate",
    "05F-05K release objective audit: complete",
):
    if marker not in release_evidence_test:
        raise SystemExit(f"scripts/test-release-evidence.sh missing release-evidence test marker: {marker}")

check_sh = read("scripts/check.sh")
if "repository gate passed for HEAD" not in check_sh:
    raise SystemExit("scripts/check.sh missing current-HEAD repository gate success marker")

metadata = json.loads(read("extension/metadata.json"))
shell_versions = metadata.get("shell-version", [])
for version in ("46", "50"):
    if version not in shell_versions:
        raise SystemExit(f"extension/metadata.json missing GNOME Shell {version} marker")
if metadata.get("version") != 1:
    raise SystemExit("extension/metadata.json must keep extension metadata version 1")

required_markers = {
    "README.md": [
        "Ubuntu 26.04 LTS/GNOME 50 compatibility as a release gate",
        "Full Ubuntu 24.04/26.04 package smoke matrix sign-off",
        "Historical root-backed package install smoke evidence",
    ],
    "docs/ACCEPTANCE.md": [
        "GNOME metadata/runtime matrix includes GNOME 50",
        "compatibility-declared intermediate",
        "Daemon auto-refresh passes",
        "Provider off semantics pass",
        "Preferences UX passes",
        "repository gate passed for HEAD",
        "The reserved start-on-login preference is not shown as an active v0.1 control.",
        "Daemon-owned refresh interval and provider enablement/source configuration",
        "release sign-off until the real root-backed install/remove/purge path is",
    ],
    "docs/gnome-design-gate.md": [
        "daemon-owned refresh/provider settings",
        "not new Shell-owned GSettings keys",
        "No visible start-on-login control for v0.1",
    ],
    "docs/release-smoke-test.md": [
        "include the Ubuntu 26.04/GNOME 50 target",
        "Re-run `sudo apt remove codexbar-linux`",
        "explicitly recording Ubuntu 26.04/GNOME 50 metadata/runtime validation",
        "records checksum and",
        "byte-for-byte comparison sidecars",
        "sudo -v",
        "scripts/package-root-smoke.sh --stage-only",
        "incomplete.txt",
        "final-release-evidence: false",
        "scripts/release-completion-audit.sh",
        "--local-gate-log",
        "saved `./scripts/check.sh` log",
        "clean git working tree",
        "release-critical manifest booleans as evidence",
        "install-from-`/tmp`, sudo",
        "GNOME 50 metadata",
        "extension metadata `version: 1`",
        "finalReleaseEvidence: false",
        "finalReleaseEvidence: true",
    ],
    "docs/release-notes-0.1.0.md": [
        "Ubuntu 26.04 LTS/GNOME 50 compatibility remains a release gate",
        "Full Ubuntu 24.04/26.04 GNOME matrix coverage is not complete",
        "matrix must explicitly record Ubuntu 26.04/GNOME 50 metadata/runtime",
        "Real `sudo apt remove codexbar-linux`",
    ],
    "docs/release-candidate-gate.md": [
        "the final `v0.1.0`",
        "Historical package smoke evidence remains useful context, but it is not",
        "`evidence.json`",
        "candidate copy to `/tmp`",
        "byte comparison",
        "`sudo -v`",
        "docs/release-audit-05f-05k.md",
        "scripts/release-completion-audit.sh",
        "scripts/validate-release-evidence.sh",
        "scripts/package-root-smoke.sh",
        "scripts/package-root-smoke.sh --deb \"$candidate\" --stage-only",
        "Release-critical manifest booleans are validated as evidence claims",
        "install-from-`/tmp`, sudo",
        "GNOME 50 metadata, enabled",
        "smokeType: package-stage",
        "finalReleaseEvidence: true",
        "--local-gate-log",
        "saved `./scripts/check.sh` log",
        "scripts/gnome-matrix-smoke.sh --require-shell 50 --require-ubuntu 26.04 --require-package-path --require-wayland",
        "Required Root-Backed Package Smoke",
        "Required GNOME Matrix Evidence",
        "GNOME matrix `installedVersion` and `installedArchitecture` fields must",
        "extension metadata `version` must remain `1`",
        "save the log that ends with `repository gate passed for HEAD ...`",
        "Do not create `v0.1.0` while any item in this document is missing",
    ],
    "docs/upstream-cli-setup.md": [
        "v0.1 does not parse or migrate upstream CodexBar config files",
    ],
    "docs/release-audit-05f-05k.md": [
        "05F",
        "05F.1",
        "05G",
        "05H",
        "05I",
        "05J",
        "05K",
        "candidate=\"dist/codexbar-linux.deb\"",
        "candidate copy to `/tmp`",
        "byte comparison between the candidate and `/tmp`",
        "`sudo -v`",
        "Blocked until sudo-backed install/remove/purge evidence exists",
        "finalReleaseEvidence: false",
        "Blocked until run on an Ubuntu 26.04/GNOME Shell 50 package session",
        "scripts/release-completion-audit.sh",
        "--local-gate-log",
        "saved `./scripts/check.sh` log",
        "false release-critical manifest booleans",
        "install-from-`/tmp`, sudo",
        "GNOME 50 metadata, enabled",
        "extension metadata version 1",
    ],
}

for rel, markers in required_markers.items():
    for marker in markers:
        require(rel, marker)

for rel in [
    "README.md",
    "docs/ACCEPTANCE.md",
    "docs/release-smoke-test.md",
    "docs/release-candidate-gate.md",
    "docs/release-audit-05f-05k.md",
    "docs/release-notes-0.1.0.md",
    "docs/ROADMAP.md",
]:
    text = read(rel)
    forbidden_patterns = [
        r"\brelease[- ]ready\b",
        r"\bready for tag\b",
        r"\btagged v0\.1\.0\b",
        r"\bfinal release sign-off (?:passed|complete)\b",
        r"\bfull Ubuntu 24\.04/26\.04 GNOME matrix coverage is complete\b",
        r"\blatest \.deb root-backed smoke passed\b",
    ]
    for pattern in forbidden_patterns:
        if re.search(pattern, text, flags=re.IGNORECASE):
            raise SystemExit(f"{rel} contains forbidden release overclaim matching {pattern!r}")

for rel in [
    "README.md",
    "docs/release-smoke-test.md",
    "docs/release-candidate-gate.md",
    "docs/release-audit-05f-05k.md",
    "docs/release-notes-0.1.0.md",
]:
    text = read(rel)
    if re.search(r"scripts/package-root-smoke\.sh\s+--deb\s+[\"']?/tmp/", text):
        raise SystemExit(
            f"{rel} tells operators to pass the /tmp package copy to package-root-smoke; "
            "pass the dist/ candidate so completion audit can verify the latest artifact"
        )

for rel, patterns in {
    "docs/ACCEPTANCE.md": [
        r"start-on-login\s+preferences save and apply",
        r"Provider enablement/source configuration remains daemon-owned and is not\s+exposed as unsupported UI",
    ],
    "docs/gnome-design-gate.md": [
        r"Future daemon-owned provider, browser,\s+refresh",
        r"No provider enablement, refresh interval, diagnostics verbosity, or source\s+adapter settings in GSettings\.",
    ],
}.items():
    text = read(rel)
    for pattern in patterns:
        if re.search(pattern, text, flags=re.IGNORECASE):
            raise SystemExit(f"{rel} contains stale prefs-gate language matching {pattern!r}")

print("Release-candidate gate documentation is explicit about remaining blockers")
PY
