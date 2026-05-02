# Provider Roadmap

## Status

Task 04A provider-priority decision. This roadmap does not promise support for
every upstream provider and does not implement provider web adapters.

## Principles

- Use upstream `codexbar` CLI where Linux CLI/API/local support exists.
- Add Linux browser-cookie web adapters only where there is a concrete product
  gap and verified provider evidence.
- Add one browser-cookie provider at a time.
- Do not normalize provider data from guesses.
- Do not let one provider failure poison all providers.
- Keep raw identity, cookies, headers, and provider payloads daemon-confined and
  redacted.

## Phase 1 Browser/Web Pilot

### Codex/OpenAI Web Dashboard

Codex is the first Linux browser-cookie web pilot.

Rationale:

- Codex is the product identity anchor for CodexBar GNOME.
- Upstream CLI Linux targeted Codex CLI data already works through the daemon.
- Upstream CodexBar docs describe a Codex/OpenAI web dashboard path for usage
  limits, credits, code review remaining, and usage breakdown.
- Linux `web`/`auto` upstream CLI modes are unsupported, so Linux-native daemon
  browser import is the correct gap-filler.

Constraints:

- The first adapter may only target provider-required OpenAI/Codex domains.
- Cookie import and provider fetch must be daemon-only.
- Success output is normalized `source="web"` and `sourceAdapter="linux_web"`.
- No raw dashboard payload is cached or exposed.
- Account identity is masked/hash-only.

Task 04D.0 implementation status:

- daemon-only Codex web adapter skeleton exists behind fake HTTP fixtures;
- static request, redirect, and browser-cookie domain policy is defined;
- fake fixture responses cover success, rejected session material, provider
  unavailable, parse error, timeout, redirect rejection, and response-size cap;
- production `linux_web` refresh has no live HTTP client configured by default
  and must not contact `chatgpt.com` or `openai.com`;
- live provider scraping, live provider HTTP, real browser profile scanning, and
  keyring access remain out of scope.

## Phase 2 Candidate

### Claude Web

Claude web is the second candidate only after Codex browser-cookie and web-fetch
paths are stable.

Rationale:

- Upstream provider docs describe Claude web API usage via browser cookies.
- Claude already has CLI and OAuth concepts, so fallback policy and failure
  wording need careful separation.

Constraints:

- Cookie absence and provider rejection must be distinct.
- OAuth/token account behavior must not be conflated with browser-cookie mode.
- No raw organization ID, email, session key, or response body may cross the
  daemon boundary.

## Phase 3 Later Browser-Cookie Providers

These providers are explicitly out of scope for the first implementation and
may be evaluated one at a time after the abstraction is stable:

- Cursor;
- OpenCode;
- Amp;
- Ollama;
- Abacus AI;
- Mistral;
- Droid/Factory;
- MiniMax;
- Kimi;
- Alibaba Coding Plan;
- other upstream browser-cookie providers.

Each later provider requires:

- verified required domains;
- verified cookie names or a documented reason for broader domain cookies;
- redacted success and failure fixtures;
- redirect and response-size tests;
- success, unauthenticated, cookie rejected, provider unavailable, timeout, and
  parse-error coverage;
- docs updated before runtime enablement.

## Provider Classification

| Provider | Current preferred Linux path | Browser-cookie priority | Notes |
| --- | --- | --- | --- |
| Codex | `upstream_cli` for CLI/local data; future `linux_web` for dashboard extras | Phase 1 | Pilot browser/web adapter. |
| Claude | `upstream_cli` or future OAuth/API work where supported; future `linux_web` candidate | Phase 2 | Web only after Codex path is stable. |
| Gemini | OAuth/API via Gemini CLI credentials upstream | Not first browser-cookie scope | No browser cookies required for first Linux plan. |
| Antigravity | Local probe upstream | Not browser-cookie scope | Local provider behavior is not a browser-cookie gap. |
| Cursor | Browser-cookie web upstream | Later | Requires dedicated domains, cookies, and fixtures. |
| OpenCode | Browser-cookie web upstream | Later | Response format and workspace behavior need fixtures. |
| Amp | Browser-cookie web upstream | Later | HTML/settings scrape must be proven stable. |
| Ollama | Browser-cookie web upstream | Later | Settings-page parse must be proven stable. |
| Abacus AI | Browser-cookie web upstream | Later | Billing/compute endpoints need provider-specific fixtures. |
| Mistral | Browser-cookie web upstream | Later | CSRF/session behavior needs careful redaction. |
| Droid/Factory | Browser cookies and token flows upstream | Later | Token and WorkOS flows increase risk; not early scope. |
| Copilot | API/OAuth token upstream | Not browser-cookie scope | Keep outside browser-cookie pilot. |
| z.ai | API token upstream | Not browser-cookie scope | Keep on API-token path. |
| Kimi | Cookie-derived/API token behavior upstream | Later | Treat as provider-specific token work, not first cookie import. |
| Kilo | API token with CLI fallback upstream | Not first browser-cookie scope | Keep on existing non-browser paths first. |
| Kiro | CLI upstream | Not browser-cookie scope | CLI provider. |
| Vertex AI | OAuth/local credential upstream | Not browser-cookie scope | OAuth/local credential provider. |
| JetBrains AI | Local file upstream | Not browser-cookie scope | Local quota file provider. |
| OpenRouter | API token upstream | Not browser-cookie scope | API-token provider. |
| Mistral API products | API token where applicable | Not browser-cookie scope | Browser console usage is separate and later. |

## Support Guarantees

Task 04A does not guarantee support for any provider beyond future Codex pilot
planning. A provider enters implementation only when its domains, cookies,
response shape, fixtures, diagnostics, and redaction behavior are documented.

## Upstream References

- `https://github.com/steipete/CodexBar/blob/main/docs/providers.md`
- `https://github.com/steipete/CodexBar/blob/main/docs/provider.md`
- `https://github.com/steipete/CodexBar/blob/main/docs/cli.md`
- `https://github.com/steipete/CodexBar/blob/main/docs/codex.md`
- `https://github.com/steipete/CodexBar/blob/main/docs/claude.md`
- `https://github.com/steipete/CodexBar/blob/main/docs/cursor.md`
- `https://github.com/steipete/CodexBar/blob/main/docs/opencode.md`
- `https://github.com/steipete/CodexBar/blob/main/docs/amp.md`
- `https://github.com/steipete/CodexBar/blob/main/docs/ollama.md`
- `https://github.com/steipete/CodexBar/blob/main/docs/factory.md`
- `https://github.com/steipete/CodexBar/blob/main/docs/minimax.md`
- `https://github.com/steipete/CodexBar/blob/main/docs/kimi.md`
- `https://github.com/steipete/CodexBar/blob/main/docs/gemini.md`
