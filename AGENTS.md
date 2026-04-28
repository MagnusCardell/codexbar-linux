# AGENTS.md — CodexBar GNOME

## Mission

Build `codexbar-linux`: a native Ubuntu/GNOME top-bar companion for upstream CodexBar. The product is a GNOME Shell extension backed by a user-scoped daemon. Upstream `codexbar` CLI is the default data plane wherever Linux support exists; Linux-native browser-cookie import fills the upstream Linux web-source gap.

## Non-negotiable architecture

- Do not build an Electron, Tauri, AppIndicator-only, browser-extension-first, or localhost-HTTP-first product.
- Shell UI runs in GNOME Shell via GJS ESModules. Use `St`, `Clutter`, `Gio`, `GLib`, `GObject`, and Shell UI modules as appropriate.
- Do not import `Gtk`, `Gdk`, or `Adw` from `extension.js` or any code loaded into the GNOME Shell process.
- Preferences run in `prefs.js` and may use GTK4/libadwaita. Do not import Shell-only libraries such as `St`, `Clutter`, `Meta`, or `Shell` in preferences.
- The daemon is user-scoped and exposes a D-Bus session API. No TCP listener by default.
- Browser cookies are read just-in-time and used in memory. Do not persist raw cookies, bearer tokens, session keys, decrypted browser secrets, or full request headers.
- Cache only normalized usage snapshots, cost summaries, timestamps, provider source/sourceAdapter metadata, and redacted diagnostics.
- Preserve upstream provider semantics: provider IDs, labels, reset windows, safe identity display/hash fields, `source`, status, credits, and cost output where available. Do not preserve raw identity fields across the daemon boundary.

## Preferred implementation stack

- Daemon: Rust, `tokio`, `zbus`, `serde`, `serde_json`, `time`, `tracing`, `rusqlite` or SQLite via a safe wrapper, `reqwest`/TLS for provider web fetches, platform keyring/Secret Service integration through a reviewed crate or direct D-Bus Secret Service calls.
- Shell extension: GJS ESModules targeting GNOME 46+ first.
- Preferences: GJS with GTK4/libadwaita, GSettings for UI preferences, daemon config writes through the D-Bus API or a documented config file path.
- Packaging: Debian package first; systemd user service plus D-Bus service activation; local development install scripts.

## Repository map

- `daemon/`: daemon implementation.
- `extension/`: Shell extension implementation.
- `schemas/`: GSettings schema.
- `spec/`: D-Bus, snapshot, settings, diagnostics schemas. Treat these as contracts.
- `docs/`: product, architecture, security, roadmap, ADRs.
- `tasks/`: implementation tickets intended for agents.
- `prompts/`: repeatable dispatch and review prompts.
- `.codex/agents/`: custom agent definitions.

## Build, lint, and test commands

These are initial target commands. If a command is not yet implemented, create or update the relevant script rather than silently skipping validation.

```bash
# Whole repo
./scripts/check.sh

# Daemon
cargo fmt --manifest-path daemon/Cargo.toml -- --check
cargo clippy --manifest-path daemon/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path daemon/Cargo.toml

# GNOME extension
./scripts/lint-gjs.sh
./scripts/test-fixtures.sh

# Schemas/contracts
./scripts/validate-schemas.sh
./scripts/validate-dbus.sh
```

## Coding conventions

- Favor small, boring modules over clever generic frameworks.
- Keep provider-specific code behind explicit adapter interfaces.
- Normalize all provider data into `spec/snapshot.schema.json` before it reaches the UI.
- Every external command invocation must have timeout, structured stderr capture, exit-code mapping, and redaction.
- Every provider fetch must distinguish: unauthenticated, cookie found but rejected, provider unavailable, parse error, upstream CLI missing, timeout, stale cache, and success.
- UI must render a stable skeleton with no layout jump between loading and loaded states.
- Manual refresh must always be available, even when stale or unauthenticated.
- Diagnostics must be one click away and copyable with secret redaction.

## Security rules

- Never log raw cookies, token values, Authorization headers, Set-Cookie headers, or full browser profile paths when avoidable.
- Redact email addresses in logs unless the log is explicitly user-facing diagnostics and still safe to copy.
- Read browser cookie DBs through a temporary copy when the source DB may be locked.
- File permissions for app-owned config/cache must be `0600` files and `0700` directories.
- Treat provider web endpoints as unstable. Adapters must fail closed and produce actionable diagnostics.
- No telemetry, remote analytics, or crash upload in MVP.

## Definition of done

A task is done only when:

1. behavior is implemented behind the agreed contract;
2. tests or fixtures cover the changed behavior;
3. `./scripts/check.sh` or the relevant narrower checks pass, or the failure is documented with exact reason;
4. secrets are redacted in logs, diagnostics, snapshots, and test fixtures;
5. docs or ADRs are updated for architectural changes;
6. the final response includes files changed, checks run, and residual risks.

## Contract freeze

Before implementing Task 01 or Task 03, read `docs/CONTRACTS.md`, `docs/adr/0005-p0a-contract-freeze.md`, and all `spec/*.schema.json`. Task 00 may create neutral skeletons, but provider/daemon/UI behavior must not bake assumptions that contradict the freeze.

## Change-control rules

- Do not alter `spec/dbus-org.codexbar.Linux1.xml` or JSON schemas casually. Contract changes require updating docs, fixtures, and affected agents.
- Do not add production dependencies without explaining why the dependency is safer than local implementation.
- Do not broaden browser-cookie scope beyond domains required by enabled providers.
- Do not implement provider-specific scraping in Shell UI. All network/data logic belongs in the daemon.
- Do not create a localhost API unless a future ADR explicitly approves it as opt-in.

## Agent orchestration

Use specialized agents for parallel work:

- `architecture_guardian`: contract review, ADRs, architecture consistency.
- `daemon_engineer`: Rust daemon, scheduler, cache, D-Bus server.
- `gnome_shell_engineer`: GJS Shell extension, panel indicator, popover.
- `browser_cookie_engineer`: browser discovery, decryption, cookie jar construction, web provider adapters.
- `packaging_ci_engineer`: Debian package, systemd user unit, CI, local install scripts.
- `qa_security_reviewer`: threat model, redaction, tests, GNOME compatibility review.

When running broad work, ask Codex to spawn agents explicitly, wait for all results, then consolidate into a single plan or patch set.
