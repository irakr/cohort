-- Honest brief rework: the owner writes the problem description; "insights"
-- holds the AI-drafted analysis (empty until the Cohort AI integration, shown
-- as N/A). Fabricated "failures" are gone entirely - real failure capture is
-- the detector's job (P1). Shared artifacts keep their app icon and pid so
-- the assist view can list them properly.

ALTER TABLE assists RENAME COLUMN goal TO description;
ALTER TABLE assists DROP COLUMN failures;
ALTER TABLE assists ADD COLUMN insights TEXT NOT NULL DEFAULT '';

ALTER TABLE assist_artifacts ADD COLUMN icon TEXT;
ALTER TABLE assist_artifacts ADD COLUMN pid INTEGER;
