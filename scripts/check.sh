#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

"$ROOT/scripts/validate-dbus.sh"
"$ROOT/scripts/validate-schemas.sh"
"$ROOT/scripts/validate-gsettings.sh"
"$ROOT/scripts/validate-packaging.sh"
"$ROOT/scripts/validate-no-browser-web-surface.sh"
"$ROOT/scripts/test-fixtures.sh"
"$ROOT/scripts/validate-upstream-cli-fixtures.sh"
"$ROOT/scripts/test-upstream-cli-capture.sh"

if [[ ! -f "$ROOT/daemon/Cargo.toml" ]]; then
  echo "daemon/Cargo.toml is required after Task 00" >&2
  exit 1
fi
cargo fmt --manifest-path "$ROOT/daemon/Cargo.toml" -- --check
cargo clippy --manifest-path "$ROOT/daemon/Cargo.toml" --all-targets -- -D warnings
# Ignored live upstream CLI smoke tests are opt-in and intentionally excluded.
cargo test --manifest-path "$ROOT/daemon/Cargo.toml"
DBUS_TEST_HOME="$(mktemp -d "${TMPDIR:-/tmp}/codexbar-dbus-test.XXXXXX")"
cleanup_dbus_test_home() {
  rm -rf "$DBUS_TEST_HOME"
}
trap cleanup_dbus_test_home EXIT
env \
  CODEXBAR_LINUX_TEST_ISOLATED_DBUS=1 \
  XDG_CACHE_HOME="$DBUS_TEST_HOME/cache" \
  XDG_CONFIG_HOME="$DBUS_TEST_HOME/config" \
  XDG_DATA_HOME="$DBUS_TEST_HOME/data" \
  dbus-run-session -- cargo test --manifest-path "$ROOT/daemon/Cargo.toml" dbus_contract

"$ROOT/scripts/lint-gjs.sh"
