-- Owner-published artifact catalog (what the owner's engine currently sees:
-- running terminals, agents, suggested paths) and a payload on scope
-- requests (e.g. the responder's SSH public key travelling with an ssh
-- request).

ALTER TABLE scope_requests ADD COLUMN payload TEXT;
ALTER TABLE assists ADD COLUMN catalog TEXT;
ALTER TABLE assists ADD COLUMN catalog_at TEXT;
