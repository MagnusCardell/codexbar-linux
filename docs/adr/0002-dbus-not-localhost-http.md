# ADR 0002 — D-Bus session API, no localhost HTTP by default

## Status

Accepted.

## Context

A localhost HTTP service would be easy to debug and integrate, but it expands the product surface into an API server and invites cross-app/web-origin confusion. This project is a desktop component, not a remote dashboard.

## Decision

Use a D-Bus session service as the primary daemon interface. Do not open a TCP listener by default.

## Consequences

Positive:

- Better desktop-native integration.
- Lower accidental exposure.
- Easier D-Bus activation and user-service lifecycle.
- Clear separation from remote/team dashboard use cases.

Negative:

- Slightly more work for integration tests.
- Less convenient for third-party scripts than HTTP.

## Future option

A localhost HTTP bridge can be considered later only as an explicit opt-in feature with a separate ADR, threat model, bind restrictions, and authentication story.
