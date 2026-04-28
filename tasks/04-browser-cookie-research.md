# Task 04 — Browser-cookie research spike

## Agent

`browser_cookie_engineer`; review by `qa_security_reviewer` and `architecture_guardian`.

## Goal

Verify current Ubuntu 24.04/26.04 browser-cookie storage behavior before implementation.

## Scope

- Verify Chrome, Chromium, Brave, and Firefox profile locations on target systems.
- Verify cookie DB schemas and locking behavior.
- Verify Chromium-family decryption/keyring behavior on GNOME/Secret Service and common fallback states.
- Verify Firefox cookie value accessibility and session-cookie behavior.
- Identify provider domains and minimal cookie names for Codex/OpenAI and Claude.
- Produce `docs/browser-cookie-research.md` with exact tested versions and commands.

## Constraints

- Do not check in real cookie values.
- Use synthetic or throwaway test profiles.
- Do not implement production adapter in this task.

## Acceptance

- Research doc lists supported browsers/profiles and unsupported cases.
- Follow-up implementation tasks are updated with concrete tested assumptions.
- Security reviewer signs off on no-secret artifacts.
