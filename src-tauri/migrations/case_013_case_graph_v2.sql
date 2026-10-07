-- B-S3-06: normalized graph-v2 contract metadata.
-- Existing v1 graph_json is retained byte-for-byte. It remains readable by legacy
-- projections and must be re-extracted before v2-only nodes/edges are consumed.
ALTER TABLE case_graph ADD COLUMN graph_contract_version TEXT NOT NULL DEFAULT '1.0.0';
ALTER TABLE case_graph ADD COLUMN migration_strategy TEXT NOT NULL DEFAULT 'v1_retained_reextract_for_v2';

UPDATE case_graph
SET graph_contract_version = schema_version,
    migration_strategy = CASE
      WHEN schema_version = '2.0.0' THEN 'normalized_v2_with_v1_read_projections'
      ELSE 'v1_retained_reextract_for_v2'
    END;

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (13, CAST(strftime('%s','now') AS INTEGER) * 1000);
