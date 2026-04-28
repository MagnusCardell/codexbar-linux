#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

"$ROOT/scripts/validate-dbus.sh"
"$ROOT/scripts/validate-schemas.sh"
"$ROOT/scripts/validate-gsettings.sh"
"$ROOT/scripts/validate-packaging.sh"
"$ROOT/scripts/test-fixtures.sh"
"$ROOT/scripts/validate-upstream-cli-fixtures.sh"

if [[ ! -f "$ROOT/daemon/Cargo.toml" ]]; then
  echo "daemon/Cargo.toml is required after Task 00" >&2
  exit 1
fi
cargo fmt --manifest-path "$ROOT/daemon/Cargo.toml" -- --check
cargo clippy --manifest-path "$ROOT/daemon/Cargo.toml" --all-targets -- -D warnings
cargo test --manifest-path "$ROOT/daemon/Cargo.toml"
dbus-run-session -- cargo test --manifest-path "$ROOT/daemon/Cargo.toml" dbus_contract

"$ROOT/scripts/lint-gjs.sh"
