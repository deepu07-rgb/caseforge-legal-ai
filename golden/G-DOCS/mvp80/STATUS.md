# Track B status — G-DOCS / B-S1-05

**Updated:** 2026-08-06

## Pipeline state

| Step | Status |
|------|--------|
| Roster | Product-owner path (`CaseForge Product Owner`) |
| Sources | `S-SYNTH-GDOCS-500` synthetic pedagogy pack |
| Staging pages | 500 synthetic `.txt` pages under `staging/*/` |
| GOLDEN fixtures | **`page_0001` … `page_0500` locked GOLDEN** |
| Dual-label sheets | `labels_A.csv`, `labels_B.csv`, `labels_final.csv` (product-owner adjudication) |
| Audit | `PRODUCT_OWNER_LOCK_AUDIT.json` |
| AC-6.1 | **PASS** 100% on 500 pages |
| Q-VISION-01 | Stage-1 freeze → `caseforge.triage.heuristic.v1` |

## Commands

```powershell
python eval/tools/gdocs_progress.py
python eval/checks/ac_6_1_triage_score.py
python eval/bakeoff/run_bakeoff.py
python eval/run_eval.py --ac AC-6.1
```

## Notes

- Synthetic pedagogy pack locked under product-owner authority to close B-S1-05.
- Prefer adding rights-cleared real scans before commercial ship audit.
- B-S1-06 is unblocked.
