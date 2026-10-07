-- CaseForge app vault — migration 003 (B-S1-16 self-correcting lexicon)
-- Global English lexicon in encrypted app DB (PRD §9, AC-9.2).

CREATE TABLE IF NOT EXISTS lexicon_entries (
  entry_id TEXT PRIMARY KEY,
  schema_version TEXT NOT NULL DEFAULT '1.0.0',
  entry_kind TEXT NOT NULL,
  language TEXT NOT NULL DEFAULT 'English',
  surface_form TEXT NOT NULL,
  corrected_form TEXT,
  pronunciation TEXT,
  pattern_type TEXT NOT NULL DEFAULT 'custom',
  source TEXT NOT NULL DEFAULT 'user',
  active INTEGER NOT NULL DEFAULT 1,
  scope TEXT NOT NULL DEFAULT 'global',
  notes TEXT,
  created_at_unix_ms INTEGER NOT NULL,
  updated_at_unix_ms INTEGER NOT NULL,
  CHECK (entry_kind IN ('stt_correction', 'tts_pronunciation_override')),
  CHECK (language = 'English'),
  CHECK (pattern_type IN ('legal_english', 'statute_name', 'proper_name', 'custom')),
  CHECK (source IN ('preseed', 'user', 'import')),
  CHECK (active IN (0, 1)),
  CHECK (scope = 'global')
);

CREATE INDEX IF NOT EXISTS idx_lexicon_kind_active
  ON lexicon_entries (entry_kind, active);

CREATE INDEX IF NOT EXISTS idx_lexicon_surface
  ON lexicon_entries (surface_form);

CREATE TABLE IF NOT EXISTS lexicon_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (3, CAST(strftime('%s','now') AS INTEGER) * 1000);
