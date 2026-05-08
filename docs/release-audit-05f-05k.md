# Release Audit 05F-05K

This audit maps the 05F through 05K release objective to concrete repository
artifacts, checks, and remaining evidence gaps. It is not a release sign-off:
the candidate remains blocked until the root-backed package smoke and Ubuntu
26.04/GNOME 50 runtime smoke are recorded and validated.

## Objective Checklist

| Item | Requirement | Evidence | Status |
| --- | --- | --- | --- |
| 05F | Daemon auto-refresh scheduler with startup refresh | `daemon/src/dbus.rs`; `dbus_scheduler_runs_startup_refresh_when_enabled` | Implemented and tested |
| 05F | Interval refresh loop | `daemon/src/dbus.rs`; `dbus_scheduler_runs_interval_refresh_when_enabled` | Implemented and tested |
| 05F | Settings reschedule without daemon restart | `daemon/src/app.rs`; `settings_patch_advances_scheduler_revision` | Implemented and tested |
| 05F | Refresh failure clears active-refresh guard | `daemon/src/app.rs`; `failed_refresh_can_be_unwedged_without_daemon_restart` | Implemented and tested |
| 05F.1 | Hide or fix visible inert controls, especially `start-daemon-on-login` | `extension/prefs.js`; `schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml`; `docs/CONTRACTS.md`; `scripts/lint-gjs.sh` | Hidden/reserved for v0.1 and statically checked |
| 05G | Final root-backed package smoke with latest `.deb` | `scripts/package-root-smoke.sh`; `scripts/validate-release-evidence.sh`; candidate staged at `/tmp/codexbar-linux_0.1.0-1_amd64.deb` | Blocked until sudo-backed install/remove/purge evidence exists |
| 05H | Release-candidate gate and tag prep | `docs/release-candidate-gate.md`; `scripts/validate-release-gate.sh`; `scripts/test-release-evidence.sh` | Gate implemented; final tag remains blocked |
| 05H | Docs demote unimplemented promises | `README.md`; `docs/ACCEPTANCE.md`; `docs/release-notes-0.1.0.md`; `docs/release-smoke-test.md`; `docs/ROADMAP.md` | Current docs name remaining blockers and reject premature release claims |
| 05I | Ubuntu matrix smoke includes GNOME 50 metadata validation | `extension/metadata.json`; `scripts/lint-gjs.sh`; `scripts/build-deb.sh`; `scripts/install-local.sh`; `scripts/validate-packaging.sh` | Static metadata validation implemented |
| 05I | Ubuntu 26.04/GNOME 50 runtime validation | `scripts/gnome-matrix-smoke.sh`; `scripts/validate-release-evidence.sh` | Blocked until run on an Ubuntu 26.04/GNOME Shell 50 package session |
| 05J | Upstream CLI/provider quality follow-up | `docs/upstream-cli-setup.md`; `daemon/tests/upstream_cli_adapter.rs`; upstream CLI fixture/capture validators | v0.1 documents no upstream config migration; adapter behavior covered |
| 05K | Useful preferences/provider UX | `extension/prefs.js`; `scripts/lint-gjs.sh` | Daemon info, refresh interval, selected provider, provider enable/source controls implemented through `SetSettingsPatch` and statically checked |

## Release Evidence Commands

Run the root-backed package smoke against the exact `dist/` candidate artifact.
The helper copies that artifact to `/tmp` and records both paths in package
evidence; passing the `/tmp` copy as `--deb` will not satisfy the completion
audit's latest-artifact path check.

```bash
arch="$(dpkg --print-architecture)"
candidate="dist/codexbar-linux_0.1.0-1_${arch}.deb"
scripts/package-root-smoke.sh --deb "$candidate" --purge
```

If cached sudo credentials are required for unattended execution, add
`--noninteractive-sudo`; a missing credential cache still records an incomplete
package-smoke marker and is not final release evidence.

Run the GNOME 50 package runtime smoke on Ubuntu 26.04/GNOME Shell 50:

```bash
scripts/gnome-matrix-smoke.sh --require-shell 50 --require-ubuntu 26.04 --require-package-path --require-wayland --pause-for-ui
```

Validate the final manifests:

```bash
scripts/validate-release-evidence.sh \
  --package-root target/release-smoke/package-root-YYYYMMDDTHHMMSSZ/evidence.json \
  --gnome-matrix target/release-smoke/gnome-matrix-YYYYMMDDTHHMMSSZ/evidence.json
```

Run the full repository gate from the committed release checkout and save its
log:

```bash
mkdir -p target/release-smoke
check_log="target/release-smoke/check-$(date -u +%Y%m%dT%H%M%SZ).log"
./scripts/check.sh 2>&1 | tee "$check_log"
```

Run the full completion audit with the same explicit manifest paths and the
saved local gate log:

```bash
scripts/release-completion-audit.sh \
  --package-root target/release-smoke/package-root-YYYYMMDDTHHMMSSZ/evidence.json \
  --gnome-matrix target/release-smoke/gnome-matrix-YYYYMMDDTHHMMSSZ/evidence.json \
  --local-gate-log "$check_log"
```

The completion audit also hashes the current `dist/` candidate and its `/tmp`
copy and requires both hashes to match the package-root manifest
`candidateSha256`. This is the final guard that the root-backed smoke covered
the latest rebuilt `.deb`, not only an internally consistent old artifact. The
audit also requires the package-root manifest `candidate` and `tmpCandidate`
paths to match the current `dist/` artifact and `/tmp` copy, so a smoke of a
byte-identical package at a different path cannot satisfy the latest-artifact
gate. It also requires a clean git working tree and a saved `./scripts/check.sh` log whose final success marker matches the current `HEAD` before it reports
complete. The saved log must include the named scheduler, refresh-unwedge,
provider-settings, and upstream CLI fixture tests, so release docs and evidence
references must be committed and the repository gate must be rerun before the
final completion audit can pass.

Final evidence validation re-hashes the recorded candidate and `/tmp` package
files and requires the smoke sidecar logs next to each `evidence.json`.
Package-root evidence must include package field/content inspection,
candidate copy to `/tmp`, byte comparison between the candidate and `/tmp`
copy, `sudo -v` or `sudo -n -v`, `sudo apt install --reinstall`, installed package metadata
from `dpkg-query`, installed daemon/D-Bus/systemd metadata, GNOME extension
list/enable/post-enable-state/info/disable/post-disable-state logs, manual
refresh, diagnostics redaction, daemon restart, `sudo apt remove`, post-remove
absence checks for package-owned daemon, service, extension, schema, and
manpage paths, `sudo apt purge`, and a failing post-purge
`dpkg-query -W codexbar-linux`. It must also have
`finalReleaseEvidence: true`, which the helper only emits for the
install/remove/purge path.
GNOME matrix evidence must include Ubuntu `/etc/os-release`, GNOME Shell
version, session type, enabled extension state, installed metadata, installed
package metadata, D-Bus snapshot/refresh/diagnostics, diagnostics redaction, and
daemon restart, including installed extension metadata version 1. The final
validator cross-checks those runtime claims against the sidecars instead of
trusting only the JSON manifest: GNOME Shell version, session type, and
`/etc/os-release` values must agree with the recorded manifest fields. Final
GNOME evidence must have `finalReleaseEvidence: true`,
which the helper only emits for Ubuntu 26.04, GNOME Shell 50, Wayland, required
package path, and the exact system extension path. A stale or hand-edited
manifest is not sufficient release evidence.
The validator also rejects false release-critical manifest booleans: package
evidence must keep install-from-`/tmp`, sudo, system extension path, manual
refresh, diagnostics redaction, and daemon restart claims true; GNOME evidence
must keep GNOME 50 metadata, enabled extension, manual refresh, diagnostics
redaction, daemon restart, Ubuntu version, Wayland, and package-path
verification claims true.

## Current Blockers

- Latest rebuilt candidate staged locally:
  `/tmp/codexbar-linux_0.1.0-1_amd64.deb`
  (`sha256: 9cc89abbe66834caa1799f642b232eeee6e59f68933d871dc66a2005e87c4cb8`).
  Non-root candidate staging, checksum, byte-compare, fields, and contents
  inspection passed with
  `scripts/package-root-smoke.sh --deb dist/codexbar-linux_0.1.0-1_amd64.deb --evidence-dir /tmp/codexbar-package-stage-current.bCSRYa --stage-only`;
  this is package
  preflight evidence only. Its manifest has `smokeType: package-stage` and
  `finalReleaseEvidence: false`, so it does not satisfy final root-backed
  smoke.
- No final root-backed package evidence has been recorded for the latest staged
  candidate. Non-interactive sudo is unavailable in the current environment.
  A non-interactive attempt at
  `/tmp/codexbar-package-root-noninteractive-attempt` ran
  `scripts/package-root-smoke.sh --deb dist/codexbar-linux_0.1.0-1_amd64.deb --evidence-dir /tmp/codexbar-package-root-noninteractive-attempt --purge --noninteractive-sudo`;
  it failed at `sudo -n -v` with `sudo: a password is required`, wrote
  `incomplete.txt` with `final-release-evidence: false`, and did not produce
  `evidence.json`; it did not install or remove the package.
  An attempted root-backed run at `/tmp/codexbar-package-root-final-attempt`
  reached `sudo -v`, prompted for the local account password, was interrupted
  before package install, and did not produce `evidence.json`; it is not release
  evidence.
- No Ubuntu 26.04/GNOME Shell 50 runtime evidence has been recorded. The local
  GNOME helper smoke at `/tmp/codexbar-gnome-dev-smoke.7IKG9C/evidence.json`
  (`sha256: be9d6021a94ee10a31d4a60046c96832f153c4f8fae9e54d496f54032d5adfd7`)
  recorded Ubuntu 24.04, GNOME Shell 46.0, Wayland, a user-local extension
  path, and `finalReleaseEvidence: false`; it is useful development evidence
  only and does not satisfy the final matrix gate.
- Do not create or push `v0.1.0` until both evidence manifests validate in final
  mode and release notes are updated with the recorded host class, artifact, and
  commands.

## Current Green Checks

The repository gate includes static release checks and synthetic evidence
validator tests:

```bash
./scripts/check.sh
```

Passing `./scripts/check.sh` is necessary but not sufficient for release
sign-off because it intentionally does not run privileged apt operations or a
live GNOME 50 desktop session.
