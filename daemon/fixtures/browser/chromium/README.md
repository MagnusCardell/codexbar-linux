# Chromium Browser Fixtures

Task 04B fixtures are synthetic definitions only. Tests create throwaway SQLite
databases from these SQL files under temporary fake browser roots.

The fixture corpus intentionally does not contain real browser profiles, real
provider domains, real account identifiers, captured provider payloads, or live
browser cookie databases. The only hostnames are under `example.invalid`, and
the only row values are synthetic markers used to prove filtering and redaction
behavior.

Expected fixture directories:

- `plaintext-default/`: Chromium-style `Network/Cookies` schema with plaintext
  synthetic rows.
- `encrypted-fake/`: same schema with synthetic encrypted-value bytes for the
  fake decryptor.
- `corrupt-db/`: marker metadata for tests that write invalid database bytes.
- `locked-or-wal/`: schema used by tests that exercise WAL companion copy
  behavior.
- `unsupported-schema/`: intentionally incomplete schema used to verify
  `browser_cookie_db_schema_unsupported`.
