# Layer 2 — Open QUESTIONS (B-S0-02)

**Rule:** These items are **blocked**. Do not invent answers in code. Product owner / Layer 2 author must resolve before the dependent batch freezes fields.

Each block follows the batch-plan QUESTION template.

---

## Q-CG-01 — Case Graph property inventory — FROZEN (2026-08-06)

```text
RESOLVED (B-S1-07)
- schema: contracts/case_graph.schema.json schema_version=2.0.0 (B-S3-06 normalized extension; v1 read projections retained)
- authority: CaseForge Product Owner — worksheet DECIDED
- worksheet: docs/worksheets/Q-CG-01_case_graph_inventory.md
```

---

## Q-CG-02 — Source pointer shape — FROZEN (2026-08-06)

```text
RESOLVED (B-S1-07)
- SourcePointer: doc_id, page_index, kind ∈ {pdf_span, image_bbox, page_only, counsel}
- pdf_span uses char_start/char_end; image_bbox uses bbox [x,y,w,h] 0–1
- defined in case_graph.schema.json $defs.SourcePointer
```

---

## Q-CG-03 — Confidence type and range — FROZEN (2026-08-06)

```text
RESOLVED (B-S1-07)
- scale: unit float [0,1] for graph items (same as words / Q-CONST-01)
- missing EXTRACTED confidence: reject / force review (do not emit without confidence)
- thresholds for highlight: confidence_thresholds.item_* in constants.json
```

---

## Q-AN-01 — Argument node property inventory — FROZEN (2026-08-08)

```text
RESOLVED (B-S2-01)
- schema: contracts/argument_node.schema.json schema_version=1.0.0
- authority: CaseForge Product Owner — worksheet DECIDED
- worksheet: docs/worksheets/Q-AN-01_argument_node.md
- scorer_id: caseforge.integrity.v1
- formula: VI = 100 − min(Fact, Evidence, Law, Exposure)
- missing_link_threshold: 60
- AC-8.1 planted gap top-2; AC-8.2 ≤5s; AC-8.3 no LLM in scoring
```

---

## Q-CIT-01 — Citation record property inventory — FROZEN (2026-08-07)

```text
RESOLVED (B-S1-13)
- schema: contracts/citation_record.schema.json schema_version=1.0.0
- authority: CaseForge Product Owner — worksheet DECIDED
- worksheet: docs/worksheets/Q-CIT-01_citation_record.md
- labels: GREEN | AMBER | RED; PLANTED_FAKE always RED (AC-4.3)
- G2: cite outside RETRIEVE set → RED
```

---

## Q-ST-01 — Session turn property inventory — FROZEN (2026-08-08)

```text
RESOLVED (B-S2-02)
- schema: contracts/session_turn.schema.json schema_version=1.0.0
- authority: CaseForge Product Owner — worksheet DECIDED
- worksheet: docs/worksheets/Q-ST-01_session_turn.md
- pipeline_id: caseforge.mock_court.v1
- five formats sequential: Bail_Interim_Motion → Final_Arguments → Witness_Cross_Examination → Appellate → Motion_Generic
- sources: case_graph + retrieval only (no raw_document)
- AC-3.3 turn_max_s=15
```

---

## Q-LX-01 — Lexicon entry property inventory

```text
RESOLVED (B-S1-16, 2026-08-08)
- batch: B-S1-16
- decision: Freeze lexicon_entry.schema.json v1.0.0 from PRD §9 known inventory
- fields: entry_id, entry_kind (stt_correction|tts_pronunciation_override), language=English,
  surface_form, corrected_form, pronunciation, pattern_type, source, active, scope=global, timestamps
- export: .cflex.json envelope (schema_version, product, kind=lexicon_export, language, entries[])
- artefact: contracts/lexicon_entry.schema.json status FROZEN
```

---

## Q-EX-01 — Experience record property inventory

```text
RESOLVED (B-S1-17, 2026-08-08)
- batch: B-S1-17
- decision: Freeze experience_record.schema.json v1.0.0
- fields: record_id, record_category, stage, summary, tags, relevance_key, payload,
  fleet_status (default local_only), case_id, jurisdiction, source, timestamps
- top_k: 5 (Stage-1 injection into retrieval corpus chunks)
- relevance_key: lowercase unique tokens from summary + tags + category
- fleet: enabled_default=false; network_upload_default=false; explicit review required (AC-11.2)
- stage1_writable: correction, lexicon_entry, draft_edit, rated_exchange
- stage2_only: limb_outcome, bench_setting
- artefact: contracts/experience_record.schema.json + constants.experience_store FROZEN
```

---

## Q-LIC-01 — Licence payload property inventory

```text
RESOLVED (B-S1-18, 2026-08-08)
- batch: B-S1-18
- decision: Freeze licence_payload.schema.json v1.0.0
- fields: licence_id, product, tier, issued_at, expires_at, fingerprint_components (4),
  features, signer_key_id, signature_alg=Ed25519, signature_hex
- fingerprint components: hostname, username, machine_guid, disk_root (SHA-256 hex); 3-of-4 match
- state machine: UNLICENSED | ACTIVE | EXPIRED (EXPIRED = read-only vault; never ransom data)
- trust: contracts/licence_trust.json
- artefact: licence_payload.schema.json + constants.licence_and_privacy FROZEN
```

---

## Q-IPC-01 — IPC method catalogue — PARTIAL (B-S1-05)

```text
PARTIAL
- batch: B-S0-02 / B-S1-05
- resolved_for: docpipe triage methods only
- frozen_in: contracts/ipc/docpipe.schema.json
- methods: docpipe.ping, docpipe.version, docpipe.triage_pages
- still_missing: infer-llm methods; infer-speech methods; docpipe recognition / batch methods
- blocked_artefact: contracts/ipc/infer-llm.schema.json; contracts/ipc/infer-speech.schema.json; remaining docpipe methods
- prd_known: Typed JSON-RPC over stdio; three sidecars named; AC-4.1 full contract round-trip suite
HALT (remainder)
```

---

## Q-VISION-01 — Vision / HTR model for docpipe — STAGE-1 FROZEN (2026-08-06)

```text
RESOLVED (Stage-1 provisional freeze)
- batch: B-S1-05
- frozen_classifier_id: caseforge.triage.heuristic.v1
- freeze_report: eval/bakeoff/reports/Q-VISION-01_freeze.md
- bakeoff: 100% on 500 GOLDEN pages (heuristic only; VLM slots not enabled)
- re-open_when: offline 2–3B VLM installed + rights-cleared image GOLDEN subset
- note: Still marked provisional vs future VLM; production triage uses frozen id above
```

---

## Q-GOLDEN-DOCS — G-DOCS 500-page GOLDEN corpus — STAGE-1 CLOSED (2026-08-06)

```text
RESOLVED (product-owner synthetic pedagogy pack)
- batch: B-S1-05
- golden_pages: 500 (page_0001 … page_0500)
- source: S-SYNTH-GDOCS-500 (synthetic-but-reviewed under product-owner authority)
- adjudicator: CaseForge Product Owner
- audit: golden/G-DOCS/mvp80/PRODUCT_OWNER_LOCK_AUDIT.json
- ac_6_1: PASS 100.0% on 500 pages
- caveat: Prefer supplementing with rights-cleared real scans before commercial ship audit
- unlocks: B-S1-06 may start
```

---

## Q-CONST-01 — Confidence thresholds (numeric) — FROZEN (2026-08-06)

```text
RESOLVED (B-S1-06)
- batch: B-S1-06
- scale: unit_float_0_1 (Option A)
- comparison: strict_below; equality at threshold = OK
- missing_confidence: force_review
- values: contracts/constants.json → confidence_thresholds.values
  word_highlight_below=0.75
  word_force_review_below=0.50
  word_suppress_below=0.25
  page_mean_warn_below=0.70
  item_highlight_below=0.80
  item_block_downstream_below=0.55
  chronology_date_strict_below=0.85
  triage_low_confidence_below=0.75
- authority: CaseForge Product Owner — worksheet proposed values DECIDED for Stage-1 recognition
- worksheet: docs/worksheets/Q-CONST-01_confidence_thresholds.md
```

---

## Q-DB-01 — Argon2id parameters — RESOLVED (B-S1-02)

```text
RESOLVED
- batch: B-S1-02
- decision: OWASP 2023 interactive (Recommended)
- frozen_in: contracts/constants.json → crypto.argon2id
- values: memory_kib=65536, iterations=3, parallelism=4, salt_len_bytes=16, key_len_bytes=32
- params_version: 1
- note: Not present in PRD Layer 1; owner-selected freeze for SQLCipher key derivation
```

---

## Q-PKG-01 — `.akcase` format

```text
QUESTION
- batch: B-S0-02
- missing: .akcase package format for AC-16.3 round-trip
- blocked_artefact: (no contract file yet)
- prd_known: AC-16.3 requires .akcase round-trip
- options_considered: none
- need_from_owner: Binary/archive layout and schema of enclosed payloads
HALT
```

---

## Q-PKG-02 — `.akpkg` format

```text
QUESTION
- batch: B-S0-02
- missing: Official pack .akpkg layout (signing, versioning, isolation rules)
- blocked_artefact: (no contract file yet); blocks B-S1-12
- prd_known: Ed25519-signed .akpkg; India/US packs; headnotes stripped; versioning and isolation "in /contracts"
- options_considered: none
- need_from_owner: Pack format contract + isolation rules document
HALT
```

---

## Q-DOC-01 — Document metadata contract path — RESOLVED (B-S1-04)

```text
RESOLVED
- batch: B-S1-04
- decision: Freeze contracts/document_intake.schema.json from PRD §5 known fields
- fields: doc_id, original_filename, type_tag, language_tag, source_format (pdf|docx|jpg|png),
  byte_size, page_count (null placeholder), per_page_quality_grade (null placeholder), ingested_at_unix_ms
- note: Phone Capture still out of scope until B-S1-11; AC-5.1 ML auto-type deferred to classifier batch
```

---

## Q-ENV-01 — Windows MSVC linker for Tauri build (B-S1-01)

```text
QUESTION / ENV BLOCKER
- batch: B-S1-01
- missing: MSVC linker link.exe (Visual Studio 2017+ or Build Tools with C++ workload)
- blocked_artefact: npm run tauri:dev / cargo build of src-tauri
- observed: rustc/cargo install OK; `cargo check` fails: linker `link.exe` not found
- options_considered: install Microsoft.VisualStudio.2022.BuildTools with VCTools workload (user approval required — not auto-installed)
- need_from_owner: Install C++ Build Tools, then re-run `npm run tauri:dev` to complete live ping/heavy-job verification
HALT (for live Tauri launch only — source shell + Vite build + structural AC checks complete)
```

---

## Q-EVAL-01 — Partial AC procedure detail (AC-10/11/14)

```text
PARTIAL RESOLVED (B-S2-03 for AC-10 only)
- AC-10.1 & AC-10.2 procedures frozen in docs/worksheets/Q-AB-01_adaptive_bench.md
- contract: contracts/adaptive_bench.json
- still open: AC-11.x already handled in B-S1-17; AC-14.x in B-S1-19
- remaining Q-EVAL-01: none for AC-10 after B-S2-03
```

---

## Frozen without QUESTION (for implementer clarity)

These are **not** open invent-or-not items; they are fixed by the PRD:

| Item | Frozen value / rule |
|------|---------------------|
| Citation labels | `GREEN` \| `AMBER` \| `RED` |
| Argument link kinds | `Fact` \| `Evidence` \| `Law` \| `Exposure` |
| Link score range | 0–100 inclusive |
| Vulnerability Index | `VI = 100 − min(links)` |
| Jurisdiction at case creation | `India` \| `United States` \| `Other` |
| Counsel-sourced fact marker | `SOURCED_BY_COUNSEL` |
| Sidecars | `infer-llm`, `infer-speech`, `docpipe` |
| IPC style | JSON-RPC over stdio |
| Retrieval pipeline stages | DECOMPOSE → RETRIEVE → GENERATE → VERIFY → SCORE → RENDER |
| Stage 2 input | only `case_graph.json` (+ retrieval over packs / Experience Store; no raw doc parse) |
| Provenance minimum (G5) | model hash, prompt hash, retrieval IDs |
| Licence crypto | Ed25519; fingerprint 3-of-4 |
| EXPIRED behaviour | read-only vault; never ransom data |
| Triage classes (§6.1) | typed English / other / mixed / clean handwriting / messy handwriting / non-text |
| Mock Court formats | Bail/Interim-Motion, Final Arguments, Witness Cross-Examination, Appellate, generic Motion |
| Performance numbers (§3) | deferred to **B-S0-03** numeric freeze (values are in PRD; not schema fields) |

---

*End of QUESTIONS for B-S0-02*
