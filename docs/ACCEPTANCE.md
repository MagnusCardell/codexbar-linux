# Acceptance criteria

## Product acceptance

### A. Install and first launch

- On Ubuntu Desktop 24.04 LTS and 26.04 LTS, installing the Debian package places the daemon, D-Bus service, systemd user unit, GSettings schema, and GNOME Shell extension files in expected locations.
- The package does not silently enable the extension.
- After explicit user enablement, a top-bar item appears without requiring X11.
- If daemon is not running, D-Bus activation starts it or the UI shows a clear recoverable state.

### B. Upstream CLI path

- If `codexbar` is on PATH and configured, daemon can fetch `usage` JSON.
- If `codexbar cost` is available, cost summaries appear in diagnostics/card secondary detail where appropriate.
- CLI missing, timeout, parse error, and non-zero exit states are distinct.
- Upstream provider IDs/order are preserved where possible.

### C. No-browser data path

- The daemon does not read browser cookies, browser profiles, browser cookie databases, keyrings, or provider web dashboards.
- `TestBrowserImport` returns a schema-valid `not_implemented` result and has no browser/cache/settings side effects.
- Browser-cookie/web-fetch implementation modules, fixtures, validators, direct dependencies, and project agent are absent.
- Static validation fails if the removed browser-cookie/web-fetch surface is reintroduced.
- Raw tokens, cookie/header strings, local paths, and raw provider payloads never appear in cache, logs, D-Bus output, fixtures, or copied diagnostics.

### D. Panel indicator

- Merged mode shows one item with two micro-bars where data exists.
- Provider mode shows one compact item per enabled provider without overflow for two to four providers.
- Minimal mode shows a low-noise icon/percent.
- Stale/error/unauthenticated states are visible but not visually noisy.

### E. Popover

- Popover renders cards for enabled providers.
- Loading and loaded states have stable dimensions.
- Manual refresh is always reachable.
- Dashboard links open in default browser.
- Diagnostics copy action redacts secrets.

### F. Preferences

- General preferences save and apply.
- Provider enable/source preferences save and apply.
- Reserved browser import test reports unsupported/no-op behavior.
- Diagnostics page shows daemon status, CLI path/version, cache path, and D-Bus service.

## Engineering acceptance

- `./scripts/check.sh` passes.
- Unit tests cover redaction, schema normalization, CLI error mapping, cache read/write, and provider state mapping.
- UI fixture tests cover success, stale, unauthenticated, cookie rejected, missing dependency, timeout, and parse error.
- ADRs are updated for architectural changes.

## v0.1 Release Acceptance

These are the explicit pass/fail gates for a release-quality upstream-CLI-only
v0.1 candidate. A candidate is not releasable until each item is recorded with
the command or smoke evidence used.

- No-browser/web guard passes: `./scripts/validate-no-browser-web-surface.sh`
  reports no browser-cookie, browser-profile, keyring, provider web-fetch,
  browser-extension, localhost/TCP, forbidden dependency, fixture, validator, or
  runtime marker surface.
- Full repository gate passes: `./scripts/check.sh` completes successfully on
  Ubuntu 24.04 or newer.
- Live GNOME 46+ Wayland smoke passes after `./scripts/install-local.sh`, an
  explicit user enablement, and a session restart when Wayland discovery
  requires it.
- Upstream CLI missing degraded UI passes: with no resolvable `codexbar`, manual
  refresh remains available, provider state is `missing_dependency`, and
  diagnostics are copyable and redacted.
- Upstream CLI available Codex refresh passes: with a configured upstream
  `codexbar`, targeted Codex usage/status refresh succeeds through
  `sourceAdapter=upstream_cli`, cost is attempted through `provider all`, and no
  raw stdout/stderr, identity, token, cookie, or local path crosses D-Bus/cache.
- Daemon stop/restart recovery passes: stopping `codexbar-linuxd.service`
  renders a recoverable daemon-unavailable UI, and restarting the service lets
  Retry or Refresh recover without reloading GNOME Shell.
- Install/uninstall smoke passes: local install places only owned runtime files
  in user-local paths, does not auto-enable the extension, compiles schemas
  strictly, reloads the user systemd manager, and uninstall removes owned files
  while preserving user config/cache.
- Package skeleton status is clear: `scripts/build-deb.sh` intentionally fails
  with the Task 08 not-implemented message. A real `.deb` release cannot be
  claimed until Task 08 wires Debian packaging to build from source.
- Diagnostics/cache/log-like output redaction passes: copied diagnostics,
  normalized cache, D-Bus payloads, fixture outputs, and command summaries
  contain no raw secrets, raw identity, raw provider payloads, raw paths,
  stdout/stderr snippets, cookie/header strings, or browser/session material.
