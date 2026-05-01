# Extension Source Modules

Task 03 keeps Shell UI, state normalization, and the D-Bus client in small
ESModule files. These boundaries are intentional:

- Shell-process modules may use GNOME Shell/GJS APIs but must not import `Gtk`,
  `Gdk`, or `Adw`.
- Preferences code stays in `prefs.js` and may use GTK4/libadwaita, but must
  not import Shell-only libraries or Shell UI modules.
- Production Shell code consumes daemon data only over D-Bus and must not read
  daemon cache files.
- `dbusClient.js` is the only production daemon boundary.
- No provider network calls, browser profile access, subprocesses, daemon cache
  reads, or daemon config writes belong in Shell-process modules.
