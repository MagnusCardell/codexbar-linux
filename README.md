# CodexBar GNOME

Native Ubuntu/GNOME top-bar usage monitoring for AI coding providers, powered
by the upstream [CodexBar](https://github.com/steipete/CodexBar) CLI.

CodexBar GNOME installs a GNOME Shell extension and a user-scoped daemon. The
extension renders the panel indicator and popover. The daemon talks to the
upstream `codexbar` executable over local process calls, normalizes the result,
caches a snapshot, and exposes it to GNOME Shell over D-Bus.

This is not Electron, Tauri, an AppIndicator tray menu, a browser extension, or
a localhost web service.

## Status

CodexBar GNOME v0.1.0 is a development `.deb` package for Ubuntu/GNOME users
who are comfortable installing a local package and configuring the upstream
CLI.

Primary target:

- Ubuntu Desktop 24.04 LTS
- GNOME Shell 46+
- Wayland-first desktop sessions
- `systemd --user` and D-Bus session activation

Release gates before v0.1.0 final:

- Ubuntu 26.04 LTS/GNOME 50 compatibility as a release gate.
- Full Ubuntu 24.04/26.04 package smoke matrix sign-off.
- Historical root-backed package install smoke evidence remains tracked for
  release audit context.

## What You Get

- A native GNOME top-bar indicator.
- A popover with provider cards, usage meters, stale/error states, daemon info,
  manual refresh, and compact diagnostics utilities.
- Preferences for panel mode, refresh interval, provider visibility, and
  provider source settings.
- A user daemon, `codexbar-linuxd`, activated through the D-Bus session service
  `org.codexbar.Linux1`.
- Normalized local cache for fast startup and stale rendering.
- Upstream `codexbar` CLI integration for Linux-supported CLI/API/local provider
  data and local cost summaries.

## Screenshot Guidance

README screenshots should show the default popover with diagnostics collapsed.
Frame the provider selector, Session, Weekly, Credits, Cost, and Refresh as the
primary product surface; keep Load diagnostics and Settings as small footer
utilities, and do not use expanded diagnostics as the hero state.

## Requirements

You need both pieces:

1. **CodexBar GNOME** from this repository.
2. **Upstream CodexBar CLI** from
   [steipete/CodexBar](https://github.com/steipete/CodexBar).

The `.deb` package does not bundle upstream `codexbar`. If the UI says
`upstream_cli_missing`, install upstream CodexBar CLI or point the daemon at its
path with `CODEXBAR_CLI`.

## Quick Start

### 1. Install Upstream CodexBar CLI

Linuxbrew:

```bash
brew install steipete/tap/codexbar
```

Or download a Linux CLI tarball from upstream releases:

```text
https://github.com/steipete/CodexBar/releases
```

Use the archive that matches your architecture, for example:

- `CodexBarCLI-v<tag>-linux-x86_64.tar.gz`
- `CodexBarCLI-v<tag>-linux-aarch64.tar.gz`

Extract it somewhere stable, such as:

```bash
mkdir -p ~/.local/bin/codexbar-upstream
tar -xzf CodexBarCLI-v<tag>-linux-x86_64.tar.gz -C ~/.local/bin/codexbar-upstream
chmod +x ~/.local/bin/codexbar-upstream/codexbar
```

Verify the upstream CLI before configuring GNOME:

```bash
codexbar --format json --json-only --provider codex --source cli
codexbar cost --format json --json-only --provider both
```

If `codexbar` is not on your interactive shell `PATH`, use the extracted path:

```bash
~/.local/bin/codexbar-upstream/codexbar --version
```

### 2. Install CodexBar GNOME

From a downloaded `.deb`:

```bash
arch="$(dpkg --print-architecture)"
sudo apt install "./codexbar-linux_0.1.0-1_${arch}.deb"
systemctl --user daemon-reload
```

From this repository:

```bash
./scripts/build-deb.sh
arch="$(dpkg --print-architecture)"
cp "dist/codexbar-linux_0.1.0-1_${arch}.deb" /tmp/
sudo apt install --reinstall "/tmp/codexbar-linux_0.1.0-1_${arch}.deb"
systemctl --user daemon-reload
```

### 3. Let the Daemon Find `codexbar`

If you installed with Linuxbrew in one of the standard Linuxbrew locations, the
daemon should find `codexbar` automatically.

If the UI shows `upstream_cli_missing`, set the executable path for the systemd
user manager and restart the daemon:

```bash
systemctl --user set-environment CODEXBAR_CLI=/absolute/path/to/codexbar
systemctl --user restart codexbar-linuxd.service
```

For a persistent login environment, write the same absolute path to
`~/.config/environment.d`:

```bash
mkdir -p ~/.config/environment.d
env_file=~/.config/environment.d/codexbar-linux.conf
printf '%s\n' 'CODEXBAR_CLI=/absolute/path/to/codexbar' > "$env_file"
systemctl --user daemon-reload
systemctl --user restart codexbar-linuxd.service
```

Do not use `~` inside `environment.d`; write the full path.

### 4. Enable the GNOME Extension

The package installs the extension, but it does not enable it for you.

```bash
gnome-extensions enable codexbar-linux@codexbar.dev
gnome-extensions info codexbar-linux@codexbar.dev
```

On Wayland, log out and back in if GNOME Shell does not discover the system
extension immediately after install.

The packaged extension path should be:

```text
/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev
```

If `gnome-extensions info` reports a path under `~/.local/share`, a development
copy is shadowing the packaged extension.

### 5. Verify the Daemon

```bash
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
journalctl --user -u codexbar-linuxd.service -n 100 --no-pager
```

Open the top-bar item and use Refresh. If provider authentication is missing,
the UI should show a recoverable provider/auth state rather than
`upstream_cli_missing`.

## Troubleshooting

### `upstream_cli_missing`

The daemon could not find an executable named `codexbar`.

Fix:

```bash
codexbar --version
systemctl --user set-environment CODEXBAR_CLI=/absolute/path/to/codexbar
systemctl --user restart codexbar-linuxd.service
```

If `codexbar --version` fails, install upstream CodexBar CLI first.

### `upstream_cli_not_executable`

The path exists, but it is not executable.

Fix:

```bash
chmod +x /absolute/path/to/codexbar
systemctl --user restart codexbar-linuxd.service
```

### Extension Does Not Appear

Check discovery:

```bash
gnome-extensions list | grep codexbar
gnome-extensions info codexbar-linux@codexbar.dev
```

On Wayland, log out and back in after installing system extension files.

### Package Install Shows `_apt` Sandbox Warning

Installing a local `.deb` from a private project directory can produce a
non-fatal `_apt` sandbox warning. Copy the package to `/tmp` and install from
there:

```bash
cp dist/codexbar-linux_0.1.0-1_$(dpkg --print-architecture).deb /tmp/
sudo apt install --reinstall /tmp/codexbar-linux_0.1.0-1_$(dpkg --print-architecture).deb
```

### Provider Shows Signed Out or Unavailable

CodexBar GNOME does not own provider login. Authenticate through the provider's
own CLI or upstream CodexBar setup, then rerun:

```bash
codexbar --format json --json-only --provider codex --source cli
```

## Privacy and Scope

CodexBar GNOME intentionally does not:

- read browser cookies;
- scan browser profiles;
- decrypt browser session material;
- read desktop keyrings or Secret Service entries;
- scrape provider dashboards;
- install a browser extension;
- expose a localhost or TCP API.

The daemon caches normalized snapshots, cost summaries, timestamps, safe source
metadata, and redacted diagnostics. Raw provider payloads and secrets are not
cached.

The retained `TestBrowserImport` D-Bus method is compatibility-only. It returns
a schema-valid `not_implemented` result without touching browser paths,
profiles, cookies, keyrings, or provider endpoints.

## How It Works

```text
GNOME Shell extension
        |
        | D-Bus session API: org.codexbar.Linux1
        v
codexbar-linuxd user daemon
        |
        | local process invocation
        v
upstream codexbar CLI and local provider tooling
```

Important paths:

```text
~/.config/codexbar-linux/config.json
~/.cache/codexbar-linux/snapshot.json
/usr/bin/codexbar-linuxd
/usr/share/dbus-1/services/org.codexbar.Linux1.service
/usr/lib/systemd/user/codexbar-linuxd.service
/usr/share/gnome-shell/extensions/codexbar-linux@codexbar.dev
```

## Supported Data Sources

The supported production data plane is upstream `codexbar` CLI plus local
provider tooling.

The first proven Linux usage/status provider is `codex` through the CLI source.
Current upstream cost output is a local Codex + Claude cost scan. CodexBar
GNOME requests both supported local cost providers:

```bash
codexbar cost --format json --json-only --provider both
```

Other providers depend on what upstream CodexBar CLI supports on Linux through
CLI, API, OAuth, or local tooling. Browser/web-only provider collection remains
out of scope for CodexBar GNOME v0.1.

## Build and Development

Run the full repository check:

```bash
./scripts/check.sh
```

Useful narrower checks:

```bash
./scripts/validate-dbus.sh
./scripts/validate-schemas.sh
./scripts/validate-gsettings.sh
./scripts/validate-packaging.sh
./scripts/validate-no-browser-web-surface.sh
./scripts/build-deb.sh --check
./scripts/test-fixtures.sh
./scripts/lint-gjs.sh
cargo fmt --manifest-path daemon/Cargo.toml -- --check
cargo clippy --manifest-path daemon/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path daemon/Cargo.toml
```

Build the development package:

```bash
./scripts/build-deb.sh
```

Run fixture-backed local development:

```bash
CODEXBAR_LINUX_ALLOW_FIXTURE=1 cargo run --manifest-path daemon/Cargo.toml
```

Optional live upstream CLI tests are ignored by default:

```bash
CODEXBAR_LIVE=1 CODEXBAR_CLI=/path/to/codexbar \
  cargo test --manifest-path daemon/Cargo.toml -- --ignored --test-threads=1
```

## Docs

- [Upstream CodexBar CLI Setup](docs/upstream-cli-setup.md)
- [Release Notes 0.1.0](docs/release-notes-0.1.0.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Security](docs/SECURITY.md)
- [Contracts](docs/CONTRACTS.md)
- [Release Smoke Test](docs/release-smoke-test.md)
- [GNOME Smoke Test](docs/gnome-smoke-test.md)
- [Upstream CLI UX States](docs/upstream-cli-ux.md)

## Relationship to Upstream CodexBar

CodexBar GNOME is a native Linux/GNOME companion for upstream CodexBar. It
preserves upstream provider semantics where Linux CLI/local sources make that
possible, but it does not fork the provider framework into browser scraping or
keyring/session extraction on Linux.

Upstream project:

```text
https://github.com/steipete/CodexBar
```

## License

See [LICENSE](LICENSE).
