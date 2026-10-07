-- CaseForge per-case vault — migration 004 (B-S1-05 docpipe triage)
-- Per-page triage labels + confidence. No OCR word boxes.

CREATE TABLE IF NOT EXISTS page_triage (
  doc_id TEXT NOT NULL,
  page_index INTEGER NOT NULL,
  triage_class TEXT NOT NULL,
  confidence REAL NOT NULL,
  classifier_id TEXT NOT NULL,
  classified_at_unix_ms INTEGER NOT NULL,
  notes TEXT,
  PRIMARY KEY (doc_id, page_index),
  FOREIGN KEY (doc_id) REFERENCES documents(doc_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_page_triage_doc
  ON page_triage (doc_id);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (4, CAST(strftime('%s','now') AS INTEGER) * 1000);
