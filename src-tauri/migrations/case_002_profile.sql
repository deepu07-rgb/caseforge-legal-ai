-- CaseForge per-case vault — migration 002 (B-S1-03)
-- Immutable jurisdiction + wizard profile. Empty vault otherwise.

CREATE TABLE IF NOT EXISTS case_profile (
  case_id TEXT PRIMARY KEY,
  cause_title TEXT NOT NULL,
  court TEXT NOT NULL,
  jurisdiction TEXT NOT NULL,
  matter_type TEXT NOT NULL,
  user_side TEXT NOT NULL,
  code_era TEXT NOT NULL,
  language TEXT NOT NULL,
  created_at_unix_ms INTEGER NOT NULL
);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (2, CAST(strftime('%s','now') AS INTEGER) * 1000);
