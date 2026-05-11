#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_ROOT="$ROOT/daemon/fixtures/upstream-cli"
MANIFEST="$FIXTURE_ROOT/manifest.json"

validate_manifest() {
  local fixture_root="$1"
  local manifest="$2"
  local mode="$3"

python3 - "$fixture_root" "$manifest" "$mode" <<'PY'
import json
import re
import sys
from pathlib import Path

fixture_root = Path(sys.argv[1])
manifest_path = Path(sys.argv[2])
mode = sys.argv[3]
enforce_committed_coverage = mode == "committed"

if not manifest_path.is_file():
    raise SystemExit(f"Missing upstream CLI manifest: {manifest_path}")

manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
if manifest.get("schemaVersion") != 1:
    raise SystemExit("manifest.schemaVersion must be 1")
if not isinstance(manifest.get("fixtures"), list) or not manifest["fixtures"]:
    raise SystemExit("manifest.fixtures must be a non-empty list")

required_categories = {
    "unsupported_source",
    "invalid_provider",
    "missing_binary",
    "timeout_synthetic",
    "parse_error_synthetic",
}
usage_categories = {"usage_success", "usage_error"}
cost_categories = {"cost_success", "cost_error"}
metadata_categories = {"version", "config_validate", "config_dump"}
allowed_categories = required_categories | usage_categories | cost_categories | metadata_categories
required_committed_fixtures = {
    "version": {
        "argv": ["codexbar", "--version"],
        "expectedCategory": "version",
    },
    "config_validate": {
        "argv": ["codexbar", "config", "validate", "--format", "json", "--json-only"],
        "expectedCategory": "config_validate",
    },
    "usage_all_cli_default": {
        "argv": ["codexbar", "--format", "json", "--json-only", "--provider", "all", "--source", "cli"],
        "expectedCategory": "usage_error",
    },
    "usage_all_cli_subcommand": {
        "argv": ["codexbar", "usage", "--format", "json", "--json-only", "--provider", "all", "--source", "cli"],
        "expectedCategory": "usage_error",
    },
    "cost_all": {
        "argv": ["codexbar", "cost", "--format", "json", "--json-only", "--provider", "all"],
        "expectedCategory": "cost_success",
    },
    "cost_both_success": {
        "argv": ["codexbar", "cost", "--format", "json", "--json-only", "--provider", "both"],
        "expectedCategory": "cost_success",
    },
    "status_all_cli": {
        "argv": ["codexbar", "--format", "json", "--json-only", "--provider", "all", "--source", "cli", "--status"],
        "expectedCategory": "usage_error",
    },
    "usage_codex_cli_default": {
        "argv": ["codexbar", "--format", "json", "--json-only", "--provider", "codex", "--source", "cli"],
        "expectedCategory": "usage_success",
    },
    "usage_codex_cli_success": {
        "argv": ["codexbar", "--format", "json", "--json-only", "--provider", "codex", "--source", "cli"],
        "expectedCategory": "usage_success",
    },
    "usage_claude_cli_success": {
        "argv": ["codexbar", "--format", "json", "--json-only", "--provider", "claude", "--source", "cli"],
        "expectedCategory": "usage_success",
    },
    "source_oauth_semantic": {
        "argv": ["codexbar", "--format", "json", "--json-only", "--provider", "codex", "--source", "oauth"],
        "expectedCategory": "usage_success",
    },
    "source_api_semantic": {
        "argv": ["codexbar", "--format", "json", "--json-only", "--provider", "codex", "--source", "api"],
        "expectedCategory": "usage_success",
    },
    "usage_codex_cli_subcommand": {
        "argv": ["codexbar", "usage", "--format", "json", "--json-only", "--provider", "codex", "--source", "cli"],
        "expectedCategory": "usage_success",
    },
    "status_codex_cli": {
        "argv": ["codexbar", "--format", "json", "--json-only", "--provider", "codex", "--source", "cli", "--status"],
        "expectedCategory": "usage_success",
    },
    "unsupported_web_source": {
        "argv": ["codexbar", "--format", "json", "--json-only", "--provider", "all", "--source", "web"],
        "expectedCategory": "unsupported_source",
    },
    "unsupported_auto_source": {
        "argv": ["codexbar", "--format", "json", "--json-only", "--provider", "all", "--source", "auto"],
        "expectedCategory": "unsupported_source",
    },
    "invalid_provider": {
        "argv": ["codexbar", "--format", "json", "--json-only", "--provider", "__codexbar_linux_invalid_provider__"],
        "expectedCategory": "invalid_provider",
    },
}

secret_patterns = [
    ("bearer_token", re.compile(r"(?i)\bBearer\s+(?!\[REDACTED_TOKEN\])\S+")),
    ("openai_api_key", re.compile(r"\bsk-[A-Za-z0-9._-]{8,}\b")),
    ("anthropic_token", re.compile(r"\bsk-ant-[A-Za-z0-9._-]{8,}\b")),
    ("authorization_header", re.compile(r"(?im)^Authorization\s*:\s*(?!\[REDACTED_TOKEN\]\s*$).+")),
    ("cookie_header", re.compile(r"(?im)^(Cookie|Set-Cookie)\s*:\s*(?!\[REDACTED_COOKIE\]\s*$).+")),
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
    candidate = fixture_root / value
    resolved = candidate.resolve()
    root_resolved = fixture_root.resolve()
    if root_resolved not in (resolved, *resolved.parents):
        raise SystemExit(f"Manifest path escapes fixture root: {value}")
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
    if entry.get("synthetic") or entry.get("docDerived"):
        return
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
    if entry["command"] != "cost" or entry.get("synthetic"):
        return
    provider = option_value(argv, "--provider")
    if provider not in {"all", "both"}:
        raise SystemExit(
            f"Cost fixture must use --provider all or both for {entry['fixtureId']}"
        )
    if "--source" in argv:
        raise SystemExit(f"Cost fixture must not include --source for {entry['fixtureId']}")


categories = set()
seen_ids = set()
paths_to_check: set[Path] = {manifest_path}
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
    category = entry["expectedCategory"]
    if category not in allowed_categories:
        raise SystemExit(f"Unexpected fixture category {category} for {fixture_id}")
    if entry["timedOut"] and category in {"usage_success", "cost_success"}:
        raise SystemExit(f"Timed-out fixture cannot be categorized as success for {fixture_id}")
    if entry["exitCode"] != 0 and category in {"usage_success", "cost_success"}:
        raise SystemExit(f"Non-zero fixture cannot be categorized as success for {fixture_id}")
    categories.add(category)
    redaction = entry["redaction"]
    if redaction.get("applied") is not True or redaction.get("policyVersion") != 1:
        raise SystemExit(f"Redaction metadata must be applied policy v1 for {fixture_id}")
    for path_key in ["stdoutPath", "stderrPath", "metadataPath"]:
        path = rel_path(entry[path_key])
        if not path.is_file():
            raise SystemExit(f"Missing manifest path for {fixture_id}: {path}")
        paths_to_check.add(path)
    stdout_path = rel_path(entry["stdoutPath"])
    if category in {"usage_success", "cost_success"} and stdout_path.suffix != ".json":
        raise SystemExit(f"Success fixture stdout must be one JSON document for {fixture_id}: {stdout_path}")
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
        if metadata_key in metadata and metadata.get(metadata_key) != entry.get(metadata_key):
            raise SystemExit(
                f"Metadata mismatch for {fixture_id}: {metadata_key} "
                f"{metadata.get(metadata_key)!r} != {entry.get(metadata_key)!r}"
            )
        if (
            not entry.get("synthetic")
            and not entry.get("docDerived")
            and metadata_key not in metadata
        ):
            raise SystemExit(f"Live metadata missing {metadata_key} for {fixture_id}")
    timeout_seconds = metadata.get("timeoutSeconds")
    if not entry.get("synthetic") and not entry.get("docDerived") and (
        not isinstance(timeout_seconds, int) or timeout_seconds <= 0
    ):
        raise SystemExit(f"Metadata timeoutSeconds must be a positive integer for {fixture_id}")
    metadata_redaction = metadata.get("redaction", {})
    if metadata_redaction.get("applied") is not True or metadata_redaction.get("policyVersion") != 1:
        raise SystemExit(f"Metadata redaction must be applied policy v1 for {fixture_id}")

if enforce_committed_coverage:
    missing = sorted(required_categories - categories)
    if missing:
        raise SystemExit(f"Missing required fixture categories: {', '.join(missing)}")
    if not categories & usage_categories:
        raise SystemExit("Missing usage_success or usage_error fixture category")
    if not categories & cost_categories:
        raise SystemExit("Missing cost_success or cost_error fixture category")
    for fixture_id, expected in required_committed_fixtures.items():
        entry = next((candidate for candidate in manifest["fixtures"] if candidate["fixtureId"] == fixture_id), None)
        if entry is None:
            raise SystemExit(f"Missing required committed fixture: {fixture_id}")
        for key, expected_value in expected.items():
            if entry.get(key) != expected_value:
                raise SystemExit(
                    f"Required fixture {fixture_id} {key} mismatch: "
                    f"{entry.get(key)!r} != {expected_value!r}"
                )

referenced_paths = {path.resolve() for path in paths_to_check}
for raw_path in fixture_root.rglob("*.raw"):
    raise SystemExit(f"Raw upstream CLI capture artifact must not be committed: {raw_path}")

for path in fixture_root.rglob("*"):
    if not path.is_file() or path.suffix not in {".json", ".txt"}:
        continue
    if path.resolve() not in referenced_paths:
        raise SystemExit(f"Unreferenced upstream CLI fixture artifact: {path}")

for path in sorted(paths_to_check):
    text = path.read_text(encoding="utf-8")
    check_text(path, text)
    if path.suffix == ".json":
        try:
            value = json.loads(text)
        except json.JSONDecodeError as exc:
            raise SystemExit(f"JSON fixture does not parse: {path}: {exc}") from exc
        check_identity_values(path, value)
    try:
        display_path = path.relative_to(fixture_root.parent.parent.parent)
    except ValueError:
        display_path = path
    print(f"Upstream CLI fixture valid: {display_path}")
PY
}

validate_manifest "$FIXTURE_ROOT" "$MANIFEST" "committed"
