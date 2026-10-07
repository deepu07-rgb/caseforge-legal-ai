# B-S3-11 — Hybrid retrieval

Status: **IMPLEMENTED for controlled development testing** (2026-08-16).

## Outcome

CaseForge now uses a fully offline hybrid legal-retrieval pipeline:

`query decomposition → deterministic metadata/currentness filters → BM25 + semantic candidates → deterministic reranking → exact source chunks → citation verification`

The semantic engine is `caseforge.legal-concept-hash-384.v1`, a first-party 384-dimensional deterministic legal-concept feature encoder. It has no downloaded weights, no network calls, and adds zero model-weight bytes. This choice keeps the development footprint below the agreed ceiling and avoids an unresolved third-party model licence. It is deliberately narrow; B-S3-13 remains responsible for evaluating supported local generative models.

## What changed

- Proper BM25 length-normalized lexical scores replace the Stage-1 TF-IDF approximation.
- Offline legal-concept vectors cover controlled vocabulary mismatches and character variation.
- Deterministic reranking combines BM25, semantic similarity, exact title/section/citation metadata, and verified-authority status.
- Jurisdiction, authority, trust, temporal, and supersession filters run before scoring.
- Known expired, not-yet-effective, and superseded material is excluded. Unknown or partially superseded material is visibly warned.
- Legal, matter/case-graph, and Experience Store hits carry distinct `corpus_kind` values.
- Every result exposes lexical, semantic, rerank, reason, trust, temporal, and checksum evidence.
- Index metadata is checksum-driven at `<vault>/indexes/legal/index.json`. Writes are flushed to a unique temporary file, preserve a recoverable backup, and commit by rename.
- `CASEFORGE_DISABLE_SEMANTIC=1` exercises the explicit `LEXICAL_ONLY` fallback; the UI warns that semantic coverage is unavailable.

The persisted metadata contract is [retrieval_index.schema.json](../contracts/retrieval_index.schema.json). Runtime code is in [retrieval/mod.rs](../src-tauri/src/retrieval/mod.rs), and explainability is rendered by [RetrievalView.tsx](../src/components/RetrievalView.tsx).

## Controlled benchmark

Corpus: the signed B-S3-10 India High Court partition/civil-appeal pack, 20 exact official page chunks.

Thresholds were frozen at Recall@8 ≥ 0.90 and MRR ≥ 0.70. The reproducible evaluator currently measures:

- Hybrid Recall@8: **1.000**
- Hybrid MRR: **0.781**
- Wrong-jurisdiction results under an India filter: **0**
- Explicitly superseded results under default currentness policy: **0**

The machine-readable evidence is [b_s3_11_hybrid_retrieval.json](../eval/reports/b_s3_11_hybrid_retrieval.json). Run it with:

```powershell
python eval/checks/hybrid_retrieval_b_s3_11.py
cargo test --manifest-path src-tauri/Cargo.toml --lib retrieval::tests::b_s3_11
npm run build
```

## Readiness boundary

The questions and relevance targets are grounded directly in the official pack text, not invented authorities. A named legal editor has not yet signed the relevance judgments. Therefore this is an engineering acceptance pass for controlled development testing, not a claim of nationwide Indian-law coverage or final legal-readiness approval. The B-S3-10 repository gaps remain enforced.

## Acceptance gates

| Gate | Result |
|---|---|
| AC-S3-11.1 Recall@8/MRR thresholds | PASS (engineering); legal-editor sign-off pending |
| AC-S3-11.2 deterministic jurisdiction/trust exclusions | PASS |
| AC-S3-11.3 temporal/supersession policy | PASS |
| AC-S3-11.4 crash-safe checksum index | PASS |
| AC-S3-11.5 offline and size budget | PASS |
| AC-S3-11.6 visible lexical fallback | PASS |

