-- CaseForge per-case vault — migration 007 (B-S1-08 review corrections)
-- Thin Experience Store hook: corrections only (full Experience Store = B-S1-17).

CREATE TABLE IF NOT EXISTS experience_corrections (
  correction_id TEXT PRIMARY KEY,
  created_at_unix_ms INTEGER NOT NULL,
  record_category TEXT NOT NULL DEFAULT 'correction',
  target_kind TEXT NOT NULL,
  -- graph_item | word_span
  collection TEXT,
  item_id TEXT,
  field_name TEXT,
  doc_id TEXT,
  page_index INTEGER,
  word_index INTEGER,
  old_value TEXT,
  new_value TEXT,
  interaction_count INTEGER NOT NULL DEFAULT 2,
  payload_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_experience_corrections_created
  ON experience_corrections (created_at_unix_ms DESC);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (7, CAST(strftime('%s','now') AS INTEGER) * 1000);
