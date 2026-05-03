# Prompt — review current branch

Review this branch against `main` for CodexBar GNOME.

Spawn one agent per review area and wait for all results:

1. `qa_security_reviewer`: secrets, threat model, D-Bus safety, no-browser/web-surface enforcement.
2. `architecture_guardian`: architectural drift, contract/schema changes, ADR completeness.
3. `gnome_shell_engineer`: GNOME Shell lifecycle, imports, UI state completeness.
4. `daemon_engineer`: daemon correctness, subprocess handling, cache, errors, tests.
5. `packaging_ci_engineer`: install/uninstall, package scripts, service activation, CI.

Prioritize concrete correctness/security findings over style. Include file and symbol references. End with merge readiness: Block / Needs changes / Ready.
