# Task 08 — Debian packaging and install lifecycle

## Agent

`packaging_ci_engineer`.

## Goal

Package the daemon, D-Bus activation, systemd user service, schemas, and GNOME extension for Ubuntu.

## Scope

- Debian package metadata.
- systemd user unit.
- D-Bus service activation file.
- GSettings schema install/compile.
- Extension install path.
- Local dev install/uninstall scripts.
- Smoke test script.

## Constraints

- Do not silently enable the extension.
- Uninstall must not delete user config/cache unless explicit purge command is used.
- Package scripts must be idempotent.

## Acceptance

- `.deb` builds locally.
- Install on clean Ubuntu VM succeeds.
- D-Bus activation starts daemon.
- Extension can be enabled manually.
- Uninstall leaves system clean.
