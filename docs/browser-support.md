# Browser Support Matrix

## Status

Task 04A planning document updated after Task 04B, Task 04B.1, and Task 04B.2
implementation. Task 04B implements daemon-only Chromium-family discovery and
cookie DB reads for synthetic/fake roots and throwaway fixtures. Task 04B.1
adds opt-in live throwaway Chromium-family verification. Task 04B.2 adds safe
cookie material summaries, plaintext Chromium row support, fail-closed
encrypted row classification, fake/test decryptor separation, and a
noninteractive Secret Service probe abstraction. Task 04B.3 adds daemon-only
Chromium Linux basic/plain `v10` decryption for the verified OSCrypt basic
password-store path. Task 04B.4 adds session-material policy support for the
Codex static dashboard request, including browser-style host-only versus domain
cookie matching and counts-only header eligibility summaries. Task 04D.1 adds
Codex web live reconnaissance against a marked throwaway fake home only. These tasks do not
enable real user profile scanning by default, real Secret Service or KWallet
extraction, default provider web fetches, or Firefox import.

## Implementation Order

1. Chromium-family first.
2. Firefox second.
3. Other browsers, Flatpak variants, and unusual profile roots later only after
   verification.

## Common Rules

- Discover only known browser roots.
- Do not recursively scan the home directory.
- Do not accept arbitrary profile paths from D-Bus.
- Use opaque profile IDs, not paths.
- Use safe display labels, not path fragments.
- Copy cookie DBs and required companion files to private temp storage before
  reading where live browser locking is possible.
- Query only provider-required domains and verified cookie names where known.
- Use synthetic or throwaway profiles for tests.
- In Task 04B, discovery only runs when the daemon receives injected
  `BrowserDiscoveryRoots` in tests or `CODEXBAR_BROWSER_IMPORT_FAKE_HOME` in a
  development process. Default runtime does not scan the real user profile
  roots.
- In Task 04B.1, `CODEXBAR_BROWSER_IMPORT_FAKE_HOME` must be an absolute,
  canonical, throwaway directory with `.codexbar-throwaway-browser-root`; it is
  rejected if it is `/`, the real `$HOME`, under the real config home, missing,
  relative, or symlinked through an escaping `.config`.
- In Task 04D.1, Codex web live reconnaissance reuses the same throwaway fake
  home gate and additionally requires `CODEXBAR_CODEX_WEB_LIVE=1`, explicit
  provider `codex`, and explicit source adapter `linux_web`. It may inspect
  `chatgpt.com` cookie material from the throwaway profile in memory only and
  may make one bounded static GET to the Codex dashboard URL. It must not use
  real default browser profiles.
- In Task 04B.3, `AppRuntime::from_env()` uses the plain backend by default.
  Plaintext rows may become in-memory material. Chromium Linux basic/plain
  `v10` rows may also become in-memory material when they match the verified
  basic OSCrypt format. Other encrypted formats still fail closed with
  redacted dependency diagnostics. Fake decryptor success is available only
  through explicit test constructors.
- Task 04B fixtures live under `daemon/fixtures/browser/chromium/` as text
  metadata/SQL definitions. Tests create throwaway SQLite DBs from those files;
  committed fixtures do not include real or binary browser cookie databases,
  raw encrypted blob values, WAL/SHM companions, or profile directories. Tests
  generate synthetic encrypted rows at runtime when decryptor coverage needs
  encrypted material.

## Task 04B.1 Live Throwaway Observations

Local opt-in smoke command:

```bash
CODEXBAR_BROWSER_LIVE=1 ./scripts/chromium-throwaway-smoke.sh
```

Observed on the local Ubuntu 24.04 host:

- Browser binary used: `google-chrome`.
- Browser family: Chrome.
- Headless mode: `--headless=new` worked.
- Password storage flag: `--password-store=basic` was used.
- Keyring prompt: none observed.
- Throwaway user-data shape: `$TMP_HOME/.config/google-chrome`.
- Profile shape: `$TMP_HOME/.config/google-chrome/Default`.
- Cookie DB shape observed: `$TMP_HOME/.config/google-chrome/Default/Cookies`.
- `Default/Network/Cookies`: not observed for this Chrome run.
- WAL companion: not observed.
- SHM companion: not observed.
- Cookie values: not printed or inspected by the smoke output; only the daemon
  synthetic query result was checked.
- `TestBrowserImport`: schema-valid and redaction-safe ignored live test passed
  against the throwaway fake home.

The smoke script is not part of `./scripts/check.sh` or CI because it requires
an installed browser. It can use Chrome, Chromium, Chromium snap wrapper, or
Brave if available. Snap Chromium is detected separately and uses a
throwaway-shaped fake home under the snap-visible common area so the daemon can
see the generated profile files; normal smoke output still reports only shape
labels, not absolute paths.

## Task 04B.2 Cookie Material Observations

Synthetic and ignored-live diagnostics now summarize Chromium cookie material
with counts and classes only:

- discovered profile count;
- candidate cookie row count;
- plaintext-value row count;
- encrypted-value row count;
- encrypted prefix counts for `v10`, `v11`, `v20`, `v24`, and unknown;
- expired row count;
- usable in-memory session cookie count;
- decryptor backend class: `fake`, `plain`, `secret_service`, or
  `unavailable`;
- decryption status: `not_needed`, `succeeded`, `failed`, `unavailable`,
  `locked`, or `prompt_required`.

The summary never prints cookie names, values, encrypted bytes, exact domains,
profile paths, SQL rows, Cookie headers, Authorization headers, account
identity, or raw provider payloads.

Current behavior after Task 04B.4:

- Chromium rows with `value` and empty `encrypted_value` are usable
  after provider domain/path validation. This is the path that covers any
  browser profile where `--password-store=basic` yields plaintext rows,
  including valid empty cookie values.
- Chromium Linux basic/plain `v10` rows are supported by the production/plain
  backend only for the verified OSCrypt basic path:
  - prefix: `v10` encrypted cookie value;
  - key source: Chromium's hardcoded Linux basic password-store OSCrypt source,
    derived locally without Secret Service, KWallet, or prompting;
  - derivation/cipher/padding: PBKDF2-HMAC-SHA1 to AES-128-CBC using
    Chromium's fixed IV and PKCS#7 padding;
  - integrity check: for cookie DB version 24 and later, the decrypted value
    must start with Chromium's SHA-256 hash of the row `host_key`; the hash is
    verified and stripped before header-safe cookie validation;
  - no MAC/tag is present in this legacy format, so malformed padding,
    malformed host-hash length, invalid UTF-8, wrong host hash, and invalid
    cookie material all fail closed.
- Synthetic `v11` rows model the keyring-backed path. The fake decryptor can
  decrypt them in tests; the production/plain backend reports
  `browser_keyring_unavailable` plus
  `browser_cookie_decryption_unavailable`.
- `v20`, `v24`, and unknown encrypted prefixes fail closed with
  `browser_cookie_decryption_unavailable` and no keyring-specific claim.
- If a provider-scoped encrypted row fails while another provider-scoped
  plaintext row exists, the daemon discards the partial material and web fetch
  is not attempted.
- Header-ineligible rows are different from decryption failures. For the static
  Codex request, rows that decrypt successfully but fail only strict
  Cookie-header name/value syntax validation are skipped when at least one
  valid header-safe cookie remains. If all request-relevant material is
  header-ineligible, the profile fails with `invalid_material`.
- `header_too_large` and `too_many_cookies` remain fail-closed header
  construction failures. Unsupported encrypted prefixes, malformed ciphertext,
  wrong keys, keyring-needed rows, and other decryption failures still discard
  partial provider material and do not send a Cookie header.
- Safe live summaries now include `decryptionFailureClass` so an ignored recon
  run can distinguish `keyring_needed`, `unsupported_format`,
  `malformed_ciphertext`, `wrong_key`, `invalid_material`,
  `header_too_large`, `too_many_cookies`, `unavailable`, and generic `failed`
  without printing cookie material.
- Task 04B.4 keeps Cookie-header construction in memory and adds a
  Codex-dashboard session policy summary with counts/classes only:
  `domainMatchedRows`, `pathMatchedRows`, `secureMatchedRows`,
  `decryptedRows`, `headerEligibleRows`, `headerRejectedRows`,
  `headerRejectedByClass`, and `cookieHeaderStatus`. The summary does not
  include cookie names, values, exact domains, profile paths, encrypted bytes,
  or Cookie headers.
- Chromium host keys without a leading dot are now treated as host-only during
  session-material URL matching; leading-dot domain cookies may match the
  registrable host and subdomains. This is synthetic policy coverage for the
  static Codex URL, not default live-profile scanning.

The Task 04B.3 local Chrome throwaway smoke used `--password-store=basic` and
reproduced the pre-fix failure mode: the seeded cookie was present but the
plain backend could not decrypt the encrypted `v10` row. Task 04B.3 is scoped
to that verified basic/plain path plus Chromium source behavior. Do not infer
that Secret Service, KWallet, app-bound encryption, or newer encrypted prefixes
are supported from this result.

The signed-in Task 04B.3 Codex throwaway recon moved the blocker forward but
did not produce usable Codex session material: all 19 provider-domain encrypted
rows were `v10`, 0 were plaintext, 2 were expired, and the plain decryptor
reached UTF-8 output, but the domain-wide cookie set included material that
failed safe Cookie-header validation. The safe summary reported
`decryptionFailureClass=invalid_material`, `usableSessionCookies=0`, and
`webFetch=not_attempted`. Task 04B.4 addresses that class of blocker with
synthetic policy coverage. The gated live recon has not been rerun in this
workspace because no marked throwaway fake home was available through
`CODEXBAR_WEB_HOME` or `CODEXBAR_BROWSER_IMPORT_FAKE_HOME`; parser work and
default live fetch remain out of scope.

## Google Chrome

Expected Linux profile roots:

- `~/.config/google-chrome`
- `~/.config/google-chrome-beta`
- `~/.config/google-chrome-unstable`
- `~/.config/google-chrome-for-testing`

Chromium documentation also notes `$CHROME_CONFIG_HOME`, `$XDG_CONFIG_HOME`,
`$CHROME_USER_DATA_DIR`, and `--user-data-dir` behavior. The daemon may only use
environment-derived roots when they are safe, canonicalized, and bounded by
policy.

Cookie DB shape at a high level:

- Chromium-family profile directories contain SQLite cookie stores.
- Newer Chromium-family builds may place cookies under profile `Network`
  storage.
- Cookie rows contain host/domain, name, path, expiry, security flags, and
  encrypted or plaintext value fields depending on browser/keyring behavior.

Encryption/decryption dependency:

- Linux Chromium-family secrets may depend on GNOME Secret Service/libsecret,
  KWallet, or basic fallback behavior.
- GNOME target path is Secret Service first.
- KWallet support is later unless verified as required for Ubuntu/GNOME MVP.

Lock/concurrency considerations:

- Browser may hold SQLite locks.
- Copy the cookie DB plus WAL/SHM companions where needed before reading.
- Locked or inconsistent copies map to diagnostics, not panics.

Task 04B/04C decision:

- Implemented for synthetic/fake roots in the Chromium-family first slice.
- Supported fake-root profile directories are direct known children such as
  `Default` and `Profile N`; profile IDs are path-free strings like
  `chrome-default`.
- Cookie DB lookup supports profile `Network/Cookies` first and legacy
  profile-level `Cookies` second.

Risks/open questions:

- The Task 04B.3 `v10` basic/plain path is verified against Chromium's Linux
  OSCrypt source and synthetic fixtures, then checked by ignored throwaway
  live recon. Ubuntu 26.04 release validation still needs to confirm current
  browser behavior.
- Secret Service and KWallet extraction are not implemented. Prompt behavior
  must remain noninteractive for background refresh and currently maps to
  `browser_keyring_prompt_required`.
- `v20` and `v24` encrypted-value prefixes are unsupported and fail closed.
- Chrome channel roots and command-line override roots need bounded support.

## Brave

Expected Linux profile roots:

- `~/.config/BraveSoftware/Brave-Browser`
- `~/.config/BraveSoftware/Brave-Browser-Beta`
- `~/.config/BraveSoftware/Brave-Browser-Dev`

These candidate roots must be verified with throwaway profiles before live
enablement.

Cookie DB shape at a high level:

- Brave is Chromium-family and is expected to use Chromium-style profile and
  cookie store structures.

Encryption/decryption dependency:

- Treat as Chromium-family Secret Service/keyring behavior until verified.

Lock/concurrency considerations:

- Same as Chromium-family: use private temp copies and handle WAL/SHM
  companions.

Task 04B/04C decision:

- Included in Task 04B synthetic/fake-root discovery for the stable Brave root
  only.

Risks/open questions:

- Package-specific profile roots need Ubuntu verification.
- Keyring service labels may differ from Chrome/Chromium.

## Chromium

Expected Linux profile roots:

- `~/.config/chromium`
- `~/snap/chromium/common/chromium`

Chromium documentation covers the standard `~/.config/chromium` root and XDG or
Chrome-specific overrides. Ubuntu packages may install Chromium as a snap
transition, so snap roots must be verified on target Ubuntu releases before
they are enabled.

Cookie DB shape at a high level:

- Chromium-family SQLite cookie stores with host/domain, name, path, expiry,
  security flags, and encrypted/plain value fields.

Encryption/decryption dependency:

- Secret Service/libsecret, KWallet, or basic fallback depending on browser and
  environment.

Lock/concurrency considerations:

- Live DB may be locked.
- Copy DB and companion files to private temp storage.
- Snap confinement may affect profile root access and path layout.

Task 04B/04C decision:

- Implemented in the Chromium-family first slice for synthetic/fake roots.
- Task 04B includes fake-root coverage for both `~/.config/chromium` and
  `~/snap/chromium/common/chromium`.
- Live snap behavior still needs throwaway-profile verification before default
  real-user scanning can be enabled.

Risks/open questions:

- Snap paths and confinement behavior need live Ubuntu verification.
- Keyring integration may differ between deb and snap builds.

## Firefox

Expected Linux profile roots:

- `~/.mozilla/firefox`
- snap-specific roots such as `~/snap/firefox/common/.mozilla/firefox` after
  verification.

Firefox profile metadata is managed through profile configuration such as
`profiles.ini`; discovery should read known metadata rather than recursively
walking arbitrary directories.

Cookie DB shape at a high level:

- Firefox stores cookies in `cookies.sqlite` inside the profile directory.
- Cookie rows include base domain, host, name, value, path, expiry, security
  flags, and origin attributes.

Encryption/decryption dependency:

- Firefox cookies are not treated as Chromium OSCrypt encrypted values in this
  design. Password storage is separate from cookie storage.

Lock/concurrency considerations:

- Firefox may lock or update the SQLite DB while running.
- Use private temp copies before reading.
- WAL/SHM companion handling must be verified.

Task 04B/04C decision:

- Implement after Chromium-family path is stable.

Risks/open questions:

- Firefox container/origin attributes may affect provider cookies.
- Snap path behavior needs Ubuntu verification.
- Provider sessions may behave differently across Firefox and Chromium-family
  browsers.

## Out Of Scope Until Later

- Safari: macOS-only, not a Linux browser target.
- Edge, Vivaldi, Arc, Dia, and other Chromium-family browsers: later only after
  explicit roots, keyring labels, and fixtures are verified.
- Flatpak browsers: later only after confinement and profile roots are
  documented.
- Manual pasted cookie headers: not part of Task 04B; would require a separate
  settings and redaction review.

## References

- Chromium user data directory: `https://chromium.googlesource.com/chromium/src/+/main/docs/user_data_dir.md`
- Chromium Linux password storage: `https://chromium.googlesource.com/chromium/src/+/main/docs/linux/password_storage.md`
- Mozilla Firefox profiles: `https://support.mozilla.org/en-US/kb/profiles-where-firefox-stores-user-data`
- Secret Service API: `https://specifications.freedesktop.org/secret-service-spec/latest-single/`
- SQLite WAL: `https://www.sqlite.org/wal.html`
