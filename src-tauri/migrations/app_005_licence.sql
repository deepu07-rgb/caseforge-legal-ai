-- CaseForge app vault — migration 005 (B-S1-18 licensing + air-gap)
-- EXPIRED => read-only vault; data never ransomed (PRD §13, AC-13.1).

CREATE TABLE IF NOT EXISTS licence_state (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  state TEXT NOT NULL DEFAULT 'UNLICENSED',
  licence_id TEXT,
  tier TEXT,
  payload_json TEXT,
  activated_at_unix_ms INTEGER,
  expires_at_unix_ms INTEGER,
  fingerprint_match_count INTEGER NOT NULL DEFAULT 0,
  air_gap_enabled INTEGER NOT NULL DEFAULT 0,
  updated_at_unix_ms INTEGER NOT NULL,
  CHECK (state IN ('UNLICENSED', 'ACTIVE', 'EXPIRED')),
  CHECK (air_gap_enabled IN (0, 1))
);

INSERT OR IGNORE INTO licence_state (
  id, state, air_gap_enabled, updated_at_unix_ms
) VALUES (
  1, 'UNLICENSED', 0, CAST(strftime('%s','now') AS INTEGER) * 1000
);

CREATE TABLE IF NOT EXISTS licence_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

INSERT OR IGNORE INTO licence_meta (key, value) VALUES ('pipeline_id', 'caseforge.licence.v1');
INSERT OR IGNORE INTO licence_meta (key, value) VALUES ('case_data_network_path', 'forbidden');
INSERT OR IGNORE INTO licence_meta (key, value) VALUES ('user_data_ransom', 'forbidden');

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (5, CAST(strftime('%s','now') AS INTEGER) * 1000);
