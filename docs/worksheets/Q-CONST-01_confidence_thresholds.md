# Q-CONST-01 — Confidence thresholds worksheet

**Status:** DECIDED (2026-08-06) — frozen in `contracts/constants.json`  
**Unblocks:** word-level recognition (B-S1-06); low-confidence UI highlights  
**Related:** Q-CG-03 (confidence type/range) · PRD §6.2  

---

## 1. Owner decision block

| Field | Answer |
|-------|--------|
| Owner name | CaseForge Product Owner |
| Date | 2026-08-06 |
| Decision status | **DECIDED** |
| Linked freeze | `contracts/constants.json` → `confidence_thresholds.status = FROZEN` |

---

## 2. Scale choice

| Option | Choose |
|--------|--------|
| **A — Unit float** [0.0, 1.0] | **✓** |

**Rationale:** Aligns with page_triage confidence and common OCR APIs.

---

## 3. Frozen values (unit float 0–1)

### Word-level (B-S1-06)

| ID | Name | Value |
|----|------|------:|
| W1 | `word_highlight_below` | **0.75** |
| W2 | `word_force_review_below` | **0.50** |
| W3 | `word_suppress_below` | **0.25** |
| W4 | `page_mean_warn_below` | **0.70** |

### Graph / skeleton (B-S1-07+)

| ID | Name | Value |
|----|------|------:|
| G1 | `item_highlight_below` | **0.80** |
| G2 | `item_block_downstream_below` | **0.55** |
| G3 | `chronology_date_strict_below` | **0.85** |

### Page triage UI

| ID | Name | Value |
|----|------|------:|
| T2 | `triage_low_confidence_below` | **0.75** |

---

## 4. Comparison rules

| Rule | Owner |
|------|-------|
| Highlight uses **strict below** (`conf < threshold`) | yes |
| Equality at threshold counts as **OK** (not low) | yes |
| Missing confidence treated as **force review** | yes |
| Counsel override stores original model conf + override flag | yes (later batches) |

---

## 5. UI honesty copy

> “Low model confidence — verify against source page. Not a finding of fact.”

---

## 6. Freeze checklist

- [x] Scale chosen (A)
- [x] W1–W4, G1–G3, T2 filled
- [x] Values in `constants.json`
- [x] status FROZEN
- [x] QUESTIONS.md Q-CONST-01 RESOLVED
- [x] B-S1-06 reads thresholds only from contracts
