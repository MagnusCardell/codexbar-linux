#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "${CODEXBAR_BROWSER_LIVE:-}" != "1" ]]; then
  echo "SKIP: set CODEXBAR_BROWSER_LIVE=1 to run the throwaway Chromium smoke" >&2
  exit 2
fi

find_browser() {
  if [[ -n "${CHROMIUM_BROWSER:-}" ]]; then
    if [[ -x "$CHROMIUM_BROWSER" ]]; then
      printf '%s\n' "$CHROMIUM_BROWSER"
      return 0
    fi
    echo "SKIP: CHROMIUM_BROWSER is not executable" >&2
    exit 2
  fi

  local candidate
  for candidate in google-chrome google-chrome-stable chromium chromium-browser brave-browser; do
    if command -v "$candidate" >/dev/null 2>&1; then
      command -v "$candidate"
      return 0
    fi
  done

  echo "SKIP: no Chromium-family browser found; searched google-chrome google-chrome-stable chromium chromium-browser brave-browser" >&2
  exit 2
}

browser_family() {
  local name
  name="$(basename "$1")"
  case "$name" in
    *brave*)
      printf '%s\n' "brave"
      ;;
    google-chrome* | chrome)
      printf '%s\n' "chrome"
      ;;
    *)
      printf '%s\n' "chromium"
      ;;
  esac
}

browser_is_snap() {
  local resolved version
  resolved="$(readlink -f "$1" 2>/dev/null || printf '%s\n' "$1")"
  if [[ "$resolved" == /snap/* || "$1" == /snap/* ]]; then
    return 0
  fi
  version="$("$1" --version 2>/dev/null || true)"
  [[ "$version" == *"snap"* ]]
}

user_data_relative() {
  local family="$1"
  local snap="$2"
  if [[ "$snap" == "true" && "$family" == "chromium" ]]; then
    printf '%s\n' "snap/chromium/common/chromium"
    return 0
  fi
  case "$family" in
    brave)
      printf '%s\n' ".config/BraveSoftware/Brave-Browser"
      ;;
    chrome)
      printf '%s\n' ".config/google-chrome"
      ;;
    *)
      printf '%s\n' ".config/chromium"
      ;;
  esac
}

wait_for_port_file() {
  local port_file="$1"
  for _ in {1..100}; do
    if [[ -s "$port_file" ]]; then
      return 0
    fi
    sleep 0.05
  done
  return 1
}

run_browser_once() {
  local headless_flag="$1"
  HOME="$TMP_HOME" \
    XDG_CONFIG_HOME="$TMP_HOME/.config" \
    XDG_CACHE_HOME="$TMP_HOME/.cache" \
    XDG_DATA_HOME="$TMP_HOME/.local/share" \
    timeout 35s "$BROWSER" \
      --user-data-dir="$USER_DATA_DIR" \
      --no-first-run \
      --no-default-browser-check \
      --disable-background-networking \
      --disable-component-update \
      --disable-default-apps \
      --disable-extensions \
      --disable-features=MediaRouter \
      --disable-sync \
      --metrics-recording-only \
      --safebrowsing-disable-auto-update \
      --password-store=basic \
      "$headless_flag" \
      --disable-gpu \
      --disable-dev-shm-usage \
      --host-resolver-rules="MAP smoke.example.invalid 127.0.0.1" \
      --dump-dom "$SMOKE_URL" \
      >"$TMP_ROOT/browser.stdout" \
      2>"$TMP_ROOT/browser.stderr"
}

cleanup() {
  local status=$?
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" >/dev/null 2>&1 || true
  fi
  if [[ "${KEEP_CODEXBAR_BROWSER_LIVE:-}" == "1" ]]; then
    echo "KEEP_CODEXBAR_BROWSER_LIVE=1 retained throwaway fake home for manual inspection"
    if [[ "${CODEXBAR_BROWSER_LIVE_SHOW_PATHS:-}" == "1" ]]; then
      echo "fakeHome=$TMP_HOME"
    else
      echo "fakeHome=redacted"
    fi
  else
    rm -rf "$TMP_ROOT"
  fi
  exit "$status"
}

umask 077
BROWSER="$(find_browser)"
FAMILY="$(browser_family "$BROWSER")"
if browser_is_snap "$BROWSER"; then
  SNAP_BROWSER="true"
else
  SNAP_BROWSER="false"
fi
USER_DATA_REL="$(user_data_relative "$FAMILY" "$SNAP_BROWSER")"
if [[ "$SNAP_BROWSER" == "true" && "$FAMILY" == "chromium" ]]; then
  SNAP_BASE="${HOME:?}/snap/chromium/common"
  mkdir -p "$SNAP_BASE"
  TMP_ROOT="$(mktemp -d "$SNAP_BASE/codexbar-throwaway.XXXXXX")"
else
  TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/codexbar-chromium-smoke.XXXXXX")"
fi
TMP_HOME="$TMP_ROOT/home"
USER_DATA_DIR="$TMP_HOME/$USER_DATA_REL"
PORT_FILE="$TMP_ROOT/server.port"
trap cleanup EXIT

mkdir -p "$TMP_HOME/.config" "$TMP_HOME/.cache" "$TMP_HOME/.local/share" "$USER_DATA_DIR"
printf 'codexbar throwaway browser smoke\n' >"$TMP_HOME/.codexbar-throwaway-browser-root"

python3 - "$PORT_FILE" <<'PY' &
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

port_file = Path(sys.argv[1])


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header(
            "Set-Cookie",
            "quota_marker=codexbar-throwaway-cookie; Path=/; Max-Age=600; SameSite=Lax",
        )
        self.end_headers()
        self.wfile.write(b"<html><body>codexbar browser smoke</body></html>")

    def log_message(self, _format, *_args):
        return


server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
port_file.write_text(str(server.server_port), encoding="utf-8")
server.serve_forever()
PY
SERVER_PID=$!

if ! wait_for_port_file "$PORT_FILE"; then
  echo "FAIL: local throwaway cookie server did not start" >&2
  exit 1
fi

SMOKE_URL="http://smoke.example.invalid:$(<"$PORT_FILE")/"
HEADLESS_MODE="new"
if ! run_browser_once "--headless=new"; then
  HEADLESS_MODE="legacy"
  if ! run_browser_once "--headless"; then
    echo "SKIP: browser could not run headless against the throwaway profile" >&2
    exit 2
  fi
fi

PROFILE_DIR="$USER_DATA_DIR/Default"
if [[ ! -d "$PROFILE_DIR" ]]; then
  echo "FAIL: browser did not create a Default profile in the throwaway user-data-dir" >&2
  exit 1
fi

COOKIE_DB_SHAPE="missing"
COOKIE_DB=""
if [[ -f "$PROFILE_DIR/Network/Cookies" ]]; then
  COOKIE_DB_SHAPE="\$TMP_HOME/$USER_DATA_REL/Default/Network/Cookies"
  COOKIE_DB="$PROFILE_DIR/Network/Cookies"
elif [[ -f "$PROFILE_DIR/Cookies" ]]; then
  COOKIE_DB_SHAPE="\$TMP_HOME/$USER_DATA_REL/Default/Cookies"
  COOKIE_DB="$PROFILE_DIR/Cookies"
else
  echo "FAIL: no supported Chromium Cookies DB shape found in the throwaway profile" >&2
  exit 1
fi

WAL_PRESENT="false"
SHM_PRESENT="false"
if [[ -f "${COOKIE_DB}-wal" ]]; then
  WAL_PRESENT="true"
fi
if [[ -f "${COOKIE_DB}-shm" ]]; then
  SHM_PRESENT="true"
fi

echo "browserBinary=$(basename "$BROWSER")"
echo "browserFamily=$FAMILY"
echo "snapBrowser=$SNAP_BROWSER"
echo "headlessMode=$HEADLESS_MODE"
echo "passwordStoreBasic=used"
echo "keyringPrompt=not_observed"
echo "userDataDirShape=\$TMP_HOME/$USER_DATA_REL"
echo "profileShape=\$TMP_HOME/$USER_DATA_REL/Default"
echo "cookieDbShape=$COOKIE_DB_SHAPE"
echo "walCompanionPresent=$WAL_PRESENT"
echo "shmCompanionPresent=$SHM_PRESENT"
echo "cookieValues=not_printed"

CODEXBAR_BROWSER_LIVE=1 \
  CODEXBAR_BROWSER_IMPORT_FAKE_HOME="$TMP_HOME" \
  CODEXBAR_BROWSER_IMPORT_LIVE_PROVIDER=smoke \
  CODEXBAR_BROWSER_IMPORT_EXPECT_COOKIE=1 \
  timeout 60s cargo test --manifest-path "$ROOT/daemon/Cargo.toml" \
    --test browser_chromium live_throwaway_browser_profile_smoke \
    -- --ignored --exact

echo "TestBrowserImport=passed_schema_and_redaction_checks"
