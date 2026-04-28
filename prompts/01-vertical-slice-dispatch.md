# Prompt — P1 vertical slice implementation

Implement P1 vertical slice for `codexbar-linux`.

Read AGENTS.md and the P1-related docs/tasks first.

Work split:

- `daemon_engineer`: implement daemon fixture D-Bus service, cache, and upstream CLI adapter stubs.
- `gnome_shell_engineer`: implement extension panel/popover consuming fixture/live D-Bus snapshots.
- `packaging_ci_engineer`: implement validation scripts and local dev install helpers.
- `qa_security_reviewer`: review the diff before final response.

Constraints:

- No browser-cookie import in this phase.
- No provider web fetches.
- No GTK/GDK/Adw in Shell process.
- All snapshots must validate against `spec/snapshot.schema.json`.

After agents finish, run relevant checks. Return:

- summary of implemented behavior;
- files changed;
- checks run and results;
- risks and follow-ups.
