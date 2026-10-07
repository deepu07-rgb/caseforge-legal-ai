# B-S1-05 — docpipe triage only

## Goal

Classify pages into PRD §6.1 classes with per-page confidence; store in case vault; report progress without freezing UI.

## Classes (frozen)

`typed_english` | `other` | `mixed` | `clean_handwriting` | `messy_handwriting` | `non_text`

## Classifier

| Field | Value |
|-------|--------|
| `classifier_id` | `caseforge.triage.heuristic.v1` |
| Status | **Provisional** heuristic |
| Vision model | Not chosen (PRD §21 #3 bake-off open → Q-VISION-01) |

Synthetic fixtures may embed:

```text
[CASEFORGE_TRIAGE_CLASS:typed_english]
```

## Architecture

| Piece | Role |
|-------|------|
| `sidecars/docpipe/` | JSON-RPC stdio process (`docpipe.ping` / `version` / `triage_pages`) |
| `src-tauri/src/docpipe/` | In-process heuristic + async triage job (off UI thread) |
| `page_triage` table | Encrypted case vault labels + confidence |
| Host IPC | `docpipe_triage_start`, `docpipe_triage_status`, `docpipe_page_list`, `docpipe_meta` |
| Progress event | `docpipe_triage_progress` |

## Out of scope

- OCR word boxes / recognition (B-S1-06)
- Skeleton / case_graph
- Concurrent vision + primary LLM
- Overnight batch resume (B-S1-10)

## AC-6.1 honesty

| Requirement | Status |
|-------------|--------|
| ≥ 92% on **500-page GOLDEN** G-DOCS | **Not PASS** until GOLDEN set authored |
| Scaffold / synthetic fixtures | Used for plumbing only; must not silent-PASS AC-6.1 |

## Path to green (AC-6.1)

1. Author ≥ 500 **GOLDEN** pages per [`../golden/G-DOCS/AUTHORING.md`](../golden/G-DOCS/AUTHORING.md).
2. Run `python eval/checks/ac_6_1_triage_score.py` until **PASS**.
3. If accuracy &lt; 92% on real scans, upgrade classifier (vision bake-off / Q-VISION-01) — **stay in this batch**.
4. Only then start **B-S1-06**.

## Verify

```powershell
python eval/checks/docpipe_triage_b_s1_05.py
python eval/checks/ac_6_1_triage_score.py --allow-skip
npm run test:triage
npm run test:ac61
npm run build
python eval/run_eval.py --ac AC-6.1
```
