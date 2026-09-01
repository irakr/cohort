-- Add the "window" scope kind (application window mirroring). SQLite cannot
-- alter a CHECK constraint, so the table is rebuilt in place.

CREATE TABLE scope_requests_new (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  assist_ref   TEXT NOT NULL REFERENCES assists(ref),
  requester_id TEXT NOT NULL REFERENCES users(id),
  kind         TEXT NOT NULL
               CHECK (kind IN ('comment','live_debug','file','terminal','agents','ssh','window')),
  target       TEXT,
  reason       TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'pending'
               CHECK (status IN ('pending','approved','denied')),
  ttl_minutes  INTEGER,
  created_at   TEXT NOT NULL,
  decided_at   TEXT,
  payload      TEXT
);
INSERT INTO scope_requests_new SELECT * FROM scope_requests;
DROP TABLE scope_requests;
ALTER TABLE scope_requests_new RENAME TO scope_requests;
