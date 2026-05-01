# GNOME Visual Target

Task 03.7 resets the GNOME Shell UI to the macOS CodexBar information
architecture, translated into a restrained GNOME dark popover. This document is
the target for the Shell UI surface, stylesheet, view-model tests, and visual
review.

The product surface is a compact quota readout: provider identity, freshness,
session usage, weekly usage, optional credits, optional cost, safe secondary
actions, and a quiet daemon capability footer. Diagnostics support the readout
but must not become the default visual story.

Visual sign-off still requires real GNOME Shell screenshots. Static tests can
verify structure, copy, and safety boundaries, but they cannot approve panel
density, popover rhythm, or perceived visual hierarchy.

## Final Layout

Default popover order:

```text
Provider strip
Divider
Selected provider title area
Divider
Usage sections
Divider
Cost section, only when meaningful
Divider
Secondary actions
Diagnostics details, only after loading
Divider
Compact footer
```

The selected provider is the only provider with a full detail surface. Other
providers remain in the provider strip. The normal view must never render a
vertical stack of provider cards.

Wireframe:

```text
Codex
━━━━━
────────────────────────────────────────
Codex                         Local
Updated just now
────────────────────────────────────────
Session
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
97% remaining                  Resets 5h

Weekly
━━━━━━━━━━━━━━━━━━━━
57% remaining                  Resets 4d

Credits
0 credits remaining
────────────────────────────────────────
Cost
Today: $57.37 · 49M tokens
Last 30 days: $333.44 · 336M tokens
────────────────────────────────────────
Usage Dashboard
Status Page
Diagnostics
Settings
────────────────────────────────────────
Daemon running · CLI available · Cost available
```

`Usage Dashboard` and `Status Page` appear only when the daemon-provided URL is
safe after Shell-side validation. If a URL is absent or unsafe, the row is
omitted. `Diagnostics` is always a secondary action. `Settings` appears only
when the Shell extension exposes a preferences entry point.

## Visual Style

Use a restrained GNOME-native dark style.

- Background: dark gray or near-black, not pure black.
- Surface: no giant outer card border and no nested card stack.
- Text: soft light primary text with muted gray secondary text.
- Accent: restrained GNOME blue or muted green; never neon.
- Separators: subtle one-pixel gray dividers between major groups.
- Meters: slim continuous bars, not segmented progress chunks.
- Selected provider strip item: subtle accent background, not a large button.
- Warning and stale states: muted amber only where useful.
- Error states: sparse red, never a large warning block.
- Typography: GNOME/Shell default; monospace only for a tiny short badge if a
  future design explicitly needs it.

Rejected visual styles:

- Terminal emulator.
- Debug or log viewer.
- Neon hacker UI.
- TUI panels or command-line prompt styling.
- Generic card dashboard.
- Green-on-black failed Task 03.5/03.6 design.
- Heavy outlines, gold borders, glowing lines, or large debug boxes.
- All-caps debug labels as the primary hierarchy.

## Top Bar

Merged mode is the default. It should read like a native GNOME status item:

- Optional 5 px state dot.
- Tiny provider label such as `COD`.
- Two stacked continuous micro-bars.
- No capsule, no border, no large background.
- Subtle GNOME panel hover/active behavior only.
- Stale/error state is represented by the dot or muted meter tone, not a large
  warning badge.

Provider mode may show up to three provider clusters plus `+N` overflow, in
daemon snapshot order. Minimal mode shows one quiet symbolic icon and opens the
same popover.

## Provider Strip

The provider strip is first in the popover.

- Keep it single-row and compact.
- Preserve daemon snapshot provider order.
- Show provider names such as `Codex`, not concatenated badge/name strings such
  as `CODCodex`.
- Include one tiny usage/state bar per visible item.
- Dim unavailable providers instead of hiding them.
- Show `+N` overflow when providers exceed the visible limit.
- The selected provider uses a subtle accent background and label emphasis.

For a single Codex provider, still render the selected item:

```text
Codex
━━━━━
```

## Selected Provider

The selected-provider title area follows the first divider.

Left side:

- Provider display name, for example `Codex`.
- Freshness/state line, for example `Updated just now` or
  `Stale data · updated 34m ago`.

Right side:

- Safe user-facing plan/source metadata when available, for example `Local`,
  `Pro Lite`, or `Upstream CLI`.
- Omit the right-side label when no useful user-facing metadata exists.

Do not show raw provider IDs, diagnostic codes, source adapter internals, or
`Partial System Degradation` in the default title path.

## Usage Sections

Usage sections copy the macOS grouping directly.

`Session`:

- Section label.
- Slim continuous bar filled by remaining quota.
- Detail row: left `97% remaining`, right `Resets 5h`.

`Weekly`:

- Same structure.
- Example: `57% remaining`, `Resets 4d`.

`Credits`:

- Show after the primary usage meters when credits exist.
- Use the label `Credits`, never `Credits (credits)`.
- If credits have no meaningful percentage, omit the bar and show one compact
  row such as `0 credits remaining`.

Do not repeat the same remaining/reset sentence elsewhere in the selected
provider surface.

## Cost

Render `Cost` only when meaningful cost rows exist.

Rows:

- `Today: $X · Y tokens`
- `Last 30 days: $X · Y tokens`

Do not show a chevron unless the row performs an action. If cost data is absent,
omit the section instead of rendering a debug placeholder.

## Actions

Visible actions are limited to working actions:

- `Usage Dashboard`, only when a safe dashboard URL exists.
- `Status Page`, only when a safe status URL exists.
- `Diagnostics`.
- `Settings`, only when preferences can be opened.

Do not show:

- Generic `Open`.
- `Add Account`.
- `Quit`.
- Provider URL actions for unsafe, local, private, tokenized, path-leaking, or
  raw-payload-looking URLs.

Shell URL launch must always pass through `safeUrl()` before
`Gio.AppInfo.launch_default_for_uri()`.

## Diagnostics

Diagnostics are collapsed by default.

Default state:

- A secondary `Diagnostics` row/button.
- No diagnostics block body.
- No raw diagnostic codes, stdout/stderr, raw payload fields, raw JSON, tokens,
  cookies, emails, browser paths, or home paths.

Expanded state:

- Appears below actions.
- Title/summary uses product wording where possible.
- Show at most four detail rows in the UI.
- Keep `Copy`.
- Copied diagnostics must be a whitelisted redacted projection. Invalid
  diagnostics copy falls back to a bounded `diagnostics unavailable` object,
  never the original payload.

## Footer

Footer is one calm muted line.

Acceptable content:

- Daemon availability.
- Upstream CLI availability.
- Cost capability.
- Browser import capability.

Examples:

- `Daemon running · CLI available · Cost available · Browser import ready`
- `Daemon unavailable · CLI unknown · Cost unknown · Browser import unknown`

Do not include raw paths, adapter payloads, stdout/stderr, provider IDs,
diagnostic codes, account identifiers, version strings, or raw timestamps unless
a future support task explicitly requires them.

## Wording Rules

Use this exact state copy map:

| State | Copy |
| --- | --- |
| `loading` | `Loading usage…` |
| `ok` | `Updated just now` or `Up to date` |
| `stale` | `Stale data` |
| `unauthenticated` | `Sign-in required` |
| `cookie_rejected` | `Browser session rejected` |
| `missing_dependency` | `Dependency missing` |
| `provider_unavailable` | `Provider unavailable` |
| `parse_error` | `Could not read provider data` |
| `timeout` | `Provider timed out` |
| `error` | `Error` |
| `daemon_unavailable` | `Daemon unavailable` |

Good default wording:

- `Updated just now`
- `Stale data · updated 34m ago`
- `Local · Pro Lite`
- `Upstream CLI`

Rejected wording:

- `Partial System Degradation`
- `provider:codex`
- `upstream_cli_command_finished`
- `Generated 2026...`
- `Stale · stale`
- `OK · Snapshot <1m ago`
- `Credits (credits)`
- raw JSON-looking object strings

## Boundaries

Task 03.7 is presentation-only.

Allowed files are Shell UI, stylesheet, state/view-model tests, GJS lint guards,
and this visual target document. Validation script or CI dependency changes are
validation hardening only and do not alter product runtime behavior.

Forbidden for this task:

- Daemon runtime changes.
- D-Bus XML changes.
- JSON schema changes.
- P0A contract changes.
- Browser-cookie access.
- Keyring access.
- Provider web scraping or network calls from Shell.
- TCP/localhost APIs.
- Upstream CodexBar CLI invocation from Shell.
- Daemon cache-file reads from Shell.
- Daemon config writes from prefs.
- New daemon-owned GSettings keys.

## Screenshot Checklist

Visual approval requires sanitized screenshots from a real GNOME Shell 46+
session:

1. Merged top-bar indicator beside adjacent GNOME indicators.
2. Open popover default state with diagnostics collapsed.
3. Diagnostics expanded for the selected provider.
4. Provider mode showing provider clusters and `+N` overflow.
5. Minimal mode closed indicator and the same popover opened.
6. Daemon unavailable state.
7. Stale/error state.

Each screenshot set should include GNOME Shell version, session type, panel
mode, theme setting, fixture vs live daemon data, display scale, and confirmation
that copied diagnostics were separately checked for redaction.

Until those screenshots exist, the static UX verdict is: implementation can be
structurally accepted, but final visual approval is blocked.
