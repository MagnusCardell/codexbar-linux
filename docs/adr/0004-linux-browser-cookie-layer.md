# ADR 0004 — Linux-native browser-cookie layer

## Status

Accepted for MVP scope, implementation gated by P0 verification.

## Context

The product promise requires existing signed-in browser sessions to “just work” for web-backed providers. Upstream Linux CLI support is CLI-only for web/auto sources today, so wrapping only the CLI cannot satisfy the happy path.

## Decision

Implement a Linux-native browser-cookie layer in the daemon:

- discover supported browser profiles;
- read cookie stores safely;
- use the user’s normal keyring/session facilities for decryption;
- build in-memory cookie jars;
- run narrow provider web adapters;
- normalize results into the same snapshot schema.

No raw cookies are persisted.

## Consequences

Positive:

- Meets product thesis.
- Keeps cookie logic out of Shell UI.
- Can be upstreamed or removed later if upstream Linux parity arrives.

Negative:

- Browser storage behavior is brittle and must be verified regularly.
- Provider web endpoints are unstable and need robust diagnostics.
- Security review burden is higher.
