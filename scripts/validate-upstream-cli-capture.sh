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
    ("raw_payload_field", re.compile(r"(?i)\"raw(response|payload)\"")),
]
raw_email = re.compile(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")
identity_email_keys = {"accountemail", "signedinemail"}
account_id_key = re.compile(r"(account.*id|provider.*id|user.*id|customer.*id|team.*id|workspace.*id)$", re.IGNORECASE)
org_key = re.compile(r"(organization|org|workspace|team)(name)?$", re.IGNORECASE)
json_string_field = re.compile(r'"(?P<key>[^"]+)"\s*:\s*"(?P<value>[^"]*)"')
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
    for match in json_string_field.finditer(text):
        key = match.group("key")
        value = match.group("value")
        normalized_key = key.replace("_", "").replace("-", "").lower()
        if normalized_key in identity_email_keys:
            if value == "[REDACTED_EMAIL]" or "***@" in value:
                continue
            raise SystemExit(f"Raw identity email field in text artifact {path}: {key}")
        if account_id_key.search(key):
            if value.startswith("[REDACTED_") and value.endswith("]"):
                continue
            raise SystemExit(f"Raw account id field in text artifact {path}: {key}")
        if org_key.search(key):
            if value.startswith("[REDACTED_") and value.endswith("]"):
                continue
            raise SystemExit(f"Raw organization field in text artifact {path}: {key}")
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
            elif account_id_key.search(key):
                if child is None or child == "[REDACTED_ACCOUNT_ID]":
                    pass
                elif isinstance(child, str) and child.startswith("[REDACTED_") and child.endswith("]"):
                    pass
                else:
                    dotted = ".".join(child_trail)
                    raise SystemExit(f"Unredacted account id value in {path}: {dotted}")
            elif org_key.search(key):
                if child is None or child == "[REDACTED_ORG]":
                    pass
                elif isinstance(child, str) and child.startswith("[REDACTED_") and child.endswith("]"):
                    pass
                else:
                    dotted = ".".join(child_trail)
                    raise SystemExit(f"Unredacted organization value in {path}: {dotted}")
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


def option_value(argv: list[str], option: str) -> str | None:
    try:
        index = argv.index(option)
    except ValueError:
        return None
    next_index = index + 1
    if next_index >= len(argv):
        raise SystemExit(f"Missing value for {option} in argv: {argv}")
    return argv[next_index]


def check_targeted_probe_id(entry: dict, argv: list[str]) -> None:
    category = entry["expectedCategory"]
    command = entry["command"]
    if category not in {"usage_success", "usage_error"} or command not in {"usage", "status"}:
        return
    provider = option_value(argv, "--provider")
    source = option_value(argv, "--source")
    if provider is None or source is None:
        return
    if command == "status":
        expected_id = f"status_{provider}_{source}"
    elif len(argv) > 1 and argv[1] == "usage":
        expected_id = f"usage_{provider}_{source}_subcommand"
    else:
        expected_id = f"usage_{provider}_{source}_default"
    if entry["fixtureId"] != expected_id:
        raise SystemExit(
            f"Targeted provider/source fixture id mismatch: "
            f"{entry['fixtureId']} != {expected_id}"
        )


def check_cost_probe(entry: dict, argv: list[str]) -> None:
    if entry["command"] != "cost":
        return
    provider = option_value(argv, "--provider")
    if provider != "both":
        raise SystemExit(f"Cost capture must use --provider both for {entry['fixtureId']}")
    if "--source" in argv:
        raise SystemExit(f"Cost capture must not include --source for {entry['fixtureId']}")


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
    check_targeted_probe_id(entry, argv)
    check_cost_probe(entry, argv)
    if entry["expectedCategory"] not in allowed_categories:
        raise SystemExit(f"Unexpected capture category {entry['expectedCategory']} for {fixture_id}")
    if entry["timedOut"] and entry["expectedCategory"] in {"usage_success", "cost_success"}:
        raise SystemExit(f"Timed-out capture cannot be categorized as success for {fixture_id}")
    if entry["exitCode"] != 0 and entry["expectedCategory"] in {"usage_success", "cost_success"}:
        raise SystemExit(f"Non-zero capture cannot be categorized as success for {fixture_id}")
    redaction = entry["redaction"]
    if redaction.get("applied") is not True or redaction.get("policyVersion") != 1:
        raise SystemExit(f"Redaction metadata must be applied policy v1 for {fixture_id}")
    for path_key in ["stdoutPath", "stderrPath", "metadataPath"]:
        path = rel_path(entry[path_key])
        if not path.is_file():
            raise SystemExit(f"Missing manifest path for {fixture_id}: {path}")
        paths_to_check.add(path)
    stdout_path = rel_path(entry["stdoutPath"])
    if entry["expectedCategory"] in {"usage_success", "cost_success"} and stdout_path.suffix != ".json":
        raise SystemExit(f"Success capture stdout must be one JSON document for {fixture_id}: {stdout_path}")
    metadata = json.loads(rel_path(entry["metadataPath"]).read_text(encoding="utf-8"))
    for metadata_key in [
        "fixtureId",
        "command",
        "expectedCategory",
        "argv",
        "stdoutPath",
        "stderrPath",
        "metadataPath",
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
    timeout_seconds = metadata.get("timeoutSeconds")
    if not isinstance(timeout_seconds, int) or timeout_seconds <= 0:
        raise SystemExit(f"Metadata timeoutSeconds must be a positive integer for {fixture_id}")
    metadata_redaction = metadata.get("redaction", {})
    if metadata_redaction.get("applied") is not True or metadata_redaction.get("policyVersion") != 1:
        raise SystemExit(f"Metadata redaction must be applied policy v1 for {fixture_id}")

for raw_path in capture_root.rglob("*.raw"):
    raise SystemExit(f"Raw file left in live capture directory: {raw_path}")

artifact_paths: set[Path] = set()
for path in capture_root.rglob("*"):
    if not path.is_file() or path.suffix not in {".json", ".txt"}:
        continue
    artifact_paths.add(path.resolve())

referenced_paths = {path.resolve() for path in paths_to_check}
for path in sorted(artifact_paths - referenced_paths):
    raise SystemExit(f"Unreferenced live capture artifact: {path}")

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
