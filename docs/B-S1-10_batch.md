# B-S1-10 — Overnight batch + resume

**Status:** DONE (2026-08-07)

## Goal

Long-running ingest is **resumable** after kill/power loss (AC-6.8). Performance budget AC-3.2 is measured on Full-tier hardware or SKIP with explicit note.

## Pipeline units

Ordered checkpoints:

1. `doc_triage` per document  
2. `doc_recognize` per document  
3. `case_skeleton` once  

Each completed unit is written to `batch_checkpoint` before the next unit starts.

## Host IPC

| Command | Role |
|---------|------|
| `batch_start` | Create job + worker (`stop_after_units` optional kill sim) |
| `batch_resume` | Continue from next pending checkpoint |
| `batch_cancel` | Soft-kill → `interrupted` |
| `batch_status` / `batch_latest` | Durable status |

## UI

Case file → **Batch** tab: start, resume, cancel, simulate kill (1 unit).

## Eval

```powershell
npm run test:batch
python eval/run_eval.py --ac AC-6.8
python eval/run_eval.py --ac AC-3.2
```

| AC | Expected |
|----|----------|
| AC-6.8 | PASS — durable checkpoints + resume wiring |
| AC-3.2 | SKIP until Full-tier 300-page / 8h measurement (budget frozen; do not lower) |

## Out of scope

- Cloud queue / multi-machine  
- Lowering AC-3.2 on miss (tune only)
