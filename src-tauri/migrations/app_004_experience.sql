-- CaseForge app vault — migration 004 (B-S1-17 Experience Store)
-- Local encrypted Experience Store (PRD §11, AC-11.1 / AC-11.2).
-- Fleet upload is NEVER default-on; fleet_enabled defaults false.

CREATE TABLE IF NOT EXISTS experience_records (
  record_id TEXT PRIMARY KEY,
  schema_version TEXT NOT NULL DEFAULT '1.0.0',
  record_category TEXT NOT NULL,
  stage TEXT NOT NULL DEFAULT 'S1',
  language TEXT NOT NULL DEFAULT 'English',
  jurisdiction TEXT,
  case_id TEXT,
  summary TEXT NOT NULL,
  tags_json TEXT NOT NULL DEFAULT '[]',
  relevance_key TEXT NOT NULL,
  payload_json TEXT NOT NULL DEFAULT '{}',
  source TEXT NOT NULL DEFAULT 'user',
  fleet_status TEXT NOT NULL DEFAULT 'local_only',
  active INTEGER NOT NULL DEFAULT 1,
  notes TEXT,
  created_at_unix_ms INTEGER NOT NULL,
  updated_at_unix_ms INTEGER NOT NULL,
  CHECK (record_category IN (
    'correction', 'lexicon_entry', 'rated_exchange',
    'draft_edit', 'limb_outcome', 'bench_setting'
  )),
  CHECK (stage IN ('S1', 'S2')),
  CHECK (source IN ('user', 'system', 'import')),
  CHECK (fleet_status IN (
    'local_only', 'pending_review', 'approved_anonymised', 'rejected'
  )),
  CHECK (active IN (0, 1))
);

CREATE INDEX IF NOT EXISTS idx_experience_category_active
  ON experience_records (record_category, active);

CREATE INDEX IF NOT EXISTS idx_experience_relevance
  ON experience_records (relevance_key);

CREATE INDEX IF NOT EXISTS idx_experience_created
  ON experience_records (created_at_unix_ms DESC);

CREATE TABLE IF NOT EXISTS experience_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- AC-11.2: fleet contribution off by default; no network path in Stage 1.
INSERT OR IGNORE INTO experience_meta (key, value) VALUES ('fleet_enabled', 'false');
INSERT OR IGNORE INTO experience_meta (key, value) VALUES ('fleet_network_upload', 'false');
INSERT OR IGNORE INTO experience_meta (key, value) VALUES ('top_k', '5');
INSERT OR IGNORE INTO experience_meta (key, value) VALUES ('pipeline_id', 'caseforge.experience.v1');

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (4, CAST(strftime('%s','now') AS INTEGER) * 1000);
