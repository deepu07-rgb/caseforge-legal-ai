# B-S2-06 — Stage 2 Experience Store + full red-team (AC-15.1)

**Status:** DONE (2026-08-09)

## Goal

1. Unlock Experience Store categories **limb_outcome** and **bench_setting** (plus continued rated exchanges).  
2. Ship full **AC-15.1** red-team suite ≥ **200** prompts with **zero** G1–G3 violations.

## Experience Store (Stage 2)

| Category | Stage | Notes |
|----------|-------|--------|
| correction, lexicon_entry, draft_edit, rated_exchange | S1 | Unchanged |
| **limb_outcome** | **S2** | Argument limb outcomes after drills |
| **bench_setting** | **S2** | Adaptive Bench slider/archetype patterns |

- Still local + encrypted app vault  
- Fleet remains **off by default** (AC-11.2)  
- top-K injection unchanged (`top_k=5`)

## AC-15.1 full suite

| Field | Value |
|-------|--------|
| Suite id | `caseforge.redteam.stage2.v1` |
| Fixture | `golden/G-REDTEAM/fixtures/stage2_200` |
| Count | 210 (70×G1 / 70×G2 / 70×G3) |
| Violations allowed | **0** |
| Runner | `guardrails_redteam_full` / Python eval |

Regenerate:

```powershell
python eval/tools/generate_stage2_redteam.py
```

## Eval

```powershell
npm run test:s2redteam
```

## IPC / UI

- Vault allows S2 category writes; stage forced to `S2`  
- Experience UI: limb_outcome + bench_setting selectable  
- Guardrails panel: AC-15.1 full suite chip  

## Halt rule

Any G1–G3 miss on the 200+ suite → **FAIL** (do not ship Stage 2).
