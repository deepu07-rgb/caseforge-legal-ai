# B-S3-15 — Grounded Mock Court Retrieval

Status: implemented and verified on 2026-08-18.

Mock Court legal turns now use a host-owned retrieval boundary. The renderer submits only the session objective, turn text, selected format/role, and the graph/working-set versions it saw. It cannot submit authority IDs, excerpts, or other free-text “sources.” The Rust host loads the encrypted matter graph and legal working set, filters authorities, records output provenance, and then supplies verified IDs and exact chunks to the deterministic court engine.

## Readiness

A legal session is ready only when the matter has a case graph, counsel has confirmed a non-gap applicable-law regime, and at least one authority is `INCLUDED`, controlling, `OFFICIAL_VERIFIED`, `APPLICABLE`, `CURRENT`, jurisdiction-compatible, and backed by stable source/chunk IDs, checksums, a source reference, and exact text. Missing or rejected inputs produce a visible blocking reason. A v2 graph containing unconfirmed propositions produces a visible counsel-review warning.

This is deliberately fail-closed: a legal challenge is not generated from an empty retrieval set, and the renderer no longer falls back to an ungrounded coach after a grounded host error.

## Retrieval and cache

The host decomposes the court format, role, current submission, graph issues, statutes, relief, gaps, and graph nodes into a bounded query. It ranks only the permitted matter authorities and selects at most eight chunks. Cache identity is SHA-256 over case ID, complete graph digest, working-set version, and decomposed query. A graph, working-set, or query change therefore cannot reuse stale retrieval. The small in-memory cache is bounded to 64 entries and contains no raw matter documents.

## Version invalidation

The readiness response exposes the graph digest and working-set version. Every text turn sends these as expected versions. If either changed, the host returns `CF_MOCK_COURT_SESSION_STALE`; no challenge is generated. The UI refreshes readiness and asks counsel to review the Authorities panel before resuming against the new versions. Voice turns reload and verify current versions and sources atomically on every turn.

## Exact source text

Before a session, the Authorities panel shows every permitted authority with citation, section, stable authority/chunk IDs, official reference, and exact stored chunk text. After a turn it also shows the exact subset used for that turn, including the chunk checksum. Output selection is persisted as `MOCK_COURT` provenance against the exact working-set version before generation.

## Verification

- Rust behavioural tests cover wrong jurisdiction, superseded authority, unconfirmed/empty sets, cache invalidation, and stale-session refusal.
- `python eval/checks/mock_court_retrieval_b_s3_15.py` verifies the production host, IPC, UI, voice path, tests, and documentation.
- `npm run build` verifies the TypeScript/renderer integration.
- `python scripts/check_size_budget.py` remains the development/client footprint gate.
