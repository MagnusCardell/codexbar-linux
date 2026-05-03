# Security and privacy model

## Security posture

CodexBar GNOME handles provider usage data, upstream CLI output, local cache, and daemon settings. The security model is therefore: least privilege, no raw secret persistence, explicit diagnostics, and local-only operation.

## Assets

Sensitive assets:

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
GNOME Shell UI ───── D-Bus JSON ───── daemon ───── upstream codexbar CLI
       │                                  │
       └──── default browser links        └──── local cache/settings
```

The Shell extension is trusted to display normalized data but not trusted with raw secrets. The daemon is trusted to run local upstream CLI commands, maintain normalized cache/settings, and redact diagnostics. Upstream CLI output is untrusted input and must be normalized before crossing D-Bus.

## Hard rules

- No raw cookie persistence.
- No Authorization header persistence.
- No Set-Cookie persistence.
- No raw provider HTML/JSON persistence.
- No network listener by default.
- No telemetry in MVP.
- No provider passwords collected, stored, or requested.
- No browser-cookie access, browser profile scanning, cookie database reads, keyring access, provider dashboard scraping, browser extension, or localhost bridge.
- Diagnostics copy path must run through the same redactor used by logs.

## Redaction policy

Redact:

- cookie values;
- bearer/API/OAuth/session tokens;
- `Authorization`, `Cookie`, `Set-Cookie`, `X-API-Key` headers;
- URL query parameters likely to contain tokens;
- local absolute browser profile paths if they ever appear in upstream output;
- account emails in logs by default;
- raw account emails, raw organization names, and raw provider account IDs in snapshots, cache, D-Bus outputs, fixtures, and diagnostics.

Allowed in user-facing diagnostics and snapshots:

- provider ID;
- last success/error timestamp;
- upstream CLI path/version;
- normalized state enum;
- masked email/organization display and non-reversible local hashes if needed for account disambiguation.

## Browser and web scope

Browser-cookie import and provider web fetches are explicitly out of scope. The
daemon must not discover browser profiles, read browser cookie stores, access
desktop keyrings, decrypt session material, construct Cookie headers, fetch
provider dashboards, or expose a browser extension/localhost bridge. The
`TestBrowserImport` D-Bus method remains only as a compatibility no-op and must
return `not_implemented` without filesystem, browser, keyring, or provider
endpoint access.

## D-Bus security

D-Bus API is session-scoped. Do not expose secrets over D-Bus. The session bus is not a secret vault; any user process may be able to call the service. Therefore all D-Bus outputs must be safe to display or copy after redaction.

## Tests required before merge

- Redactor unit tests with representative tokens/cookies/headers.
- Snapshot schema validation tests.
- D-Bus output contains no secrets.
- Browser import no-op tests assert schema-valid unsupported behavior and no browser/cache/settings side effects.
- Static no-browser/web guard fails on removed browser-cookie, keyring, web-fetch, dependency, fixture, and validator surfaces.
- Shell UI tests/fixtures for stale, unauthenticated, cookie rejected, missing CLI, timeout, and success.
