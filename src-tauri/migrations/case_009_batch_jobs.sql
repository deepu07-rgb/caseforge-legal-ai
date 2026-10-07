-- CaseForge per-case vault — migration 009 (B-S1-10 overnight batch + resume)
-- Durable job + per-unit checkpoints so kill/power loss can resume (AC-6.8).

CREATE TABLE IF NOT EXISTS batch_job (
  job_id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL,
  status TEXT NOT NULL,
  -- queued | running | interrupted | completed | failed | cancelled
  stage TEXT NOT NULL,
  -- triage | recognize | skeleton | done
  progress_percent INTEGER NOT NULL DEFAULT 0,
  message TEXT,
  total_units INTEGER NOT NULL DEFAULT 0,
  completed_units INTEGER NOT NULL DEFAULT 0,
  page_count_estimate INTEGER,
  created_at_unix_ms INTEGER NOT NULL,
  updated_at_unix_ms INTEGER NOT NULL,
  last_checkpoint_at_unix_ms INTEGER,
  error TEXT
);

CREATE TABLE IF NOT EXISTS batch_checkpoint (
  job_id TEXT NOT NULL,
  unit_kind TEXT NOT NULL,
  -- doc_triage | doc_recognize | case_skeleton
  unit_id TEXT NOT NULL,
  -- doc_id or "*" for case-level
  status TEXT NOT NULL,
  -- pending | done | failed | skipped
  detail TEXT,
  completed_at_unix_ms INTEGER,
  PRIMARY KEY (job_id, unit_kind, unit_id),
  FOREIGN KEY (job_id) REFERENCES batch_job(job_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_batch_job_case
  ON batch_job (case_id, updated_at_unix_ms DESC);

CREATE INDEX IF NOT EXISTS idx_batch_checkpoint_pending
  ON batch_checkpoint (job_id, status);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (9, CAST(strftime('%s','now') AS INTEGER) * 1000);
