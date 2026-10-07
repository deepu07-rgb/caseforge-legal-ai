-- CaseForge per-case vault — migration 005 (B-S1-06 recognition text layer)
-- Word-level confidence + char spans for low-confidence highlight. No case_graph.

CREATE TABLE IF NOT EXISTS page_text_layer (
  doc_id TEXT NOT NULL,
  page_index INTEGER NOT NULL,
  recognizer_id TEXT NOT NULL,
  recognized_at_unix_ms INTEGER NOT NULL,
  full_text TEXT NOT NULL,
  mean_confidence REAL NOT NULL,
  page_quality_warn INTEGER NOT NULL,
  notes TEXT,
  PRIMARY KEY (doc_id, page_index),
  FOREIGN KEY (doc_id) REFERENCES documents(doc_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS page_words (
  doc_id TEXT NOT NULL,
  page_index INTEGER NOT NULL,
  word_index INTEGER NOT NULL,
  text TEXT NOT NULL,
  confidence REAL NOT NULL,
  char_start INTEGER NOT NULL,
  char_end INTEGER NOT NULL,
  low_confidence INTEGER NOT NULL,
  force_review INTEGER NOT NULL,
  suppress_from_auto INTEGER NOT NULL,
  bbox_json TEXT,
  PRIMARY KEY (doc_id, page_index, word_index),
  FOREIGN KEY (doc_id, page_index) REFERENCES page_text_layer(doc_id, page_index) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_page_text_layer_doc
  ON page_text_layer (doc_id);

CREATE INDEX IF NOT EXISTS idx_page_words_doc_page
  ON page_words (doc_id, page_index);

CREATE INDEX IF NOT EXISTS idx_page_words_low
  ON page_words (doc_id, low_confidence);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (5, CAST(strftime('%s','now') AS INTEGER) * 1000);
