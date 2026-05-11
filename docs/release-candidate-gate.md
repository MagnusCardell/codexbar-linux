# Release Candidate Gate

This document is the v0.1 tag-prep checklist. It is intentionally conservative:
the repository may prepare a release candidate package, but the final `v0.1.0`
tag must not be created until every required smoke result below is recorded
against the exact candidate artifact.

For the prompt-to-artifact checklist across 05F through 05K, see
`docs/release-audit-05f-05k.md`.

## Current Decision

The current v0.1 candidate is blocked from final tag creation until:

- the latest rebuilt `.deb` candidate has a real root-backed
  install/remove/purge smoke from `/tmp`;
- Ubuntu 26.04/GNOME 50 metadata and runtime validation is recorded;
- package-extension UI smoke proves the system extension path under
  `/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev`;
- release notes and smoke docs name the exact package artifact, host class,
  GNOME Shell version, and commands used.

Historical package smoke evidence remains useful context, but it is not
evidence for the latest rebuilt `.deb`.

## Candidate Artifact

Build the candidate from the checkout being tagged:

```bash
mkdir -p target/release-smoke
check_log="target/release-smoke/check-$(date -u +%Y%m%dT%H%M%SZ).log"
./scripts/check.sh 2>&1 | tee "$check_log"
./scripts/build-deb.sh
candidate="dist/codexbar-linux.deb"
test -f "$candidate"
cp -f "$candidate" /tmp/codexbar-linux.deb
```

The root package gate can be captured with:

```bash
scripts/package-root-smoke.sh --deb "$candidate" --purge
```

On automation hosts where sudo credentials are already cached, add
`--noninteractive-sudo` to fail fast instead of prompting. The equivalent
environment override is `CODEXBAR_LINUX_PACKAGE_SMOKE_SUDO_NONINTERACTIVE=1`:

```bash
scripts/package-root-smoke.sh --deb "$candidate" --purge --noninteractive-sudo
```

If sudo is unavailable, run a candidate-only preflight instead:

```bash
scripts/package-root-smoke.sh --deb "$candidate" --stage-only
```

That preflight records package fields, checksums, byte comparison against the
`/tmp` candidate, and required package-owned paths. It writes a
`smokeType: package-stage` manifest with `finalReleaseEvidence: false`, which
the final evidence validator rejects as root-backed package evidence. It is
useful for catching a bad artifact early, but it is not release evidence and
does not replace the install/remove/purge command above.

The Ubuntu 26.04/GNOME 50 runtime gate can be captured from the installed
package session with:

```bash
scripts/gnome-matrix-smoke.sh --require-shell 50 --require-ubuntu 26.04 --require-package-path --require-wayland --pause-for-ui
```

Validate both generated evidence manifests before considering the candidate for
tagging. The validator does not trust `evidence.json` by itself: it re-hashes
the recorded candidate and `/tmp` package files, requires and inspects the
package smoke sidecar logs for candidate copy to `/tmp`, byte comparison,
`sudo -v` or `sudo -n -v`, apt install/remove/purge, manual refresh, diagnostics, daemon
restart, installed package metadata from `dpkg-query`,
systemd user daemon-reload after install/remove/purge,
installed daemon/service metadata, GNOME extension
list/enable/post-enable-state/info/disable/post-disable-state logs, and the
candidate package contents for the daemon, D-Bus service, systemd user unit,
GSettings schema, GNOME metadata, and manpage. The remove evidence must also
record absent checks for package-owned executable, D-Bus service, systemd user
unit, extension directory, GSettings schema, and manpage paths. The purge
evidence must record a failing `dpkg-query -W codexbar-linux` after `sudo apt
purge`. It also requires and inspects the GNOME matrix sidecar logs for Ubuntu
`/etc/os-release`, shell version, session type, enabled extension state,
metadata, manual refresh, diagnostics, daemon stop/restart, and installed
package metadata from `dpkg-query`.
The GNOME matrix `installedVersion` and `installedArchitecture` fields must
match the package-root evidence, so a GNOME 50 runtime capture cannot satisfy
the gate with a different installed package. Both final manifests must also
carry `finalReleaseEvidence: true`; package-stage preflight and development
GNOME manifests are rejected in final mode.
The installed GNOME extension metadata must match the v0.1 package contract:
UUID, settings schema, GNOME 46/50 shell-version validation anchors, compatibility-declared GNOME 47-49 entries, and extension metadata `version` must remain `1`.
The validator cross-checks GNOME runtime claims against the sidecars: the last
captured payload in `gnome-shell-version.txt` must match the manifest shell
version, the last payload in `session-type.txt` must match the manifest session
type, and `/etc/os-release` sidecar values must match the manifest OS fields.
Release-critical manifest booleans are validated as evidence claims, not
informational fields: package evidence must keep install-from-`/tmp`, sudo,
system extension path, manual refresh, diagnostics redaction, and daemon
restart booleans true; GNOME evidence must keep GNOME 50 metadata, enabled
extension, manual refresh, diagnostics redaction, daemon restart, Ubuntu
version, Wayland, and package-path verification booleans true.

```bash
package_evidence="target/release-smoke/package-root-YYYYMMDDTHHMMSSZ/evidence.json"
gnome_evidence="target/release-smoke/gnome-matrix-YYYYMMDDTHHMMSSZ/evidence.json"
scripts/validate-release-evidence.sh \
  --package-root "$package_evidence" \
  --gnome-matrix "$gnome_evidence"
```

Run the completion audit against those same explicit manifests before tag
creation. It intentionally does not discover "latest" evidence automatically,
so stale manifests cannot satisfy the gate by accident. It also compares the
package-root evidence hash to the current `dist/` candidate and `/tmp` copy, so
the final root-backed smoke must correspond to the latest rebuilt `.deb`. It
also requires the package-root manifest `candidate` and `tmpCandidate` paths to
match the current `dist/` artifact and `/tmp` copy, so a byte-identical smoke of
a different path cannot satisfy the latest-artifact gate. The completion audit
reports complete only from a clean git working tree and a saved `./scripts/check.sh` log whose final success marker matches the current `HEAD`.
That saved log must also include the explicit scheduler, refresh unwedge,
provider-settings, and upstream CLI fixture tests that map to the 05F, 05J, and
05K release requirements, so commit release note/evidence updates and rerun the
repository gate before the final audit run:

```bash
scripts/release-completion-audit.sh \
  --package-root "$package_evidence" \
  --gnome-matrix "$gnome_evidence" \
  --local-gate-log "$check_log"
```

Record:

- `git rev-parse HEAD`
- the `./scripts/check.sh` log passed as `--local-gate-log`
- `dpkg-deb --field "$candidate"`
- `dpkg-deb --contents "$candidate"`
- checksum of both `"$candidate"` and `"/tmp/$(basename "$candidate")"`
- the `evidence.json` file written by each smoke helper under
  `target/release-smoke/`
- the sidecar logs in the same evidence directories; final validation requires
  these logs to exist alongside each `evidence.json`

## Required Root-Backed Package Smoke

Run these against the copied `/tmp` artifact on the target Ubuntu GNOME host:

```bash
cp -f dist/codexbar-linux.deb /tmp/codexbar-linux.deb
sha256sum dist/codexbar-linux.deb /tmp/codexbar-linux.deb
cmp dist/codexbar-linux.deb /tmp/codexbar-linux.deb
sudo -v
sudo apt install --reinstall /tmp/codexbar-linux.deb
systemctl --user daemon-reload
/usr/bin/codexbar-linuxd --version
/usr/bin/codexbar-linuxd --check
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
gnome-extensions info codexbar-linux@codexbar.dev
sudo apt remove codexbar-linux
systemctl --user daemon-reload
```

Final release purge gate:

```bash
sudo apt purge codexbar-linux
systemctl --user daemon-reload
```

Final v0.1 evidence is not valid unless the package smoke records both
`sudo apt remove` and `sudo apt purge` sidecar logs. The package smoke is not
valid unless `gnome-extensions info` reports:

```text
Path: /usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev
```

If the reported path is under `~/.local/share`, a user-local development
extension is shadowing the package and the package UI smoke must be rerun after
that shadow is removed or moved aside.

## Required GNOME Matrix Evidence

Record at least:

- Ubuntu 24.04 LTS / GNOME 46 local or package smoke;
- Ubuntu 26.04 LTS / GNOME 50 package metadata and runtime smoke;
- Wayland session type;
- GNOME Shell version from `gnome-shell --version`;
- extension path from `gnome-extensions info`;
- daemon D-Bus activation result;
- daemon stop/restart recovery result;
- manual refresh and diagnostics redaction result.

Static evidence that `metadata.json` lists GNOME 50 is necessary but not
sufficient for this gate.

## Pre-Tag Checklist

Only after the root-backed package smoke and GNOME matrix evidence are recorded:

- update `docs/release-notes-0.1.0.md` with the final smoke evidence summary;
- update `docs/release-smoke-test.md` if any command changed during the smoke;
- confirm `./scripts/check.sh` still passes after documentation updates and
  save the log that ends with `repository gate passed for HEAD ...`;
- confirm `git status --short` contains only intentional release changes;
- run `scripts/release-completion-audit.sh` with the final package evidence,
  final GNOME evidence, and the saved `--local-gate-log`;
- create the annotated tag from the audited commit.

Suggested tag command after all gates pass:

```bash
git tag -a v0.1.0 -m "codexbar-linux v0.1.0"
```

Do not create `v0.1.0` while any item in this document is missing or only
covered by historical smoke evidence.
