# Codex Web Fixtures

Task 04D.0 fixtures are synthetic fake HTTP bodies and descriptors only. They
are not captured from any real provider account or browser session.

The corpus intentionally avoids real provider responses, raw secret material,
browser profile paths, request metadata, response metadata, auth credentials,
and live domains. Tests use these bodies through `FakeWebClient` only.

`dashboard_too_large.marker` documents the too-large case; tests generate the
oversized body in memory so a large fixture is not committed.

Task 04D.1G adds synthetic structural parser shapes only:

- `next_data_usage_success.html` for a bounded synthetic next-data script;
- `inline_state_usage_success.html` for an allowlisted inline JSON assignment;
- `app_shell_no_data.html` for an authenticated app shell with no embedded data;
- `login_shell.html` for a login shell text fallback;
- `embedded_json_missing_usage.html` for a candidate missing usage fields;
- `embedded_json_redaction_rejected.html` for candidate-level redaction
  rejection.
