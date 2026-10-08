# B-S4-29 — On-disk legal store

Search used to read every installed pack into memory and tokenize every passage on every question. With the Central
Acts and judgment packs installed that is tens of thousands of passages per question. The store replaces it for the
paths that run on every search: `retrieval::run_pipeline` and the working-set suggestions.

## Design

`retrieval/store.rs` builds one index per installed v2 pack version under `<vault>/indexes/legal/packs/<pack>-<hash>/`:

| File | Content |
|---|---|
| `docs.dat` / `docs.off` | one zlib-compressed JSON record per passage, random access by offset |
| `postings.dat` / `terms.dat` | BM25 postings (delta + varint) and the term dictionary |
| `meta.dat` | per passage: five filter codes (jurisdiction, authority type, trust, temporal, superseded) + keyword length |
| `ids.txt` | chunk ids in pack order (also the row order of the pack's meaning vectors) |
| `index.json` | format, pack id, signed `content_sha256`, SHA-256 of every file except `docs.dat` |

Two-stage retrieval: stage 1 takes about 300 BM25 candidates over the whole store (corpus-wide statistics, filters
applied before ranking, known-invalid material excluded) plus the 100 nearest by meaning; stage 2 loads only those
passages and runs the existing re-ranker and fusion (`retrieve_core`). Passages in older (v1) packs, case-graph and
Experience Store material stay in memory (`build_small_corpus`) and join stage 2.

Citation checks (`verify_and_score`) receive the candidate passages plus the passages most likely to contain each
authority string the user typed or forced (`must_contain`), instead of the whole corpus.

## Integrity

The index is derived data: built only from a pack that passed signature, hash and lineage verification
(`load_legal_chunks`), named by the pack's signed content hash, rebuilt when the pack changes or any checked file
differs, and every passage read back is verified against its own SHA-256 before use. Older versions of an index are
removed. Index files in the user's vault are as trustworthy as the vault itself (a local attacker with vault write
access could alter filter codes); passage text cannot be altered undetected.

## Measured (GitHub Actions Linux runner, `store-bench.yml`, real signed packs installed through the real installer)

| | Central Acts only (51 packs, 34,217 passages) | Everything (55 packs, 71,714 passages) | Previous path, everything |
|---|---|---|---|
| Install all packs | 9.5 s | 16.2 s | – |
| First use (build indexes), once per pack version | 6.8 s | 19.6 s | – |
| Open on later runs | 0.09 s | 0.13 s | 6.7 s (load all passages) |
| One question | median 87 ms, max 113 ms | median 70 ms, max 93 ms | 12.9 s |
| Process memory (RSS, whole benchmark) | 101 MB | 165 MB (peak 201 MB, includes installing) | 427 MB (peak 441 MB) |

"Everything" = 51 Central Acts packs + 3 landmark judgment parts (600 most-cited) + the 400 newest born-digital
judgments. The previous path grows linearly with installed material (12.9 s per question at this size); the store's
per-question cost grows with the number of candidates, not with the corpus.

Installing the real packs found three defects in the pack pipeline that the Python validators had missed, all fixed:
a headnote scan that rejected a statute proviso ("shall be held:"), duplicate judgments in the corpus, and reporter
headnote sign-offs left in judgment text (see `packs::headnote_scan_pack`, `eval/tools/pack_dedupe.py`,
`eval/tools/build_judgment_packs.py`).

## Not yet covered

`build_corpus` (whole-corpus load) is still used by the citation audit, opposition scan, rule-table validation and the
Code Bridge, which run on occasional actions rather than on every search. They work, but load every installed
passage each time they run (about 7 s and 250 MB at 72k passages). Phase 2 moves them to store lookups.
