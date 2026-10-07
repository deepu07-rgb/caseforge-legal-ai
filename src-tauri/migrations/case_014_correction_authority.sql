-- B-S3-07 immutable authority events, current overlays and resolvable conflicts.
CREATE TABLE IF NOT EXISTS correction_events (
  event_id TEXT PRIMARY KEY, created_at_unix_ms INTEGER NOT NULL,
  event_type TEXT NOT NULL CHECK(event_type IN ('CORRECT','REVERT','DELETE','RESOLVE_KEEP_COUNSEL','RESOLVE_ACCEPT_GENERATED')),
  target_kind TEXT NOT NULL CHECK(target_kind IN ('graph_item','word_span')), target_key TEXT NOT NULL,
  base_value TEXT, counsel_value TEXT, reason TEXT NOT NULL,
  actor TEXT NOT NULL DEFAULT 'COUNSEL' CHECK(actor = 'COUNSEL'), supersedes_event_id TEXT,
  anchor_json TEXT NOT NULL, payload_json TEXT NOT NULL, immutable_hash TEXT NOT NULL,
  FOREIGN KEY(supersedes_event_id) REFERENCES correction_events(event_id)
);
CREATE INDEX IF NOT EXISTS idx_correction_events_target ON correction_events(target_key, created_at_unix_ms DESC);
CREATE TRIGGER IF NOT EXISTS correction_events_no_update
BEFORE UPDATE ON correction_events BEGIN SELECT RAISE(ABORT, 'CF_CORRECTION_EVENT_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS correction_events_no_delete
BEFORE DELETE ON correction_events BEGIN SELECT RAISE(ABORT, 'CF_CORRECTION_EVENT_IMMUTABLE'); END;

CREATE TABLE IF NOT EXISTS authority_overlays (
  target_key TEXT PRIMARY KEY, latest_event_id TEXT NOT NULL, target_kind TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1)), collection_name TEXT, item_id TEXT,
  field_name TEXT, doc_id TEXT, page_index INTEGER, word_index INTEGER, base_value TEXT,
  counsel_value TEXT, anchor_json TEXT NOT NULL, updated_at_unix_ms INTEGER NOT NULL,
  FOREIGN KEY(latest_event_id) REFERENCES correction_events(event_id)
);

CREATE TABLE IF NOT EXISTS correction_conflicts (
  conflict_id TEXT PRIMARY KEY, overlay_event_id TEXT NOT NULL, target_key TEXT NOT NULL,
  target_kind TEXT NOT NULL, conflict_kind TEXT NOT NULL CHECK(conflict_kind IN ('MATERIAL_DISAGREEMENT','TARGET_DELETED','AMBIGUOUS_MATCH')),
  generated_value TEXT, counsel_value TEXT, anchor_json TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'OPEN' CHECK(status IN ('OPEN','KEEP_COUNSEL','ACCEPT_GENERATED')),
  created_at_unix_ms INTEGER NOT NULL, resolved_at_unix_ms INTEGER, resolution_event_id TEXT,
  UNIQUE(overlay_event_id, conflict_kind, generated_value)
);
CREATE INDEX IF NOT EXISTS idx_correction_conflicts_status ON correction_conflicts(status, created_at_unix_ms DESC);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (14, CAST(strftime('%s','now') AS INTEGER) * 1000);
