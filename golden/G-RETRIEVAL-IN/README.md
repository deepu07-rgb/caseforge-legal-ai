# G-RETRIEVAL-IN — India authority retrieval

**Purpose:** Queries whose known-relevant authorities must appear in top-8 (AC-7.2); planted fake citations must be RED (AC-4.3).  
**Jurisdiction:** `India`  
**PRD:** §4.3, §7  

## Metrics

| AC | Target |
|----|--------|
| AC-7.2 | Known-relevant authority in top-8 for ≥ 85% of golden queries |
| AC-4.3 | Planted fake citation always RED; excluded unless user confirms |

## Micro-fixture

Includes an explicit **`PLANTED_FAKE`** citation string for negative testing (allowed synthetic; not legal truth).

## GOLDEN (B-S1-13)

Product-owner synthetic pack `S-SYNTH-GRETRIEVAL-v1`:

- `fixtures/synth_hitrate_20` — 20 queries + corpus; **status=GOLDEN** (not law)
- Metrics: AC-7.2 top-8 hit rate; AC-4.3 planted RED

```powershell
python eval/tools/close_b_s1_13_retrieval_golden.py
npm run test:retrieval
```
