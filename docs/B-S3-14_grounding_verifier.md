# B-S3-14 — Proposition ledger and hard grounding verifier

## Outcome

Generated reasoning can no longer become trusted prose merely because the model prints a citation or source ID. Controlled generation must first return structured propositions. The Rust host independently resolves every claimed support ID, verifies exact excerpts and quotations, applies legal-source policy, and only then produces rendered text.

## Proposition classes

- `FACTUAL_BASIS` — requires an exact excerpt from a structured matter source with a document ID.
- `LEGAL_BASIS` — requires an exact excerpt from a permitted working-set authority that is `OFFICIAL_VERIFIED`, `APPLICABLE`, and `CURRENT`, with stable source and chunk IDs.
- `INFERENCE` — remains visibly labelled and must identify earlier non-RED proposition bases.
- `COUNSEL_INPUT` — remains visibly distinct from evidence and requires an exact counsel-input record.
- `UNSUPPORTED_OR_GAP` — renders as an explicit repository gap and never as trusted law or fact.

## Trust boundary and rendering

The model may emit only proposition candidates, support claims, and exact excerpts. Model-supplied `GREEN`, `TRUSTED`, pages, paragraphs, citations, and fabricated IDs are ignored. The host assigns:

- `GREEN / TRUSTED` only after all class-specific checks pass;
- `AMBER / LABELLED` for linked counsel input and explicit inference;
- `RED / REPOSITORY_GAP` when legal authority is absent;
- `RED / BLOCKED` for unsupported facts, broken inference chains, or altered quotations.

Every direct quotation must be an exact stored substring. The verifier returns character start/end offsets into the stored source. Fuzzy similarity is never enough for quotation verification.

## Integrated surfaces

The local reasoning/Argument Builder path now requests `caseforge.proposition-ledger.v1`, supplies structured matter document excerpts and the exact counsel-controlled legal working set, and renders only the host-produced ledger text. A missing or malformed proposition array fails closed. Deterministic Digest and Drafting outputs continue to use their existing case-graph source pointers; any future model-assisted span must use this verifier API.

## Verification

```powershell
npm run test:s3:14
cargo test --manifest-path src-tauri/Cargo.toml proposition::tests
npm run build
npm run test:size
```

Acceptance evidence is written to `eval/reports/b_s3_14_grounding_verifier.json`.
