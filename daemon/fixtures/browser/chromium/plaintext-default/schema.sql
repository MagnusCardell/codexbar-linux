CREATE TABLE meta(key LONGVARCHAR NOT NULL UNIQUE PRIMARY KEY, value LONGVARCHAR);
INSERT INTO meta(key, value) VALUES('version', '24');

CREATE TABLE cookies(
  creation_utc INTEGER NOT NULL,
  host_key TEXT NOT NULL,
  top_frame_site_key TEXT NOT NULL DEFAULT '',
  name TEXT NOT NULL,
  value TEXT NOT NULL,
  encrypted_value BLOB NOT NULL DEFAULT X'',
  path TEXT NOT NULL,
  expires_utc INTEGER NOT NULL,
  is_secure INTEGER NOT NULL,
  is_httponly INTEGER NOT NULL,
  last_access_utc INTEGER NOT NULL,
  has_expires INTEGER NOT NULL DEFAULT 1,
  is_persistent INTEGER NOT NULL DEFAULT 1,
  priority INTEGER NOT NULL DEFAULT 1,
  samesite INTEGER NOT NULL DEFAULT -1,
  source_scheme INTEGER NOT NULL DEFAULT 2,
  source_port INTEGER NOT NULL DEFAULT 443,
  last_update_utc INTEGER NOT NULL DEFAULT 0,
  source_type INTEGER NOT NULL DEFAULT 0,
  has_cross_site_ancestor INTEGER NOT NULL DEFAULT 0
);

INSERT INTO cookies(
  creation_utc, host_key, name, value, encrypted_value, path, expires_utc,
  is_secure, is_httponly, last_access_utc
) VALUES
  (1, 'codex.example.invalid', 'quota_marker', 'fixture-value-alpha', X'', '/', 20000000000000000, 1, 1, 1),
  (1, '.openai.example.invalid', 'usage_marker', 'fixture-value-beta', X'', '/', 20000000000000000, 1, 1, 1),
  (1, 'distractor.example.invalid', 'quota_marker', 'fixture-value-distractor', X'', '/', 20000000000000000, 1, 1, 1),
  (1, 'codex.example.invalid', 'quota_marker', 'fixture-value-expired', X'', '/', 1, 1, 1, 1);
