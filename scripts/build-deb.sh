#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_NAME="codexbar-linux"
EXTENSION_UUID="codexbar-linux@codexbar.dev"
SCHEMA_ID="org.gnome.shell.extensions.codexbar-linux"
DIST_DIR="${DIST_DIR:-$ROOT/dist}"
BUILD_ROOT="$ROOT/target/debian/$PACKAGE_NAME"
PKG_ROOT="$BUILD_ROOT/pkgroot"
MODE="build"

usage() {
  cat <<'EOF'
Usage: scripts/build-deb.sh [--check] [--output-dir DIR]

Build a local development Debian package for CodexBar GNOME.

Options:
  --check           Validate package inputs without compiling or building.
  --output-dir DIR  Write the .deb to DIR instead of ./dist.
  -h, --help        Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --check)
      MODE="check"
      shift
      ;;
    --output-dir)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "Missing argument for --output-dir" >&2
        exit 2
      fi
      DIST_DIR="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required tool: $1" >&2
    exit 1
  fi
}

require_file() {
  if [[ ! -f "$ROOT/$1" ]]; then
    echo "Missing required packaging input: $1" >&2
    exit 1
  fi
}

package_version() {
  sed -n "1s/^$PACKAGE_NAME (\([^)]*\)).*/\1/p" "$ROOT/packaging/debian/changelog"
}

release_rustflags() {
  local flags=()
  if [[ -n "${RUSTFLAGS:-}" ]]; then
    flags+=("$RUSTFLAGS")
  fi
  flags+=("--remap-path-prefix=$ROOT=codexbar-linux")
  if [[ -n "${HOME:-}" ]]; then
    flags+=("--remap-path-prefix=$HOME=home")
  fi
  local cargo_home="${CARGO_HOME:-}"
  if [[ -z "$cargo_home" && -n "${HOME:-}" ]]; then
    cargo_home="$HOME/.cargo"
  fi
  if [[ -n "$cargo_home" ]]; then
    flags+=("--remap-path-prefix=$cargo_home=cargo-home")
  fi
  local rustup_home="${RUSTUP_HOME:-}"
  if [[ -z "$rustup_home" && -n "${HOME:-}" ]]; then
    rustup_home="$HOME/.rustup"
  fi
  if [[ -n "$rustup_home" ]]; then
    flags+=("--remap-path-prefix=$rustup_home=rustup-home")
  fi
  printf '%s\n' "${flags[*]}"
}

validate_no_build_path_leaks() {
  local binary="$1"
  local leaks="" leak_label="" cargo_home rustup_home
  cargo_home="${CARGO_HOME:-}"
  if [[ -z "$cargo_home" && -n "${HOME:-}" ]]; then
    cargo_home="$HOME/.cargo"
  fi
  rustup_home="${RUSTUP_HOME:-}"
  if [[ -z "$rustup_home" && -n "${HOME:-}" ]]; then
    rustup_home="$HOME/.rustup"
  fi

  for marker in "$ROOT" "$BUILD_ROOT" "$PKG_ROOT" "$cargo_home" "$rustup_home" "${HOME:-}"; do
    if [[ -z "$marker" ]]; then
      continue
    fi
    leaks="$(strings -a "$binary" | LC_ALL=C grep -F "$marker" || true)"
    if [[ -n "$leaks" ]]; then
      case "$marker" in
        "$ROOT") leak_label="source-root" ;;
        "$BUILD_ROOT") leak_label="package-build-root" ;;
        "$PKG_ROOT") leak_label="package-staging-root" ;;
        "$cargo_home") leak_label="cargo-home" ;;
        "$rustup_home") leak_label="rustup-home" ;;
        "${HOME:-}") leak_label="home" ;;
        *) leak_label="private-path" ;;
      esac
      break
    fi
  done
  if [[ -z "$leaks" ]]; then
    leaks="$(strings -a "$binary" | LC_ALL=C grep -E '/Users/|/private/var/|target/debian|daemon/target/release' || true)"
    if [[ -n "$leaks" ]]; then
      leak_label="path-pattern"
    fi
  fi
  if [[ -n "$leaks" ]]; then
    local leak_count
    leak_count="$(printf '%s\n' "$leaks" | sed '/^$/d' | wc -l | awk '{print $1}')"
    echo "Packaged daemon contains build-host path strings; release build path remapping failed." >&2
    echo "Redacted leak class: ${leak_label:-private-path}; matching strings: $leak_count" >&2
    exit 1
  fi
}

check_inputs() {
  require_tool cargo
  require_tool dpkg
  require_tool dpkg-deb
  require_tool awk
  require_tool du
  require_tool find
  require_tool glib-compile-schemas
  require_tool gzip
  require_tool install
  require_tool mktemp
  require_tool python3
  require_tool sed
  require_tool strip
  require_tool strings

  require_file "daemon/Cargo.toml"
  require_file "scripts/codexbar-linux-setup"
  require_file "packaging/debian/changelog"
  require_file "packaging/debian/control"
  require_file "packaging/debian/postinst"
  require_file "packaging/debian/postrm"
  require_file "packaging/debian/copyright"
  require_file "packaging/man/codexbar-linuxd.1"
  require_file "packaging/dbus/org.codexbar.Linux1.service"
  require_file "packaging/systemd/codexbar-linuxd.service"
  require_file "extension/metadata.json"
  require_file "extension/extension.js"
  require_file "extension/prefs.js"
  require_file "extension/stylesheet.css"
  require_file "schemas/$SCHEMA_ID.gschema.xml"
  require_file "README.md"
  require_file "LICENSE"
  require_file "docs/gnome-smoke-test.md"
  require_file "docs/release-smoke-test.md"
  require_file "docs/release-notes-0.1.0.md"

  if [[ -z "$(package_version)" ]]; then
    echo "Could not parse package version from packaging/debian/changelog" >&2
    exit 1
  fi

  grep -Fx "Exec=/usr/bin/codexbar-linuxd" "$ROOT/packaging/dbus/org.codexbar.Linux1.service" >/dev/null
  grep -Fx "SystemdService=codexbar-linuxd.service" "$ROOT/packaging/dbus/org.codexbar.Linux1.service" >/dev/null
  grep -Fx "ExecStart=/usr/bin/codexbar-linuxd" "$ROOT/packaging/systemd/codexbar-linuxd.service" >/dev/null
  grep -Fx "Type=dbus" "$ROOT/packaging/systemd/codexbar-linuxd.service" >/dev/null
  grep -Fx "BusName=org.codexbar.Linux1" "$ROOT/packaging/systemd/codexbar-linuxd.service" >/dev/null

  python3 - "$ROOT" "$EXTENSION_UUID" "$SCHEMA_ID" <<'PY'
import json
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

root = Path(sys.argv[1])
expected_uuid = sys.argv[2]
expected_schema = sys.argv[3]
metadata = json.loads((root / "extension/metadata.json").read_text(encoding="utf-8"))
if metadata.get("uuid") != expected_uuid:
    raise SystemExit("extension metadata UUID does not match package extension path")
if metadata.get("settings-schema") != expected_schema:
    raise SystemExit("extension metadata settings-schema does not match packaged schema")
if metadata.get("version") != 1:
    raise SystemExit("extension metadata version must be 1 for the v0.1 package")
if "46" not in metadata.get("shell-version", []):
    raise SystemExit("extension metadata must include GNOME Shell 46 support")
if "50" not in metadata.get("shell-version", []):
    raise SystemExit("extension metadata must include GNOME Shell 50 validation target")
schema = ET.parse(root / "schemas" / f"{expected_schema}.gschema.xml")
ids = {node.attrib.get("id") for node in schema.findall(".//schema")}
if expected_schema not in ids:
    raise SystemExit("GSettings schema id does not match extension metadata")
PY

  glib-compile-schemas --strict --dry-run "$ROOT/schemas"
}

install_extension_files() {
  local ext_dir="$PKG_ROOT/usr/share/gnome-shell/extensions/$EXTENSION_UUID"
  install -d -m 0755 "$ext_dir/src"
  for rel_file in metadata.json extension.js prefs.js stylesheet.css; do
    install -m 0644 "$ROOT/extension/$rel_file" "$ext_dir/$rel_file"
  done
  for src_file in "$ROOT"/extension/src/*.js; do
    [[ -e "$src_file" ]] || continue
    install -m 0644 "$src_file" "$ext_dir/src/$(basename "$src_file")"
  done
}

write_control_file() {
  local version="$1"
  local architecture="$2"
  local installed_size="$3"
  install -d -m 0755 "$PKG_ROOT/DEBIAN"
  cat > "$PKG_ROOT/DEBIAN/control" <<EOF
Package: $PACKAGE_NAME
Version: $version
Section: gnome
Priority: optional
Architecture: $architecture
Maintainer: CodexBar GNOME Maintainers <maintainers@example.invalid>
Installed-Size: $installed_size
Depends: dbus-user-session | dbus-session-bus, gir1.2-adw-1, gir1.2-gtk-4.0, gnome-shell (>= 46), libgcc-s1, libglib2.0-bin, systemd, libc6
Description: Native GNOME companion for upstream CodexBar usage snapshots
 CodexBar GNOME installs a user-scoped Rust daemon, D-Bus session
 activation, a systemd user unit, GSettings schema, and GNOME Shell extension
 assets for the upstream-CLI-only CodexBar data path. It includes a user-run
 setup helper for activation checks and does not enable the extension
 automatically.
EOF
}

stage_package() {
  local version="$1"
  case "$BUILD_ROOT" in
    "$ROOT"/target/debian/*)
      rm -rf "$BUILD_ROOT"
      ;;
    *)
      echo "Refusing to remove unexpected build root: $BUILD_ROOT" >&2
      exit 1
      ;;
  esac

  install -Dm755 "$ROOT/daemon/target/release/codexbar-linuxd" "$PKG_ROOT/usr/bin/codexbar-linuxd"
  strip --strip-unneeded "$PKG_ROOT/usr/bin/codexbar-linuxd"
  validate_no_build_path_leaks "$PKG_ROOT/usr/bin/codexbar-linuxd"
  install -Dm755 "$ROOT/scripts/codexbar-linux-setup" "$PKG_ROOT/usr/bin/codexbar-linux-setup"
  install -Dm644 "$ROOT/packaging/dbus/org.codexbar.Linux1.service" \
    "$PKG_ROOT/usr/share/dbus-1/services/org.codexbar.Linux1.service"
  install -Dm644 "$ROOT/packaging/systemd/codexbar-linuxd.service" \
    "$PKG_ROOT/usr/lib/systemd/user/codexbar-linuxd.service"
  install -Dm644 "$ROOT/schemas/$SCHEMA_ID.gschema.xml" \
    "$PKG_ROOT/usr/share/glib-2.0/schemas/$SCHEMA_ID.gschema.xml"
  install_extension_files

  install -Dm644 "$ROOT/README.md" "$PKG_ROOT/usr/share/doc/$PACKAGE_NAME/README.md"
  install -Dm644 "$ROOT/LICENSE" "$PKG_ROOT/usr/share/doc/$PACKAGE_NAME/LICENSE"
  install -Dm644 "$ROOT/docs/gnome-smoke-test.md" "$PKG_ROOT/usr/share/doc/$PACKAGE_NAME/gnome-smoke-test.md"
  install -Dm644 "$ROOT/docs/release-smoke-test.md" "$PKG_ROOT/usr/share/doc/$PACKAGE_NAME/release-smoke-test.md"
  install -Dm644 "$ROOT/docs/release-notes-0.1.0.md" "$PKG_ROOT/usr/share/doc/$PACKAGE_NAME/release-notes-0.1.0.md"
  install -Dm644 "$ROOT/packaging/debian/copyright" "$PKG_ROOT/usr/share/doc/$PACKAGE_NAME/copyright"
  gzip -cn9 "$ROOT/packaging/debian/changelog" > "$PKG_ROOT/usr/share/doc/$PACKAGE_NAME/changelog.Debian.gz"
  chmod 0644 "$PKG_ROOT/usr/share/doc/$PACKAGE_NAME/changelog.Debian.gz"
  install -d -m 0755 "$PKG_ROOT/usr/share/man/man1"
  gzip -cn9 "$ROOT/packaging/man/codexbar-linuxd.1" > "$PKG_ROOT/usr/share/man/man1/codexbar-linuxd.1.gz"
  chmod 0644 "$PKG_ROOT/usr/share/man/man1/codexbar-linuxd.1.gz"

  install -d -m 0755 "$PKG_ROOT/DEBIAN"
  install -m 0755 "$ROOT/packaging/debian/postinst" "$PKG_ROOT/DEBIAN/postinst"
  install -m 0755 "$ROOT/packaging/debian/postrm" "$PKG_ROOT/DEBIAN/postrm"

  local architecture installed_size
  architecture="$(dpkg --print-architecture)"
  installed_size="$(du -sk "$PKG_ROOT/usr" | awk '{print $1}')"
  write_control_file "$version" "$architecture" "$installed_size"
}

check_inputs

if [[ "$MODE" == "check" ]]; then
  echo "build-deb.sh package inputs valid for development .deb target"
  exit 0
fi

VERSION="$(package_version)"
ARCHITECTURE="$(dpkg --print-architecture)"
DEB_PATH="$DIST_DIR/${PACKAGE_NAME}_${VERSION}_${ARCHITECTURE}.deb"

RELEASE_RUSTFLAGS="$(release_rustflags)"
env RUSTFLAGS="$RELEASE_RUSTFLAGS" cargo build --manifest-path "$ROOT/daemon/Cargo.toml" --release --locked
stage_package "$VERSION"
CHECK_HOME="$(mktemp -d "${TMPDIR:-/tmp}/codexbar-linuxd-package-check.XXXXXX")"
cleanup_check_home() {
  rm -rf "$CHECK_HOME"
}
trap cleanup_check_home EXIT
env \
  XDG_CACHE_HOME="$CHECK_HOME/cache" \
  XDG_CONFIG_HOME="$CHECK_HOME/config" \
  "$PKG_ROOT/usr/bin/codexbar-linuxd" --check
glib-compile-schemas --strict --dry-run "$PKG_ROOT/usr/share/glib-2.0/schemas"
install -d -m 0755 "$DIST_DIR"
dpkg-deb --root-owner-group --build "$PKG_ROOT" "$DEB_PATH"

echo "Built $DEB_PATH"
