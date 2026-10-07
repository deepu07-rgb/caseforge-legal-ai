# B-S1-15 — Drafting surface + DOCX/PDF export

**Status:** DONE (2026-08-08)

## Goal

Stage 1 **DRAFTING** surface: templates + court-format **DOCX / PDF** export with required **G4 footer** and **G5 provenance** on AI / export spans.

## Surfaces (PRD §12)

| Surface | Role |
|---------|------|
| CASEBOOK | Verification desk (B-S1-14) |
| CHAMBER | Working desk (B-S1-14) |
| DIGEST | Synthesis (B-S1-14) |
| **DRAFTING** | Templates + export (this batch) |

## Templates (`caseforge.draft_export.v1`)

| id | Export content |
|----|----------------|
| `court_formatted_digest` | Full digest sections I–VIII from `case_graph` |
| `list_of_dates` | Chronology-only dated list |
| `synopsis` | Short preparation synopsis (not outcome prediction) |
| `counsel_memo` | Editable preparation memo skeleton |

Source of truth: **`case_graph` only**. Out of scope: auto-filing to court portals.

## G4 export footer

| | |
|--|--|
| Footer id | `caseforge.export_footer.v1` |
| Contract | `contracts/constants.json` → `export_footer_g4` (**FROZEN**) |
| Required | On **every** DOCX and PDF export |

**Frozen lines (no new legal disclaimer invented):**

1. Existing Stage-1 honesty notice from `infer_llm`:  
   *Local model output for preparation only. Not legal advice. Citations require pack verification.*
2. Frozen G6 preparation language from B-S1-14:  
   *Preparation completeness only — never win prediction, outcome odds, or verdict language.*
3. Structural G4 marker with `footer_id`.

## G5 provenance

| | |
|--|--|
| Contract | `contracts/constants.json` → `export_provenance_g5` (**FROZEN**) |
| Per AI span | `model_hash`, `prompt_hash`, `retrieval_ids` |
| Deterministic graph export | stamps `deterministic:case_graph` / `deterministic:template` defaults |
| Export appendix | full provenance log in plain text / DOCX / PDF |

## Implementation

| Artefact | Path |
|----------|------|
| Export core | `src/lib/exportDraft.ts` |
| UI | `src/components/DraftingView.tsx` (via Digest → **DRAFTING** tab) |
| Styles | `src/styles.css` (draft editor / footer preview) |

Formats:

- **DOCX** — minimal OOXML ZIP (Word-openable)
- **PDF** — minimal PDF 1.4 text (reader-openable)

## Eval

```powershell
npm run test:drafting
```

Checks: frozen contracts, UI wiring, G4 footer presence, G5 fields, DOCX `PK` + text extract, PDF `%PDF` + footer marker, all four templates.

## Out of scope

- Auto-filing to court portals  
- Stage 2 Argument Tree / Mock Court  
- Inventing new legal disclaimer wording beyond frozen reuse of existing honesty + G6
