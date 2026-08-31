-- Cohort hub schema. Vocabulary: assists, responders, scope requests,
-- credits, resolution records. Never session/ticket/issue/case.
-- No priority or severity column, by design (project plan section 5).
-- No aggregate of help received anywhere, by design (project plan section 8).

CREATE TABLE users (
  id       TEXT PRIMARY KEY,          -- 'u-alex'
  name     TEXT NOT NULL,
  initials TEXT NOT NULL
);

CREATE TABLE assists (
  ref         TEXT PRIMARY KEY,       -- 'S-2411'
  title       TEXT NOT NULL,
  status      TEXT NOT NULL DEFAULT 'open'
              CHECK (status IN ('open','dormant','done')),
  category    TEXT
              CHECK (category IS NULL OR category IN
                ('broken','environment','approach','review','knowledge','agent_loop')),
  owner_id    TEXT NOT NULL REFERENCES users(id),
  anonymous   INTEGER NOT NULL DEFAULT 0,
  goal        TEXT NOT NULL DEFAULT '',   -- brief: markdown
  failures    TEXT NOT NULL DEFAULT '[]', -- brief: JSON array of {label, note}
  environment TEXT NOT NULL DEFAULT '[]', -- brief: JSON array of chip strings
  live_data   TEXT,                       -- JSON: file tree, file contents, terminal feed, agent chat
  created_at  TEXT NOT NULL,              -- RFC3339
  closed_at   TEXT
);

CREATE TABLE assist_artifacts (
  assist_ref TEXT NOT NULL REFERENCES assists(ref),
  id         TEXT NOT NULL,
  kind       TEXT NOT NULL CHECK (kind IN ('terminal','file','ai_agent','custom')),
  label      TEXT NOT NULL,
  detail     TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (assist_ref, id)
);

CREATE TABLE assist_tags (
  assist_ref TEXT NOT NULL REFERENCES assists(ref),
  tag        TEXT NOT NULL,
  PRIMARY KEY (assist_ref, tag)
);

CREATE TABLE responders (
  assist_ref TEXT NOT NULL REFERENCES assists(ref),
  user_id    TEXT NOT NULL REFERENCES users(id),
  joined_at  TEXT NOT NULL,
  PRIMARY KEY (assist_ref, user_id)
);

CREATE TABLE scope_requests (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  assist_ref   TEXT NOT NULL REFERENCES assists(ref),
  requester_id TEXT NOT NULL REFERENCES users(id),
  kind         TEXT NOT NULL
               CHECK (kind IN ('comment','live_debug','file','terminal','agents','ssh')),
  target       TEXT,                  -- path / terminal name; NULL for comment & live_debug
  reason       TEXT NOT NULL,         -- for kind='comment' this IS the comment body
  status       TEXT NOT NULL DEFAULT 'pending'
               CHECK (status IN ('pending','approved','denied')),
  ttl_minutes  INTEGER,               -- NULL = until assist closes
  created_at   TEXT NOT NULL,
  decided_at   TEXT
);
-- Grants are DERIVED, not stored: approved scope_requests that have not
-- expired, on an assist that is not done. Real enforcement attaches here
-- later without schema change.

CREATE TABLE credits (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  assist_ref      TEXT NOT NULL REFERENCES assists(ref),
  from_owner_id   TEXT NOT NULL REFERENCES users(id),
  to_responder_id TEXT NOT NULL REFERENCES users(id),
  created_at      TEXT NOT NULL,
  UNIQUE (assist_ref, to_responder_id)   -- once per assist; never prompted twice
);

CREATE TABLE resolution_records (
  assist_ref           TEXT PRIMARY KEY REFERENCES assists(ref),
  outcome              TEXT NOT NULL
                       CHECK (outcome IN ('resolved','worked_around','abandoned','self_resolved')),
  symptom              TEXT NOT NULL DEFAULT '',
  env_fingerprint      TEXT NOT NULL DEFAULT '',
  scopes_that_mattered TEXT NOT NULL DEFAULT '',
  dead_ends            TEXT NOT NULL DEFAULT '',
  fix                  TEXT NOT NULL DEFAULT '',
  created_at           TEXT NOT NULL
);
