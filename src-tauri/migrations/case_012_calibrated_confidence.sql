-- B-S3-05: preserve raw engine confidence separately from calibrated confidence.
-- Critical fields remain encrypted in the per-case SQLCipher vault.

ALTER TABLE page_words ADD COLUMN raw_engine_confidence REAL;
ALTER TABLE page_words ADD COLUMN calibrated_confidence REAL;
ALTER TABLE page_words ADD COLUMN confidence_state TEXT NOT NULL DEFAULT 'missing';
ALTER TABLE page_words ADD COLUMN counsel_confirmed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE page_text_layer ADD COLUMN critical_fields_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE page_text_layer ADD COLUMN calibration_id TEXT;

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (12, CAST(strftime('%s','now') AS INTEGER) * 1000);
