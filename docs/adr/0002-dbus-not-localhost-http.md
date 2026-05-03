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

No localhost/TCP bridge is planned. Reconsidering one would require a future ADR that explicitly reverses this boundary, plus a threat model, bind restrictions, authentication story, and UI/packaging review.
