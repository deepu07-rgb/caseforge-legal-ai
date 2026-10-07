-- B-S3-18: immutable source-specific debrief snapshots and separately mutable counsel tasks.
CREATE TABLE IF NOT EXISTS mock_court_debrief_v2 (
  debrief_id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL,
  session_id TEXT NOT NULL,
  created_at_unix_ms INTEGER NOT NULL,
  source_fingerprint TEXT NOT NULL,
  report_json TEXT NOT NULL,
  UNIQUE(case_id, session_id)
);
CREATE TABLE IF NOT EXISTS preparation_task (
  case_id TEXT NOT NULL,
  task_id TEXT NOT NULL,
  debrief_id TEXT NOT NULL,
  original_json TEXT NOT NULL,
  title TEXT NOT NULL,
  status TEXT NOT NULL,
  counsel_note TEXT,
  updated_at_unix_ms INTEGER NOT NULL,
  PRIMARY KEY(case_id, task_id)
);
CREATE INDEX IF NOT EXISTS idx_preparation_task_status ON preparation_task(case_id, status, updated_at_unix_ms);
INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms) VALUES (16, CAST(strftime('%s','now') AS INTEGER) * 1000);
