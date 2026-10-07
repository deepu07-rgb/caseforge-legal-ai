# B-S1-06 — docpipe recognition + confidence

**Status:** DONE (2026-08-06)  
**Depends on:** B-S1-05 PASS (AC-6.1 green)

## Goal

Every word has a confidence score. Thresholds come only from `contracts/constants.json`. UI shows low-confidence highlights.

## Artefacts

| Item | Path |
|------|------|
| Thresholds (Q-CONST-01) | `contracts/constants.json` → `confidence_thresholds` |
| Schema | `contracts/page_text_layer.schema.json` |
| Python recognizer | `sidecars/docpipe/recognize.py` |
| Rust recognizer | `src-tauri/src/docpipe/recognize.rs` |
| Vault migration | `src-tauri/migrations/case_005_page_text_layer.sql` |
| Eval | `eval/checks/docpipe_recognize_b_s1_06.py` (`npm run test:recognize`) |

## Recognizer

- **id:** `caseforge.recognize.provisional.v1`
- **Scope:** text / DOCX / crude PDF text extract; images without vision OCR → empty layer + quality warn
- **Not in scope:** case_graph, parties/chronology NLP, vision word boxes (later when VLM installed)

## Thresholds (unit float 0–1, strict below)

| Key | Value |
|-----|------:|
| word_highlight_below | 0.75 |
| word_force_review_below | 0.50 |
| word_suppress_below | 0.25 |
| page_mean_warn_below | 0.70 |

## UI

Case file → **Text layer** tab: run recognition, amber underline for low confidence, wavy red for force-review.

## Done criteria

- [x] Text layer with per-word confidence + char spans  
- [x] Flags applied from contracts only  
- [x] Minimal low-confidence UI  
- [x] Structural eval PASS  
