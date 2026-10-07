-- CaseForge app vault — migration 001 (B-S1-02)
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at_unix_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS vault_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cases (
  case_id TEXT PRIMARY KEY,
  created_at_unix_ms INTEGER NOT NULL,
  label TEXT NOT NULL DEFAULT ''
);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (1, CAST(strftime('%s','now') AS INTEGER) * 1000);
