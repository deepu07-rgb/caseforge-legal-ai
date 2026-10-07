# G-AUDIO — English voice / STT / lexicon

**Purpose:** Word-error and lexicon persistence fixtures; voice-only rehearsal scripts.  
**PRD:** §9, AC-9.1–9.4  
**Open:** Whisper small vs medium (PRD §21 #2) — decide using this set once GOLDEN audio exists.

## Notes

- Binary audio files are **not** committed in B-S0-04 (placeholders only).
- When authoring GOLDEN clips, prefer short English legal-phrase samples with transcripts in `known_answer.json`.

## Micro-fixture

| ID | Status | Role |
|----|--------|------|
| `fixtures/micro_001` | `SYNTHETIC_PLACEHOLDER` | Transcript-only stub; no audio blob |
