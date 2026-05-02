# Security and privacy model

## Security posture

CodexBar GNOME handles authentication-adjacent browser cookies and provider usage data. The security model is therefore: least privilege, no raw persistence, explicit diagnostics, and local-only operation.

## Assets

Sensitive assets:

- Browser cookies and decrypted cookie values.
- Provider session cookies.
- Upstream `~/.codexbar/config.json` secrets.
- Provider API keys and OAuth tokens managed upstream.
- Account email/organization identifiers and provider account IDs.
- Provider usage details that may reveal work patterns.

Non-sensitive or lower-sensitivity assets:

- Provider display names.
- Usage percentages.
- Reset timestamps.
- Stale/error state.
- Daemon version and CLI version.

## Trust boundaries

```text
GNOME Shell UI ───── D-Bus JSON ───── daemon ───── browser stores/keyring
       │                                  │
       └──── default browser links        └──── upstream codexbar CLI/provider web endpoints
```

The Shell extension is trusted to display normalized data but not trusted with raw secrets. The daemon is trusted to transiently access cookies and upstream config. Provider web endpoints are untrusted inputs.

## Hard rules

- No raw cookie persistence.
- No Authorization header persistence.
- No Set-Cookie persistence.
- No raw provider HTML/JSON persistence unless a debug flag is explicitly enabled and the file is redacted or stored under a clearly named volatile diagnostics directory.
- No network listener by default.
- No telemetry in MVP.
- No provider passwords collected, stored, or requested.
- Diagnostics copy path must run through the same redactor used by logs.

## Redaction policy

Redact:

- cookie values;
- bearer/API/OAuth/session tokens;
- `Authorization`, `Cookie`, `Set-Cookie`, `X-API-Key` headers;
- URL query parameters likely to contain tokens;
- local absolute browser profile paths where not required;
- account emails in logs by default;
- raw account emails, raw organization names, and raw provider account IDs in snapshots, cache, D-Bus outputs, fixtures, and diagnostics.

Allowed in user-facing diagnostics and snapshots:

- provider ID;
- browser family/profile display label;
- high-level keyring state;
- last success/error timestamp;
- upstream CLI path/version;
- normalized state enum;
- masked email/organization display and non-reversible local hashes if needed for account disambiguation.

## Browser-cookie handling

Required flow:

1. Determine enabled provider domains.
2. Discover browser profiles.
3. Copy cookie DB to a private temp directory when needed.
4. Query only required domains/names when possible.
5. Decrypt only selected values.
6. Construct an in-memory cookie jar.
7. Perform provider request.
8. Drop cookie jar and decrypted values.
9. Cache only normalized output.

The Task 04A browser-cookie-specific threat model is maintained in
`docs/browser-cookie-threat-model.md`. It is authoritative for future browser
profile discovery, cookie DB copying, keyring/decryption, provider web fetch,
diagnostics, and fixture-safety work.

## D-Bus security

D-Bus API is session-scoped. Do not expose secrets over D-Bus. The session bus is not a secret vault; any user process may be able to call the service. Therefore all D-Bus outputs must be safe to display or copy after redaction.

## Provider web adapters

Provider responses are unstable and potentially hostile. Adapter rules:

- strict timeouts;
- bounded response sizes;
- schema/shape checks;
- no script execution;
- no arbitrary redirects to untrusted domains without explicit allowlist;
- fail closed with diagnostics.

## Tests required before merge

- Redactor unit tests with representative tokens/cookies/headers.
- Snapshot schema validation tests.
- D-Bus output contains no secrets.
- Browser import fixture tests using synthetic cookie DBs.
- Provider adapter tests using recorded redacted fixtures.
- Shell UI tests/fixtures for stale, unauthenticated, cookie rejected, missing CLI, timeout, and success.
