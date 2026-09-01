-- Remove the Terminal view feature: the "terminal" kind is gone from
-- scope requests and assist artifacts, along with any existing terminal
-- requests, grants and shared-artifact rows.

DELETE FROM scope_requests WHERE kind = 'terminal';

CREATE TABLE scope_requests_new (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  assist_ref   TEXT NOT NULL REFERENCES assists(ref),
  requester_id TEXT NOT NULL REFERENCES users(id),
  kind         TEXT NOT NULL
               CHECK (kind IN ('comment','live_debug','file','agents','ssh','window')),
  target       TEXT,
  reason       TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'pending'
               CHECK (status IN ('pending','approved','denied','revoked')),
  ttl_minutes  INTEGER,
  created_at   TEXT NOT NULL,
  decided_at   TEXT,
  payload      TEXT
);
INSERT INTO scope_requests_new SELECT * FROM scope_requests;
DROP TABLE scope_requests;
ALTER TABLE scope_requests_new RENAME TO scope_requests;

DELETE FROM assist_artifacts WHERE kind = 'terminal';

CREATE TABLE assist_artifacts_new (
  assist_ref TEXT NOT NULL REFERENCES assists(ref),
  id         TEXT NOT NULL,
  kind       TEXT NOT NULL CHECK (kind IN ('file','ai_agent','custom')),
  label      TEXT NOT NULL,
  detail     TEXT NOT NULL DEFAULT '',
  icon       TEXT,
  pid        INTEGER,
  PRIMARY KEY (assist_ref, id)
);
INSERT INTO assist_artifacts_new SELECT * FROM assist_artifacts;
DROP TABLE assist_artifacts;
ALTER TABLE assist_artifacts_new RENAME TO assist_artifacts;
