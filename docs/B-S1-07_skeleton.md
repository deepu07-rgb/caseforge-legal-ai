# B-S1-07 — Skeleton extraction → case_graph

**Status:** DONE (2026-08-06)

## Frozen contracts

| Item | Path |
|------|------|
| Case graph schema v1.0.0 | `contracts/case_graph.schema.json` |
| Q-CG-01/02/03 | `contracts/QUESTIONS.md` + worksheet |
| Extractor id | `caseforge.skeleton.deterministic.v1` |

## Runtime

| Layer | Location |
|-------|----------|
| Python | `sidecars/docpipe/skeleton.py` + `docpipe.extract_skeleton` |
| Rust | `src-tauri/src/docpipe/skeleton.rs` |
| Vault | migration `case_006_case_graph.sql` |
| Host | `docpipe_skeleton_start`, `docpipe_case_graph_get` |
| UI | Case file → **Skeleton** tab |

## AC-6.2

- Scorer: `eval/checks/ac_6_2_chronology_recall.py`
- GOLDEN pack: `golden/G-CASE-IN/fixtures/synth_chrono_40` (product-owner synthetic 40 pages, 20 known dates)
- Result: **100% recall** on that pack

```powershell
npm run test:skeleton
npm run test:ac62
python eval/run_eval.py --ac AC-6.2
```

## Invariants

- Every **EXTRACTED** item has `provenance.source` + `confidence`
- Completeness score is deterministic prep metric — **not** win prediction
- Stage 2 must consume only `case_graph.json` (no raw PDF parse)

## Next

B-S1-08 — side-by-side verification review UI (click fact → source span).
