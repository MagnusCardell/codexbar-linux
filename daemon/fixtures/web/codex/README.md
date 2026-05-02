# Codex Web Fixtures

Task 04D.0 fixtures are synthetic fake HTTP bodies and descriptors only. They
are not captured from any real provider account or browser session.

The corpus intentionally avoids real provider responses, raw secret material,
browser profile paths, request metadata, response metadata, auth credentials,
and live domains. Tests use these bodies through `FakeWebClient` only.

`dashboard_too_large.marker` documents the too-large case; tests generate the
oversized body in memory so a large fixture is not committed.
