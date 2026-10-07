-- CaseForge per-case vault — migration 008 (B-S1-09 Guided Intake Interview)
-- Session state is resumable / skippable / re-openable (AC-6.6).

CREATE TABLE IF NOT EXISTS interview_session (
  session_id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL,
  status TEXT NOT NULL,
  -- open | paused | completed
  created_at_unix_ms INTEGER NOT NULL,
  updated_at_unix_ms INTEGER NOT NULL,
  max_questions INTEGER NOT NULL,
  cursor_index INTEGER NOT NULL DEFAULT 0,
  completion_percent INTEGER NOT NULL DEFAULT 0,
  notes TEXT
);

CREATE TABLE IF NOT EXISTS interview_question (
  session_id TEXT NOT NULL,
  question_index INTEGER NOT NULL,
  gap_id TEXT,
  topic TEXT NOT NULL,
  prompt TEXT NOT NULL,
  status TEXT NOT NULL,
  -- pending | answered | skipped
  answer_text TEXT,
  answered_fact_id TEXT,
  answered_at_unix_ms INTEGER,
  PRIMARY KEY (session_id, question_index),
  FOREIGN KEY (session_id) REFERENCES interview_session(session_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_interview_session_case
  ON interview_session (case_id, updated_at_unix_ms DESC);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (8, CAST(strftime('%s','now') AS INTEGER) * 1000);
