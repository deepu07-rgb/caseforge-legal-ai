-- CaseForge per-case vault — migration 001 (B-S1-02)
-- No case_graph / documents yet (later batches).

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at_unix_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS vault_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (1, CAST(strftime('%s','now') AS INTEGER) * 1000);
