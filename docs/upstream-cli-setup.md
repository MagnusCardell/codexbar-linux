# Upstream CodexBar CLI Setup

CodexBar GNOME uses the upstream `codexbar` CLI and local provider tooling as
its production data plane. The GNOME Shell extension talks only to the
user-scoped daemon over D-Bus; provider data is collected by the daemon through
the CLI. The v0.2.0 compatibility target is upstream CodexBar CLI v0.25.1.

## Install Upstream CLI

Use one of the upstream CodexBar CLI installation paths, then verify the binary
before configuring `codexbar-linuxd`:

```bash
brew install steipete/tap/codexbar
```

or download a Linux release archive such as
`CodexBarCLI-v<tag>-linux-<arch>.tar.gz` from:

```text
https://github.com/steipete/CodexBar/releases
```

Install the extracted `codexbar` binary somewhere executable. If it is not on
the daemon's `PATH`, configure `CODEXBAR_CLI` as shown below.

## Verify CLI

Check the binary:

```bash
codexbar --version
```

Check Codex and Claude usage through the Linux CLI source:

```bash
codexbar --format json --json-only --provider codex --source cli
codexbar --format json --json-only --provider claude --source cli
```

Check local Codex + Claude cost summaries:

```bash
codexbar cost --format json --json-only --provider both
```

Check upstream config validation:

```bash
codexbar config validate --format json --json-only
```

If a provider reports that sign-in is required, authenticate the provider using
the provider's own CLI or upstream CodexBar setup flow, then rerun the command.

## Configure Packaged Daemon

v0.1 does not parse or migrate upstream CodexBar config files. The daemon only
discovers the upstream CLI executable through `CODEXBAR_CLI` or its service
environment `PATH`; provider authentication and provider-specific config remain
owned by upstream CodexBar and the provider CLIs.

When the `.deb` package is installed, run the user setup helper from the desktop
session. It reloads the user systemd manager, verifies daemon and D-Bus
activation, and enables the GNOME extension when the running Shell already
discovers it:

```bash
codexbar-linux-setup
```

If the upstream CLI is not on the daemon's service `PATH`, pass an absolute path
to the same helper. This sets the path in the systemd user manager environment
and restarts the user service:

```bash
codexbar-linux-setup --codexbar-cli /path/to/codexbar
```

Confirm D-Bus activation can see the configured daemon:

```bash
busctl --user call org.codexbar.Linux1 /org/codexbar/Linux1 org.codexbar.Linux1 GetDaemonInfo
```

Clear the override:

```bash
systemctl --user unset-environment CODEXBAR_CLI
systemctl --user restart codexbar-linuxd.service
```

## Expected Product Behavior

- If `codexbar` is missing, the UI shows a setup state and Refresh remains
  available.
- If `CODEXBAR_CLI` points at a non-executable file, diagnostics report that the
  configured path is not executable without exposing the raw path.
- If provider usage succeeds but local cost fails, usage remains visible and
  cost is shown as unavailable.
- If a live refresh fails and a useful normalized cache exists, the UI may show
  stale cached usage data.

Browser cookies, browser profile discovery, provider web fetches, browser
extensions, keyring access, and localhost/TCP APIs are intentionally unsupported
by this project.

## Optional v0.25.1 Capture

Normal CI does not require a live upstream v0.25.1 binary. Operators who need
fresh local evidence can capture redacted sidecars outside the committed fixture
tree:

```bash
CODEXBAR_CAPTURE_LIVE=1 CODEXBAR_CLI=/path/to/codexbar \
  ./scripts/capture-upstream-cli-samples.sh \
  --output /tmp/codexbar-upstream-cli-v0251 \
  --allow-provider-network \
  --providers codex,claude \
  --provider-source cli \
  --include-config-validate

./scripts/validate-upstream-cli-capture.sh /tmp/codexbar-upstream-cli-v0251
```

Review redacted stdout, stderr, metadata, and the manifest manually before
promoting any selected fixture. Do not commit raw terminal output or private
provider payloads.
