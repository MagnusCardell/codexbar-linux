# Prompt — project kickoff

You are coordinating Codex agents for `codexbar-linux`.

Read:

- AGENTS.md
- docs/PRD.md
- docs/ARCHITECTURE.md
- docs/SECURITY.md
- docs/ROADMAP.md
- spec/dbus-org.codexbar.Linux1.xml
- spec/snapshot.schema.json
- tasks/00-project-bootstrap.md through tasks/03-gnome-shell-vertical-slice.md

Spawn these agents and wait for all results:

1. `architecture_guardian`: identify contract risks before P1.
2. `daemon_engineer`: propose daemon crate/module plan for Tasks 00–02.
3. `gnome_shell_engineer`: propose extension module plan for Task 03.
4. `packaging_ci_engineer`: propose scripts and package skeleton for Task 00.
5. `qa_security_reviewer`: identify redaction and lifecycle tests needed before code lands.

Return a consolidated implementation plan with task order, files to create, checks to run, and unresolved decisions. Do not write code until the plan is accepted.
