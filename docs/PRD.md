# PRD — CodexBar GNOME for Ubuntu

## 1. Product thesis

Build a native Ubuntu/GNOME top-bar usage monitor for AI coding assistants, powered by upstream `steipete/CodexBar` Linux CLI where possible, and augmented by a Linux-native browser-cookie/web-fetch layer where upstream Linux CLI parity does not yet exist.

The core bet:

> A good Ubuntu version is not a tray app. It is a GNOME Shell extension backed by a user-scoped daemon, with browser-cookie import good enough that signed-in browser sessions “just work.”

This avoids two traps:

- a visually weak AppIndicator menu that cannot compete with the macOS SwiftUI popover;
- a fragile localhost service surface that feels more like an integration API than a desktop component.

Upstream CodexBar remains macOS-first as a GUI product. Linux support is CLI-only, and upstream Linux CLI currently does not support `web`/`auto` sources. CodexBar GNOME wraps upstream CLI for CLI/API/local provider paths and supplies a Linux-native cookie-backed web path for providers where that is necessary for the “just works” promise.

## 2. Name and positioning

- Product name: **CodexBar GNOME**
- Repo/package name: **`codexbar-linux`**
- Positioning: **A native GNOME top-bar companion for CodexBar. Powered by CodexBar CLI. Native for GNOME.**

This leaves room to upstream Linux improvements later without implying a permanent provider-framework fork.

## 3. Goals

### Primary goals

1. **Native stock Ubuntu experience**
   - GNOME top-bar indicator.
   - Polished popover rather than legacy tray menu.
   - Wayland-first.
   - GNOME Shell extension, not Electron/Tauri.

2. **Upstream CodexBar CLI as default data plane**
   - Consume `codexbar --format json`.
   - Consume `codexbar cost --format json`.
   - Respect `~/.codexbar/config.json` where possible.
   - Preserve upstream provider semantics, labels, reset windows, identity fields, `source`, status, and cost output.

3. **First-class browser-cookie import**
   - Existing browser login should be enough for supported web-backed providers.
   - No provider password prompts beyond normal keyring unlock.
   - No manual token copy/paste in the happy path.
   - No raw cookie persistence by this project.

4. **Daemon stability over localhost API convenience**
   - User-scoped daemon.
   - D-Bus session API as primary interface.
   - No TCP listener by default.
   - Cache for fast UI startup and stale-state rendering.

5. **Mac-like density and visual quality**
   - Two-bar status icon.
   - Rich provider cards.
   - Stable layout.
   - Clear stale/error/auth states.
   - Fast refresh affordance.

## 4. Non-goals

- Not a full rewrite of CodexBar provider logic.
- Not a generic AI billing analytics suite.
- Not a remote dashboard or team monitoring server.
- Not a browser extension as the primary product.
- Not an AppIndicator-only tray menu.
- Not a localhost HTTP API by default.
- Not a credential manager that stores provider passwords.

## 5. Target users

A Linux/Ubuntu developer who uses one or more AI coding assistants daily and wants quota/reset visibility without opening dashboards or running terminal commands.

Typical setup:

- Ubuntu Desktop 24.04 LTS or 26.04 LTS.
- GNOME Shell on Wayland.
- Chrome/Chromium/Brave/Firefox signed into ChatGPT, Claude, Cursor, or similar providers.
- Local CLIs installed for Codex, Claude Code, Gemini, or similar.
- Comfortable with terminal install, but expects the running product to be graphical and low-friction.

Support floor:

- Serious support floor: GNOME 46+.
- Release gate: Ubuntu 24.04 LTS and Ubuntu 26.04 LTS smoke tests.

## 6. Core requirements

### 6.1 Top-bar indicator

The indicator supports three visual modes.

#### Merged mode — default

- One GNOME panel item.
- Shows selected or most constrained provider.
- Two micro-bars:
  - top bar: session/window usage;
  - bottom bar: weekly/monthly/secondary usage.
- Provider glyph or short label.
- Dimmed/stale/error rendering.

#### Provider mode

- One compact item per enabled provider.
- Useful for users with two to four providers.
- Must avoid panel clutter.

#### Minimal mode

- Single icon or percent.
- For low visual noise.

CodexBar’s identity is tied to session and weekly meters, reset countdowns, credits, and a two-bar icon. CodexBar GNOME preserves that metaphor.

### 6.2 Popover

The popover is the product. It should feel closer to the upstream macOS SwiftUI menu than to a plain GTK menu.

UX requirements:

- No layout shift between loading and loaded states.
- No raw JSON in normal UI.
- Clear stale-but-usable state.
- Clear unauthenticated state.
- Clear “cookie found but provider rejected it” state.
- Per-provider diagnostics one click away.
- Manual refresh always available.
- Dashboard links open in default browser.
- Copy diagnostics action redacts all secrets.

Implementation constraints:

- `extension.js` and Shell-process modules are GJS ESModules and must not import GTK/Adwaita.
- Preferences are separate and use GTK4/libadwaita from `prefs.js`.

### 6.3 Preferences

Preferences should be boring, native, and debuggable.

Required sections:

#### General

- Start daemon on login.
- Refresh interval.
- Panel mode: merged/provider/minimal.
- Reset time format: countdown/absolute/both.
- Theme: system/compact/high contrast.

#### Providers

- Enable/disable provider display.
- Preferred source: upstream CLI/API/local, Linux browser web, auto.
- Browser source policy: auto, Chromium-family only, Firefox only, off.
- Provider dashboard link.
- Last successful refresh.
- Last error summary.

#### Browser sessions

- Detected browsers and profiles.
- Keyring status.
- Import test button.
- Explain privacy model.

#### Diagnostics

- Daemon status.
- Upstream CLI path/version.
- Last CLI exit code.
- Cache path.
- D-Bus service name.
- Copy redacted diagnostics.

### 6.4 Daemon

Required behavior:

- Runs as user service.
- D-Bus activatable.
- Reads upstream `~/.codexbar/config.json` without taking ownership of it.
- Invokes upstream CLI with timeout and structured error mapping.
- Maintains normalized cache.
- Emits snapshot-change signals.
- Supports manual refresh and scheduled refresh.
- Performs browser-cookie import and provider web fetches where implemented.
- Never exposes raw secrets over D-Bus.

### 6.5 Packaging

MVP packaging:

- Debian package for Ubuntu Desktop.
- Local dev install script.
- systemd user unit.
- D-Bus service file.
- GSettings schema.
- GNOME Shell extension files.

The extension should not be auto-enabled without explicit user action in install scripts/docs.

## 7. Success criteria

MVP success:

- On fresh Ubuntu 24.04 or 26.04, after installing package and enabling extension, the user sees a GNOME top-bar item.
- If upstream `codexbar` CLI is installed and configured, usage appears without manual JSON configuration in this project.
- If supported browser sessions exist, web-backed provider data can be fetched without raw-token copy/paste.
- Killing/restarting GNOME Shell does not lose daemon state.
- Killing/restarting daemon leaves the UI in a stale-but-usable state until D-Bus recovers.
- Copy diagnostics contains no raw cookies/tokens.
