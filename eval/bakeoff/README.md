# Vision / triage bake-off (PRD §21 #3 · Q-VISION-01)

**Purpose:** Compare triage classifiers on the **same GOLDEN (or DRAFT-labeled) G-DOCS pages** before freezing a production vision model.

**Does not:** Download models, invent GOLDEN labels, or PASS AC-6.1 by itself.

---

## Quick start

```powershell
# 1. Score enabled candidates (default: eval/bakeoff/candidates.json — heuristic only until vision slots enabled)
python eval/bakeoff/run_bakeoff.py

# 2. Shortlist + how to enable vision slots
#    see eval/bakeoff/CANDIDATES.md

# 3. JSON report
python eval/bakeoff/run_bakeoff.py --json --out eval/bakeoff/reports/last_bakeoff.json
```

## Inputs

| Source | How used |
|--------|----------|
| `golden/G-DOCS/fixtures/*/meta.json` with `GOLDEN` | Primary bake-off set |
| Same with `DRAFT` | Optional `--include-draft` for pre-lock experiments |
| `SYNTHETIC_PLACEHOLDER` | **Excluded** by default (markers inflate scores) |

## Outputs

Per candidate:

- page count scored  
- accuracy %  
- confusion matrix (expected → predicted)  
- latency summary if adapter reports it  
- peak RSS note if provided  

## Adding a candidate

1. Implement `eval/bakeoff/adapters/<name>.py` with `classify_pages(pages) -> list[result]`.  
2. Register in `candidates.json`.  
3. See [`protocol.md`](./protocol.md) for RAM / concurrency rules (8 GB choreography).

## Relation to AC-6.1

| Tool | Role |
|------|------|
| `eval/bakeoff/run_bakeoff.py` | Compare models on whatever labeled set you have |
| `eval/checks/ac_6_1_triage_score.py` | **Ship gate** — GOLDEN only, ≥ 500 pages, ≥ 92% |

A bake-off winner that scores 95% on 80 pages still needs full AC-6.1 for B-S1-06.
