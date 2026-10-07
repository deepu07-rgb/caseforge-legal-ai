-- CaseForge per-case vault — migration 010 (B-S2-01 argument tree + integrity)
-- Single JSON document per case; versioned for overwrite on rebuild/score.

CREATE TABLE IF NOT EXISTS argument_tree (
  case_id TEXT NOT NULL PRIMARY KEY,
  schema_version TEXT NOT NULL,
  tree_json TEXT NOT NULL,
  scorer_id TEXT NOT NULL,
  generated_at_unix_ms INTEGER NOT NULL,
  weakest_vi INTEGER
);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (10, CAST(strftime('%s','now') AS INTEGER) * 1000);
