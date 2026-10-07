# B-S3-08 — Legal source and chunk metadata

CaseForge `.akpkg` v2 separates authoritative source text from derived artefacts and gives every
retrievable legal chunk stable source/chunk identities, exact UTF-8 text, SHA-256 lineage, legal
taxonomy, temporal applicability, trust, supersession, and publisher provenance.

The controlled taxonomy is recorded in `legal_source.schema.json` and `legal_chunk.schema.json`.
`UNKNOWN` is explicit—not an inference of current, applicable, binding, verified, or controlling
status. A retrieval match may be GREEN only when it is `AUTHORITATIVE_TEXT` with
`OFFICIAL_VERIFIED` trust, `APPLICABLE` temporal status, `CURRENT` supersession, a known authority
type, and verified source/chunk checksums. Custom, derived, legacy, unknown, expired, future, or
superseded material remains visible for counsel but cannot masquerade as controlling authority.

Manifest v2 signs `content_sha256`, covering every listed content and metadata file. Its
`authoritative_content_sha256` separately anchors chunks to raw legal content without a
self-referential metadata hash. Official v2 packs must be controlled-beta eligible and verified;
custom packs are never beta eligible and retain `CUSTOM` trust.

The installer validates exact source containment, stable IDs, source/chunk/pack checksums, trust
consistency, derived-summary separation, and missing lineage before installation. Retrieval consumes
validated chunks directly and exposes jurisdiction, authority type, trust, temporal status, and
supersession as exact filters. Citation records carry the same lineage.

Legacy v1 packs, including repository placeholders, remain usable only as `DEVELOPMENT_ONLY` with
`controlled_beta_eligible=false`; their unknown metadata cannot produce GREEN. Use
`python scripts/legal_pack_tool.py build <pack-dir>` and `validate <pack-dir>` for v2 pack work.

Acceptance evidence is written to `eval/reports/b_s3_08_legal_metadata.json`.

Verification on 2026-08-14: all five B-S3-08 acceptance gates passed, all 53 Rust library tests
passed, the TypeScript/Vite production build passed, and the prior B-S3-07, B-S3-06, and B-S1-13
regression gates remained green. The workspace measured 3.270 GiB; with the shared 4.361 GiB
Ollama store, the development footprint was 7.631 GiB, below the 8-9 GiB ceiling.
