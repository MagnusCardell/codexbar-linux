# GNOME Design Gate

Task 03.5 redesigns the GNOME Shell surface into a calm, daily-use CodexBar
companion. The macOS CodexBar UI is the information-architecture reference:
provider first, meters first, diagnostics secondary. This is not a literal
macOS visual clone.

## Visual Goals

- Feel native to the GNOME top bar and Shell popover model.
- Present provider usage before daemon/debug detail.
- Keep copy product-facing, concise, and non-alarming.
- Use restrained color: neutral surfaces, subtle stale/error signals, no large
  warning outlines in the default path.

## Top-Bar Indicator

- Merged mode shows a short provider label, two tiny stacked meters, and a
  small state dot/icon.
- Provider mode shows a bounded compact provider group with `+N` overflow and
  never pushes more than three provider clusters into the panel when overflow is
  present.
- Minimal mode shows one quiet icon/label and opens the full popover.
- The panel item should not read as a large pill or selected tab.

## Popover Structure

1. Provider selector strip: compact provider items with selected focus.
2. Divider.
3. Selected provider title area: name, compact refresh/retry, updated age,
   state, and safe metadata.
4. Divider.
5. Usage sections: session, weekly/monthly/secondary, and credits when present.
6. Divider.
7. Cost section only when meaningful.
8. Secondary actions: diagnostics/copy/settings only when valid.
9. Divider.
10. Footer: one calm daemon/capability row.

## Provider Selector Behavior

- Preserve snapshot provider order.
- Use `selected-provider` GSettings when valid.
- Clicking a provider may set the existing `selected-provider` UI key.
- Dim unavailable providers; do not hide them from the selector.
- Overflow should degrade cleanly rather than adding unbounded panel clutter.

## Meter Semantics

- Preserve CodexBar’s two primary usage concepts: session/window and
  weekly/monthly/secondary.
- Use slim continuous bars, not chunky segmented debug bars.
- Clamp invalid percentages before display.
- Missing usage renders as unavailable, never as an exception.

## Diagnostics Behavior

- Diagnostics are not part of the default main view.
- Default UI shows only a small safe summary or availability hint.
- Loaded diagnostics may show a short bounded detail stack, but copy remains the
  canonical full redacted projection.
- Copy uses the whitelisted redacted projection.
- Diagnostic codes may appear only in secondary/loaded diagnostics, never as
  primary provider status.

## State Wording

Use the frozen product-facing copy map:
`Loading usage…`, `Up to date`, `Stale data`, `Sign-in required`,
`Browser session rejected`, `Dependency missing`, `Provider unavailable`,
`Could not read provider data`, `Provider timed out`, `Error`, and
`Daemon unavailable`.

## GNOME-Native Constraints

- Shell code uses GJS ESModules and Shell UI modules only.
- No GTK, GDK, or libadwaita inside Shell-process modules.
- No daemon cache reads, subprocesses, provider network calls, browser-profile
  inspection, or keyring access from Shell.
- Manual refresh sends the frozen Shell `MANUAL_REFRESH_OPTIONS` payload and
  must not invoke upstream `codexbar` directly. Changes to adapter policy are
  data-plane behavior and require contract/daemon review, not visual polish.
- For Task 03.5, preferences remain GTK/libadwaita and use only existing
  Shell UI/autostart GSettings keys. Future daemon-owned provider, browser,
  refresh, or diagnostics settings belong behind the daemon D-Bus settings API
  or the documented daemon config flow, not new Shell-owned GSettings keys.

## Non-Goals

- No daemon, D-Bus XML, JSON schema, or P0A contract changes.
- No browser import UI that implies implemented behavior.
- No provider enablement, refresh interval, diagnostics verbosity, or source
  adapter settings in GSettings.
- No macOS-only interactions or Quit behavior.
