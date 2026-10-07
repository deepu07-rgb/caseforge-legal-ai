-- CaseForge app vault — migration 002 (B-S1-03 New Case wizard fields)
-- PRD §5 wizard metadata on case registry (no document bodies).

ALTER TABLE cases ADD COLUMN cause_title TEXT NOT NULL DEFAULT '';
ALTER TABLE cases ADD COLUMN court TEXT NOT NULL DEFAULT '';
ALTER TABLE cases ADD COLUMN jurisdiction TEXT NOT NULL DEFAULT '';
ALTER TABLE cases ADD COLUMN matter_type TEXT NOT NULL DEFAULT '';
ALTER TABLE cases ADD COLUMN user_side TEXT NOT NULL DEFAULT '';
ALTER TABLE cases ADD COLUMN code_era TEXT NOT NULL DEFAULT '';
ALTER TABLE cases ADD COLUMN language TEXT NOT NULL DEFAULT 'English';

INSERT OR IGNORE INTO schema_migrations (version, applied_at_unix_ms)
VALUES (2, CAST(strftime('%s','now') AS INTEGER) * 1000);
