#!/usr/bin/env python3
"""Redact upstream CodexBar CLI stdout/stderr before fixture persistence."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


SENSITIVE_KEY = re.compile(
    r"(authorization|cookie|secret|token|password|api.?key|sessionkey|session[_-]?key|headers|profile.*path|auth.*path|raw)",
    re.IGNORECASE,
)
RAW_PAYLOAD_KEY = re.compile(r"^raw(?:[_-]?(?:response|payload))?$", re.IGNORECASE)
EMAIL_KEY = re.compile(r"(^|[_-])(?:account)?email$", re.IGNORECASE)
ACCOUNT_KEY = re.compile(r"(account.*id|provider.*id|user.*id|customer.*id|team.*id|workspace.*id)$", re.IGNORECASE)
ORG_KEY = re.compile(r"(organization|org|workspace|team)(name)?$", re.IGNORECASE)
EMAIL = re.compile(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")
HOME_PATH = re.compile(r"(?i)(/home/[^/\s\"']+|/Users/[^/\s\"']+|[A-Za-z]:\\Users\\[^\\\s\"']+)(?:[/\\][^\s\"']*)?")
LOCAL_SHARE_PATH = re.compile(r"(?i)~[/\\]\.local[/\\]share[/\\][^\s\"']+")
AUTH_JSON_PATH = re.compile(r"(?i)(?:[^\s\"']*[/\\])?auth\.json")
HEADER_SECRET = re.compile(r"(?im)\b(authorization|cookie|set-cookie|x-api-key)\s*:\s*[^\r\n]+")
BEARER = re.compile(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]+")
OPENAI_KEY = re.compile(r"\bsk-[A-Za-z0-9_-]{6,}\b")
GITHUB_TOKEN = re.compile(r"\b(?:ghp|gho|ghu|ghs|ghr|github_pat)_[A-Za-z0-9_]{12,}\b")
SLACK_TOKEN = re.compile(r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b")
GOOGLE_API_KEY = re.compile(r"\bAIza[0-9A-Za-z_-]{20,}\b")
JWT = re.compile(r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b")
SECRET_ASSIGNMENT = re.compile(
    r"(?i)\b(api[_-]?key|access[_-]?token|refresh[_-]?token|"
    r"session[_-]?key|session[_-]?token|token|secret|password)\b\s*[:=]\s*[\"']?[^\"'\s,}]+"
)
SECRET_QUERY = re.compile(
    r"(?i)([?&])(?:api[_-]?key|access[_-]?token|refresh[_-]?token|"
    r"session[_-]?key|session[_-]?token|token|secret|code|key)=[^&#\s\"']+"
)

FORBIDDEN = [
    ("authorization_header", re.compile(r"authorization\s*:", re.IGNORECASE)),
    ("set_cookie_header", re.compile(r"set-cookie", re.IGNORECASE)),
    ("cookie_header", re.compile(r"\bcookie\s*:", re.IGNORECASE)),
    ("bearer_token", re.compile(r"\bbearer\s+", re.IGNORECASE)),
    ("api_key", re.compile(r"\bsk-[A-Za-z0-9]", re.IGNORECASE)),
    ("github_token", GITHUB_TOKEN),
    ("slack_token", SLACK_TOKEN),
    ("google_api_key", GOOGLE_API_KEY),
    ("jwt", JWT),
    ("secret_assignment", SECRET_ASSIGNMENT),
    ("home_path", re.compile(r"(?i)(/home/(?!\[REDACTED_USER\])|/Users/(?!\[REDACTED_USER\])|[A-Za-z]:\\Users\\)")),
    ("local_share_path", re.compile(r"(?i)~[/\\]\.local[/\\]share[/\\]")),
    ("auth_json_path", re.compile(r"(?i)auth\.json")),
    ("browser_profile_path", re.compile(r"(?i)(\.config/chrom|\.mozilla/firefox|Network/Cookies|Login Data)")),
    ("raw_payload", re.compile(r"(?i)\"raw(payload|response)\"")),
]

ALLOWED_ENV = {
    "HOME",
    "PATH",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "NO_COLOR",
    "TERM",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
}


def now_rfc3339() -> str:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def decode(value: Any) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return str(value)


def cap_text(text: str, max_bytes: int) -> tuple[str, bool]:
    encoded = text.encode("utf-8", errors="replace")
    if len(encoded) <= max_bytes:
        return text, False
    capped = encoded[:max_bytes].decode("utf-8", errors="replace")
    return capped + "\n<redacted:truncated>", True


def mask_email(_match: re.Match[str]) -> str:
    return "[REDACTED_EMAIL]"


def redact_text(text: str) -> str:
    text = HEADER_SECRET.sub("[REDACTED_HEADER]", text)
    text = BEARER.sub("[REDACTED_TOKEN]", text)
    text = OPENAI_KEY.sub("[REDACTED_TOKEN]", text)
    text = GITHUB_TOKEN.sub("[REDACTED_TOKEN]", text)
    text = SLACK_TOKEN.sub("[REDACTED_TOKEN]", text)
    text = GOOGLE_API_KEY.sub("[REDACTED_TOKEN]", text)
    text = JWT.sub("[REDACTED_TOKEN]", text)
    text = SECRET_QUERY.sub(r"\1redacted=[REDACTED_TOKEN]", text)
    text = SECRET_ASSIGNMENT.sub("[REDACTED_SECRET]", text)
    text = LOCAL_SHARE_PATH.sub("[REDACTED_PATH]", text)
    text = AUTH_JSON_PATH.sub("[REDACTED_PATH]", text)
    text = HOME_PATH.sub("[REDACTED_PATH]", text)
    text = EMAIL.sub(mask_email, text)
    return text


def redact_json_stream_text(text: str) -> str | None:
    """Redact newline/multi-document JSON streams while preserving text output."""
    decoder = json.JSONDecoder()
    index = 0
    parts: list[str] = []
    decoded_any = False

    while index < len(text):
        whitespace = re.match(r"\s+", text[index:])
        if whitespace:
            end = index + whitespace.end()
            parts.append(text[index:end])
            index = end
            continue

        try:
            value, end = decoder.raw_decode(text, index)
        except json.JSONDecodeError:
            candidates = [
                position
                for position in (text.find("{", index + 1), text.find("[", index + 1))
                if position != -1
            ]
            if not candidates:
                parts.append(redact_text(text[index:]))
                break
            next_index = min(candidates)
            parts.append(redact_text(text[index:next_index]))
            index = next_index
            continue

        parts.append(json.dumps(redact_value(value), separators=(",", ":"), sort_keys=False))
        decoded_any = True
        index = end

    if not decoded_any:
        return None
    return "".join(parts)


def redact_value(value: Any) -> Any:
    if isinstance(value, dict):
        redacted: dict[str, Any] = {}
        for key, child in value.items():
            if RAW_PAYLOAD_KEY.search(key):
                redacted[_next_available_key(redacted, "redactedRawField")] = "[REDACTED_SECRET]"
            elif SENSITIVE_KEY.search(key):
                redacted[key] = "[REDACTED_SECRET]"
            elif EMAIL_KEY.search(key):
                redacted[key] = None if child is None else "[REDACTED_EMAIL]"
            elif ACCOUNT_KEY.search(key):
                redacted[key] = None if child is None else "[REDACTED_ACCOUNT_ID]"
            elif ORG_KEY.search(key):
                redacted[key] = None if child is None else "[REDACTED_ORG]"
            else:
                redacted[key] = redact_value(child)
        return redacted
    if isinstance(value, list):
        return [redact_value(child) for child in value]
    if isinstance(value, str):
        return redact_text(value)
    return value


def _next_available_key(value: dict[str, Any], base: str) -> str:
    if base not in value:
        return base
    suffix = 2
    while f"{base}{suffix}" in value:
        suffix += 1
    return f"{base}{suffix}"


def parse_output(text: str) -> tuple[str, Any]:
    stripped = text.strip()
    if not stripped:
        return "text", ""
    try:
        return "json", redact_value(json.loads(stripped))
    except json.JSONDecodeError:
        stream_text = redact_json_stream_text(text)
        if stream_text is not None:
            return "text", stream_text
        return "text", redact_text(text)


def status_from_exit(exit_code: int, timed_out: bool) -> str:
    if timed_out:
        return "timeout"
    if exit_code == 0:
        return "success"
    if exit_code == 127:
        return "missing_dependency"
    return "non_zero_exit"


def run_command(command: list[str], timeout_seconds: float, max_bytes: int) -> tuple[dict[str, Any], str, str]:
    env = {key: value for key, value in os.environ.items() if key in ALLOWED_ENV}
    if "PATH" not in env:
        env["PATH"] = "/usr/local/bin:/usr/bin:/bin"

    started = time.monotonic()
    timed_out = False
    try:
        completed = subprocess.run(
            command,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=False,
            timeout=timeout_seconds,
            env=env,
            check=False,
        )
        exit_code = completed.returncode
        stdout = decode(completed.stdout)
        stderr = decode(completed.stderr)
    except FileNotFoundError:
        exit_code = 127
        stdout = ""
        stderr = f"{command[0]} executable was not found on PATH"
    except subprocess.TimeoutExpired as exc:
        exit_code = 124
        timed_out = True
        stdout = decode(exc.stdout)
        stderr = decode(exc.stderr) or f"command timed out after {int(timeout_seconds)} seconds"

    duration_ms = int((time.monotonic() - started) * 1000)
    stdout, stdout_truncated = cap_text(stdout, max_bytes)
    stderr, stderr_truncated = cap_text(stderr, max_bytes)
    capture = {
        "command": [redact_text(part) for part in command],
        "exitCode": exit_code,
        "durationMs": duration_ms,
        "timedOut": timed_out,
        "status": status_from_exit(exit_code, timed_out),
        "stdoutTruncated": stdout_truncated,
        "stderrTruncated": stderr_truncated,
    }
    return capture, stdout, stderr


def envelope_from_capture(args: argparse.Namespace) -> dict[str, Any]:
    command = args.command
    if command and command[0] == "--":
        command = command[1:]
    if not command:
        raise SystemExit("--capture requires a command after --")
    if not args.category or not args.name:
        raise SystemExit("--capture requires --category and --name")

    capture, stdout, stderr = run_command(command, args.timeout, args.max_bytes)
    output_kind, output_value = parse_output(stdout)
    envelope: dict[str, Any] = {
        "sampleVersion": 1,
        "category": args.category,
        "name": args.name,
        "capturedAt": now_rfc3339(),
        "provenance": "live_capture_redacted",
        "capture": capture,
        "redaction": {
            "applied": True,
            "policyVersion": 1,
            "classes": ["secrets", "identity", "headers", "paths"],
        },
    }
    if output_kind == "json":
        envelope["stdoutJson"] = output_value
    else:
        envelope["stdoutText"] = output_value
    envelope["stderrText"] = redact_text(stderr)
    return envelope


def envelope_from_input(args: argparse.Namespace) -> Any:
    if not args.input:
        raise SystemExit("--input is required unless --capture is used")
    text = Path(args.input).read_text(encoding="utf-8")
    try:
        value = json.loads(text)
    except json.JSONDecodeError:
        return redact_json_stream_text(text) or redact_text(text)
    return redact_value(value)


def assert_safe(text: str) -> None:
    for code, pattern in FORBIDDEN:
        if pattern.search(text):
            raise SystemExit(f"redacted sample still contains forbidden content: {code}")
    for match in EMAIL.finditer(text):
        if "***@" not in match.group(0):
            raise SystemExit("redacted sample still contains a raw email address")


def write_output(path: Path, value: Any) -> None:
    if isinstance(value, str):
        text = value
        if text and not text.endswith("\n"):
            text += "\n"
    else:
        text = json.dumps(value, indent=2, sort_keys=False) + "\n"
    assert_safe(text)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    try:
        path.chmod(0o600)
    except OSError:
        pass


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture", action="store_true", help="run a CLI command and write a redacted sample envelope")
    parser.add_argument("--input", help="existing raw text or JSON file to redact")
    parser.add_argument("--output", required=True, help="redacted JSON output path")
    parser.add_argument("--category", choices=["usage", "cost", "errors", "status"])
    parser.add_argument("--name")
    parser.add_argument("--timeout", type=float, default=15.0)
    parser.add_argument("--max-bytes", type=int, default=65536)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()

    envelope = envelope_from_capture(args) if args.capture else envelope_from_input(args)
    write_output(Path(args.output), envelope)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
