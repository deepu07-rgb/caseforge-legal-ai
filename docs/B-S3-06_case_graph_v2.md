# B-S3-06 — Case graph v2

## Outcome

CaseForge now writes `case_graph` schema `2.0.0`. The authoritative representation is a typed
`nodes` and `edges` graph. The existing `parties`, `chronology`, `statutes`, `evidence`,
`procedural_posture`, `prayer_relief`, `contradictions`, `gaps`, and `counsel_facts` fields remain
as read projections so Digest, Guided Interview, Argument Integrity, Argument Builder, exports,
and Mock Court retain compatibility while their existing graph-only boundary stays intact.

## Frozen implementation semantics

- Node types separately represent parties, claims/grounds, defences, counterclaims, issues,
  sub-issues, relief, fact states, evidence/exhibits/documents, witnesses/statements, chronology,
  procedure/applications/hearings/orders, law, arguments, and interpretations.
- Relationship fields are typed. Fact/document, fact/evidence, issue/fact, issue/law, and
  argument/support are not overloaded into free text.
- Every node and edge contains one or more source pointers, calibrated item confidence, origin,
  pipeline version, UTC timestamp, counsel-confirmation state, and correction lineage.
- The pipeline records separate extractor, exact-alias resolver, and relationship-builder IDs.
  Items with `origin=INFERRED` are hypotheses and never extracted document facts.
- Numeric dates where both day/month readings are possible keep both candidates and remain
  `ambiguous`. Relative dates remain `unresolved`. The UI shows this state and does not silently
  sort it as an exact date.
- Contradiction candidates require opposing semantic polarity and cite both source sides.
  Merely sharing a date can never create a contradiction.
- Missing concept families create explicit open gaps, including missing support links. Empty
  matters therefore report incompleteness instead of a falsely complete graph.

## Migration

Migration 013 adds graph contract and migration-strategy metadata. Existing v1 JSON is retained
without lossy rewriting and remains readable by legacy consumers. A v1 matter must be re-extracted
to gain v2 nodes/edges; new extraction writes v2 plus v1 projections. This behavior is explicit as
`v1_retained_reextract_for_v2`, avoiding fabricated provenance on historical v1 items.

## Verification

`golden/G-CASE-GRAPH` contains controlled synthetic India and United States matters. The Rust
golden test exercises the production extractor, checks every expected concept count, validates
ambiguous-date and contradiction behavior, and audits node/edge provenance. The evaluator writes
`eval/reports/b_s3_06_case_graph.json`.

These controlled fixtures are not representative legal-document evidence. Formal AC-S3-6.3 is
blocked because owner-approved per-concept precision/recall thresholds and representative held-out
India/US graph goldens do not exist. The implementation does not invent those thresholds.

## Size impact

Graph v2 adds source code, JSON contracts, two tiny text fixtures, and structured rows only. It adds
no model, OCR runtime, embedding index, or network dependency.
