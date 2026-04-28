# Task 00 — Project bootstrap

## Agent

`packaging_ci_engineer` with review by `architecture_guardian`.

## Goal

Create the initial buildable repository skeleton without implementing provider logic.

## Scope

- Initialize `daemon/` as a Rust crate named `codexbar-linuxd`.
- Create `extension/` skeleton with `metadata.json`, `extension.js`, `prefs.js`, `stylesheet.css`, and module directories.
- Create `schemas/org.gnome.shell.extensions.codexbar-linux.gschema.xml`.
- Create `scripts/check.sh`, `scripts/lint-gjs.sh`, `scripts/validate-schemas.sh`, `scripts/validate-dbus.sh`.
- Add CI workflow placeholders if GitHub Actions is used.

## Constraints

- Do not implement browser-cookie access.
- Do not implement provider network calls.
- Keep Shell and prefs imports separated.

## Acceptance

- `./scripts/check.sh` runs and reports unimplemented checks clearly or passes.
- `cargo test --manifest-path daemon/Cargo.toml` runs.
- GNOME extension metadata validates structurally.

## Contract references

Read `docs/CONTRACTS.md`, `docs/adr/0005-p0a-contract-freeze.md`, and all relevant `spec/*.schema.json` before changing behavior. Do not contradict the P0A source taxonomy, identity redaction rules, refresh semantics, settings ownership, or Shell/daemon boundary.
