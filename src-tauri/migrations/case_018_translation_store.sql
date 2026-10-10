-- B-S4-15: aligned Hindi-to-English translation segments, counsel's confirmations of translated critical fields (append-only events)
-- and the facts read from a translation. Namespaced keys: seg:<doc_id>:<page>:<segment>, evt:<ms>:<segment_id>:<field_id>, graph:<doc_id>.
CREATE TABLE IF NOT EXISTS translation_store (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at_unix_ms INTEGER NOT NULL
);
INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms) VALUES (18, CAST(strftime('%s','now') AS INTEGER) * 1000);
