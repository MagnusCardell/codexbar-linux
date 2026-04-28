# Contract freeze dispatch

You are the orchestrator for P0A contract freeze.

Spawn and wait for these agents:

- `architecture_guardian`
- `daemon_engineer`
- `gnome_shell_engineer`
- `qa_security_reviewer`

Read first:

- `tasks/00a-contract-freeze.md`
- `docs/CONTRACTS.md`
- `docs/ARCHITECTURE.md`
- `docs/SECURITY.md`
- `docs/adr/0005-p0a-contract-freeze.md`
- `spec/dbus-org.codexbar.Linux1.xml`
- all `spec/*.schema.json`

Do not implement daemon, provider, browser-cookie, or Shell UI behavior in this pass.

Required output:

1. A patch that only updates contracts/docs/tasks/scripts/fixtures if needed.
2. A short decision log for any contract change.
3. Checks run:
   - `./scripts/validate-dbus.sh`
   - `./scripts/validate-schemas.sh`
   - `./scripts/test-fixtures.sh`
4. Residual risks blocking Task 01 or Task 03.

Hard constraints:

- No raw identity fields in snapshots/cache/D-Bus contracts.
- No Shell production cache-file reads.
- No D-Bus localhost/TCP surface.
- No `auto` in emitted snapshot `source` or `sourceAdapter`.
- `ProviderChanged` must not carry partial provider patches in v1.
