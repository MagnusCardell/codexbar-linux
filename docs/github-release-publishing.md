# GitHub Release Publishing

CodexBar GNOME can publish the generated `.deb` as a GitHub Release asset.
This is a download distribution path, not an apt repository. Users still
install the package with `apt install ./package.deb`, and the package still
expects upstream CodexBar CLI to be installed separately.

## What To Publish

Publish the package artifact printed by:

```bash
./scripts/build-deb.sh
```

The build script writes `dist/codexbar-linux.deb`. Attach that `.deb` to the
matching GitHub Release. A checksum sidecar is useful for downloads:

```bash
sha256sum dist/codexbar-linux.deb > dist/SHA256SUMS
```

Release naming uses one semantic version, for example `v0.2.0`.

## Release Page Workflow

Before publishing an asset:

```bash
./scripts/check.sh
./scripts/build-deb.sh --check
./scripts/build-deb.sh
```

Then publish the `.deb` through one of these paths:

- GitHub web UI: create or edit the release, then attach the `.deb` and
  `SHA256SUMS` file as release assets.
- GitHub CLI, when available:

```text
gh release create v0.2.0 \
  dist/codexbar-linux.deb \
  dist/SHA256SUMS \
  --repo MagnusCardell/codexbar-linux \
  --title "CodexBar GNOME v0.2.0" \
  --notes-file docs/release-notes-0.2.0.md
```

For an existing draft or published release:

```text
gh release upload v0.2.0 \
  dist/codexbar-linux.deb \
  dist/SHA256SUMS \
  --repo MagnusCardell/codexbar-linux
```

## User Install Text

Release notes can point users at the GitHub Release asset and then use:

```text
cp -f ./codexbar-linux.deb /tmp/codexbar-linux.deb
sudo apt install --reinstall /tmp/codexbar-linux.deb
codexbar-linux-setup
```

The `/tmp` copy is intentional: it overwrites any previous local test artifact
and avoids non-fatal `_apt` sandbox warnings from private download directories.

## Boundaries

Publishing the `.deb` as a GitHub Release asset does not change the runtime
architecture:

- no bundled upstream CodexBar CLI;
- no browser-cookie or browser-profile access;
- no keyring/session extraction;
- no provider dashboard scraping;
- no localhost or TCP API;
- no Shell subprocess data plane.

Do not attach live upstream CLI captures, private smoke logs, or unredacted
diagnostics to a public release.
