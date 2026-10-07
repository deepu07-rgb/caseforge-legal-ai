-- B-S3-20: encrypted text/provenance commit; source audio is deliberately never stored.
CREATE TABLE IF NOT EXISTS speech_transcript_commit (
  commit_id TEXT PRIMARY KEY,
  case_id TEXT,
  session_id TEXT,
  input_kind TEXT NOT NULL,
  audio_sha256 TEXT NOT NULL,
  raw_transcript TEXT NOT NULL,
  staged_transcript TEXT NOT NULL,
  correction_applied INTEGER NOT NULL,
  counsel_confirmed INTEGER NOT NULL,
  backend TEXT NOT NULL,
  pipeline_id TEXT NOT NULL,
  audio_retained INTEGER NOT NULL DEFAULT 0,
  created_at_unix_ms INTEGER NOT NULL,
  CHECK (input_kind IN ('REAL_AUDIO', 'REAL_AUDIO_CONFIRMED')),
  CHECK (length(audio_sha256) = 64),
  CHECK (counsel_confirmed = 1),
  CHECK (audio_retained = 0)
);
CREATE INDEX IF NOT EXISTS idx_speech_transcript_session
  ON speech_transcript_commit (case_id, session_id, created_at_unix_ms);
INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (6, CAST(strftime('%s','now') AS INTEGER) * 1000);
