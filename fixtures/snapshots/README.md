# Snapshot fixtures

Shared normalized snapshot fixtures for daemon and Shell tests live here.

Rules:

- Fixtures must conform to `spec/snapshot.schema.json`.
- Fixtures must not contain raw emails, raw organization names, cookies, tokens, headers, raw provider payloads, browser profile absolute paths, or unredacted errors.
- Use masked identity display values such as `m***@example.invalid` only when needed. Prefer hashes/placeholders.
- Shell-only visual fixtures may wrap these snapshots, but production Shell code must never read daemon cache files.
