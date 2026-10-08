-- B-S4-07/08: statement register, counsel witness merge/split decisions (append-only, revocable) and counsel status
-- on impeachment findings. Namespaced keys: register:<doc_id>, decision:<seq>, finding:<finding_id>.
CREATE TABLE IF NOT EXISTS witness_store (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at_unix_ms INTEGER NOT NULL
);
INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms) VALUES (17, CAST(strftime('%s','now') AS INTEGER) * 1000);
