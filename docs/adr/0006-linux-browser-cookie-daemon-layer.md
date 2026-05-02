# ADR 0006 - Linux Browser-Cookie Daemon Layer

## Status

Accepted for Task 04A architecture freeze. Implementation remains gated by
future Task 04B/04C/04D work.

## Context

CodexBar GNOME is a native GNOME Shell extension backed by a user-scoped Rust
daemon. Upstream `codexbar` CLI is the default data plane wherever Linux support
exists. Upstream CLI documentation identifies `web` and `auto` sources as
macOS-only on Linux, which leaves a product gap for the signed-in browser
session path.

The Shell has already passed its design gate and must remain presentation-only.
Browser-cookie import touches authentication-adjacent data, local browser
profiles, and the user's keyring. That work requires stricter isolation than
ordinary provider UI rendering.

## Decision

Linux browser-cookie import and web-backed provider fetches are daemon-owned,
memory-only, redacted, and exposed through normalized snapshots over D-Bus.

The daemon owns:

- browser profile discovery;
- cookie DB temp copies;
- cookie filtering and decryption;
- Secret Service/keyring interaction;
- in-memory cookie jars;
- provider web requests;
- provider response normalization;
- redacted diagnostics.

The Shell consumes only:

- normalized snapshots;
- refresh results;
- daemon info;
- redaction-safe diagnostics;
- safe browser-import test results.

Raw cookies, decrypted cookie values, full request headers, `Authorization`,
`Set-Cookie`, tokens, browser profile paths, raw provider responses, raw
identity, and raw error bodies must not cross D-Bus, enter cache, logs,
diagnostics, fixtures, screenshots, or copied UI output.

Cookie access and provider fetch remain separate modules. The browser module
obtains scoped in-memory session material. Provider web adapters consume that
material and produce normalized provider snapshots. The normalized snapshot is
the only durable output.

## Consequences

Positive:

- Preserves the accepted GNOME architecture.
- Keeps browser/keyring/provider I/O out of GNOME Shell.
- Gives Linux a path to web-backed providers while upstream CLI web parity is
  absent.
- Allows fixture-first browser and provider tests.
- Keeps D-Bus payloads safe for same-user callers.

Negative:

- Requires careful browser/keyring compatibility work on Ubuntu 24.04/26.04.
- Provider web endpoints are unstable and must fail closed.
- More dependencies may be needed later for SQLite, Secret Service, TLS, URL
  handling, and cryptography.
- Diagnostics must remain useful without exposing sensitive data.

## Alternatives Rejected

### Shell Reads Browser Cookies

Rejected. The Shell process must stay presentation-only and must not read
browser profiles, cookie DBs, keyrings, provider endpoints, subprocess output,
or daemon cache files.

### Browser Extension As Primary Product

Rejected. The product is a GNOME top-bar companion with a daemon data plane, not
a browser-extension-first product. A browser extension would not cover CLI,
local-cost, OAuth, systemd, D-Bus, or native GNOME integration consistently.

### Localhost HTTP Bridge

Rejected. D-Bus session API remains the only UI integration surface. A TCP or
localhost API would need a new ADR, authentication story, bind policy, and
threat model.

### Persisting Raw Cookies

Rejected. Raw cookies, decrypted secrets, session keys, bearer tokens, and full
headers are memory-only and must not be cached, logged, stored in settings,
included in diagnostics, or committed as fixtures.

### Copying Upstream macOS WebKit Behavior Literally

Rejected. Upstream macOS behavior relies on WebKit and macOS storage/keychain
facilities. Linux must use daemon-side Linux browser/keyring behavior, verified
against Ubuntu/GNOME targets, and must not import macOS assumptions wholesale.

### Starting With All Providers At Once

Rejected. Each provider has different domains, cookie names, response shapes,
identity behavior, and failure modes. Codex/OpenAI web is the pilot. Additional
providers follow one at a time after fixtures and redaction review.
