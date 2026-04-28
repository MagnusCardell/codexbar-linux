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

### C. Browser-cookie path

- Browser import detects supported browser profiles.
- Keyring locked/unavailable state is actionable.
- Cookie absent and cookie rejected are distinct.
- Raw cookies never appear in cache, logs, D-Bus output, or copied diagnostics.
- Provider web adapter output normalizes into the same snapshot shape as CLI output.

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
- Browser import test produces clear results.
- Diagnostics page shows daemon status, CLI path/version, cache path, and D-Bus service.

## Engineering acceptance

- `./scripts/check.sh` passes.
- Unit tests cover redaction, schema normalization, CLI error mapping, cache read/write, and provider state mapping.
- UI fixture tests cover success, stale, unauthenticated, cookie rejected, missing dependency, timeout, and parse error.
- ADRs are updated for architectural changes.
