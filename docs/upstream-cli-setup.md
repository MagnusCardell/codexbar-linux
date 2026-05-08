# Upstream CodexBar CLI Setup

CodexBar GNOME uses the upstream `codexbar` CLI and local provider tooling as
its production data plane. The GNOME Shell extension talks only to the
user-scoped daemon over D-Bus; provider data is collected by the daemon through
the CLI.

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

Check Codex usage through the Linux CLI source:

```bash
codexbar --format json --json-only --provider codex --source cli
```

Check local cost summaries:

```bash
codexbar cost --format json --json-only --provider both
```

If a provider reports that sign-in is required, authenticate the provider using
the provider's own CLI or upstream CodexBar setup flow, then rerun the command.

## Configure Packaged Daemon

v0.1 does not parse or migrate upstream CodexBar config files. The daemon only
discovers the upstream CLI executable through `CODEXBAR_CLI` or its service
environment `PATH`; provider authentication and provider-specific config remain
owned by upstream CodexBar and the provider CLIs.

When the `.deb` package is installed, set the CLI path in the systemd user
manager environment and restart the user service:

```bash
systemctl --user set-environment CODEXBAR_CLI=/path/to/codexbar
systemctl --user restart codexbar-linuxd.service
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
