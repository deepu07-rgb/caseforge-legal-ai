# B-S1-13 — FAISS retrieval + citation GREEN/AMBER/RED

**Status:** DONE (2026-08-07)

## Goal

Retrieval-first pipeline over installed packs + optional `case_graph`; every authority labeled **GREEN / AMBER / RED**. Planted fakes always RED (AC-4.3). Top-8 hit rate ≥ 85% on India + US GOLDEN sets (AC-7.2).

## Pipeline

```text
DECOMPOSE → RETRIEVE → GENERATE (constrained) → VERIFY → SCORE → RENDER
```

| Stage | Behaviour |
|-------|-----------|
| DECOMPOSE | Tokenize query; extract citation-like candidates / PLANTED_FAKE |
| RETRIEVE | Lexical TF-IDF top-k over pack + case_graph chunks |
| GENERATE | Authorities **only** from retrieved material (+ forced verify list) |
| VERIFY/SCORE | Match against retrieve set → GREEN/AMBER/RED |
| RENDER | Citation records for UI / downstream |

## Index

| Id | Notes |
|----|--------|
| `caseforge.retrieve.lexical.v1` | Stage-1 offline lexical (no network). **FAISS embedding backend is a drop-in** when §21 model bake-off freezes. |

Hard rules:

- Cite outside RETRIEVE set → **RED** (Guardrail G2) — no bypass.
- `PLANTED_FAKE` → always **RED**, `exclude_from_downstream` unless `user_confirmed`.

## Schema

- Frozen: `contracts/citation_record.schema.json` v1.0.0  
- Worksheet: `docs/worksheets/Q-CIT-01_citation_record.md` (Q-CIT-01 DECIDED)

## Host IPC

- `retrieval_policy`
- `retrieval_query` — full pipeline (optional `extra_pack_dirs`, `force_authorities`, `case_id`)
- `retrieval_verify` — single authority against hit set

## UI

**Library** → Knowledge packs + **Authority retrieval** panel (`RetrievalView`).

## GOLDEN (product-owner synthetic)

Pack `S-SYNTH-GRETRIEVAL-v1` (not law):

- `golden/G-RETRIEVAL-IN/fixtures/synth_hitrate_20`
- `golden/G-RETRIEVAL-US/fixtures/synth_hitrate_20`

Regenerate:

```powershell
python eval/tools/close_b_s1_13_retrieval_golden.py
```

## Eval

```powershell
npm run test:retrieval
python eval/run_eval.py --ac AC-4.3
python eval/run_eval.py --ac AC-7.2
```

## Out of scope

- Mock Court adversary  
- Stage 2 integrity scoring  
- Full embedding model selection (tracked under §21 #1)
