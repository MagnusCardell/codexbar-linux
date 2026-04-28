# Prompt — P2 browser-cookie path

Prepare and implement the Linux browser-cookie path.

Start with Task 04 research. Do not code production decryption until the research doc records tested browser versions and storage behavior.

Spawn:

- `browser_cookie_engineer`: perform research, then implement adapter behind tests.
- `daemon_engineer`: wire adapter outputs into daemon state machine after adapter tests exist.
- `qa_security_reviewer`: review secret handling before any provider web adapter lands.
- `architecture_guardian`: approve any contract/schema changes.

Hard constraints:

- No real cookies in fixtures, logs, docs, or commits.
- No raw cookie persistence.
- Query only provider-relevant domains and cookie names when known.
- Distinguish absent cookie from rejected cookie.

Return a staged patch plan if implementation is too large for one diff.
