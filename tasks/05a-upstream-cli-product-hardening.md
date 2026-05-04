Execute Task 05A only: release-grade upstream CLI product hardening.

Baseline:
Use main at commit 250ab3ace1a0639a945b58dbf59a1e83aa943b95 as authoritative.

Context:
Task 04R removed the browser-cookie/web-fetch branch and restored the product to a safe upstream-CLI-only direction.

Current supported production data plane:
- upstream CodexBar CLI
- local provider tooling exposed through upstream CLI
- normalized daemon cache
- D-Bus session API
- GNOME Shell UI

Explicitly out of scope:
- browser-cookie access
- browser profile scanning
- cookie DB reads
- keyring/Secret Service/KWallet access
- provider web fetches/dashboard scraping
- browser extensions
- localhost/TCP API

Goal:
Harden the upstream-CLI-only product path toward a release-quality v0.1 on Ubuntu/GNOME.

Hard scope:
Allowed:
- upstream CLI resolver polish
- upstream CLI diagnostics polish
- provider selection/settings polish
- daemon activation/install hardening
- D-Bus activation/systemd user service hardening
- packaging/deb skeleton hardening
- no-browser static guard hardening
- README/docs/smoke-test updates
- GNOME manual QA checklist updates
- tests for missing CLI/provider errors
- tests for install/uninstall behavior where practical

Forbidden:
- no browser-cookie access
- no browser profile discovery
- no cookie DB access
- no keyring access
- no provider web fetch
- no HTTP client for provider dashboards
- no browser extension
- no localhost/TCP API
- no Shell cache reads
- no Shell subprocesses
- no D-Bus XML changes unless a concrete release blocker exists
- no JSON schema changes unless a concrete release blocker exists

Read before writing:
- README.md
- AGENTS.md
- docs/ARCHITECTURE.md
- docs/SECURITY.md
- docs/CONTRACTS.md
- docs/ROADMAP.md
- docs/ACCEPTANCE.md
- docs/adr/0003-upstream-cli-as-default-data-plane.md
- docs/adr/0006-no-browser-cookie-or-web-fetch.md
- docs/upstream-cli-adapter.md
- docs/upstream-cli-observations.md
- docs/gnome-smoke-test.md
- daemon/src/cli/*
- daemon/src/app.rs
- daemon/src/dbus.rs
- daemon/src/config.rs
- daemon/src/redact.rs
- extension/src/*
- scripts/install-local.sh
- scripts/uninstall-local.sh
- scripts/check.sh
- scripts/validate-no-browser-web-surface.sh
- packaging/dbus/org.codexbar.Linux1.service
- packaging/systemd/codexbar-linuxd.service
- packaging/debian/*

Spawn and wait for:
- architecture_guardian as scope/release reviewer
- daemon_engineer as upstream CLI/daemon reviewer
- gnome_shell_engineer as GNOME runtime reviewer
- qa_security_reviewer as redaction/no-browser reviewer
- packaging_ci_engineer as primary packaging/install reviewer

Do not spawn:
- browser_cookie_engineer

Required work:

1. Confirm no-browser state remains enforced

Keep and strengthen:
- scripts/validate-no-browser-web-surface.sh
- check.sh integration
- CI integration

The guard must fail if any of these return:
- daemon/src/browser
- daemon/src/web
- browser/web fixtures
- browser/web tasks
- browser-cookie/web-fetch direct dependencies
- browser-keyring/cookie/web-fetch runtime markers

Do not make docs explaining rejected alternatives fail the scan.

2. Upstream CLI resolver hardening

Review and harden:
- CODEXBAR_CLI explicit path
- PATH lookup
- Linuxbrew paths
- not-executable behavior
- version text behavior
- missing binary diagnostics

Required behavior:
- missing CLI produces clear degraded/missing_dependency state
- not-executable path produces clear diagnostic
- non-semver version text remains accepted
- no shell invocation
- no environment leakage
- no raw stderr/stdout leakage
- no provider refresh crash when CLI missing

3. Upstream CLI command strategy polish

Keep evidence-driven strategy:
- usage/status default target provider: codex, unless settings/request specify providers
- do not default usage/status to --provider all
- cost remains provider all without --source
- explicit all-provider usage/status remains allowed only when requested
- all-provider timeout remains non-fatal and diagnostic-safe

Add or improve tests for:
- default provider target is codex
- configured provider list is respected
- explicit providers from RefreshOptions are respected
- explicit all-provider timeout does not erase useful stale cache
- status/cost failure does not fail usage success
- usage success with diagnostics remains schema-valid

4. Diagnostics UX polish

Review daemon diagnostics surfaced to Shell.

Improve safeMessage/codes for common v0 conditions:
- upstream CLI missing
- upstream CLI not executable
- upstream CLI timed out
- upstream CLI returned malformed JSON
- upstream CLI provider unavailable
- provider CLI not installed/authenticated
- stale cache used
- fixture disallowed in production
- TestBrowserImport unsupported/no-op

No raw stderr/stdout.
No raw paths unless redacted.
No raw identity.

5. Install/local service hardening

Review:
- scripts/install-local.sh
- scripts/uninstall-local.sh
- packaging/systemd/codexbar-linuxd.service
- packaging/dbus/org.codexbar.Linux1.service

Ensure:
- installs daemon to user-local bin
- installs GNOME extension to canonical user extension path
- installs D-Bus service to user data dir
- installs systemd user unit to user config dir
- compiles schemas strictly
- reloads systemd user daemon
- does not auto-enable GNOME extension
- uninstall removes only owned files
- uninstall tolerates missing files
- no root/system daemon
- no TCP service

6. Debian packaging skeleton hardening

Do not pretend packaging is complete.

Required:
- build-deb.sh must still fail clearly unless packaging is actually wired
- Debian control should reflect current dependencies only
- no browser/web packages remain
- package names and installed paths are consistent
- packaging docs identify what remains before real `.deb`

7. GNOME runtime smoke checklist

Update docs/gnome-smoke-test.md if needed.

Checklist should cover:
- install local
- logout/login if needed on Wayland
- enable extension
- panel item appears
- popover opens
- manual refresh
- upstream CLI available/missing cases
- daemon stop/restart recovery
- merged/provider/minimal modes
- disable/re-enable no duplicates
- uninstall cleanup

8. Release acceptance doc

Update docs/ACCEPTANCE.md with a v0.1 acceptance checklist:

Must include:
- no-browser guard passes
- full check.sh passes
- live GNOME 46 Wayland smoke passes
- upstream CLI missing degraded UI passes
- upstream CLI available Codex refresh passes
- daemon stop/restart UI recovery passes
- install/uninstall smoke passes
- package skeleton status clear
- no raw secrets in diagnostics/cache/log-like output

9. Tests

Add/update tests where practical:
- no-browser guard
- TestBrowserImport no-op has no side effects
- fixture refresh rejected in production
- missing upstream CLI does not panic
- not-executable CLI path diagnostic
- upstream CLI timeout preserves stale cache
- install script shellcheck-like bash syntax if available or bash -n
- packaging validation for no browser deps

10. Validation commands

Run:

./scripts/validate-dbus.sh
./scripts/validate-schemas.sh
./scripts/test-fixtures.sh
./scripts/validate-upstream-cli-fixtures.sh
./scripts/test-upstream-cli-capture.sh
./scripts/validate-no-browser-web-surface.sh
./scripts/validate-gsettings.sh
./scripts/validate-packaging.sh
./scripts/lint-gjs.sh
./scripts/check.sh
cargo fmt --manifest-path daemon/Cargo.toml -- --check
cargo clippy --manifest-path daemon/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path daemon/Cargo.toml
dbus-run-session -- cargo test --manifest-path daemon/Cargo.toml dbus_contract
cargo run --manifest-path daemon/Cargo.toml -- --check
cargo tree --manifest-path daemon/Cargo.toml -e features
git diff --check

If a live GNOME session and upstream CodexBar CLI are available, also run:
- local install smoke
- daemon start/stop smoke
- extension enable/disable smoke
- manual refresh smoke

Final response must include:
- files changed
- no-browser guard status
- upstream CLI hardening changes
- diagnostics changes
- install/packaging changes
- tests added
- checks run and pass/fail
- live smoke run/skipped
- whether D-Bus XML/schema changed
- confirmation no browser-cookie/keyring/web-fetch/localhost API/Shell data-plane behavior was added
- next recommended task