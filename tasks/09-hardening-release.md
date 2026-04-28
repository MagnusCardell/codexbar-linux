# Task 09 — Hardening and release gate

## Agent

`qa_security_reviewer` with all implementation agents fixing findings.

## Goal

Prepare MVP for first public alpha.

## Scope

- Threat model review.
- Secret redaction audit.
- GNOME lifecycle audit.
- D-Bus contract compatibility audit.
- Ubuntu 24.04/26.04 smoke matrix.
- Accessibility and high contrast review.
- Release notes and known limitations.

## Acceptance

- No known raw secret leakage in logs/cache/D-Bus/diagnostics.
- `./scripts/check.sh` passes.
- Smoke matrix documented.
- Known limitations are explicit and user-understandable.
