# Browser Support Matrix

## Status

Task 04A planning document updated after Task 04B and Task 04B.1
implementation. Task 04B implements daemon-only Chromium-family discovery and
cookie DB reads for synthetic/fake roots and throwaway fixtures. Task 04B.1
adds opt-in live throwaway Chromium-family verification. Task 04D.1 adds
Codex web live reconnaissance against a marked throwaway fake home only. These
tasks do not enable real user profile scanning by default, real keyring access,
default provider web fetches, or Firefox import.

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
- Task 04B fixtures live under `daemon/fixtures/browser/chromium/` as text
  metadata/SQL definitions. Tests create throwaway SQLite DBs from those files;
  committed fixtures do not include real or binary browser cookie databases.

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

- Exact encrypted cookie format and key derivation must be verified on Ubuntu
  24.04 and 26.04.
- Secret Service prompt behavior must be noninteractive for background refresh.
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
