-- Cohort hub schema. The single source of truth: applied in full when the
-- hub opens its pool, and edited in place when the design changes (there is
-- no migration history - reset the database with `make db-reset`).
--
-- Vocabulary: assists, responders, scope requests, credits, resolution
-- records. Never session/ticket/issue/case.
-- No priority or severity column, by design (project plan section 5).
-- No aggregate of help received anywhere, by design (project plan section 8).

CREATE TABLE IF NOT EXISTS users (
  id       TEXT PRIMARY KEY,          -- 'u-alex'
  name     TEXT NOT NULL,
  initials TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS assists (
  ref         TEXT PRIMARY KEY,       -- 'S-2411'
  title       TEXT NOT NULL,
  status      TEXT NOT NULL DEFAULT 'open'
              CHECK (status IN ('open','dormant','done')),
  category    TEXT
              CHECK (category IS NULL OR category IN
                ('broken','environment','approach','review','knowledge','agent_loop')),
  owner_id    TEXT NOT NULL REFERENCES users(id),
  anonymous   INTEGER NOT NULL DEFAULT 0,
  description TEXT NOT NULL DEFAULT '',   -- brief: the owner's own words, markdown
  insights    TEXT NOT NULL DEFAULT '',   -- brief: AI-drafted analysis, empty means N/A
  environment TEXT NOT NULL DEFAULT '[]', -- brief: JSON array of chip strings
  live_data   TEXT,                       -- JSON: file tree, file contents, agent chat
  catalog     TEXT,                       -- JSON: what the owner's engine currently sees
  catalog_at  TEXT,
  created_at  TEXT NOT NULL,              -- RFC3339
  closed_at   TEXT
);

-- Artifacts the owner actually shared, with the app icon and process id
-- captured at share time.
CREATE TABLE IF NOT EXISTS assist_artifacts (
  assist_ref TEXT NOT NULL REFERENCES assists(ref),
  id         TEXT NOT NULL,
  kind       TEXT NOT NULL CHECK (kind IN ('file','ai_agent','custom')),
  label      TEXT NOT NULL,
  detail     TEXT NOT NULL DEFAULT '',
  icon       TEXT,
  pid        INTEGER,
  PRIMARY KEY (assist_ref, id)
);

CREATE TABLE IF NOT EXISTS assist_tags (
  assist_ref TEXT NOT NULL REFERENCES assists(ref),
  tag        TEXT NOT NULL,
  PRIMARY KEY (assist_ref, tag)
);

CREATE TABLE IF NOT EXISTS responders (
  assist_ref TEXT NOT NULL REFERENCES assists(ref),
  user_id    TEXT NOT NULL REFERENCES users(id),
  joined_at  TEXT NOT NULL,
  PRIMARY KEY (assist_ref, user_id)
);

CREATE TABLE IF NOT EXISTS scope_requests (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  assist_ref   TEXT NOT NULL REFERENCES assists(ref),
  requester_id TEXT NOT NULL REFERENCES users(id),
  kind         TEXT NOT NULL
               CHECK (kind IN ('comment','live_debug','file','agents','ssh','window')),
  target       TEXT,                  -- path / window name; NULL for comment & live_debug
  reason       TEXT NOT NULL,         -- for kind='comment' this IS the comment body
  status       TEXT NOT NULL DEFAULT 'pending'
               CHECK (status IN ('pending','approved','denied','revoked')),
  payload      TEXT,                  -- e.g. the responder's SSH public key
  ttl_minutes  INTEGER,               -- NULL = until assist closes
  created_at   TEXT NOT NULL,
  decided_at   TEXT
);
-- Grants are DERIVED, not stored: approved scope_requests that have not
-- expired or been revoked, on an assist that is not done. Real enforcement
-- attaches here later without schema change.

CREATE TABLE IF NOT EXISTS credits (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  assist_ref      TEXT NOT NULL REFERENCES assists(ref),
  from_owner_id   TEXT NOT NULL REFERENCES users(id),
  to_responder_id TEXT NOT NULL REFERENCES users(id),
  created_at      TEXT NOT NULL,
  UNIQUE (assist_ref, to_responder_id)   -- once per assist; never prompted twice
);

CREATE TABLE IF NOT EXISTS resolution_records (
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
