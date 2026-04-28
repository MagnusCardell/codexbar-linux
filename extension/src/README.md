# Extension Source Modules

Task 00 intentionally keeps the Shell extension as a minimal loadable skeleton.

Future Task 03 modules live here and must preserve these boundaries:

- Shell-process modules may use GNOME Shell/GJS APIs but must not import `Gtk`,
  `Gdk`, or `Adw`.
- Preferences code stays in `prefs.js` and may use GTK4/libadwaita, but must
  not import Shell-only libraries or Shell UI modules.
- Production Shell code consumes daemon data only over D-Bus and must not read
  daemon cache files.
- No provider network calls, browser profile access, subprocesses, or real
  D-Bus runtime behavior belongs in the Task 00 skeleton.
