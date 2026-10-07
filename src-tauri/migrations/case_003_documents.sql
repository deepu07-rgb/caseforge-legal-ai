-- CaseForge per-case vault — migration 003 (B-S1-04 document intake)
-- Originals stored as BLOBs inside SQLCipher (encrypted case store).
-- OCR / triage / quality grades come later; per_page_quality_grade is placeholder.

CREATE TABLE IF NOT EXISTS documents (
  doc_id TEXT PRIMARY KEY,
  original_filename TEXT NOT NULL,
  type_tag TEXT NOT NULL,
  language_tag TEXT NOT NULL DEFAULT 'en',
  source_format TEXT NOT NULL,
  byte_size INTEGER NOT NULL,
  page_count INTEGER,
  per_page_quality_grade TEXT,
  ingested_at_unix_ms INTEGER NOT NULL,
  content BLOB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_documents_ingested
  ON documents (ingested_at_unix_ms DESC);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (3, CAST(strftime('%s','now') AS INTEGER) * 1000);
