# B-S1-14 — Digest surface + completeness score

**Status:** DONE (2026-08-07)

## Goal

Full Stage 1 **DIGEST** UI with §12 surfaces and a **deterministic Preparation Completeness Score** derived only from `case_graph` (no LLM). Guardrail **G6**: preparation language only.

## Surfaces (PRD §12)

| Surface | Role |
|---------|------|
| **CASEBOOK** | Verification desk — low-confidence heat, jump to Review (AC-6.3/6.4) |
| **CHAMBER** | Working desk — open gaps, guided interview, counsel facts |
| **DIGEST** | Synthesis — chronology, parties, statutes, evidence, contradictions, relief |
| **DRAFTING** | Templates + DOCX/PDF export (B-S1-15) |

## Preparation Completeness

| | |
|--|--|
| Score id | `caseforge.prep_completeness.v1` |
| Contract | `contracts/constants.json` → `preparation_completeness` (**FROZEN**) |
| Formula | 20 points × non-empty: parties, chronology, statutes, evidence, prayer_relief (max 100) |
| Source | `case_graph` fields only |
| Forbidden | LLM judgment, win probability, outcome prediction |

Recompute: `src/lib/completeness.ts` → `computePrepCompleteness(graph)`.  
Skeleton extractor stamps the same formula/score_id on write.

## UI entry

Nav **Digest** → `DigestView` (matter must be open for graph actions).

## Eval

```powershell
npm run test:digest
```

## Out of scope

- ARGUMENT TREE / Mock Court / Bench Prep  
- DOCX/PDF export (B-S1-15)  
- Win prediction of any kind (hard fail / G6)
