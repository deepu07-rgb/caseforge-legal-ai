# G-CASE-IN — India case skeleton known-answer

**Purpose:** Chronology, parties, statutes, relief, contradictions, interview gaps for **India** jurisdiction.  
**PRD:** §2 Stage 1, §5, §6.3–§6.5, §6.8  
**Jurisdiction:** `India` only.

## Target metrics (from constants)

| AC | Target |
|----|--------|
| AC-6.2 | ≥ 95% recall of known dated events on golden 40-page document |
| AC-6.3 | Click fact → source page + span |
| AC-6.4 | ≤ 2 interactions to correct highlighted word |
| AC-6.5 | ≤ 12 questions → ≥ 90% known-answer graph |
| AC-6.6 | Interview resumable / skippable / re-openable |
| AC-6.8 | Batch resume after kill |

## Schema target

Runtime graph instances: `case_graph.json` per `contracts/case_graph.schema.json` (currently BLOCKED field inventory — known_answer shells stay placeholder-shaped).

## Micro-fixture

| ID | Status | Role |
|----|--------|------|
| `fixtures/micro_001` | `SYNTHETIC_PLACEHOLDER` | Harness discovery only — **not** legal truth |

## DRAFT scaffold tool

See [`AUTHORING.md`](./AUTHORING.md).

```powershell
python eval/tools/scaffold_gcase_in_fixture.py --id matter_0001 --author "you"
# or: npm run gcase-in:scaffold -- --id matter_0001 --author "you"
```

Creates `fixtures/<id>/` with `status=DRAFT` and **empty** known-answer shells (no invented legal facts).
