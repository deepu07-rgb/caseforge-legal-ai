-- CaseForge per-case encrypted vault — migration 015 (B-S3-12 matter legal working set)

CREATE TABLE IF NOT EXISTS legal_working_set_meta (
  case_id TEXT NOT NULL PRIMARY KEY,
  schema_version TEXT NOT NULL,
  version INTEGER NOT NULL,
  regime_status TEXT NOT NULL,
  confirmed_regime_id TEXT,
  regime_candidates_json TEXT NOT NULL,
  updated_at_unix_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS legal_working_set_item (
  case_id TEXT NOT NULL,
  item_id TEXT NOT NULL,
  authority_id TEXT NOT NULL,
  source_id TEXT NOT NULL,
  chunk_id TEXT NOT NULL,
  pack_id TEXT NOT NULL,
  title TEXT NOT NULL,
  citation TEXT NOT NULL,
  section_label TEXT NOT NULL,
  jurisdiction TEXT NOT NULL,
  authority_type TEXT NOT NULL,
  trust_level TEXT NOT NULL,
  temporal_status TEXT NOT NULL,
  superseded_status TEXT NOT NULL,
  source_path TEXT NOT NULL,
  source_reference TEXT NOT NULL,
  source_checksum TEXT NOT NULL,
  chunk_checksum TEXT NOT NULL,
  exact_text TEXT NOT NULL,
  decision TEXT NOT NULL,
  pinned INTEGER NOT NULL DEFAULT 0,
  controlling INTEGER NOT NULL DEFAULT 0,
  counsel_rationale TEXT,
  first_added_at_unix_ms INTEGER NOT NULL,
  updated_at_unix_ms INTEGER NOT NULL,
  last_seen_index_checksum TEXT NOT NULL,
  PRIMARY KEY (case_id, item_id),
  UNIQUE (case_id, authority_id)
);

CREATE TABLE IF NOT EXISTS legal_working_set_revision (
  case_id TEXT NOT NULL,
  version INTEGER NOT NULL,
  created_at_unix_ms INTEGER NOT NULL,
  event_type TEXT NOT NULL,
  snapshot_json TEXT NOT NULL,
  PRIMARY KEY (case_id, version)
);

CREATE TABLE IF NOT EXISTS legal_output_authority_provenance (
  output_id TEXT NOT NULL PRIMARY KEY,
  case_id TEXT NOT NULL,
  output_kind TEXT NOT NULL,
  working_set_version INTEGER NOT NULL,
  authority_ids_json TEXT NOT NULL,
  created_at_unix_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_working_set_decision
ON legal_working_set_item(case_id, decision, pinned, controlling);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (15, CAST(strftime('%s','now') AS INTEGER) * 1000);
