# Prompt — alpha release gate

Run the alpha release gate for CodexBar GNOME.

Read:

- docs/ACCEPTANCE.md
- docs/SECURITY.md
- docs/ROADMAP.md
- packaging docs/scripts
- latest smoke matrix results

Check:

- Ubuntu 24.04 LTS GNOME 46 smoke result.
- Ubuntu 26.04 LTS smoke result.
- Wayland behavior.
- D-Bus activation.
- systemd user service lifecycle.
- extension enable/disable cleanup.
- upstream CLI path.
- no-browser/web-surface validation.
- diagnostics redaction.
- uninstall/purge behavior.

Return:

- release decision: Block / Alpha ok / Release ok;
- blocker list;
- known limitations text suitable for README;
- exact checks and versions used.
