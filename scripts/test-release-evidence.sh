#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VALIDATOR="$ROOT/scripts/validate-release-evidence.sh"
PACKAGE_SMOKE="$ROOT/scripts/package-root-smoke.sh"
COMPLETION_AUDIT="$ROOT/scripts/release-completion-audit.sh"

bash -n "$VALIDATOR"
bash -n "$PACKAGE_SMOKE"
bash -n "$COMPLETION_AUDIT"

TMP="$(mktemp -d)"
STAGE_DEB="/tmp/codexbar-package-stage-only-${BASHPID}.deb"
REL_STAGE_DIR="$ROOT/target/release-evidence-stage-${BASHPID}"
trap 'rm -rf "$TMP" "$REL_STAGE_DIR"; rm -f "$STAGE_DEB"' EXIT

build_stage_deb() {
  local out="$1"
  local work="$2"
  mkdir -p \
    "$work/DEBIAN" \
    "$work/usr/bin" \
    "$work/usr/lib/systemd/user" \
    "$work/usr/share/dbus-1/services" \
    "$work/usr/share/glib-2.0/schemas" \
    "$work/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
  cat >"$work/DEBIAN/control" <<'EOF'
Package: codexbar-linux
Version: 0.1.0-1
Section: utils
Priority: optional
Architecture: amd64
Maintainer: CodexBar Linux Tests <codexbar@example.invalid>
Description: CodexBar Linux stage-only smoke fixture
EOF
  printf '#!/bin/sh\nexit 0\n' >"$work/usr/bin/codexbar-linuxd"
  chmod 755 "$work/usr/bin/codexbar-linuxd"
  printf '#!/bin/sh\nprintf "Default daemon providers: codex and claude via upstream_cli\\n"\n' >"$work/usr/bin/codexbar-linux-setup"
  chmod 755 "$work/usr/bin/codexbar-linux-setup"
  printf 'Exec=/usr/bin/codexbar-linuxd\n' \
    >"$work/usr/share/dbus-1/services/org.codexbar.Linux1.service"
  printf 'ExecStart=/usr/bin/codexbar-linuxd\n' \
    >"$work/usr/lib/systemd/user/codexbar-linuxd.service"
  printf '<schemalist/>\n' \
    >"$work/usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml"
  printf '{"uuid":"codexbar-linux@codexbar.dev"}\n' \
    >"$work/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/metadata.json"
  mkdir -p "$work/usr/share/man/man1"
  printf 'stage-only manpage\n' >"$work/usr/share/man/man1/codexbar-linuxd.1.gz"
  dpkg-deb --build "$work" "$out" >/dev/null
}

write_sidecars() {
  local dir="$1"
  shift
  mkdir -p "$dir"
  for name in "$@"; do
    printf 'test sidecar: %s\n' "$name" >"$dir/$name"
  done
}

write_check_log() {
  local out="$1"
  local head
  head="$(git -C "$ROOT" rev-parse HEAD)"
  cat >"$out" <<EOF
Release-candidate gate documentation is explicit about remaining blockers
Release evidence validator tests passed
No browser-cookie/web-fetch surface present
Upstream CLI capture harness tests passed
test dbus_scheduler_runs_startup_refresh_when_enabled ... ok
test dbus_scheduler_runs_interval_refresh_when_enabled ... ok
test dbus_scheduler_interval_zero_disables_interval_loop_but_allows_startup ... ok
test dbus_scheduler_backs_off_repeated_upstream_cli_failures ... ok
test dbus_refresh_all_configured_providers_disabled_returns_noop ... ok
test settings_patch_advances_scheduler_revision ... ok
test failed_refresh_can_be_unwedged_without_daemon_restart ... ok
test app_refresh_uses_configured_provider_targets ... ok
test app_refresh_all_configured_providers_disabled_noops_without_defaulting_to_codex ... ok
test app_refresh_explicit_providers_override_settings ... ok
test upstream_cli_required_live_matrix_is_present ... ok
test result: ok.
GJS Shell-process boundary smoke check passed
extension state tests passed
repository gate passed for HEAD $head
EOF
}

write_package_sidecars() {
  local dir="$1"
  local candidate="$2"
  local tmp_candidate="$3"
  local sha="$4"
  local arch="$5"
  cat >"$dir/copy-candidate-to-tmp.txt" <<EOF
$ cp $candidate $tmp_candidate
EOF
  cat >"$dir/candidate-checksums.txt" <<EOF
$ sha256sum $candidate $tmp_candidate
$sha  $candidate
$sha  $tmp_candidate
EOF
  cat >"$dir/candidate-byte-compare.txt" <<EOF
$ cmp $candidate $tmp_candidate
EOF
  cat >"$dir/candidate-fields.txt" <<EOF
Package: codexbar-linux
Version: 0.1.0-1
Architecture: $arch
EOF
  cat >"$dir/candidate-contents.txt" <<'EOF'
./usr/bin/codexbar-linuxd
./usr/bin/codexbar-linux-setup
./usr/share/dbus-1/services/org.codexbar.Linux1.service
./usr/lib/systemd/user/codexbar-linuxd.service
./usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml
./usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/metadata.json
./usr/share/man/man1/codexbar-linuxd.1.gz
EOF
  cat >"$dir/sudo-validate.txt" <<'EOF'
$ sudo -v
EOF
  cat >"$dir/apt-install-reinstall.txt" <<EOF
$ sudo apt install --reinstall -y $tmp_candidate
EOF
  cat >"$dir/systemd-user-daemon-reload-after-install.txt" <<'EOF'
$ systemctl --user daemon-reload
EOF
  cat >"$dir/installed-dpkg-query.txt" <<EOF
$ dpkg-query -W -f=\${binary:Package}\\t\${Version}\\t\${Architecture}\\n codexbar-linux
codexbar-linux	0.1.0-1	$arch
EOF
  cat >"$dir/installed-daemon-version.txt" <<'EOF'
$ /usr/bin/codexbar-linuxd --version
codexbar-linuxd 0.1.0
EOF
  cat >"$dir/installed-daemon-check.txt" <<'EOF'
$ /usr/bin/codexbar-linuxd --check
EOF
  cat >"$dir/installed-setup-helper.txt" <<'EOF'
$ /usr/bin/codexbar-linux-setup --dry-run --no-daemon-reload --codexbar-cli /tmp/codexbar
Default daemon providers: codex and claude via upstream_cli
  gnome-extensions enable codexbar-linux@codexbar.dev
EOF
  cat >"$dir/installed-dbus-service.txt" <<'EOF'
Exec=/usr/bin/codexbar-linuxd
EOF
  cat >"$dir/installed-systemd-user-service.txt" <<'EOF'
ExecStart=/usr/bin/codexbar-linuxd
EOF
  cat >"$dir/daemon-info.txt" <<'EOF'
$ busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
EOF
  cat >"$dir/gnome-extensions-list.txt" <<'EOF'
$ gnome-extensions list
codexbar-linux@codexbar.dev
EOF
  cat >"$dir/gnome-extensions-enable.txt" <<'EOF'
$ gnome-extensions enable codexbar-linux@codexbar.dev
EOF
  cat >"$dir/enabled-extensions-after-enable.txt" <<'EOF'
$ gsettings get org.gnome.shell enabled-extensions
['codexbar-linux@codexbar.dev']
EOF
  cat >"$dir/gnome-extensions-info.txt" <<'EOF'
Path: /usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev
EOF
  cat >"$dir/manual-refresh.txt" <<'EOF'
$ busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 Refresh s {}
EOF
  cat >"$dir/global-diagnostics.txt" <<'EOF'
$ busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDiagnostics s global
EOF
  cat >"$dir/diagnostics-redaction-scan.txt" <<'EOF'
diagnostics redaction scan passed
EOF
  cat >"$dir/systemd-user-stop.txt" <<'EOF'
$ systemctl --user stop codexbar-linuxd.service
EOF
  cat >"$dir/systemd-user-restart.txt" <<'EOF'
$ systemctl --user restart codexbar-linuxd.service
EOF
  cat >"$dir/daemon-info-after-restart.txt" <<'EOF'
$ busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
EOF
  cat >"$dir/gnome-extensions-disable.txt" <<'EOF'
$ gnome-extensions disable codexbar-linux@codexbar.dev
EOF
  cat >"$dir/enabled-extensions-after-disable.txt" <<'EOF'
$ gsettings get org.gnome.shell enabled-extensions
[]
EOF
  cat >"$dir/apt-remove.txt" <<'EOF'
$ sudo apt remove -y codexbar-linux
EOF
  cat >"$dir/systemd-user-daemon-reload-after-remove.txt" <<'EOF'
$ systemctl --user daemon-reload
EOF
  cat >"$dir/removed-daemon-absent.txt" <<'EOF'
$ test ! -e /usr/bin/codexbar-linuxd
EOF
  cat >"$dir/removed-setup-helper-absent.txt" <<'EOF'
$ test ! -e /usr/bin/codexbar-linux-setup
EOF
  cat >"$dir/removed-dbus-service-absent.txt" <<'EOF'
$ test ! -e /usr/share/dbus-1/services/org.codexbar.Linux1.service
EOF
  cat >"$dir/removed-systemd-user-service-absent.txt" <<'EOF'
$ test ! -e /usr/lib/systemd/user/codexbar-linuxd.service
EOF
  cat >"$dir/removed-extension-dir-absent.txt" <<'EOF'
$ test ! -e /usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev
EOF
  cat >"$dir/removed-gsettings-schema-absent.txt" <<'EOF'
$ test ! -e /usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml
EOF
  cat >"$dir/removed-manpage-absent.txt" <<'EOF'
$ test ! -e /usr/share/man/man1/codexbar-linuxd.1.gz
EOF
  cat >"$dir/apt-purge.txt" <<'EOF'
$ sudo apt purge -y codexbar-linux
EOF
  cat >"$dir/systemd-user-daemon-reload-after-purge.txt" <<'EOF'
$ systemctl --user daemon-reload
EOF
  cat >"$dir/purged-dpkg-query.txt" <<'EOF'
$ dpkg-query -W codexbar-linux
dpkg-query: no packages found matching codexbar-linux
exit-status: 1
EOF
}

write_gnome_sidecars() {
  local dir="$1"
  local shell_version="$2"
  local session_type="$3"
  local extension_path="$4"
  local ubuntu_version="${5:-26.04}"
  cat >"$dir/gnome-shell-version.txt" <<EOF
$ gnome-shell --version
$shell_version
EOF
  cat >"$dir/os-release.txt" <<EOF
$ python3 -c ...
ID=ubuntu
VERSION_ID="$ubuntu_version"
EOF
  cat >"$dir/os-release-validation.txt" <<EOF
os-release matches Ubuntu $ubuntu_version
EOF
  cat >"$dir/session-type.txt" <<EOF
$ bash -c printf
$session_type
EOF
  cat >"$dir/gnome-shell-processes.txt" <<'EOF'
$ pgrep -af gnome-shell
1234 /usr/bin/gnome-shell
EOF
  cat >"$dir/gnome-shell-latest-process.txt" <<'EOF'
$ ps -o pid,lstart,cmd -p 1234
  PID                  STARTED CMD
 1234 Fri May  8 12:00:00 2026 /usr/bin/gnome-shell
EOF
  cat >"$dir/enabled-extensions.txt" <<'EOF'
$ gsettings get org.gnome.shell enabled-extensions
['codexbar-linux@codexbar.dev']
EOF
  cat >"$dir/gnome-extensions-info.txt" <<EOF
Path: $extension_path
EOF
  cat >"$dir/installed-dpkg-query.txt" <<'EOF'
$ dpkg-query -W -f=${binary:Package}\t${Version}\t${Architecture}\n codexbar-linux
codexbar-linux	0.1.0-1	amd64
EOF
  cat >"$dir/installed-extension-metadata.txt" <<'EOF'
{
  "settings-schema": "org.gnome.shell.extensions.codexbar-linux",
  "shell-version": [
    "46",
    "47",
    "48",
    "49",
    "50"
  ],
  "uuid": "codexbar-linux@codexbar.dev",
  "version": 1
}
EOF
  cat >"$dir/metadata-validation.txt" <<'EOF'
metadata includes GNOME Shell 46 support floor, GNOME Shell 50 validation target, and extension version 1
EOF
  cat >"$dir/daemon-info.txt" <<'EOF'
$ busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
EOF
  cat >"$dir/snapshot.txt" <<'EOF'
$ busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetSnapshot
EOF
  cat >"$dir/manual-refresh.txt" <<'EOF'
$ busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 Refresh s {}
EOF
  cat >"$dir/global-diagnostics.txt" <<'EOF'
$ busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDiagnostics s global
EOF
  cat >"$dir/diagnostics-redaction-scan.txt" <<'EOF'
diagnostics redaction scan passed
EOF
  cat >"$dir/systemd-user-stop.txt" <<'EOF'
$ systemctl --user stop codexbar-linuxd.service
EOF
  cat >"$dir/systemd-user-restart.txt" <<'EOF'
$ systemctl --user restart codexbar-linuxd.service
EOF
  cat >"$dir/daemon-info-after-restart.txt" <<'EOF'
$ busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
EOF
}

PACKAGE_SIDECARS=(
  copy-candidate-to-tmp.txt
  candidate-checksums.txt
  candidate-byte-compare.txt
  candidate-fields.txt
  candidate-contents.txt
  sudo-validate.txt
  apt-install-reinstall.txt
  systemd-user-daemon-reload-after-install.txt
  installed-dpkg-query.txt
  installed-daemon-version.txt
  installed-daemon-check.txt
  installed-setup-helper.txt
  installed-dbus-service.txt
  installed-systemd-user-service.txt
  daemon-info.txt
  gnome-extensions-list.txt
  gnome-extensions-enable.txt
  enabled-extensions-after-enable.txt
  gnome-extensions-info.txt
  manual-refresh.txt
  global-diagnostics.txt
  diagnostics-redaction-scan.txt
  systemd-user-stop.txt
  systemd-user-restart.txt
  daemon-info-after-restart.txt
  gnome-extensions-disable.txt
  enabled-extensions-after-disable.txt
  apt-remove.txt
  systemd-user-daemon-reload-after-remove.txt
  removed-daemon-absent.txt
  removed-setup-helper-absent.txt
  removed-dbus-service-absent.txt
  removed-systemd-user-service-absent.txt
  removed-extension-dir-absent.txt
  removed-gsettings-schema-absent.txt
  removed-manpage-absent.txt
  apt-purge.txt
  systemd-user-daemon-reload-after-purge.txt
  purged-dpkg-query.txt
)
GNOME_SIDECARS=(
  gnome-shell-version.txt
  os-release.txt
  os-release-validation.txt
  session-type.txt
  gnome-shell-processes.txt
  gnome-shell-latest-process.txt
  enabled-extensions.txt
  gnome-extensions-info.txt
  installed-dpkg-query.txt
  installed-extension-metadata.txt
  metadata-validation.txt
  daemon-info.txt
  snapshot.txt
  manual-refresh.txt
  global-diagnostics.txt
  diagnostics-redaction-scan.txt
  systemd-user-stop.txt
  systemd-user-restart.txt
  daemon-info-after-restart.txt
)

CANDIDATE_FILE="$TMP/codexbar-linux.candidate.deb"
TMP_CANDIDATE_FILE="$TMP/codexbar-linux.deb"
ALT_CANDIDATE_FILE="$TMP/codexbar-linux.alt.deb"
printf "package candidate\n" >"$CANDIDATE_FILE"
cp -p "$CANDIDATE_FILE" "$TMP_CANDIDATE_FILE"
cp -p "$CANDIDATE_FILE" "$ALT_CANDIDATE_FILE"
read -r SHA _ < <(sha256sum "$CANDIDATE_FILE")

PACKAGE_FINAL_DIR="$TMP/package-final"
PACKAGE_NONINTERACTIVE_SUDO_DIR="$TMP/package-noninteractive-sudo"
PACKAGE_KEEP_DIR="$TMP/package-keep-installed"
PACKAGE_NO_PURGE_DIR="$TMP/package-no-purge"
PACKAGE_STALE_DIR="$TMP/package-stale-sha"
PACKAGE_BAD_ARCH_DIR="$TMP/package-bad-arch-sidecar"
PACKAGE_BAD_COPY_DIR="$TMP/package-bad-copy-sidecar"
PACKAGE_BAD_BYTE_COMPARE_DIR="$TMP/package-bad-byte-compare-sidecar"
PACKAGE_BAD_SUDO_DIR="$TMP/package-bad-sudo-sidecar"
PACKAGE_BAD_INSTALL_QUERY_DIR="$TMP/package-bad-install-query-sidecar"
PACKAGE_BAD_DBUS_DIR="$TMP/package-bad-dbus-sidecar"
PACKAGE_BAD_CONTENTS_DIR="$TMP/package-bad-contents-sidecar"
PACKAGE_BAD_DAEMON_RELOAD_DIR="$TMP/package-bad-daemon-reload-sidecar"
PACKAGE_BAD_EXTENSION_ENABLE_DIR="$TMP/package-bad-extension-enable-sidecar"
PACKAGE_BAD_EXTENSION_ENABLED_DIR="$TMP/package-bad-extension-enabled-sidecar"
PACKAGE_BAD_EXTENSION_DISABLE_DIR="$TMP/package-bad-extension-disable-sidecar"
PACKAGE_BAD_REMOVE_ABSENCE_DIR="$TMP/package-bad-remove-absence-sidecar"
PACKAGE_BAD_PURGE_QUERY_DIR="$TMP/package-bad-purge-query-sidecar"
PACKAGE_BAD_SIDECAR_DIR="$TMP/package-bad-sidecar"
PACKAGE_BAD_FINAL_FLAG_DIR="$TMP/package-bad-final-release-flag"
PACKAGE_BAD_INCOMPLETE_MARKER_DIR="$TMP/package-bad-incomplete-marker"
PACKAGE_BAD_CURRENT_PATH_DIR="$TMP/package-bad-current-candidate-path"
GNOME_FINAL_DIR="$TMP/gnome-final"
GNOME_DEV_DIR="$TMP/gnome-development"
GNOME_BAD_UBUNTU_DIR="$TMP/gnome-bad-ubuntu-version"
GNOME_BAD_SHELL_MAJOR_DIR="$TMP/gnome-bad-shell-major"
GNOME_BAD_SHELL_VERSION_SIDECAR_DIR="$TMP/gnome-bad-shell-version-sidecar"
GNOME_BAD_OS_RELEASE_SIDECAR_DIR="$TMP/gnome-bad-os-release-sidecar"
GNOME_BAD_SESSION_SIDECAR_DIR="$TMP/gnome-bad-session-sidecar"
GNOME_BAD_SESSION_DIR="$TMP/gnome-bad-session"
GNOME_BAD_PACKAGE_PATH_FLAG_DIR="$TMP/gnome-bad-package-path-flag"
GNOME_BAD_METADATA_DIR="$TMP/gnome-bad-metadata-sidecar"
GNOME_BAD_METADATA_VERSION_DIR="$TMP/gnome-bad-metadata-version-sidecar"
GNOME_BAD_RUNTIME_DIR="$TMP/gnome-bad-runtime-sidecar"
GNOME_BAD_ENABLED_DIR="$TMP/gnome-bad-enabled-sidecar"
GNOME_BAD_INSTALLED_PACKAGE_DIR="$TMP/gnome-bad-installed-package"
GNOME_BAD_ARCHITECTURE_DIR="$TMP/gnome-bad-architecture"
GNOME_BAD_FINAL_FLAG_DIR="$TMP/gnome-bad-final-release-flag"
PACKAGE_FINAL="$PACKAGE_FINAL_DIR/evidence.json"
PACKAGE_NONINTERACTIVE_SUDO="$PACKAGE_NONINTERACTIVE_SUDO_DIR/evidence.json"
PACKAGE_KEEP="$PACKAGE_KEEP_DIR/evidence.json"
PACKAGE_NO_PURGE="$PACKAGE_NO_PURGE_DIR/evidence.json"
PACKAGE_STALE="$PACKAGE_STALE_DIR/evidence.json"
PACKAGE_BAD_ARCH="$PACKAGE_BAD_ARCH_DIR/evidence.json"
PACKAGE_BAD_COPY="$PACKAGE_BAD_COPY_DIR/evidence.json"
PACKAGE_BAD_BYTE_COMPARE="$PACKAGE_BAD_BYTE_COMPARE_DIR/evidence.json"
PACKAGE_BAD_SUDO="$PACKAGE_BAD_SUDO_DIR/evidence.json"
PACKAGE_BAD_INSTALL_QUERY="$PACKAGE_BAD_INSTALL_QUERY_DIR/evidence.json"
PACKAGE_BAD_DBUS="$PACKAGE_BAD_DBUS_DIR/evidence.json"
PACKAGE_BAD_CONTENTS="$PACKAGE_BAD_CONTENTS_DIR/evidence.json"
PACKAGE_BAD_DAEMON_RELOAD="$PACKAGE_BAD_DAEMON_RELOAD_DIR/evidence.json"
PACKAGE_BAD_EXTENSION_ENABLE="$PACKAGE_BAD_EXTENSION_ENABLE_DIR/evidence.json"
PACKAGE_BAD_EXTENSION_ENABLED="$PACKAGE_BAD_EXTENSION_ENABLED_DIR/evidence.json"
PACKAGE_BAD_EXTENSION_DISABLE="$PACKAGE_BAD_EXTENSION_DISABLE_DIR/evidence.json"
PACKAGE_BAD_REMOVE_ABSENCE="$PACKAGE_BAD_REMOVE_ABSENCE_DIR/evidence.json"
PACKAGE_BAD_PURGE_QUERY="$PACKAGE_BAD_PURGE_QUERY_DIR/evidence.json"
PACKAGE_BAD_SIDECAR="$PACKAGE_BAD_SIDECAR_DIR/evidence.json"
PACKAGE_BAD_FINAL_FLAG="$PACKAGE_BAD_FINAL_FLAG_DIR/evidence.json"
PACKAGE_BAD_INCOMPLETE_MARKER="$PACKAGE_BAD_INCOMPLETE_MARKER_DIR/evidence.json"
PACKAGE_BAD_CURRENT_PATH="$PACKAGE_BAD_CURRENT_PATH_DIR/evidence.json"
GNOME_FINAL="$GNOME_FINAL_DIR/evidence.json"
GNOME_DEV="$GNOME_DEV_DIR/evidence.json"
GNOME_BAD_UBUNTU="$GNOME_BAD_UBUNTU_DIR/evidence.json"
GNOME_BAD_SHELL_MAJOR="$GNOME_BAD_SHELL_MAJOR_DIR/evidence.json"
GNOME_BAD_SHELL_VERSION_SIDECAR="$GNOME_BAD_SHELL_VERSION_SIDECAR_DIR/evidence.json"
GNOME_BAD_OS_RELEASE_SIDECAR="$GNOME_BAD_OS_RELEASE_SIDECAR_DIR/evidence.json"
GNOME_BAD_SESSION_SIDECAR="$GNOME_BAD_SESSION_SIDECAR_DIR/evidence.json"
GNOME_BAD_SESSION="$GNOME_BAD_SESSION_DIR/evidence.json"
GNOME_BAD_PACKAGE_PATH_FLAG="$GNOME_BAD_PACKAGE_PATH_FLAG_DIR/evidence.json"
GNOME_BAD_METADATA="$GNOME_BAD_METADATA_DIR/evidence.json"
GNOME_BAD_METADATA_VERSION="$GNOME_BAD_METADATA_VERSION_DIR/evidence.json"
GNOME_BAD_RUNTIME="$GNOME_BAD_RUNTIME_DIR/evidence.json"
GNOME_BAD_ENABLED="$GNOME_BAD_ENABLED_DIR/evidence.json"
GNOME_BAD_INSTALLED_PACKAGE="$GNOME_BAD_INSTALLED_PACKAGE_DIR/evidence.json"
GNOME_BAD_ARCHITECTURE="$GNOME_BAD_ARCHITECTURE_DIR/evidence.json"
GNOME_BAD_FINAL_FLAG="$GNOME_BAD_FINAL_FLAG_DIR/evidence.json"
write_sidecars "$PACKAGE_FINAL_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_NONINTERACTIVE_SUDO_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_KEEP_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_NO_PURGE_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_STALE_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_ARCH_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_COPY_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_BYTE_COMPARE_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_SUDO_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_INSTALL_QUERY_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_DBUS_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_CONTENTS_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_DAEMON_RELOAD_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_EXTENSION_ENABLE_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_EXTENSION_ENABLED_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_EXTENSION_DISABLE_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_REMOVE_ABSENCE_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_PURGE_QUERY_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_SIDECAR_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_FINAL_FLAG_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_INCOMPLETE_MARKER_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$PACKAGE_BAD_CURRENT_PATH_DIR" "${PACKAGE_SIDECARS[@]}"
write_sidecars "$GNOME_FINAL_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_DEV_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_BAD_UBUNTU_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_BAD_SHELL_MAJOR_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_BAD_SHELL_VERSION_SIDECAR_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_BAD_OS_RELEASE_SIDECAR_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_BAD_SESSION_SIDECAR_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_BAD_SESSION_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_BAD_PACKAGE_PATH_FLAG_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_BAD_METADATA_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_BAD_METADATA_VERSION_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_BAD_RUNTIME_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_BAD_ENABLED_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_BAD_INSTALLED_PACKAGE_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_BAD_ARCHITECTURE_DIR" "${GNOME_SIDECARS[@]}"
write_sidecars "$GNOME_BAD_FINAL_FLAG_DIR" "${GNOME_SIDECARS[@]}"
for dir in "$PACKAGE_FINAL_DIR" "$PACKAGE_NONINTERACTIVE_SUDO_DIR" "$PACKAGE_KEEP_DIR" "$PACKAGE_NO_PURGE_DIR" "$PACKAGE_STALE_DIR" "$PACKAGE_BAD_ARCH_DIR" "$PACKAGE_BAD_COPY_DIR" "$PACKAGE_BAD_BYTE_COMPARE_DIR" "$PACKAGE_BAD_SUDO_DIR" "$PACKAGE_BAD_INSTALL_QUERY_DIR" "$PACKAGE_BAD_DBUS_DIR" "$PACKAGE_BAD_CONTENTS_DIR" "$PACKAGE_BAD_DAEMON_RELOAD_DIR" "$PACKAGE_BAD_EXTENSION_ENABLE_DIR" "$PACKAGE_BAD_EXTENSION_ENABLED_DIR" "$PACKAGE_BAD_EXTENSION_DISABLE_DIR" "$PACKAGE_BAD_REMOVE_ABSENCE_DIR" "$PACKAGE_BAD_PURGE_QUERY_DIR" "$PACKAGE_BAD_SIDECAR_DIR" "$PACKAGE_BAD_FINAL_FLAG_DIR" "$PACKAGE_BAD_INCOMPLETE_MARKER_DIR" "$PACKAGE_BAD_CURRENT_PATH_DIR"; do
  write_package_sidecars "$dir" "$CANDIDATE_FILE" "$TMP_CANDIDATE_FILE" "$SHA" "amd64"
done
write_gnome_sidecars "$GNOME_FINAL_DIR" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
write_gnome_sidecars "$GNOME_DEV_DIR" "GNOME Shell 46.0" "wayland" "/home/example/.local/share/gnome-shell/extensions/codexbar-linux@codexbar.dev" "24.04"
write_gnome_sidecars "$GNOME_BAD_UBUNTU_DIR" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev" "24.04"
write_gnome_sidecars "$GNOME_BAD_SHELL_MAJOR_DIR" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
write_gnome_sidecars "$GNOME_BAD_SHELL_VERSION_SIDECAR_DIR" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
write_gnome_sidecars "$GNOME_BAD_OS_RELEASE_SIDECAR_DIR" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
write_gnome_sidecars "$GNOME_BAD_SESSION_SIDECAR_DIR" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
write_gnome_sidecars "$GNOME_BAD_SESSION_DIR" "GNOME Shell 50.0" "x11" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
write_gnome_sidecars "$GNOME_BAD_PACKAGE_PATH_FLAG_DIR" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
write_gnome_sidecars "$GNOME_BAD_METADATA_DIR" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
write_gnome_sidecars "$GNOME_BAD_METADATA_VERSION_DIR" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
write_gnome_sidecars "$GNOME_BAD_RUNTIME_DIR" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
write_gnome_sidecars "$GNOME_BAD_ENABLED_DIR" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
write_gnome_sidecars "$GNOME_BAD_INSTALLED_PACKAGE_DIR" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
write_gnome_sidecars "$GNOME_BAD_ARCHITECTURE_DIR" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
write_gnome_sidecars "$GNOME_BAD_FINAL_FLAG_DIR" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"

cat >"$PACKAGE_FINAL" <<EOF
{
  "architecture": "amd64",
  "candidate": "$CANDIDATE_FILE",
  "candidateSha256": "$SHA",
  "daemonRestartVerified": true,
  "daemonVersion": "codexbar-linuxd 0.1.0",
  "diagnosticsRedactionScanPassed": true,
  "finalReleaseEvidence": true,
  "installedVersion": "0.1.0-1",
  "keepInstalled": false,
  "manualRefreshVerified": true,
  "purgeAfterRemove": true,
  "removeVerified": true,
  "schemaVersion": 1,
  "smokeType": "package-root",
  "status": "passed",
  "sudoValidated": true,
  "systemExtensionPathVerified": true,
  "tmpCandidate": "$TMP_CANDIDATE_FILE",
  "tmpCandidateSha256": "$SHA",
  "usedAptReinstallFromTmp": true
}
EOF
cp "$PACKAGE_FINAL" "$PACKAGE_NONINTERACTIVE_SUDO"
python3 - "$PACKAGE_NONINTERACTIVE_SUDO" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
data = json.loads(path.read_text(encoding="utf-8"))
data["sudoNonInteractive"] = True
path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
cat >"$PACKAGE_NONINTERACTIVE_SUDO_DIR/sudo-validate.txt" <<'EOF'
$ sudo -n -v
EOF

write_package_sidecars "$PACKAGE_BAD_CURRENT_PATH_DIR" "$ALT_CANDIDATE_FILE" "$TMP_CANDIDATE_FILE" "$SHA" "amd64"
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_CURRENT_PATH"
python3 - "$PACKAGE_BAD_CURRENT_PATH" "$ALT_CANDIDATE_FILE" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
data = json.loads(path.read_text(encoding="utf-8"))
data["candidate"] = sys.argv[2]
path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY

cat >"$PACKAGE_KEEP" <<EOF
{
  "architecture": "amd64",
  "candidate": "$CANDIDATE_FILE",
  "candidateSha256": "$SHA",
  "daemonRestartVerified": true,
  "daemonVersion": "codexbar-linuxd 0.1.0",
  "diagnosticsRedactionScanPassed": true,
  "finalReleaseEvidence": false,
  "installedVersion": "0.1.0-1",
  "keepInstalled": true,
  "manualRefreshVerified": true,
  "purgeAfterRemove": false,
  "removeVerified": false,
  "schemaVersion": 1,
  "smokeType": "package-root",
  "status": "passed",
  "sudoValidated": true,
  "systemExtensionPathVerified": true,
  "tmpCandidate": "$TMP_CANDIDATE_FILE",
  "tmpCandidateSha256": "$SHA",
  "usedAptReinstallFromTmp": true
}
EOF

cat >"$PACKAGE_STALE" <<EOF
{
  "architecture": "amd64",
  "candidate": "$CANDIDATE_FILE",
  "candidateSha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "daemonRestartVerified": true,
  "daemonVersion": "codexbar-linuxd 0.1.0",
  "diagnosticsRedactionScanPassed": true,
  "finalReleaseEvidence": true,
  "installedVersion": "0.1.0-1",
  "keepInstalled": false,
  "manualRefreshVerified": true,
  "purgeAfterRemove": true,
  "removeVerified": true,
  "schemaVersion": 1,
  "smokeType": "package-root",
  "status": "passed",
  "sudoValidated": true,
  "systemExtensionPathVerified": true,
  "tmpCandidate": "$TMP_CANDIDATE_FILE",
  "tmpCandidateSha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "usedAptReinstallFromTmp": true
}
EOF

cat >"$PACKAGE_NO_PURGE" <<EOF
{
  "architecture": "amd64",
  "candidate": "$CANDIDATE_FILE",
  "candidateSha256": "$SHA",
  "daemonRestartVerified": true,
  "daemonVersion": "codexbar-linuxd 0.1.0",
  "diagnosticsRedactionScanPassed": true,
  "finalReleaseEvidence": false,
  "installedVersion": "0.1.0-1",
  "keepInstalled": false,
  "manualRefreshVerified": true,
  "purgeAfterRemove": false,
  "removeVerified": true,
  "schemaVersion": 1,
  "smokeType": "package-root",
  "status": "passed",
  "sudoValidated": true,
  "systemExtensionPathVerified": true,
  "tmpCandidate": "$TMP_CANDIDATE_FILE",
  "tmpCandidateSha256": "$SHA",
  "usedAptReinstallFromTmp": true
}
EOF
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_ARCH"
sed -i 's/^Architecture: amd64$/Architecture: arm64/' "$PACKAGE_BAD_ARCH_DIR/candidate-fields.txt"
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_COPY"
printf 'wrong copy sidecar\n' >"$PACKAGE_BAD_COPY_DIR/copy-candidate-to-tmp.txt"
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_BYTE_COMPARE"
printf 'wrong byte compare sidecar\n' >"$PACKAGE_BAD_BYTE_COMPARE_DIR/candidate-byte-compare.txt"
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_SUDO"
printf 'wrong sudo sidecar\n' >"$PACKAGE_BAD_SUDO_DIR/sudo-validate.txt"
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_INSTALL_QUERY"
printf '$ dpkg-query -W codexbar-linux\ncodexbar-linux\t0.0.0\tamd64\nexpected-version-token-only: 0.1.0-1\n' >"$PACKAGE_BAD_INSTALL_QUERY_DIR/installed-dpkg-query.txt"
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_DBUS"
printf 'wrong dbus sidecar\n' >"$PACKAGE_BAD_DBUS_DIR/daemon-info.txt"
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_CONTENTS"
sed -i '/org.gnome.shell.extensions.codexbar-linux.gschema.xml/d' "$PACKAGE_BAD_CONTENTS_DIR/candidate-contents.txt"
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_DAEMON_RELOAD"
printf 'wrong daemon reload sidecar\n' >"$PACKAGE_BAD_DAEMON_RELOAD_DIR/systemd-user-daemon-reload-after-remove.txt"
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_EXTENSION_ENABLE"
printf 'wrong extension enable sidecar\n' >"$PACKAGE_BAD_EXTENSION_ENABLE_DIR/gnome-extensions-enable.txt"
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_EXTENSION_ENABLED"
printf "[]\n" >"$PACKAGE_BAD_EXTENSION_ENABLED_DIR/enabled-extensions-after-enable.txt"
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_EXTENSION_DISABLE"
cat >"$PACKAGE_BAD_EXTENSION_DISABLE_DIR/enabled-extensions-after-disable.txt" <<'EOF'
$ gsettings get org.gnome.shell enabled-extensions
['codexbar-linux@codexbar.dev']
EOF
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_REMOVE_ABSENCE"
printf '$ test ! -e /wrong/path\n' >"$PACKAGE_BAD_REMOVE_ABSENCE_DIR/removed-manpage-absent.txt"
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_PURGE_QUERY"
cat >"$PACKAGE_BAD_PURGE_QUERY_DIR/purged-dpkg-query.txt" <<'EOF'
$ dpkg-query -W codexbar-linux
codexbar-linux	0.1.0-1
exit-status: 0
EOF
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_SIDECAR"
printf 'codexbar-linuxd 0.0.0\n' >"$PACKAGE_BAD_SIDECAR_DIR/installed-daemon-version.txt"
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_FINAL_FLAG"
sed -i 's/"finalReleaseEvidence": true/"finalReleaseEvidence": false/' "$PACKAGE_BAD_FINAL_FLAG"
cp "$PACKAGE_FINAL" "$PACKAGE_BAD_INCOMPLETE_MARKER"
cat >"$PACKAGE_BAD_INCOMPLETE_MARKER_DIR/incomplete.txt" <<'EOF'
package-root-smoke: incomplete
exit-status: 130
evidence-json: present
final-release-evidence: false
reason: command failed or was interrupted before a successful smoke completed
EOF

cat >"$GNOME_FINAL" <<'EOF'
{
  "daemonRestartVerified": true,
  "diagnosticsRedactionScanPassed": true,
  "enabledExtensionVerified": true,
  "expectedShell": "50",
  "extensionPath": "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev",
  "finalReleaseEvidence": true,
  "installedArchitecture": "amd64",
  "installedVersion": "0.1.0-1",
  "manualRefreshVerified": true,
  "metadataIncludesGnome50": true,
  "metadataShellVersions": ["46", "47", "48", "49", "50"],
  "osId": "ubuntu",
  "osVersionId": "26.04",
  "packagePathVerified": true,
  "requireUbuntuVersion": "26.04",
  "requirePackagePath": true,
  "requireWayland": true,
  "schemaVersion": 1,
  "sessionType": "wayland",
  "shellMajor": "50",
  "shellVersion": "GNOME Shell 50.0",
  "smokeType": "gnome-matrix",
  "status": "passed",
  "ubuntuVersionVerified": true
}
EOF

cat >"$GNOME_DEV" <<'EOF'
{
  "daemonRestartVerified": true,
  "diagnosticsRedactionScanPassed": true,
  "enabledExtensionVerified": true,
  "expectedShell": "46",
  "extensionPath": "/home/example/.local/share/gnome-shell/extensions/codexbar-linux@codexbar.dev",
  "finalReleaseEvidence": false,
  "installedArchitecture": "amd64",
  "installedVersion": "0.1.0-1",
  "manualRefreshVerified": true,
  "metadataIncludesGnome50": true,
  "metadataShellVersions": ["46", "47", "48", "49", "50"],
  "osId": "ubuntu",
  "osVersionId": "24.04",
  "packagePathVerified": false,
  "requireUbuntuVersion": null,
  "requirePackagePath": false,
  "requireWayland": true,
  "schemaVersion": 1,
  "sessionType": "wayland",
  "shellMajor": "46",
  "shellVersion": "GNOME Shell 46.0",
  "smokeType": "gnome-matrix",
  "status": "passed",
  "ubuntuVersionVerified": true
}
EOF

cp "$GNOME_FINAL" "$GNOME_BAD_UBUNTU"
sed -i 's/"osVersionId": "26.04"/"osVersionId": "24.04"/' "$GNOME_BAD_UBUNTU"
sed -i 's/"ubuntuVersionVerified": true/"ubuntuVersionVerified": false/' "$GNOME_BAD_UBUNTU"
cp "$GNOME_FINAL" "$GNOME_BAD_SHELL_MAJOR"
sed -i 's/"shellMajor": "50"/"shellMajor": "49"/' "$GNOME_BAD_SHELL_MAJOR"
cp "$GNOME_FINAL" "$GNOME_BAD_SHELL_VERSION_SIDECAR"
printf 'GNOME Shell 49.0\n' >>"$GNOME_BAD_SHELL_VERSION_SIDECAR_DIR/gnome-shell-version.txt"
cp "$GNOME_FINAL" "$GNOME_BAD_OS_RELEASE_SIDECAR"
printf 'VERSION_ID="24.04"\n' >>"$GNOME_BAD_OS_RELEASE_SIDECAR_DIR/os-release.txt"
cp "$GNOME_FINAL" "$GNOME_BAD_SESSION_SIDECAR"
printf 'x11\n' >>"$GNOME_BAD_SESSION_SIDECAR_DIR/session-type.txt"
cp "$GNOME_FINAL" "$GNOME_BAD_SESSION"
sed -i 's/"sessionType": "wayland"/"sessionType": "x11"/' "$GNOME_BAD_SESSION"
sed -i 's/"requireWayland": true/"requireWayland": false/' "$GNOME_BAD_SESSION"
cp "$GNOME_FINAL" "$GNOME_BAD_PACKAGE_PATH_FLAG"
sed -i 's/"packagePathVerified": true/"packagePathVerified": false/' "$GNOME_BAD_PACKAGE_PATH_FLAG"
sed -i 's/"requirePackagePath": true/"requirePackagePath": false/' "$GNOME_BAD_PACKAGE_PATH_FLAG"
cp "$GNOME_FINAL" "$GNOME_BAD_METADATA"
sed -i '/"50"/d' "$GNOME_BAD_METADATA_DIR/installed-extension-metadata.txt"
cp "$GNOME_FINAL" "$GNOME_BAD_METADATA_VERSION"
sed -i 's/"version": 1/"version": 0/' "$GNOME_BAD_METADATA_VERSION_DIR/installed-extension-metadata.txt"
cp "$GNOME_FINAL" "$GNOME_BAD_RUNTIME"
printf 'wrong runtime sidecar\n' >"$GNOME_BAD_RUNTIME_DIR/snapshot.txt"
cp "$GNOME_FINAL" "$GNOME_BAD_ENABLED"
printf "[]\n" >"$GNOME_BAD_ENABLED_DIR/enabled-extensions.txt"
cp "$GNOME_FINAL" "$GNOME_BAD_INSTALLED_PACKAGE"
sed -i 's/"installedVersion": "0.1.0-1"/"installedVersion": "0.0.0"/' "$GNOME_BAD_INSTALLED_PACKAGE"
cat >"$GNOME_BAD_INSTALLED_PACKAGE_DIR/installed-dpkg-query.txt" <<'EOF'
$ dpkg-query -W -f=${binary:Package}\t${Version}\t${Architecture}\n codexbar-linux
codexbar-linux	0.0.0	amd64
EOF
cp "$GNOME_FINAL" "$GNOME_BAD_ARCHITECTURE"
sed -i 's/"installedArchitecture": "amd64"/"installedArchitecture": "arm64"/' "$GNOME_BAD_ARCHITECTURE"
cat >"$GNOME_BAD_ARCHITECTURE_DIR/installed-dpkg-query.txt" <<'EOF'
$ dpkg-query -W -f=${binary:Package}\t${Version}\t${Architecture}\n codexbar-linux
codexbar-linux	0.1.0-1	arm64
EOF
cp "$GNOME_FINAL" "$GNOME_BAD_FINAL_FLAG"
sed -i 's/"finalReleaseEvidence": true/"finalReleaseEvidence": false/' "$GNOME_BAD_FINAL_FLAG"

expect_package_bool_rejected() {
  local key="$1"
  local dir="$TMP/package-bad-bool-$key"
  local evidence="$dir/evidence.json"
  mkdir -p "$dir"
  write_sidecars "$dir" "${PACKAGE_SIDECARS[@]}"
  write_package_sidecars "$dir" "$CANDIDATE_FILE" "$TMP_CANDIDATE_FILE" "$SHA" "amd64"
  cp "$PACKAGE_FINAL" "$evidence"
  sed -i "s/\"$key\": true/\"$key\": false/" "$evidence"
  if "$VALIDATOR" --package-root "$evidence" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-bool-$key.out" 2>"$TMP/bad-bool-$key.err"; then
    echo "Package evidence with false release-critical package boolean must not satisfy final release evidence: $key" >&2
    exit 1
  fi
  grep -F "$key=true" "$TMP/bad-bool-$key.err" >/dev/null
}

expect_gnome_bool_rejected() {
  local key="$1"
  local dir="$TMP/gnome-bad-bool-$key"
  local evidence="$dir/evidence.json"
  mkdir -p "$dir"
  write_sidecars "$dir" "${GNOME_SIDECARS[@]}"
  write_gnome_sidecars "$dir" "GNOME Shell 50.0" "wayland" "/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev"
  cp "$GNOME_FINAL" "$evidence"
  sed -i "s/\"$key\": true/\"$key\": false/" "$evidence"
  if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$evidence" >"$TMP/bad-gnome-bool-$key.out" 2>"$TMP/bad-gnome-bool-$key.err"; then
    echo "GNOME evidence with false release-critical GNOME boolean must not satisfy final release evidence: $key" >&2
    exit 1
  fi
  grep -F "$key=true" "$TMP/bad-gnome-bool-$key.err" >/dev/null
}

"$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_FINAL" >/dev/null
"$VALIDATOR" --package-root "$PACKAGE_NONINTERACTIVE_SUDO" --gnome-matrix "$GNOME_FINAL" >/dev/null
"$VALIDATOR" --allow-development-gnome --gnome-matrix "$GNOME_DEV" >/dev/null
CHECK_LOG="$TMP/check.log"
BAD_CHECK_LOG="$TMP/check-stale-head.log"
BAD_CHECK_MISSING_SCHEDULER_LOG="$TMP/check-missing-scheduler.log"
write_check_log "$CHECK_LOG"
CODEXBAR_LINUX_RELEASE_CANDIDATE="$CANDIDATE_FILE" \
CODEXBAR_LINUX_RELEASE_TMP_CANDIDATE="$TMP_CANDIDATE_FILE" \
CODEXBAR_LINUX_TEST_ALLOW_DIRTY=1 \
  "$COMPLETION_AUDIT" \
    --package-root "$PACKAGE_FINAL" \
    --gnome-matrix "$GNOME_FINAL" \
    --local-gate-log "$CHECK_LOG" \
    >"$TMP/completion-audit-final.out"
grep -F "Current release candidate matches package-root evidence" "$TMP/completion-audit-final.out" >/dev/null
grep -F "Git working tree cleanliness skipped for test fixture" "$TMP/completion-audit-final.out" >/dev/null
grep -F "Local repository gate evidence matches current HEAD" "$TMP/completion-audit-final.out" >/dev/null
grep -F "05F-05K release objective audit: complete" "$TMP/completion-audit-final.out" >/dev/null

if CODEXBAR_LINUX_RELEASE_CANDIDATE="$CANDIDATE_FILE" \
  CODEXBAR_LINUX_RELEASE_TMP_CANDIDATE="$TMP_CANDIDATE_FILE" \
  CODEXBAR_LINUX_TEST_ALLOW_DIRTY=1 \
  "$COMPLETION_AUDIT" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_FINAL" >"$TMP/completion-audit-missing-local-gate.out" 2>"$TMP/completion-audit-missing-local-gate.err"; then
  echo "Completion audit without local check log must not satisfy tag-prep gate" >&2
  exit 1
fi
grep -F "missing --local-gate-log final ./scripts/check.sh evidence" "$TMP/completion-audit-missing-local-gate.err" >/dev/null

cp "$CHECK_LOG" "$BAD_CHECK_LOG"
sed -i 's/repository gate passed for HEAD .*/repository gate passed for HEAD stale-head/' "$BAD_CHECK_LOG"
if CODEXBAR_LINUX_RELEASE_CANDIDATE="$CANDIDATE_FILE" \
  CODEXBAR_LINUX_RELEASE_TMP_CANDIDATE="$TMP_CANDIDATE_FILE" \
  CODEXBAR_LINUX_TEST_ALLOW_DIRTY=1 \
  "$COMPLETION_AUDIT" \
    --package-root "$PACKAGE_FINAL" \
    --gnome-matrix "$GNOME_FINAL" \
    --local-gate-log "$BAD_CHECK_LOG" \
    >"$TMP/completion-audit-stale-local-gate.out" 2>"$TMP/completion-audit-stale-local-gate.err"; then
  echo "Completion audit with stale local check log must not satisfy tag-prep gate" >&2
  exit 1
fi
grep -F "local gate log does not include required current-HEAD gate marker" "$TMP/completion-audit-stale-local-gate.err" >/dev/null

grep -v "dbus_scheduler_runs_startup_refresh_when_enabled" "$CHECK_LOG" >"$BAD_CHECK_MISSING_SCHEDULER_LOG"
if CODEXBAR_LINUX_RELEASE_CANDIDATE="$CANDIDATE_FILE" \
  CODEXBAR_LINUX_RELEASE_TMP_CANDIDATE="$TMP_CANDIDATE_FILE" \
  CODEXBAR_LINUX_TEST_ALLOW_DIRTY=1 \
  "$COMPLETION_AUDIT" \
    --package-root "$PACKAGE_FINAL" \
    --gnome-matrix "$GNOME_FINAL" \
    --local-gate-log "$BAD_CHECK_MISSING_SCHEDULER_LOG" \
    >"$TMP/completion-audit-missing-scheduler-log.out" 2>"$TMP/completion-audit-missing-scheduler-log.err"; then
  echo "Completion audit with local check log missing scheduler tests must not satisfy tag-prep gate" >&2
  exit 1
fi
grep -F "dbus_scheduler_runs_startup_refresh_when_enabled" "$TMP/completion-audit-missing-scheduler-log.err" >/dev/null

if CODEXBAR_LINUX_RELEASE_CANDIDATE="$CANDIDATE_FILE" \
  CODEXBAR_LINUX_RELEASE_TMP_CANDIDATE="$TMP_CANDIDATE_FILE" \
  "$COMPLETION_AUDIT" --package-root "$PACKAGE_FINAL" >"$TMP/completion-audit-missing.out" 2>"$TMP/completion-audit-missing.err"; then
  echo "Completion audit without GNOME evidence must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "missing --gnome-matrix final GNOME matrix evidence" "$TMP/completion-audit-missing.out" >/dev/null
grep -F "Required final evidence commands:" "$TMP/completion-audit-missing.out" >/dev/null
grep -F './scripts/package-root-smoke.sh --deb "$candidate" --purge' "$TMP/completion-audit-missing.out" >/dev/null
grep -F "CODEXBAR_LINUX_PACKAGE_SMOKE_SUDO_NONINTERACTIVE=1" "$TMP/completion-audit-missing.out" >/dev/null
grep -F "./scripts/gnome-matrix-smoke.sh --require-shell 50 --require-ubuntu 26.04 --require-package-path --require-wayland --pause-for-ui" "$TMP/completion-audit-missing.out" >/dev/null
grep -F "./scripts/release-completion-audit.sh --package-root PATH/package-root/evidence.json --gnome-matrix PATH/gnome-matrix/evidence.json --local-gate-log PATH/check.log" "$TMP/completion-audit-missing.out" >/dev/null
grep -F "05F-05K release objective audit: not complete" "$TMP/completion-audit-missing.err" >/dev/null

if CODEXBAR_LINUX_RELEASE_CANDIDATE="$CANDIDATE_FILE" \
  CODEXBAR_LINUX_RELEASE_TMP_CANDIDATE="$TMP_CANDIDATE_FILE" \
  CODEXBAR_LINUX_TEST_FORCE_DIRTY=1 \
  "$COMPLETION_AUDIT" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_FINAL" >"$TMP/completion-audit-dirty.out" 2>"$TMP/completion-audit-dirty.err"; then
  echo "Completion audit with dirty git worktree must not satisfy tag-prep gate" >&2
  exit 1
fi
grep -F "git working tree is not clean" "$TMP/completion-audit-dirty.err" >/dev/null

printf 'different current candidate\n' >"$TMP/different-current-candidate.deb"
if CODEXBAR_LINUX_RELEASE_CANDIDATE="$TMP/different-current-candidate.deb" \
  CODEXBAR_LINUX_RELEASE_TMP_CANDIDATE="$TMP_CANDIDATE_FILE" \
  "$COMPLETION_AUDIT" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_FINAL" >"$TMP/completion-audit-stale-candidate.out" 2>"$TMP/completion-audit-stale-candidate.err"; then
  echo "Completion audit with stale package-root evidence must not satisfy latest .deb gate" >&2
  exit 1
fi
grep -F "package evidence candidate path does not match current dist candidate" "$TMP/completion-audit-stale-candidate.err" >/dev/null

if CODEXBAR_LINUX_RELEASE_CANDIDATE="$CANDIDATE_FILE" \
  CODEXBAR_LINUX_RELEASE_TMP_CANDIDATE="$TMP_CANDIDATE_FILE" \
  "$COMPLETION_AUDIT" --package-root "$PACKAGE_BAD_CURRENT_PATH" --gnome-matrix "$GNOME_FINAL" >"$TMP/completion-audit-wrong-candidate-path.out" 2>"$TMP/completion-audit-wrong-candidate-path.err"; then
  echo "Completion audit with wrong package-root candidate path must not satisfy latest .deb gate" >&2
  exit 1
fi
grep -F "package evidence candidate path does not match current dist candidate" "$TMP/completion-audit-wrong-candidate-path.err" >/dev/null

build_stage_deb "$STAGE_DEB" "$TMP/stage-deb-absolute"
CODEXBAR_LINUX_TEST_ARCH=amd64 "$PACKAGE_SMOKE" \
  --deb "$STAGE_DEB" \
  --evidence-dir "$TMP/package-stage-only" \
  --stage-only \
  >"$TMP/package-stage-only.out"
grep -F "Package candidate staging passed" "$TMP/package-stage-only.out" >/dev/null
grep -F "source already matches /tmp candidate; copy skipped" \
  "$TMP/package-stage-only/copy-candidate-to-tmp.txt" >/dev/null
grep -F "$STAGE_DEB" "$TMP/package-stage-only/candidate-checksums.txt" >/dev/null
grep -Fx "Package: codexbar-linux" "$TMP/package-stage-only/candidate-fields.txt" >/dev/null
grep -F "usr/bin/codexbar-linuxd" "$TMP/package-stage-only/candidate-contents.txt" >/dev/null
grep -F "usr/bin/codexbar-linux-setup" "$TMP/package-stage-only/candidate-contents.txt" >/dev/null
grep -F "usr/share/dbus-1/services/org.codexbar.Linux1.service" "$TMP/package-stage-only/candidate-contents.txt" >/dev/null
grep -F "usr/lib/systemd/user/codexbar-linuxd.service" "$TMP/package-stage-only/candidate-contents.txt" >/dev/null
grep -F "usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml" "$TMP/package-stage-only/candidate-contents.txt" >/dev/null
grep -F "usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev/metadata.json" "$TMP/package-stage-only/candidate-contents.txt" >/dev/null
grep -F "usr/share/man/man1/codexbar-linuxd.1.gz" "$TMP/package-stage-only/candidate-contents.txt" >/dev/null
grep -F '"smokeType": "package-stage"' "$TMP/package-stage-only/evidence.json" >/dev/null
grep -F '"finalReleaseEvidence": false' "$TMP/package-stage-only/evidence.json" >/dev/null

CODEXBAR_LINUX_TEST_ARCH=amd64 "$PACKAGE_SMOKE" \
  --deb "$STAGE_DEB" \
  --evidence-dir "$TMP/package-stage-noninteractive-flag" \
  --stage-only \
  --noninteractive-sudo \
  >"$TMP/package-stage-noninteractive-flag.out"
grep -F "Package candidate staging passed" "$TMP/package-stage-noninteractive-flag.out" >/dev/null

FAKE_SUDO_BIN="$TMP/fake-noninteractive-sudo-bin"
mkdir -p "$FAKE_SUDO_BIN"
cat >"$FAKE_SUDO_BIN/sudo" <<'EOF'
#!/bin/sh
if [ "$1" = "-n" ] && [ "$2" = "-v" ]; then
  echo "sudo: a password is required" >&2
  exit 1
fi
echo "unexpected sudo invocation: $*" >&2
exit 99
EOF
chmod 755 "$FAKE_SUDO_BIN/sudo"
for tool in apt busctl dpkg-query gnome-extensions gsettings systemctl; do
  cat >"$FAKE_SUDO_BIN/$tool" <<'EOF'
#!/bin/sh
echo "unexpected pre-sudo tool invocation: $0 $*" >&2
exit 99
EOF
  chmod 755 "$FAKE_SUDO_BIN/$tool"
done
if PATH="$FAKE_SUDO_BIN:$PATH" "$PACKAGE_SMOKE" \
  --deb "$STAGE_DEB" \
  --evidence-dir "$TMP/package-root-noninteractive-sudo-missing" \
  --purge \
  --noninteractive-sudo \
  >"$TMP/package-root-noninteractive-sudo-missing.out" \
  2>"$TMP/package-root-noninteractive-sudo-missing.err"; then
  echo "Package root smoke without cached sudo credentials must not exit successfully" >&2
  exit 1
fi
grep -F "Checking non-interactive sudo access." "$TMP/package-root-noninteractive-sudo-missing.out" >/dev/null
grep -F "package-root-smoke: incomplete" "$TMP/package-root-noninteractive-sudo-missing/incomplete.txt" >/dev/null
grep -F "final-release-evidence: false" "$TMP/package-root-noninteractive-sudo-missing/incomplete.txt" >/dev/null
grep -F "evidence-json: absent" "$TMP/package-root-noninteractive-sudo-missing/incomplete.txt" >/dev/null
grep -F '$ sudo -n -v' "$TMP/package-root-noninteractive-sudo-missing/sudo-validate.txt" >/dev/null
grep -F "sudo: a password is required" "$TMP/package-root-noninteractive-sudo-missing/sudo-validate.txt" >/dev/null
if [[ -f "$TMP/package-root-noninteractive-sudo-missing/evidence.json" ]]; then
  echo "Failed non-interactive root smoke must not write release evidence.json" >&2
  exit 1
fi
if [[ -f "$TMP/package-root-noninteractive-sudo-missing/apt-install-reinstall.txt" ]]; then
  echo "Failed non-interactive root smoke must not reach apt install" >&2
  exit 1
fi

if CODEXBAR_LINUX_PACKAGE_SMOKE_SUDO_NONINTERACTIVE=1 \
  PATH="$FAKE_SUDO_BIN:$PATH" "$PACKAGE_SMOKE" \
  --deb "$STAGE_DEB" \
  --evidence-dir "$TMP/package-root-noninteractive-sudo-env-missing" \
  --purge \
  >"$TMP/package-root-noninteractive-sudo-env-missing.out" \
  2>"$TMP/package-root-noninteractive-sudo-env-missing.err"; then
  echo "Package root smoke without cached sudo credentials must not exit successfully when env forces non-interactive sudo" >&2
  exit 1
fi
grep -F "Checking non-interactive sudo access." "$TMP/package-root-noninteractive-sudo-env-missing.out" >/dev/null
grep -F "package-root-smoke: incomplete" "$TMP/package-root-noninteractive-sudo-env-missing/incomplete.txt" >/dev/null
grep -F "final-release-evidence: false" "$TMP/package-root-noninteractive-sudo-env-missing/incomplete.txt" >/dev/null
grep -F '$ sudo -n -v' "$TMP/package-root-noninteractive-sudo-env-missing/sudo-validate.txt" >/dev/null
if [[ -f "$TMP/package-root-noninteractive-sudo-env-missing/evidence.json" ]]; then
  echo "Failed env-forced non-interactive root smoke must not write release evidence.json" >&2
  exit 1
fi
if [[ -f "$TMP/package-root-noninteractive-sudo-env-missing/apt-install-reinstall.txt" ]]; then
  echo "Failed env-forced non-interactive root smoke must not reach apt install" >&2
  exit 1
fi

if CODEXBAR_LINUX_TEST_ARCH=arm64 "$PACKAGE_SMOKE" \
  --deb "$STAGE_DEB" \
  --evidence-dir "$TMP/package-stage-incomplete" \
  --stage-only \
  >"$TMP/package-stage-incomplete.out" 2>"$TMP/package-stage-incomplete.err"; then
  echo "Failed package smoke must not exit successfully" >&2
  exit 1
fi
grep -F "package-root-smoke: incomplete" "$TMP/package-stage-incomplete/incomplete.txt" >/dev/null
grep -F "final-release-evidence: false" "$TMP/package-stage-incomplete/incomplete.txt" >/dev/null
grep -F "evidence-json: absent" "$TMP/package-stage-incomplete/incomplete.txt" >/dev/null
if [[ -f "$TMP/package-stage-incomplete/evidence.json" ]]; then
  echo "Failed package smoke must not write release evidence.json" >&2
  exit 1
fi

mkdir -p "$REL_STAGE_DIR"
REL_STAGE_REL="target/release-evidence-stage-${BASHPID}/candidate.deb"
REL_STAGE_ABS="$REL_STAGE_DIR/candidate.deb"
build_stage_deb "$REL_STAGE_ABS" "$TMP/stage-deb-relative"
(
  cd "$ROOT"
  CODEXBAR_LINUX_TEST_ARCH=amd64 "$PACKAGE_SMOKE" \
    --deb "$REL_STAGE_REL" \
    --evidence-dir "$TMP/package-stage-relative" \
    --stage-only \
    >"$TMP/package-stage-relative.out"
)
grep -F "Package candidate staging passed" "$TMP/package-stage-relative.out" >/dev/null
grep -F "candidate: $REL_STAGE_ABS" "$TMP/package-stage-relative/summary.txt" >/dev/null

if "$VALIDATOR" --package-root "$TMP/package-stage-only/evidence.json" --gnome-matrix "$GNOME_FINAL" >"$TMP/stage-only-package.out" 2>"$TMP/stage-only-package.err"; then
  echo "Stage-only package evidence must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "package-stage preflight evidence, not final root-backed package smoke" "$TMP/stage-only-package.err" >/dev/null

if CODEXBAR_LINUX_RELEASE_CANDIDATE="$CANDIDATE_FILE" \
  CODEXBAR_LINUX_RELEASE_TMP_CANDIDATE="$TMP_CANDIDATE_FILE" \
  CODEXBAR_LINUX_TEST_ALLOW_DIRTY=1 \
  "$COMPLETION_AUDIT" --package-root "$TMP/package-stage-only/evidence.json" --gnome-matrix "$GNOME_FINAL" >"$TMP/stage-only-completion.out" 2>"$TMP/stage-only-completion.err"; then
  echo "Stage-only package evidence must not satisfy completion audit" >&2
  exit 1
fi
grep -F "package-stage preflight evidence, not final root-backed package smoke" "$TMP/stage-only-completion.err" >/dev/null
grep -F "Required final evidence commands:" "$TMP/stage-only-completion.out" >/dev/null
grep -F './scripts/package-root-smoke.sh --deb "$candidate" --purge' "$TMP/stage-only-completion.out" >/dev/null
grep -F "./scripts/gnome-matrix-smoke.sh --require-shell 50 --require-ubuntu 26.04 --require-package-path --require-wayland --pause-for-ui" "$TMP/stage-only-completion.out" >/dev/null

if CODEXBAR_LINUX_RELEASE_CANDIDATE="$CANDIDATE_FILE" \
  CODEXBAR_LINUX_RELEASE_TMP_CANDIDATE="$TMP_CANDIDATE_FILE" \
  CODEXBAR_LINUX_TEST_ALLOW_DIRTY=1 \
  "$COMPLETION_AUDIT" \
    --package-root "$TMP/package-stage-only/evidence.json" \
    --gnome-matrix "$GNOME_DEV" \
    --local-gate-log "$CHECK_LOG" \
    >"$TMP/multi-failure-completion.out" 2>"$TMP/multi-failure-completion.err"; then
  echo "Completion audit with stage-only package and development GNOME evidence must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "package-stage preflight evidence, not final root-backed package smoke" "$TMP/multi-failure-completion.err" >/dev/null
grep -F "is not final Ubuntu 26.04/GNOME 50 package-path evidence" "$TMP/multi-failure-completion.err" >/dev/null
grep -F "osVersionId='26.04' required, found '24.04'" "$TMP/multi-failure-completion.err" >/dev/null
grep -F "Local repository gate evidence matches current HEAD" "$TMP/multi-failure-completion.out" >/dev/null
grep -F "Required final evidence commands:" "$TMP/multi-failure-completion.out" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_DEV" >"$TMP/final-dev-gnome.out" 2>"$TMP/final-dev-gnome.err"; then
  echo "GNOME 46 development evidence must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "final GNOME evidence must have expectedShell=50" "$TMP/final-dev-gnome.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_BAD_UBUNTU" >"$TMP/bad-gnome-ubuntu.out" 2>"$TMP/bad-gnome-ubuntu.err"; then
  echo "GNOME evidence with non-26.04 Ubuntu runtime must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "final GNOME evidence must have osVersionId=26.04" "$TMP/bad-gnome-ubuntu.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_BAD_SHELL_MAJOR" >"$TMP/bad-gnome-shell-major.out" 2>"$TMP/bad-gnome-shell-major.err"; then
  echo "GNOME evidence with mismatched shell major must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "final GNOME evidence must have shellMajor=50" "$TMP/bad-gnome-shell-major.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_BAD_SHELL_VERSION_SIDECAR" >"$TMP/bad-gnome-shell-version-sidecar.out" 2>"$TMP/bad-gnome-shell-version-sidecar.err"; then
  echo "GNOME evidence with mismatched shell-version sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "gnome-shell-version.txt last payload must be: GNOME Shell 50.0" "$TMP/bad-gnome-shell-version-sidecar.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_BAD_OS_RELEASE_SIDECAR" >"$TMP/bad-gnome-os-release-sidecar.out" 2>"$TMP/bad-gnome-os-release-sidecar.err"; then
  echo "GNOME evidence with mismatched os-release sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "os-release.txt VERSION_ID does not match evidence osVersionId" "$TMP/bad-gnome-os-release-sidecar.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_BAD_SESSION_SIDECAR" >"$TMP/bad-gnome-session-sidecar.out" 2>"$TMP/bad-gnome-session-sidecar.err"; then
  echo "GNOME evidence with mismatched session-type sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "session-type.txt last payload must be: wayland" "$TMP/bad-gnome-session-sidecar.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_BAD_SESSION" >"$TMP/bad-gnome-session.out" 2>"$TMP/bad-gnome-session.err"; then
  echo "GNOME evidence without Wayland session must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "final GNOME evidence must have sessionType=wayland" "$TMP/bad-gnome-session.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_BAD_PACKAGE_PATH_FLAG" >"$TMP/bad-gnome-package-path-flag.out" 2>"$TMP/bad-gnome-package-path-flag.err"; then
  echo "GNOME evidence without package path verification must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "requirePackagePath=true" "$TMP/bad-gnome-package-path-flag.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_BAD_METADATA" >"$TMP/bad-gnome-metadata.out" 2>"$TMP/bad-gnome-metadata.err"; then
  echo "GNOME evidence with mismatched metadata sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F 'installed-extension-metadata.txt missing expected content: "50"' "$TMP/bad-gnome-metadata.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_BAD_METADATA_VERSION" >"$TMP/bad-gnome-metadata-version.out" 2>"$TMP/bad-gnome-metadata-version.err"; then
  echo "GNOME evidence with mismatched metadata version sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F 'installed-extension-metadata.txt missing expected content: "version": 1' "$TMP/bad-gnome-metadata-version.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_BAD_RUNTIME" >"$TMP/bad-gnome-runtime.out" 2>"$TMP/bad-gnome-runtime.err"; then
  echo "GNOME evidence with mismatched runtime sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "snapshot.txt missing expected content: GetSnapshot" "$TMP/bad-gnome-runtime.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_BAD_ENABLED" >"$TMP/bad-gnome-enabled.out" 2>"$TMP/bad-gnome-enabled.err"; then
  echo "GNOME evidence with mismatched enabled-extension sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "enabled-extensions.txt missing expected content: codexbar-linux@codexbar.dev" "$TMP/bad-gnome-enabled.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_BAD_INSTALLED_PACKAGE" >"$TMP/bad-gnome-installed-package.out" 2>"$TMP/bad-gnome-installed-package.err"; then
  echo "GNOME evidence with mismatched installed package metadata must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "GNOME evidence installedVersion does not match package-root installedVersion" "$TMP/bad-gnome-installed-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_BAD_ARCHITECTURE" >"$TMP/bad-gnome-architecture.out" 2>"$TMP/bad-gnome-architecture.err"; then
  echo "GNOME evidence with mismatched installed architecture must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "GNOME evidence installedArchitecture does not match package-root architecture" "$TMP/bad-gnome-architecture.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_FINAL" --gnome-matrix "$GNOME_BAD_FINAL_FLAG" >"$TMP/bad-gnome-final-flag.out" 2>"$TMP/bad-gnome-final-flag.err"; then
  echo "GNOME evidence with finalReleaseEvidence=false must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "finalReleaseEvidence=true" "$TMP/bad-gnome-final-flag.err" >/dev/null

for key in \
  metadataIncludesGnome50 \
  enabledExtensionVerified \
  manualRefreshVerified \
  diagnosticsRedactionScanPassed \
  daemonRestartVerified \
  ubuntuVersionVerified \
  requirePackagePath \
  requireWayland \
  packagePathVerified
do
  expect_gnome_bool_rejected "$key"
done

if "$VALIDATOR" --package-root "$PACKAGE_KEEP" --gnome-matrix "$GNOME_FINAL" >"$TMP/keep-package.out" 2>"$TMP/keep-package.err"; then
  echo "Package evidence with keepInstalled=true must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "removeVerified=true" "$TMP/keep-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_NO_PURGE" --gnome-matrix "$GNOME_FINAL" >"$TMP/no-purge-package.out" 2>"$TMP/no-purge-package.err"; then
  echo "Package evidence without purge must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "purgeAfterRemove=true" "$TMP/no-purge-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_FINAL_FLAG" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-package-final-flag.out" 2>"$TMP/bad-package-final-flag.err"; then
  echo "Package evidence with finalReleaseEvidence=false must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "finalReleaseEvidence=true" "$TMP/bad-package-final-flag.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_INCOMPLETE_MARKER" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-package-incomplete-marker.out" 2>"$TMP/bad-package-incomplete-marker.err"; then
  echo "Package evidence with incomplete marker must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "has incomplete package-smoke marker: incomplete.txt" "$TMP/bad-package-incomplete-marker.err" >/dev/null

for key in \
  usedAptReinstallFromTmp \
  sudoValidated \
  systemExtensionPathVerified \
  manualRefreshVerified \
  diagnosticsRedactionScanPassed \
  daemonRestartVerified
do
  expect_package_bool_rejected "$key"
done

if "$VALIDATOR" --package-root "$PACKAGE_STALE" --gnome-matrix "$GNOME_FINAL" >"$TMP/stale-package.out" 2>"$TMP/stale-package.err"; then
  echo "Package evidence with stale candidate sha must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "candidate file sha256 does not match candidateSha256" "$TMP/stale-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_ARCH" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-arch-package.out" 2>"$TMP/bad-arch-package.err"; then
  echo "Package evidence with mismatched architecture sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "candidate-fields.txt missing expected content: Architecture: amd64" "$TMP/bad-arch-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_COPY" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-copy-package.out" 2>"$TMP/bad-copy-package.err"; then
  echo "Package evidence with mismatched candidate copy sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "copy-candidate-to-tmp.txt missing expected content: cp" "$TMP/bad-copy-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_BYTE_COMPARE" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-byte-compare-package.out" 2>"$TMP/bad-byte-compare-package.err"; then
  echo "Package evidence with mismatched candidate byte-compare sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "candidate-byte-compare.txt missing expected content: cmp" "$TMP/bad-byte-compare-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_SUDO" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-sudo-package.out" 2>"$TMP/bad-sudo-package.err"; then
  echo "Package evidence with mismatched sudo validation sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "sudo-validate.txt missing expected content: sudo -v or sudo -n -v" "$TMP/bad-sudo-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_INSTALL_QUERY" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-install-query-package.out" 2>"$TMP/bad-install-query-package.err"; then
  echo "Package evidence with mismatched installed package query sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "installed-dpkg-query.txt payload lines must be" "$TMP/bad-install-query-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_DBUS" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-dbus-package.out" 2>"$TMP/bad-dbus-package.err"; then
  echo "Package evidence with mismatched D-Bus sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "daemon-info.txt missing expected content: GetDaemonInfo" "$TMP/bad-dbus-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_CONTENTS" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-contents-package.out" 2>"$TMP/bad-contents-package.err"; then
  echo "Package evidence with mismatched candidate contents sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "candidate-contents.txt missing expected content: usr/share/glib-2.0/schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml" "$TMP/bad-contents-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_DAEMON_RELOAD" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-daemon-reload-package.out" 2>"$TMP/bad-daemon-reload-package.err"; then
  echo "Package evidence with mismatched systemd daemon-reload sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "systemd-user-daemon-reload-after-remove.txt missing expected content: systemctl --user daemon-reload" "$TMP/bad-daemon-reload-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_EXTENSION_ENABLE" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-extension-enable-package.out" 2>"$TMP/bad-extension-enable-package.err"; then
  echo "Package evidence with mismatched extension enable sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "gnome-extensions-enable.txt missing expected content: gnome-extensions enable codexbar-linux@codexbar.dev" "$TMP/bad-extension-enable-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_EXTENSION_ENABLED" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-extension-enabled-package.out" 2>"$TMP/bad-extension-enabled-package.err"; then
  echo "Package evidence with mismatched enabled-extension state sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "enabled-extensions-after-enable.txt missing expected content: codexbar-linux@codexbar.dev" "$TMP/bad-extension-enabled-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_EXTENSION_DISABLE" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-extension-disable-package.out" 2>"$TMP/bad-extension-disable-package.err"; then
  echo "Package evidence with still-enabled post-disable sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "enabled-extensions-after-disable.txt contains forbidden content: codexbar-linux@codexbar.dev" "$TMP/bad-extension-disable-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_REMOVE_ABSENCE" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-remove-absence-package.out" 2>"$TMP/bad-remove-absence-package.err"; then
  echo "Package evidence with mismatched remove absence sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "removed-manpage-absent.txt missing expected content: /usr/share/man/man1/codexbar-linuxd.1.gz" "$TMP/bad-remove-absence-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_PURGE_QUERY" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-purge-query-package.out" 2>"$TMP/bad-purge-query-package.err"; then
  echo "Package evidence with successful post-purge dpkg query must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "purged-dpkg-query.txt contains forbidden content: exit-status: 0" "$TMP/bad-purge-query-package.err" >/dev/null

if "$VALIDATOR" --package-root "$PACKAGE_BAD_SIDECAR" --gnome-matrix "$GNOME_FINAL" >"$TMP/bad-sidecar-package.out" 2>"$TMP/bad-sidecar-package.err"; then
  echo "Package evidence with mismatched daemon-version sidecar must not satisfy final release evidence" >&2
  exit 1
fi
grep -F "installed-daemon-version.txt missing expected content" "$TMP/bad-sidecar-package.err" >/dev/null

if "$VALIDATOR" --gnome-matrix "$GNOME_FINAL" >"$TMP/missing-package.out" 2>"$TMP/missing-package.err"; then
  echo "Final release evidence must require package-root evidence" >&2
  exit 1
fi
grep -F "Final release evidence requires both --package-root and --gnome-matrix" "$TMP/missing-package.err" >/dev/null

echo "Release evidence validator tests passed"
