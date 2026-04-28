# Source notes for agents

This file is a research index, not a substitute for current verification during implementation.

## Upstream CodexBar

- Repository: https://github.com/steipete/CodexBar
- CLI docs: https://github.com/steipete/CodexBar/blob/main/docs/cli.md
- Configuration docs: https://github.com/steipete/CodexBar/blob/main/docs/configuration.md
- UI reference image: https://github.com/steipete/CodexBar/blob/main/codexbar.png

Implementation notes to verify:

- Linux install paths and CLI binary naming.
- Exact current JSON payload shape.
- Current Linux behavior for `--source auto` and `--source web`.
- Current config schema and provider IDs.

## GNOME/GJS

- GNOME 45+ ESModule requirement for extensions.
- GNOME Shell extension review rule: do not import GTK libraries in Shell process; do not import Shell libraries in prefs process.
- GTK4/libadwaita preferences process for GNOME 40+.

## Ubuntu support floor

- Ubuntu 24.04 LTS: GNOME 46, standard support through May 2029.
- Ubuntu 26.04 LTS: released 23 April 2026, standard support around April/May 2031 depending on source page wording.

## OpenAI Codex project files

- Codex reads `AGENTS.md` instruction files.
- Project-scoped custom agents can live under `.codex/agents/`.
- Keep root `AGENTS.md` concise enough for Codex instruction limits.
