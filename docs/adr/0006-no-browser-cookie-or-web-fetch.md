# ADR 0006 — No Browser-Cookie Or Web-Fetch Data Plane

## Status

Accepted.

## Context

Task 04 explored a Linux browser-cookie and provider web-fetch direction. That
line of work expanded the daemon from a local upstream-CLI companion into code
that would need browser profile discovery, cookie database reads, cookie
decryption, desktop keyring/session access, provider dashboard parsing, HTTP
redirect policy, and a larger secret-handling surface.

The project now prioritizes a smaller and safer GNOME companion: Shell UI over
D-Bus, a user-scoped daemon, normalized cache/settings, and upstream CodexBar CLI
or local provider tooling where available. Keeping browser-cookie/web-fetch code
would make the security model and release gate materially harder without being
required for the supported production data path.

## Decision

CodexBar GNOME will not read browser cookies, browser profiles, browser cookie
databases, desktop keyrings, provider web dashboards, or provider session
material. The supported production data plane is upstream CodexBar CLI plus
local provider tooling where available.

The daemon must not implement provider web scraping or dashboard HTTP clients.
The product must not add a browser extension or localhost/TCP bridge as part of
this scope.

For v1 contract stability, the existing `TestBrowserImport` D-Bus method,
browser import schemas, `browserImport`/`linuxWebAdapters` capability booleans,
and related enum values may remain. They are compatibility surface only:

- capabilities report `false`;
- browser settings are normalized off;
- `TestBrowserImport` validates input and returns schema-valid
  `not_implemented`;
- no browser, keyring, cookie, provider endpoint, cache, or settings access is
  performed by that method.

## Consequences

Positive:

- Smaller daemon attack surface.
- No browser profile, cookie DB, keyring, or provider dashboard handling.
- Fewer dependencies and system package requirements.
- CI can enforce the absence of browser/web modules, fixtures, validators, and
  dependencies.
- Release work can focus on packaging, GNOME lifecycle, upstream CLI fidelity,
  diagnostics, and stale/error UX.

Negative:

- Providers that are only available through browser-backed web dashboards remain
  unsupported until upstream CLI or local provider tooling covers them.
- Some v1 schema and D-Bus compatibility fields remain visible even though they
  are inactive, which requires clear docs and tests.
- `cookie_rejected` and `linux_web` remain reserved vocabulary in frozen schemas
  until a future v2 contract cleanup removes or renames them.

## Alternatives Rejected

- **Browser-cookie import:** rejected because it requires browser profile
  discovery, cookie DB handling, cookie decryption, and careful secret lifetime
  management.
- **Provider web scraping:** rejected because provider dashboard HTML/JSON shapes
  are unstable, authentication state is sensitive, and failures would be hard to
  diagnose safely.
- **Browser extension:** rejected because it changes the product from a native
  GNOME companion into a browser-extension-first workflow and adds browser
  distribution/review surfaces.
- **Localhost bridge:** rejected because it expands the desktop component into a
  local web/API service with cross-origin and local-network exposure risks.
- **Keyring/session extraction:** rejected because desktop keyring and session
  material are outside the supported data plane and would increase the secret
  handling burden.

## Future Reconsideration Criteria

Reconsideration requires a new ADR that explicitly supersedes this one. At
minimum, it must provide:

- a proven upstream or provider-supported Linux API that avoids browser cookie
  extraction;
- a threat model reviewed by `qa_security_reviewer`;
- fixtures/tests proving no raw secrets cross D-Bus, logs, diagnostics, cache, or
  fixtures;
- dependency and packaging justification;
- an opt-in UX that does not affect the default upstream-CLI-only product path.
