-- B-S3-03 production page source/layout model. Stored inside SQLCipher case DB.
-- Originals remain in documents.content; OCR render bytes are not persisted here.

CREATE TABLE IF NOT EXISTS page_source (
  doc_id TEXT NOT NULL,
  page_index INTEGER NOT NULL,
  parser_id TEXT NOT NULL,
  source_kind TEXT NOT NULL,
  state TEXT NOT NULL,
  width_points REAL NOT NULL,
  height_points REAL NOT NULL,
  rotation_degrees INTEGER NOT NULL,
  source_sha256 TEXT NOT NULL,
  page_source_json TEXT NOT NULL,
  parsed_at_unix_ms INTEGER NOT NULL,
  PRIMARY KEY (doc_id, page_index),
  FOREIGN KEY (doc_id) REFERENCES documents(doc_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_page_source_doc ON page_source (doc_id, page_index);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (11, CAST(strftime('%s','now') AS INTEGER) * 1000);
