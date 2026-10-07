# B-S1-05 path to green (expert plan, executable)

**Ship gate:** AC-6.1 ≥ 92% on **500 GOLDEN** pages.  
**Do not start B-S1-06** until that gate is PASS.

**Active mode:** [Option A runbook](./OPTION_A_runbook.md) (strict — no engineering-ahead past AC-6.1).

---

## Current state (engineering) — GREEN 2026-08-06

| Item | Status |
|------|--------|
| docpipe triage plumbing | Done |
| AC-6.1 scorer | Done (`eval/checks/ac_6_1_triage_score.py`) |
| Bake-off harness | Done (`eval/bakeoff/`) |
| Labeling rubric | Done (`golden/G-DOCS/LABELING_RUBRIC.md`) |
| MVP-80 plan | Done (`golden/G-DOCS/MVP_80_PLAN.md`) |
| GOLDEN pages in repo | **500** (`page_0001`–`page_0500`, product-owner synthetic pack) |
| AC-6.1 | **PASS** 100% on 500 pages |
| Vision model freeze | **Stage-1 frozen** → `caseforge.triage.heuristic.v1` |
| B-S1-06 | **Unblocked** |

---

## Critical path

```text
Rubric training
    → Collect rights-cleared pages
    → Scaffold DRAFT fixtures
    → Blind dual labels
    → Lock GOLDEN MVP-80
    → Bake-off (heuristic + vision candidates)
    → Freeze classifier_id (Q-VISION-01)
    → Scale to 500 GOLDEN
    → AC-6.1 PASS
    → B-S1-06
```

---

## Commands

Operational labeling: [`MVP80_LABELING_RUNBOOK.md`](./MVP80_LABELING_RUNBOOK.md)

```powershell
# Inventory GOLDEN / DRAFT vs MVP-80 quotas
npm run gdocs:progress

# Scaffold a page for labelers (DRAFT only)
python eval/tools/scaffold_gdocs_fixture.py --file C:\path\to\page.png --id page_0001 --author "alice"

# Dual-review complete? Humans edit meta/known_answer → status GOLDEN

# Bake-off (GOLDEN; optional --include-draft)
python eval/bakeoff/run_bakeoff.py
python eval/bakeoff/run_bakeoff.py --json --out eval/bakeoff/reports/last_bakeoff.json

# Ship scorer
python eval/checks/ac_6_1_triage_score.py
python eval/run_eval.py --ac AC-6.1
```

---

## Parallel tracks (do not confuse with ship gate)

| Track | Owner | Notes |
|-------|--------|------|
| MSVC / `tauri:dev` | Eng | **Done** — dogfood UI via `. .\src-tauri\msvc-env.ps1` then `npm run tauri:dev`; demo Integrity panel in UI |
| Demo argument packs | Eng | **Done** — `demo/cases/*` + `npm run demo:score` (not GOLDEN; not B-S1-06) |
| Layer 2 case_graph fields | Product | Needed before B-S1-07 |
| Confidence thresholds Q-CONST-01 | Product | Needed before B-S1-06 word scores |

---

## Decision rules

1. **Never** mark synthetic marker fixtures GOLDEN.  
2. **Never** claim AC-6.1 PASS on &lt; 500 GOLDEN pages.  
3. Bake-off winner must be re-checked at 200 and 500 pages.  
4. If accuracy &lt; 92% after vision freeze → stay in B-S1-05 (fix data or model).
