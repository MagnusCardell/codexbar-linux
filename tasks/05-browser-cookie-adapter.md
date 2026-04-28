# Task 05 — Browser-cookie adapter

## Agent

`browser_cookie_engineer`.

## Goal

Implement safe browser profile discovery and in-memory cookie jar construction.

## Scope

- Discover supported browser profiles.
- Copy cookie DBs to private temp dir before reading where needed.
- Query only provider-relevant domains/names.
- Decrypt Chromium-family cookies using verified Linux keyring behavior.
- Read Firefox cookies using verified profile behavior.
- Return structured import diagnostics.
- Ensure no raw persistence.

## Constraints

- Must be based on Task 04 research.
- No provider web fetching in this task except synthetic test endpoint/fixture.
- Redactor tests are mandatory.

## Acceptance

- Unit tests with synthetic DB fixtures.
- Integration test with throwaway browser profile where possible.
- Diagnostics distinguish no profile, DB locked, keyring locked, cookie absent, decrypt failure, and success.
