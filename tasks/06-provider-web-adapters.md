# Task 06 — Initial provider web adapters

## Agent

`browser_cookie_engineer`; `daemon_engineer` may pair on normalization.

## Goal

Implement Linux web adapters for initial web-backed providers required for happy-path parity.

## Scope

- Codex/OpenAI web adapter.
- Claude web adapter.
- Domain allowlist and redirect restrictions.
- Bounded response sizes and request timeouts.
- Redacted provider fixtures.
- Normalize into `Snapshot.providers[]`.

## Constraints

- Use only in-memory cookies from Task 05.
- Do not execute scripts.
- Do not persist raw provider responses by default.
- Fail closed on unexpected shapes.

## Acceptance

- Success and failure fixtures for both providers.
- `cookie_rejected` state when cookies exist but provider rejects session.
- `unauthenticated` when cookies are absent.
- `parse_error` for unexpected response shape.
