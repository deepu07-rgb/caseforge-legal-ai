# G-RETRIEVAL-US — United States authority retrieval

**Purpose:** Same as G-RETRIEVAL-IN for **United States** (separate query set).  
**Jurisdiction:** `United States`  
**PRD:** §4.3, §7  

## Micro-fixture

Synthetic query + `PLANTED_FAKE` citation for AC-4.3 wiring only.

## GOLDEN (B-S1-13)

Product-owner synthetic pack `S-SYNTH-GRETRIEVAL-v1`:

- `fixtures/synth_hitrate_20` — 20 queries + corpus; **status=GOLDEN** (not law)
- Metrics: AC-7.2 top-8 hit rate; AC-4.3 planted RED

```powershell
python eval/tools/close_b_s1_13_retrieval_golden.py
npm run test:retrieval
```
