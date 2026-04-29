#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 /path/to/manifest.live-*.json-or-capture-dir" >&2
  exit 2
fi

TARGET="$1"

python3 - "$TARGET" <<'PY'
import json
import os
import re
import stat
import sys
from pathlib import Path

target = Path(sys.argv[1])
if target.is_dir():
    matches = sorted(target.glob("manifest.live-*.json"))
    if len(matches) != 1:
        raise SystemExit(f"Expected exactly one manifest.live-*.json in {target}, found {len(matches)}")
    manifest_path = matches[0]
else:
    manifest_path = target
capture_root = manifest_path.parent

if not manifest_path.is_file():
    raise SystemExit(f"Missing live capture manifest: {manifest_path}")

secret_patterns = [
    ("bearer_token", re.compile(r"(?i)\bBearer\s+(?!\[REDACTED_TOKEN\])\S+")),
    ("openai_api_key", re.compile(r"\bsk-[A-Za-z0-9._-]{8,}\b")),
    ("anthropic_token", re.compile(r"\bsk-ant-[A-Za-z0-9._-]{8,}\b")),
    ("github_token", re.compile(r"\b(?:ghp|gho|ghu|ghs|ghr|github_pat)_[A-Za-z0-9_]{12,}\b")),
    ("slack_token", re.compile(r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b")),
    ("google_api_key", re.compile(r"\bAIza[0-9A-Za-z_-]{20,}\b")),
    ("authorization_header", re.compile(r"(?im)^Authorization\s*:")),
    ("cookie_header", re.compile(r"(?im)^(Cookie|Set-Cookie)\s*:")),
    (
        "token_assignment",
        re.compile(
            r"(?i)\b(access[_-]?token|refresh[_-]?token|session[_-]?key|session[_-]?token|api[_-]?key|password|secret)\b\s*['\"]?\s*[:=](?!\s*['\"]?\[REDACTED_(TOKEN|SECRET)\]|\s*['\"]?null|\s*['\"]?false|\s*['\"]?true)\s*['\"]?"
        ),
    ),
    ("raw_home_path", re.compile(r"/home/(?!\[REDACTED_USER\])[^/\s\"']+")),
    ("raw_users_path", re.compile(r"/Users/(?!\[REDACTED_USER\])[^/\s\"']+")),
    ("local_share_path", re.compile(r"(?i)~[/\\]\.local[/\\]share[/\\]")),
    ("auth_json_path", re.compile(r"(?i)(^|[/\\])auth\.json\b")),
    ("browser_profile_path", re.compile(r"(?i)(Network/Cookies|Login Data|\.config/(google-chrome|chromium|BraveSoftware)|\.mozilla/firefox)")),
]
raw_email = re.compile(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")
identity_email_keys = {"accountemail", "signedinemail"}
allowed_categories = {
    "version",
    "config_validate",
    "config_dump",
    "usage_success",
    "usage_error",
    "cost_success",
    "cost_error",
    "unsupported_source",
    "invalid_provider",
}


def check_text(path: Path, text: str) -> None:
    for match in raw_email.finditer(text):
        value = match.group(0)
        if value.endswith("@example.invalid") or "***@" in value:
            continue
        raise SystemExit(f"Raw email-like value in {path}: {value}")
    for code, pattern in secret_patterns:
        if pattern.search(text):
            raise SystemExit(f"Potential secret pattern {code} in {path}")


def check_file_mode(path: Path) -> None:
    if os.name != "posix":
        return
    mode = stat.S_IMODE(path.stat().st_mode)
    if mode & 0o077:
        raise SystemExit(f"Live capture artifact is not private (expected 0600): {path} mode {oct(mode)}")


def check_identity_values(path: Path, value, trail: tuple[str, ...] = ()) -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            normalized_key = key.replace("_", "").replace("-", "").lower()
            child_trail = (*trail, key)
            if normalized_key in identity_email_keys:
                if child is None or child == "[REDACTED_EMAIL]":
                    pass
                elif isinstance(child, str) and "***@" in child:
                    pass
                else:
                    dotted = ".".join(child_trail)
                    raise SystemExit(f"Unredacted identity email value in {path}: {dotted}")
            check_identity_values(path, child, child_trail)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            check_identity_values(path, child, (*trail, str(index)))


def rel_path(value: str) -> Path:
    candidate = capture_root / value
    resolved = candidate.resolve()
    root_resolved = capture_root.resolve()
    if root_resolved not in (resolved, *resolved.parents):
        raise SystemExit(f"Manifest path escapes capture root: {value}")
    return candidate


manifest_text = manifest_path.read_text(encoding="utf-8")
check_text(manifest_path, manifest_text)
manifest = json.loads(manifest_text)
if manifest.get("schemaVersion") != 1:
    raise SystemExit("manifest.schemaVersion must be 1")
if not isinstance(manifest.get("fixtures"), list) or not manifest["fixtures"]:
    raise SystemExit("manifest.fixtures must be a non-empty list")

paths_to_check: set[Path] = {manifest_path}
seen_ids: set[str] = set()
for entry in manifest["fixtures"]:
    for key in [
        "fixtureId",
        "command",
        "argv",
        "upstreamVersion",
        "platform",
        "capturedAt",
        "exitCode",
        "timedOut",
        "stdoutPath",
        "stderrPath",
        "metadataPath",
        "expectedCategory",
        "redaction",
    ]:
        if key not in entry:
            raise SystemExit(f"Manifest entry missing {key}: {entry}")
    fixture_id = entry["fixtureId"]
    if fixture_id in seen_ids:
        raise SystemExit(f"Duplicate fixtureId: {fixture_id}")
    seen_ids.add(fixture_id)
    argv = entry["argv"]
    if not isinstance(argv, list) or not argv or argv[0] != "codexbar":
        raise SystemExit(f"Manifest argv must start with codexbar for {fixture_id}")
    if entry["expectedCategory"] not in allowed_categories:
        raise SystemExit(f"Unexpected capture category {entry['expectedCategory']} for {fixture_id}")
    redaction = entry["redaction"]
    if redaction.get("applied") is not True or redaction.get("policyVersion") != 1:
        raise SystemExit(f"Redaction metadata must be applied policy v1 for {fixture_id}")
    for path_key in ["stdoutPath", "stderrPath", "metadataPath"]:
        path = rel_path(entry[path_key])
        if not path.is_file():
            raise SystemExit(f"Missing manifest path for {fixture_id}: {path}")
        paths_to_check.add(path)
    metadata = json.loads(rel_path(entry["metadataPath"]).read_text(encoding="utf-8"))
    for metadata_key in [
        "fixtureId",
        "command",
        "argv",
        "upstreamVersion",
        "platform",
        "capturedAt",
        "exitCode",
        "timedOut",
    ]:
        if metadata.get(metadata_key) != entry.get(metadata_key):
            raise SystemExit(
                f"Metadata mismatch for {fixture_id}: {metadata_key} "
                f"{metadata.get(metadata_key)!r} != {entry.get(metadata_key)!r}"
            )
    metadata_redaction = metadata.get("redaction", {})
    if metadata_redaction.get("applied") is not True or metadata_redaction.get("policyVersion") != 1:
        raise SystemExit(f"Metadata redaction must be applied policy v1 for {fixture_id}")

for raw_path in capture_root.rglob("*.raw"):
    raise SystemExit(f"Raw file left in live capture directory: {raw_path}")

for path in sorted(paths_to_check):
    text = path.read_text(encoding="utf-8")
    check_text(path, text)
    check_file_mode(path)
    if path.suffix == ".json":
        try:
            value = json.loads(text)
        except json.JSONDecodeError as exc:
            raise SystemExit(f"JSON capture artifact does not parse: {path}: {exc}") from exc
        check_identity_values(path, value)
    print(f"Upstream CLI live capture valid: {path}")
PY
