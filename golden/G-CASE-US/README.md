# G-CASE-US — United States case skeleton known-answer

**Purpose:** Chronology, parties, statutes/rules, relief, contradictions, interview gaps for **United States** jurisdiction.  
**PRD:** §2 Stage 1, §5, §6.3–§6.5, §6.8  
**Jurisdiction:** `United States` only.

## Target metrics

Same AC-6.2–AC-6.6 / AC-6.8 as G-CASE-IN; separate known-answers (do not mix jurisdictions).

## Micro-fixture

| ID | Status | Role |
|----|--------|------|
| `fixtures/micro_001` | `SYNTHETIC_PLACEHOLDER` | Harness discovery only — **not** legal truth |

## DRAFT scaffold tool

See [`AUTHORING.md`](./AUTHORING.md).

```powershell
python eval/tools/scaffold_gcase_us_fixture.py --id matter_0001 --author "you"
# or: npm run gcase-us:scaffold -- --id matter_0001 --author "you"
```

Creates `fixtures/<id>/` with `status=DRAFT` and **empty** known-answer shells (no invented legal facts).
