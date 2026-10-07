-- CaseForge per-case vault — migration 006 (B-S1-07 case_graph skeleton)
-- Single JSON document per case; versioned for overwrite on re-extract.

CREATE TABLE IF NOT EXISTS case_graph (
  case_id TEXT NOT NULL PRIMARY KEY,
  schema_version TEXT NOT NULL,
  graph_json TEXT NOT NULL,
  extractor_id TEXT NOT NULL,
  generated_at_unix_ms INTEGER NOT NULL,
  completeness_score INTEGER
);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (6, CAST(strftime('%s','now') AS INTEGER) * 1000);
