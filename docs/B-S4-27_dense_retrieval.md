# B-S4-27 — Meaning-based retrieval (offline embeddings)

Keyword search finds a section only when the question shares its words. A client asks "can I get protection before the
police arrest me"; the law says "direction for grant of bail to person apprehending arrest". B-S4-27 adds a second,
meaning-based ranking that runs fully offline and is fused with the existing keyword/legal-concept ranking.

## Model

| | |
|---|---|
| Model | `intfloat/multilingual-e5-small` (MIT), pinned commit `614241f622f53c4eeff9890bdc4f31cfecc418b3` |
| Format | ONNX, dynamic int8 quantisation (portable CPU), built by `.github/workflows/models.yml` → release `models-v1` |
| Fidelity | min cosine fp32 vs int8 = **0.9871** on the check set (gate ≥ 0.98) |
| Size | 118 MB model + 17 MB tokenizer; ~30 MB of wheels (onnxruntime, tokenizers, numpy) in the bundled Python |
| Integrity | SHA-256 of both files pinned in `sidecars/docpipe/embed.py`; any other file is refused (`CF_EMBED_MODEL_TAMPERED`) |
| Languages | English and Indian languages (Hindi queries reach English sections) |

## Pack vectors

`eval/tools/embed_pack.py` writes, before signing:

- `metadata/embeddings.e5s.i8` — int8 unit vectors × 127, 384 bytes per passage, rows in `legal_chunks.json` order;
- `metadata/embeddings.json` — model id, dims, rows, SHA-256 of the ordered chunk-id list.

Both are `DERIVED_ARTEFACT` files covered by `content_sha256` and the pack signature. The installer accepts exactly this
one binary path (`metadata/embeddings.e5s.i8`, raw numbers, never parsed as code); any other binary is still rejected.
At load, vectors whose model, row count or chunk order do not match are ignored, never misattributed.

Workflow: `sc-packs.yml` / `central-acts.yml` build → `embed-packs.yml` adds vectors → owner signs the new manifests
locally (`sign_pack_manifests.py`) → `central-acts.yml stage=publish-signed` inserts the verified signatures.

## Ranking (src-tauri/src/retrieval)

1. The existing ranking runs unchanged (BM25 + legal-concept vector + metadata + authority).
2. If installed packs carry vectors and the local model answers, the user's own question (not the stop-word-stripped
   terms) is embedded and the top 100 passages by similarity (> 0) form the meaning ranking.
3. Reciprocal-rank fusion: score = 1/(61 + keyword rank) + 1/(61 + meaning rank). Only passages with real keyword
   evidence hold a keyword rank (the authority boost alone does not). Passages found only by meaning are added with the
   reason "meaning match (offline multilingual embedding)". Mode is reported as `HYBRID_DENSE`.
4. A question with no keyword terms (e.g. Hindi, which the keyword index does not read) is answered by meaning alone.

Fallback: no vectors, no model, or a helper failure → ranking is exactly the previous one; the helper is retried after
five minutes. The query helper (`embed.py`, line JSON-RPC) stays running between searches (no window, 60 s first-answer
timeout for model load, 15 s afterwards); queries are cached. Unit tests never start it (`cfg!(test)`).

Memory: one contiguous int8 block per pack plus a 64-bit key per passage (~400 bytes per passage; ~40 MB per 100k
passages). Grounding is unchanged: every hit is still a verbatim pack passage subject to G2 and citation verification.

## Evidence

| Gate | What it proves |
|---|---|
| `retrieval::tests::b_s4_27_dense_fusion_adds_meaning_matches_and_is_off_without_vectors` | fusion adds meaning-only passages, keeps keyword hits, wrong-size vectors fall back to the unchanged keyword result, Hindi-style empty-term queries answered by meaning |
| `packs::tests::b_s4_27_pack_vectors_pass_install_checks_and_register` | a pack with vectors passes the install checks; vectors register; reordered chunk lists are ignored; any other binary is rejected |
| `eval/checks/dense_retrieval_b_s4_27.py` (`dense-eval.yml`) | golden **G-DENSE** (42 plain-language/Hindi questions → criminal-code sections, pinned) over the official criminal-codes pack: KEYWORD vs MEANING vs FUSED; hard gate FUSED ≥ KEYWORD on Recall@10 and MRR@10, int8 vectors faithful |
| `smoke_installed_layout.ps1` | the installed copy loads the model with the bundled Python (`embed.status`) |

AC-S4-27.2 (absolute FUSED Recall@10 ≥ 0.80) is **PROPOSED** pending owner approval of retrieval thresholds (Q-S4-01).
