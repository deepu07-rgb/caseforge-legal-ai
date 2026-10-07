# CaseForge Stage 3 — Pre-Beta Trustworthiness Agentic Playbook

**Document status:** Proposed execution authority for Stage 3  
**Target release:** `v3.0.0-controlled-beta`  
**Purpose:** Close all 22 gaps identified in the pre-beta repository audit and make CaseForge trustworthy enough for controlled English-language litigation testing.  
**Primary success criterion:** Grounded, traceable, useful preparation—not legal-sounding prose.

---

## 1. How to use this playbook

This document is written for autonomous coding agents and their human reviewer.

1. Execute batches in numeric order. The order is a hard dependency order.
2. Work on one batch at a time. Do not implement later batches “for convenience.”
3. Before editing, read this playbook, the files listed under the batch’s **Required reading**, and any applicable `AGENTS.md`.
4. Inspect the current repository before assuming a file or interface still has the shape described here.
5. Preserve unrelated user changes. Never reset or overwrite a dirty worktree.
6. Freeze or version contracts before changing producers and consumers.
7. Add executable tests in the same batch as the behavior they protect.
8. A structural grep test is supporting evidence, not proof that a legal workflow works.
9. `PASS` requires the stated runtime or golden-data evidence. A missing model, missing corpus, or missing reference machine is `BLOCKED` or `NOT TESTED`, never a silent pass.
10. Every batch is cumulative: all earlier Stage 3 gates must remain green.
11. If required legal source material, licensing authority, model rights, or a product decision is missing, stop and emit a `QUESTION` block. Never invent legal content or silently select a legal regime.
12. Do not use confidential client matters as fixtures. Use rights-cleared, public-domain, licensed, or explicitly synthetic material.

### Mandatory batch loop

Every agent must follow this loop:

```text
READ
  → inspect repository + contracts + prior batch reports
FREEZE
  → update/version schemas, constants, errors, and test fixtures first
IMPLEMENT
  → smallest complete vertical slice for the current batch
VERIFY
  → unit + integration + golden + UI/runtime checks as specified
ADVERSARIAL CHECK
  → test failure, malformed input, missing source, and rerun paths
DOCUMENT
  → batch note, commands, measurements, limitations
HAND OFF
  → evidence-backed status; no unsupported “done” claims
```

### Required agent handoff format

```text
Batch:
Status: PASS | FAIL | BLOCKED | PARTIAL

Implemented:
- ...

Contracts/migrations changed:
- ...

Tests run:
- command → result

Golden/runtime measurements:
- metric → result → threshold

Security and provenance checks:
- ...

Known limitations:
- ...

Files changed:
- ...

Next batch unlocked: Yes/No
```

### Mandatory QUESTION format

```text
QUESTION
- batch: B-S3-xx
- decision/source missing:
- blocked behavior or artefact:
- repository evidence inspected:
- safe options considered:
- exact decision/input needed from owner:
- work that can continue safely, if any:
HALT
```

---

## 2. Global Stage 3 rules

### Product truth

CaseForge is an offline case-preparation system. It is not a generic legal chatbot, outcome predictor, judge profiler, or substitute for counsel.

The trusted reasoning boundary is:

```text
Matter documents
  + counsel-confirmed case memory
  + retrieved installed legal authority
  + local model reasoning
  = preparation output with inspectable provenance
```

### Authority hierarchy

All Stage 3 implementations must enforce:

```text
Counsel-confirmed
    >
Document-extracted
    >
Model-inferred
```

The system may ask counsel to revisit a correction, but it must never silently replace one.

### Legal grounding rules

- Default mode for controlled beta: **Use only installed legal sources**.
- No legal proposition without a retrieved, permitted authority chunk.
- No quotation unless it exactly maps to stored source text.
- Missing authority produces a repository-gap warning.
- Matter evidence and legal authority must never be presented as the same source type.
- Custom/user material must never inherit “official” trust merely because it was indexed.
- Memorized model knowledge must not become product-path authority.

### Definition of a valid test

A valid test must identify:

- input provenance;
- expected result;
- metric and threshold;
- test environment;
- whether data is synthetic, rights-cleared real-world, or authoritative law;
- failure behavior;
- reproducible command or manual protocol.

### Stage 3 severity rule

- **BLOCKER:** failure can materially misstate a document, counsel correction, authority, or source.
- **HIGH:** failure can mislead or prevent a lawyer from completing the intended workflow.
- **MEDIUM:** usable in controlled testing with a clear warning or workaround.
- **POLISH:** does not affect trustworthiness or completion of the controlled workflow.

---

## 3. Batch map to the 22 audit findings

| Stage 3 batch | Audit finding closed |
|---|---|
| B-S3-01 | #22 Encoding corruption |
| B-S3-02 | #13 Real evaluation evidence and deferred measurements |
| B-S3-03 | #2 Production PDF extraction |
| B-S3-04 | #1 Real offline OCR |
| B-S3-05 | #3 Genuine confidence and critical-field validation |
| B-S3-06 | #4 Litigation-grade case graph |
| B-S3-07 | #5 Counsel correction authority and rerun preservation |
| B-S3-08 | #11 Legal source/chunk metadata |
| B-S3-09 | #19 Knowledge-pack import hardening |
| B-S3-10 | #6 Usable authoritative legal repository |
| B-S3-11 | #10 Hybrid legal retrieval |
| B-S3-12 | #18 Case-specific legal working library |
| B-S3-13 | #12 Local model fitness and repeatable evaluation |
| B-S3-14 | #9 Proposition-level grounding and citation verification |
| B-S3-15 | #7 Mock Court retrieval integration |
| B-S3-16 | #8 Local-LLM-powered grounded Mock Court |
| B-S3-17 | #17 Visible factual/legal/inference/counsel provenance |
| B-S3-18 | #14 Source-specific Mock Court debrief |
| B-S3-19 | #16 Lawyer-oriented workflow and navigation |
| B-S3-20 | #15 Production offline voice path |
| B-S3-21 | #20 Desktop CSP and renderer hardening |
| B-S3-22 | #21 Local-backend boundary plus cumulative Stage 3 ship gate |

---

# TRACK S3 — Controlled-Beta Trustworthiness

## B-S3-01 — UTF-8 and content-integrity baseline

| Field | Requirement |
|---|---|
| **Goal** | Remove visible mojibake and prevent corrupted text from entering source, exports, prompts, legal packs, or generated reports. |
| **Closes** | Audit finding #22 |
| **Depends on** | Stage 2 repository |
| **Required reading** | `README.md`, `src/App.tsx`, `src/styles.css`, `contracts/**`, export code, current Markdown docs |

### Agent mission

1. Inventory likely mojibake sequences such as `â€”`, `â†’`, `Â§`, `â€œ`, `â€`, `â‰¤`, and replacement characters.
2. Distinguish real corruption from deliberately malformed adversarial fixtures.
3. Normalize production source, contracts, migrations, UI copy, and active documentation to UTF-8.
4. Do not rewrite or “correct” golden inputs whose purpose is encoding-error detection; mark them explicitly.
5. Add an automated scan that fails on disallowed mojibake in production paths.
6. Add round-trip tests for Unicode punctuation in DOCX/PDF exports, prompts, vault strings, and pack source text.

### Deliverables

- `scripts/check_text_encoding.py`
- `eval/checks/encoding_b_s3_01.py`
- `docs/B-S3-01_encoding.md`
- `package.json` command: `test:s3-encoding`
- Clean UTF-8 production files

### Acceptance gates

- **AC-S3-1.1:** Zero unallowlisted mojibake matches in `src`, `src-tauri/src`, `sidecars`, `contracts`, active docs, and pack manifests.
- **AC-S3-1.2:** En dash, em dash, section sign, smart quotes, rupee/dollar values, and arrows survive vault → UI → export round trips.
- **AC-S3-1.3:** The scanner supports an explicit fixture allowlist with reasons; no blanket directory exclusions.
- **AC-S3-1.4:** `npm run build` and existing Stage 1/2 ship checks remain green.

### Halt if

- A suspicious sequence occurs in a golden fixture and its intended truth cannot be determined.
- Normalization would change hashes or signatures of installed/fixture packs without a corresponding intentional pack rebuild.

### Done when

Encoding checks run in the cumulative ship gate and visible production copy has no known corruption.

---

## B-S3-02 — Real-world golden sets and honest readiness metrics

| Field | Requirement |
|---|---|
| **Goal** | Replace structural/synthetic-only readiness claims with reproducible real-world, adversarial, and reference-hardware evidence. |
| **Closes** | Audit finding #13 |
| **Depends on** | B-S3-01 |
| **Required reading** | `golden/**/README.md`, `eval/**`, Stage 1/2 ship reports and waivers, `docs/RELEASE_CHECKLIST_v2.0.md` |

### Agent mission

Freeze the Stage 3 evaluation protocol before changing OCR, extraction, retrieval, or Mock Court behavior.

Create manifests for:

- rights-cleared native PDFs;
- scanned PDFs;
- mixed native/scanned PDFs;
- DOCX, TXT, JPG, and PNG;
- low-quality, rotated, stamped, signed, table-heavy, footnote-heavy pages;
- India and US controlled case packets;
- authoritative-repository retrieval questions;
- prompt-injection documents;
- contradiction and unknown-law cases;
- 40-, 100-, and 300-page performance profiles;
- English legal speech clips when rights-cleared audio is available.

Each fixture must declare `source_class`, rights status, checksum, expected answer author, adjudication status, and whether it may be distributed.

### Deliverables

- `contracts/eval_fixture_manifest.schema.json`
- `golden/STAGE3_MANIFEST.json`
- `golden/G-DOCS-REAL/**`
- `golden/G-CASE-CONTROLLED/**`
- `golden/G-LEGAL-GROUNDING/**`
- `golden/G-INJECTION/**`
- `eval/stage3_metrics.json`
- `eval/run_stage3.py`
- `docs/STAGE3_EVALUATION_PROTOCOL.md`

If rights-cleared files are unavailable, create their manifests and mark the affected gates `BLOCKED`; do not fill them with synthetic pages and call them real.

### Minimum frozen metrics

- Page count/order accuracy
- Character/word error rate by document class
- Critical-field accuracy: party names, dates, money, provisions, citations, exhibit labels
- Dated-event recall and precision
- Fact/issue/evidence/law graph precision and recall
- Correction survival across reruns
- Retrieval Recall@8, MRR, jurisdiction leakage rate, superseded-authority leakage rate
- Unsupported proposition rate
- Quote fidelity rate
- Unknown-law refusal rate
- Prompt-injection success rate (must be zero)
- Mock Court novel-useful-weakness rate under human review
- 40/100/300-page latency, peak RSS, cold start, first/repeated retrieval, and turn latency
- Voice WER and critical-term error rate

### Acceptance gates

- **AC-S3-2.1:** Every Stage 3 metric has a formula, threshold, fixture source class, and test command.
- **AC-S3-2.2:** Synthetic and real-world results are reported separately.
- **AC-S3-2.3:** Missing evidence renders `BLOCKED/NOT TESTED`, never `PASS`.
- **AC-S3-2.4:** Reports include OS, CPU, RAM, model/backend, quantization/model digest, pack versions, and commit/source fingerprint.
- **AC-S3-2.5:** Human-scored tests require two reviewers or documented adjudication for disagreements.

### Halt if

- Fixture licensing or confidentiality is unclear.
- A metric threshold requires product-owner judgment and is not frozen.

### Done when

All later batches have trustworthy, discoverable fixtures and cannot pass using source-code string checks alone.

---

## B-S3-03 — Production PDF parsing and page model

| Field | Requirement |
|---|---|
| **Goal** | Replace raw-byte PDF heuristics with a production-quality offline parser preserving actual page boundaries, text, layout, and failure status. |
| **Closes** | Audit finding #2 |
| **Depends on** | B-S3-02 |
| **Required reading** | `sidecars/docpipe/triage.py`, Rust docpipe modules, document/page schemas, G-DOCS-REAL protocol |

### Agent mission

1. Select and document an offline PDF parsing/rendering dependency with compatible licensing.
2. Extract native text per real page—not duplicated strings.
3. Render pages for OCR when no trustworthy native layer exists.
4. Classify each page as native, scanned, mixed, empty, malformed, encrypted, or unsupported.
5. Preserve page dimensions, rotation, text spans, and stable source coordinates.
6. Detect password-protected PDFs and provide a safe user flow; never log passwords.
7. Place parser work off the UI thread with cancellation and resumable checkpoints.
8. Keep originals encrypted and accessible for source inspection.

### Deliverables

- Versioned `page_source`/layout contract and migration
- Replacement PDF adapter under `sidecars/docpipe` and/or Rust host
- Parser dependency/licence decision record
- Golden parser tests
- Updated document-quality UI status
- `docs/B-S3-03_pdf_pipeline.md`

### Acceptance gates

- **AC-S3-3.1:** 100% page count and order on the frozen native/mixed PDF set.
- **AC-S3-3.2:** No cross-page text duplication unless the source genuinely repeats it.
- **AC-S3-3.3:** Native text spans return to the correct page and visible region.
- **AC-S3-3.4:** Scanned/mixed pages route to OCR; native pages avoid unnecessary OCR unless configured for verification.
- **AC-S3-3.5:** Password-protected, malformed, oversized, and unsupported files yield explicit non-destructive states.
- **AC-S3-3.6:** Tables, footnotes, headings, signatures, stamps, and page numbers are retained as layout blocks or explicitly marked unsupported.

### Halt if

- Parser licensing is incompatible with intended commercial distribution.
- Page coordinate conventions are not frozen before storage/UI implementation.

### Done when

The provisional raw-byte PDF extraction path is removed from production and retained only, if needed, as an explicitly named test fixture adapter.

---

## B-S3-04 — Offline English OCR and document-image recognition

| Field | Requirement |
|---|---|
| **Goal** | Produce reliable, offline English text and word geometry for scans, images, and mixed PDFs. |
| **Closes** | Audit finding #1 |
| **Depends on** | B-S3-03 |
| **Required reading** | Current recognition modules, model choreography, G-DOCS-REAL, hardware tiers |

### Agent mission

1. Run a documented bake-off of viable offline OCR engines/models on the frozen real-scan subset.
2. Freeze engine/model ID, version, model checksum, licence, language, preprocessing, and hardware tier.
3. Implement orientation correction, deskew, denoise/contrast policy, and page segmentation where supported.
4. Return word text, engine confidence, bounding boxes, reading order, and page-level quality diagnostics.
5. Preserve original and processed-image provenance.
6. Refuse downstream auto-extraction for pages below the frozen critical quality threshold until reviewed.
7. Support cancellation, progress, batch resume, and model choreography.

### Deliverables

- OCR bake-off protocol and result
- Frozen OCR model configuration
- Recognition adapter and host integration
- Word-box/page-quality migrations if required
- UI overlay for word-to-image inspection
- `eval/checks/ocr_b_s3_04.py`
- `docs/B-S3-04_ocr.md`

### Acceptance gates

- **AC-S3-4.1:** OCR meets frozen overall WER/CER thresholds on rights-cleared English legal scans.
- **AC-S3-4.2:** Critical-field accuracy separately meets the stricter thresholds for dates, amounts, citations, provisions, names, and exhibit labels.
- **AC-S3-4.3:** Word bounding boxes align with visible source regions under rotation and scaling.
- **AC-S3-4.4:** Empty/failed OCR can never be presented as successfully understood.
- **AC-S3-4.5:** No network access occurs during OCR.
- **AC-S3-4.6:** Peak RSS and per-page timing are recorded for Lite and Full reference tiers.

### Halt if

- No candidate meets the critical-field threshold. Remain in the bake-off; do not lower thresholds silently.
- Model redistribution rights are unknown.

### Done when

Scanned PDFs/JPG/PNG no longer produce empty production text layers and real-scan gates pass.

---

## B-S3-05 — Calibrated confidence and legal critical-field validation

| Field | Requirement |
|---|---|
| **Goal** | Replace token-shape confidence with calibrated engine confidence plus deterministic validation of legally sensitive fields. |
| **Closes** | Audit finding #3 |
| **Depends on** | B-S3-04 |
| **Required reading** | Confidence constants, page text schema, review UI, recognition code |

### Agent mission

- Store raw engine confidence separately from calibrated confidence.
- Calibrate by source/engine class using held-out data; do not calibrate on the evaluation set.
- Detect and validate names, dates, monetary amounts, citations, statutory/rule numbers, case numbers, and exhibit labels.
- Assign field-level confidence and reasons.
- Block or force review for critical fields below threshold.
- Make confidence absence an explicit state, not a default number.
- Display why an item is flagged and the exact source region.

### Deliverables

- Versioned confidence/calibration contract
- Calibration artefact with dataset fingerprint
- Critical-field validator module
- Updated review UI
- Calibration and critical-field golden tests
- `docs/B-S3-05_confidence.md`

### Acceptance gates

- **AC-S3-5.1:** Expected calibration error/Brier metric meets the frozen threshold on held-out pages.
- **AC-S3-5.2:** A syntactically plausible but OCR-corrupted date/amount/citation is not automatically marked high confidence.
- **AC-S3-5.3:** Critical fields below threshold cannot silently enter trusted graph facts or quotations.
- **AC-S3-5.4:** UI distinguishes raw OCR confidence, validation warnings, counsel confirmation, and missing confidence.
- **AC-S3-5.5:** The former surface-form scorer is unreachable in production.

### Halt if

- Calibration thresholds or critical-field error costs are not owner-approved.

### Done when

Confidence means measured reliability and no longer creates false assurance from word shape.

---

## B-S3-06 — Litigation-grade case memory and graph extraction

| Field | Requirement |
|---|---|
| **Goal** | Represent and extract the litigation concepts required for Digest, Argument Integrity, and Mock Court. |
| **Closes** | Audit finding #4 |
| **Depends on** | B-S3-05 |
| **Required reading** | `case_graph.schema.json`, skeleton extractors, review/interview/digest/integrity consumers |

### Required graph coverage

- Parties, aliases, roles, and sides
- Claims, causes/grounds, defences, and counterclaims
- Issues and sub-issues
- Relief/prayers
- Material, admitted, disputed, and inferred facts
- Evidence, exhibits, document support, witnesses, and statements
- Chronology with ambiguity, ranges, relative dates, disputes, and event type
- Procedural history, applications, hearings, and orders
- Statutes, rules, authorities, and pleaded provisions
- Fact → document, fact → evidence, issue → fact, issue → law, argument → support links
- Contradictions, missing links, uncertainty, and alternative interpretations
- Counsel-confirmed items and correction lineage

### Agent mission

1. Version the graph contract and migration strategy before extractor changes.
2. Preserve source pointers and provenance on nodes and edges.
3. Separate extraction, entity resolution, relationship construction, and inference.
4. Never use a model inference as an extracted document fact.
5. Add ambiguity-aware date handling rather than silently assuming date format.
6. Add meaningful contradiction candidates with both source sides and confidence.
7. Update Digest, Interview, Argument Builder, Integrity, and Mock Court consumers.

### Deliverables

- `case_graph` v2 schema and migration
- Extractor/entity-resolution pipeline
- Graph validation and provenance audit
- Controlled India/US graph goldens
- Graph review UI updates
- `docs/B-S3-06_case_graph_v2.md`

### Acceptance gates

- **AC-S3-6.1:** Schema represents every required concept and relationship without overloaded free-text fields.
- **AC-S3-6.2:** Every extracted node/edge has a source pointer, confidence, origin, pipeline version, and timestamp.
- **AC-S3-6.3:** Precision/recall meets frozen thresholds per concept—not only aggregate chronology recall.
- **AC-S3-6.4:** Ambiguous dates remain ambiguous until evidence or counsel resolves them.
- **AC-S3-6.5:** Contradiction candidates cite both conflicting sources and do not equate same-date events with contradiction.
- **AC-S3-6.6:** Empty/incomplete matters produce explicit gaps, not inflated completeness.

### Halt if

- Graph field semantics or migration behavior are not frozen.
- Any important output lacks traceable provenance.

### Done when

The graph can faithfully support a controlled litigation matter and all consumers use v2 without compatibility loss.

---

## B-S3-07 — Counsel authority, immutable corrections, and rerun merge

| Field | Requirement |
|---|---|
| **Goal** | Ensure re-OCR, re-extraction, graph rebuild, document addition, batch resume, and model upgrades cannot silently overwrite counsel-confirmed information. |
| **Closes** | Audit finding #5 |
| **Depends on** | B-S3-06 |
| **Required reading** | Vault graph/text replacement code, correction store, migrations, batch pipeline |

### Agent mission

1. Create stable identities/lineage for correctable text spans, nodes, and edges.
2. Store corrections as authoritative overlays/events, not only mutations of generated snapshots.
3. Implement deterministic three-way merge: prior generated state, counsel overlay, newly generated state.
4. Preserve counsel values; produce a review conflict if a new source materially disagrees.
5. Support correction history, reason, timestamp, and optional revert.
6. Prevent deletion of counsel-confirmed items during rebuild unless counsel explicitly deletes them.
7. Apply the same policy to word corrections and graph corrections.

### Deliverables

- Authority/lineage contract
- Correction-event migration
- Rerun merge engine
- Conflict review surface
- Regression matrix for every rerun path
- `docs/B-S3-07_correction_authority.md`

### Acceptance gates

- **AC-S3-7.1:** Counsel graph correction survives skeleton rebuild and document addition.
- **AC-S3-7.2:** Counsel word correction survives recognition rerun or appears as an explicit resolvable conflict.
- **AC-S3-7.3:** Batch restart and model/pipeline upgrade preserve overlays.
- **AC-S3-7.4:** New contradictory evidence triggers a conflict notice; it never replaces counsel truth silently.
- **AC-S3-7.5:** Full correction history is auditable and encrypted.
- **AC-S3-7.6:** Authority precedence is enforced in all downstream contexts and exports.

### Halt if

- Stable identity/matching semantics cannot distinguish a moved span from a deleted source.

### Done when

The full rerun regression suite proves correction persistence across all named paths.

---

## B-S3-08 — Legal source and chunk metadata contract

| Field | Requirement |
|---|---|
| **Goal** | Give every legal source and retrievable chunk enough metadata for jurisdictional, temporal, authority, trust, and provenance decisions. |
| **Closes** | Audit finding #11 |
| **Depends on** | B-S3-02, B-S3-01 |
| **Required reading** | `.akpkg` schema, citation schema, pack installer, retrieval corpus builder |

### Required metadata

At minimum:

```text
source_id, title, jurisdiction, jurisdiction_path, authority_type,
issuing_authority, citation, section/rule/article, effective_from,
effective_to, version/amendment, source_file, page, paragraph,
heading/path, exact_text, checksum, pack_id, trust_level,
superseded_status, publisher, source_reference
```

For judgments also include court, case name/number, decision date, paragraph number, holding/proposition tags, and precedential status where verified.

### Agent mission

- Version `.akpkg`, legal-source, legal-chunk, and citation contracts.
- Define authority/trust and supersession enums.
- Define temporal applicability and “unknown” behavior.
- Preserve exact authoritative text; summaries are separate derived artefacts.
- Define stable IDs and checksums.
- Build schema validators and migration support for older dev packs.

### Deliverables

- `contracts/legal_source.schema.json`
- `contracts/legal_chunk.schema.json`
- `.akpkg` manifest v2
- Citation schema update
- Pack build/validation tools
- `docs/B-S3-08_legal_metadata.md`

### Acceptance gates

- **AC-S3-8.1:** Every indexed official chunk maps to exact source text and a verified pack/source checksum.
- **AC-S3-8.2:** Jurisdiction, authority type, effective date, trust, and supersession are filterable fields.
- **AC-S3-8.3:** Unknown metadata is explicit and cannot be interpreted as current/controlling.
- **AC-S3-8.4:** Generated summaries cannot replace or masquerade as authoritative text.
- **AC-S3-8.5:** Old placeholder packs are rejected, migrated as `development_only`, or visibly isolated from controlled-beta use.

### Halt if

- Legal taxonomy or trust semantics require a product/legal editorial decision that is not recorded.

### Done when

All legal ingestion and retrieval consumers use the versioned source/chunk contract.

---

## B-S3-09 — Malicious and malformed knowledge-pack hardening

| Field | Requirement |
|---|---|
| **Goal** | Safely import official/custom packs without traversal, collision, active-content, resource-exhaustion, or trust-escalation vulnerabilities. |
| **Closes** | Audit finding #19 |
| **Depends on** | B-S3-08 |
| **Required reading** | Pack installer/protection modules, Tauri IPC, trust stores |

### Agent mission

- Enforce schema validation at runtime.
- Validate `pack_id`, versions, paths, hashes, signatures, file counts, and sizes.
- Reject absolute paths, `..`, alternate separators, device paths, symlinks/reparse points, and canonical paths outside the pack root.
- Reject executable/active content and unexpected file types.
- Defend against archive bombs if archive transport is added.
- Detect duplicate source IDs and collisions across packs.
- Keep official and custom trust domains separate throughout retrieval.
- Validate protected content after encryption and fail closed if protection fails for official packs.
- Make installation transactional; never delete a working version before the new one validates.

### Deliverables

- Hardened installer
- Malicious-pack fixture suite
- Transactional install/rollback
- Pack security report UI
- `docs/B-S3-09_pack_security.md`

### Acceptance gates

- **AC-S3-9.1:** Traversal, absolute path, symlink/reparse, invalid ID, collision, hash mismatch, signature failure, oversized, and unsupported-content fixtures are rejected.
- **AC-S3-9.2:** Failed upgrade leaves the prior installed pack usable.
- **AC-S3-9.3:** Official protection failure fails installation; no best-effort success.
- **AC-S3-9.4:** Custom material never receives official trust labels.
- **AC-S3-9.5:** No pack content is executed or rendered as active HTML/script.

### Halt if

- An archive format is introduced without a frozen extraction/containment contract.

### Done when

The malicious-pack suite is cumulative and all pack operations are fail-closed and recoverable.

---

## B-S3-10 — First usable authoritative legal repository

| Field | Requirement |
|---|---|
| **Goal** | Replace placeholder legal packs with a narrow, verified, inspectable repository sufficient for the first controlled case. |
| **Closes** | Audit finding #6 |
| **Depends on** | B-S3-09 |
| **Required reading** | Legal metadata contracts, pack authoring tools, selected test-case profile |

### Agent mission

1. Select one narrow first-test legal framework with the product owner—for example one jurisdiction, court level, matter type, and procedural posture.
2. Obtain only authoritative, rights-cleared sources.
3. Record publisher/issuing authority, acquisition date, version/effective date, checksums, editorial review, and permitted use.
4. Create signed official packs using exact source text.
5. Include a deliberately absent authority in the controlled test to verify repository-gap behavior.
6. Add pack freshness/supersession display and an update manifest.
7. Keep India/US breadth out of scope unless verified source content is available; depth for one controlled test is preferable to fake breadth.

### Deliverables

- At least one non-placeholder official controlled-beta pack
- Source ledger and editorial checklist
- Pack signature and trust-key procedure
- Pack update/version display
- Retrieval golden questions authored from the pack
- `docs/B-S3-10_first_legal_repository.md`

### Acceptance gates

- **AC-S3-10.1:** Every official source has authority, rights, checksum, version, effective date/status, and reviewer evidence.
- **AC-S3-10.2:** Exact stored passages can be opened from pack → source → paragraph/page.
- **AC-S3-10.3:** Placeholder/synthetic packs cannot be selected for controlled legal testing without a blocking warning.
- **AC-S3-10.4:** Missing requested authority produces a repository-gap result.
- **AC-S3-10.5:** Pack version, jurisdiction, source publisher, update date, signature, and supersession warning are visible.

### Halt if

- The owner has not selected the first legal framework.
- Source authority, currentness, or rights are uncertain.

### Done when

The controlled test can rely on a real verified pack without model memory or placeholder law.

---

## B-S3-11 — Hybrid retrieval, filtering, reranking, and index persistence

| Field | Requirement |
|---|---|
| **Goal** | Replace naive lexical-only retrieval with an offline hybrid pipeline appropriate for legal sources. |
| **Closes** | Audit finding #10 |
| **Depends on** | B-S3-10 |
| **Required reading** | Retrieval module, legal chunk contract, model choreography, performance metrics |

### Required pipeline

```text
Issue/argument
  → query decomposition
  → deterministic jurisdiction/authority/effective-date/trust filters
  → lexical retrieval
  + semantic retrieval
  → reranking
  → exact chunks + metadata
  → citation/proposition verifier
```

### Agent mission

- Freeze and benchmark an offline embedding model with licence/checksum.
- Implement BM25/FTS and semantic indexes.
- Persist indexes to disk and invalidate deterministically on pack updates.
- Apply filters before or during retrieval; never rely on prompt instructions for jurisdiction.
- Add reranking with deterministic fallback.
- Separate matter retrieval, legal retrieval, and experience retrieval result types.
- Record component scores and reasons for inspectability.
- Measure cold, warm, and repeated retrieval latency/RSS.

### Deliverables

- Hybrid index implementation
- Index metadata/version contract
- Retrieval benchmark and bake-off
- Golden relevance judgments
- Explainable retrieval UI details
- `docs/B-S3-11_hybrid_retrieval.md`

### Acceptance gates

- **AC-S3-11.1:** Recall@8 and MRR meet frozen thresholds on real authoritative questions for the selected framework.
- **AC-S3-11.2:** Zero wrong-jurisdiction or disallowed-trust results when deterministic filters exclude them.
- **AC-S3-11.3:** Superseded/out-of-effective-date material is excluded or explicitly warned according to policy.
- **AC-S3-11.4:** Index rebuild/update is crash-safe and checksum-driven.
- **AC-S3-11.5:** Offline operation is verified; model, index, and app fit reference hardware budgets.
- **AC-S3-11.6:** Lexical-only fallback is visible and does not claim semantic coverage.

### Halt if

- Embedding model licence or hardware fit is unresolved.
- Relevance judgments are synthetic-only for a claimed legal-readiness pass.

### Done when

Hybrid retrieval reliably returns inspectable controlling/relevant text for the controlled repository.

---

## B-S3-12 — Matter legal working set

| Field | Requirement |
|---|---|
| **Goal** | Give each case an inspectable, counsel-controlled set of authorities used by Digest, Drafting, Argument Integrity, and Mock Court. |
| **Closes** | Audit finding #18 |
| **Depends on** | B-S3-11 |
| **Required reading** | Case profile, retrieval UI, pack manager, downstream reasoning surfaces |

### Agent mission

- Generate proposed authorities from case jurisdiction, court, matter type, procedural posture, pleaded provisions, issues, and selected packs.
- Require confirmation when multiple regimes may apply.
- Let counsel add, remove, pin, exclude, mark irrelevant, flag superseded, and confirm controlling authorities.
- Preserve counsel decisions across retrieval/index/model updates.
- Version the working set and record which version each output used.
- Never copy full authoritative text into an untraceable summary-only store.

### Deliverables

- Working-set schema and encrypted migration
- Suggest/confirm/update APIs
- Lawyer-facing working-library UI
- Downstream working-set selector
- Rerun persistence tests
- `docs/B-S3-12_working_library.md`

### Acceptance gates

- **AC-S3-12.1:** Counsel can inspect exact text and metadata for every working-set item.
- **AC-S3-12.2:** Pinned/removed/confirmed decisions survive app restart, pack update, and retrieval rerun.
- **AC-S3-12.3:** Ambiguous legal regimes are surfaced for confirmation.
- **AC-S3-12.4:** Every downstream output records the working-set version and authority IDs used.
- **AC-S3-12.5:** Superseded or removed authority cannot silently re-enter reasoning.

### Halt if

- Applicable-law confirmation semantics are not product-owner approved.

### Done when

Counsel can answer “Which law is CaseForge relying on for this matter?” before running Mock Court.

---

## B-S3-13 — Local model fitness, configuration freeze, and legal safety evaluation

| Field | Requirement |
|---|---|
| **Goal** | Establish exactly what model is used, what it can safely do, and whether it meets controlled-beta reasoning and refusal requirements. |
| **Closes** | Audit finding #12 |
| **Depends on** | B-S3-02, B-S3-12 |
| **Required reading** | `infer_llm.py`, backend adapters, guardrails, choreography, model README |

### Agent mission

1. Report model name/version/tag, backend, quantization, context, model/checkpoint digest, prompt template, system prompt version, generation parameters, RAM, and speed.
2. Classify it accurately: stock, quantized stock, prompted, adapter, or independently fine-tuned.
3. Do not label `qwen2.5:7b` or another base model legally fine-tuned without checkpoint evidence.
4. Build a repeatable evaluation suite for fact grounding, citation grounding, unknown-law refusal, contradictions, opposition behavior, long context, prompt injection, nonexistent authorities, quotation fidelity, and source inspection.
5. Test backend variations allowed by the product and freeze the supported controlled-beta configuration.
6. Version prompts and store hashes in output provenance.

### Deliverables

- Model manifest contract
- Supported-model lock/allowlist
- Prompt/version registry
- Model evaluation runner and reports
- Reference-hardware measurements
- `docs/B-S3-13_model_fitness.md`

### Acceptance gates

- **AC-S3-13.1:** Runtime UI/report displays exact backend/model/quantization-or-tag/context/prompt version and digest where available.
- **AC-S3-13.2:** Unknown-law refusal, fake-citation refusal, prompt-injection resistance, fact grounding, and quote fidelity meet frozen thresholds.
- **AC-S3-13.3:** Contradictory evidence is surfaced rather than silently resolved.
- **AC-S3-13.4:** Long matters use retrieval/working sets; context truncation cannot silently remove controlling sources.
- **AC-S3-13.5:** Live generation latency and peak RSS meet reference-tier budgets or the feature is honestly gated.
- **AC-S3-13.6:** Unsupported model/backend configurations cannot claim controlled-beta readiness.

### Halt if

- Model licence, digest, context behavior, or reference hardware performance is unknown.
- Safety thresholds fail; do not compensate only with UI disclaimers.

### Done when

The supported local model is evidence-backed as a reasoning engine—not a legal database.

---

## B-S3-14 — Proposition ledger and hard citation/quotation verification

| Field | Requirement |
|---|---|
| **Goal** | Make every important generated proposition traceable and prevent unsupported factual or legal claims from rendering as trusted output. |
| **Closes** | Audit finding #9 |
| **Depends on** | B-S3-13 |
| **Required reading** | Citation records, retrieval results, source pointers, guardrails, all generation paths |

### Required proposition classes

```text
FACTUAL_BASIS
LEGAL_BASIS
INFERENCE
COUNSEL_INPUT
UNSUPPORTED_OR_GAP
```

### Agent mission

- Define proposition and support-edge contracts.
- Require generation to emit structured propositions before rendering prose.
- Verify factual support against graph/source pointers.
- Verify legal support against permitted retrieved chunks and effective/trust policy.
- Verify quotations as exact normalized substrings with source offsets.
- Fail closed: remove, block, or visibly label unsupported claims according to output type.
- Treat source IDs in model text as untrusted until host verification succeeds.
- Make GREEN/AMBER/RED reflect actual proposition support, not string similarity alone.

### Deliverables

- Proposition-ledger schema
- Host-side verifier
- Rendering policy
- Adversarial verifier suite
- Integration into drafting/digest/reasoning APIs
- `docs/B-S3-14_grounding_verifier.md`

### Acceptance gates

- **AC-S3-14.1:** 100% of legal propositions in controlled outputs have verified permitted authority or render as repository gaps.
- **AC-S3-14.2:** 100% of direct quotations map to exact stored source text and offsets.
- **AC-S3-14.3:** Factual propositions identify matter source/counsel input; inferences identify their bases.
- **AC-S3-14.4:** Fabricated source IDs, citations, pages, or paragraphs cannot pass host verification.
- **AC-S3-14.5:** Planted unsupported persuasive arguments are blocked or clearly labelled unsupported.
- **AC-S3-14.6:** Absence of authority never triggers silent model-memory fallback.

### Halt if

- A generation surface cannot emit or be transformed into verifiable structured propositions.

### Done when

The host—not the LLM—decides whether factual/legal support is sufficient to render as trusted.

---

## B-S3-15 — Automatic legal retrieval in Mock Court

| Field | Requirement |
|---|---|
| **Goal** | Ensure every relevant Mock Court turn uses the confirmed matter working set and retrieved legal authority instead of empty retrieval arrays. |
| **Closes** | Audit finding #7 |
| **Depends on** | B-S3-14 |
| **Required reading** | Mock Court UI/IPC/host engine, working library, retrieval APIs |

### Agent mission

- Decompose the session objective and current turn into factual and legal retrieval needs.
- Load the exact graph version and working-set version.
- Retrieve permitted matter anchors and legal chunks.
- Block start or warn explicitly if the case graph, counsel review, working set, or required authority is missing.
- Pass verified IDs/snippets/metadata—not UI-constructed free text—to the turn engine.
- Cache safely by case/graph/working-set/query version and invalidate on changes.
- Show the authorities available to the rehearsal before and during the session.

### Deliverables

- Mock Court retrieval orchestrator
- Versioned IPC request/response
- Readiness gate UI
- Source panel per turn
- Missing-authority and wrong-jurisdiction tests
- `docs/B-S3-15_mock_court_retrieval.md`

### Acceptance gates

- **AC-S3-15.1:** Production Mock Court never sends empty retrieval by default when legal issues require authority.
- **AC-S3-15.2:** Every legal challenge references verified working-set/retrieval IDs or states that authority is absent.
- **AC-S3-15.3:** Wrong-jurisdiction, superseded, removed, RED, and unconfirmed custom sources are excluded according to policy.
- **AC-S3-15.4:** Counsel can open exact authority text from a turn.
- **AC-S3-15.5:** Graph/working-set updates invalidate stale sessions or require explicit resume against a new version.

### Halt if

- The first controlled case has no confirmed working set.

### Done when

Mock Court is genuinely grounded in installed law rather than graph-only templates.

---

## B-S3-16 — Grounded local-LLM Mock Court

| Field | Requirement |
|---|---|
| **Goal** | Replace the production deterministic template path with adversarial local-model reasoning constrained and verified by the host. |
| **Closes** | Audit finding #8 |
| **Depends on** | B-S3-15 |
| **Required reading** | Model fitness report, proposition verifier, Mock Court formats, adaptive bench, guardrails |

### Agent mission

1. Use the frozen supported local model for production turns.
2. Construct prompts from graph facts, counsel-confirmed items, retrieved law, role, objective, format, and session history.
3. Treat all document/source text as quoted data, never instructions.
4. Require structured model output containing challenges, propositions, support IDs, unanswered questions, and next focus.
5. Run hard host verification before rendering.
6. Retry safely only for schema repair; never relax grounding to obtain an answer.
7. Preserve deterministic templates as an explicitly labelled degraded fallback that cannot claim AI adversarial analysis.
8. Keep no-outcome/no-named-judge/no-fabrication rules.

### Deliverables

- Local-LLM Mock Court adapter
- Structured turn schema v2
- Prompt registry/version
- Verified render pipeline
- Deterministic degraded-mode label
- Multi-turn controlled-case tests
- `docs/B-S3-16_grounded_mock_court.md`

### Acceptance gates

- **AC-S3-16.1:** Production turns report real model/backend/prompt hashes; deterministic mode is never mislabeled as model reasoning.
- **AC-S3-16.2:** Unsupported model propositions are removed/refused before display.
- **AC-S3-16.3:** The adversary presses planted weak links, contradictions, missing evidence, and absent authority at frozen success rates.
- **AC-S3-16.4:** Prompt instructions embedded in matter/legal documents have zero control over model behavior.
- **AC-S3-16.5:** Turn latency and RSS meet tier budgets; timeout/failure yields safe degraded behavior.
- **AC-S3-16.6:** No outcome prediction, named-judge profiling, invented evidence, or invented authority passes the full red-team suite.

### Halt if

- The model or verifier fails grounding/safety thresholds.
- The only way to meet latency is to omit required sources silently.

### Done when

Mock Court demonstrates source-bounded adversarial reasoning rather than generic fixed prompts.

---

## B-S3-17 — Universal provenance UX

| Field | Requirement |
|---|---|
| **Goal** | Make factual evidence, counsel input, legal authority, inference, and unsupported gaps visibly distinct and inspectable throughout the product. |
| **Closes** | Audit finding #17 |
| **Depends on** | B-S3-16 |
| **Required reading** | Review, Digest, Drafting, Argument Tree, Retrieval, Mock Court, export code |

### Agent mission

- Create one consistent provenance component and vocabulary.
- Show source type, trust/support state, title, page/paragraph, version, and verification status.
- Provide one action to open the underlying matter page/region or legal passage.
- Display counsel-confirmed and model-inferred states distinctly.
- Preserve provenance in DOCX/PDF exports and copied text where applicable.
- Add accessible text/icon/color patterns; never rely on color alone.

### Deliverables

- Shared provenance UI component
- Source inspection drawer/router
- Updated Digest, Drafting, Integrity, Review, Retrieval, Mock Court, and exports
- Accessibility tests
- `docs/B-S3-17_provenance_ux.md`

### Acceptance gates

- **AC-S3-17.1:** Every important proposition on named surfaces exposes its class and support.
- **AC-S3-17.2:** Matter sources open to correct document/page/region; legal sources open to correct pack/source/section/paragraph.
- **AC-S3-17.3:** Counsel edits and model inferences are unmistakable.
- **AC-S3-17.4:** AMBER/RED/unsupported states cannot be mistaken for verified authority.
- **AC-S3-17.5:** Keyboard and screen-reader users can inspect provenance.
- **AC-S3-17.6:** Exports retain sufficient citations/provenance and preparation disclaimers.

### Halt if

- A surface cannot provide stable source identifiers.

### Done when

A lawyer can answer “Where did CaseForge get that?” from every core preparation surface.

---

## B-S3-18 — Source-specific Mock Court debrief and preparation tasks

| Field | Requirement |
|---|---|
| **Goal** | Convert each rehearsal into concrete, traceable work the lawyer should complete before court. |
| **Closes** | Audit finding #14 |
| **Depends on** | B-S3-17 |
| **Required reading** | Voice/text session records, proposition ledger, Integrity missing links, working set |

### Agent mission

Generate a structured debrief containing:

- weaknesses discovered;
- contradictions pressed;
- questions counsel did not answer;
- factual/evidentiary/legal gaps;
- missed or misused authorities;
- unsupported persuasive arguments;
- source-specific preparation tasks;
- improvement compared with earlier turns/sessions, without outcome prediction.

Each finding must cite the relevant turn and factual/legal bases. Counsel must be able to accept, dismiss, edit, and mark tasks complete.

### Deliverables

- Debrief v2 schema
- Debrief generator using verified session ledger
- Preparation-task store and UI
- Session comparison view
- Controlled human-review rubric
- `docs/B-S3-18_debrief.md`

### Acceptance gates

- **AC-S3-18.1:** Every debrief weakness/task links to session turn(s) and verified matter/legal sources, or is explicitly an inference/gap.
- **AC-S3-18.2:** Planted weaknesses appear in the debrief at the frozen recall threshold.
- **AC-S3-18.3:** Generic filler tasks are rejected by human usefulness rubric.
- **AC-S3-18.4:** Counsel can dismiss incorrect suggestions without altering underlying source truth.
- **AC-S3-18.5:** Debrief never claims a likely outcome, guilt, sentence, or judge behavior.

### Halt if

- The debrief cannot distinguish observed session performance from model inference.

### Done when

The lawyer can clearly state what the rehearsal uncovered and what preparation must change.

---

## B-S3-19 — Lawyer-oriented end-to-end workflow

| Field | Requirement |
|---|---|
| **Goal** | Replace implementation-driven navigation with a coherent case-preparation journey while retaining advanced diagnostics. |
| **Closes** | Audit finding #16 |
| **Depends on** | B-S3-18 |
| **Required reading** | `src/App.tsx`, all core views, UI styles, controlled test protocol |

### Target primary journey

```text
1. Add Case Material
2. Review Document Quality
3. Review What CaseForge Understood
4. Complete Missing Facts
5. Case Digest
6. Legal Authorities
7. Argument Analysis
8. Mock Court
9. Preparation Tasks
```

### Agent mission

- Derive stage completion/readiness from real persisted state.
- Make the next safe action obvious.
- Move Triage, Text Layer, Skeleton, Batch, model status, and pipeline IDs into advanced/details surfaces.
- Provide clear empty, loading, partial, failed, blocked, cancelled, and stale states.
- Prevent Mock Court when prerequisites are unsafe; explain exactly what to fix.
- Preserve source-return navigation and long-text readability.
- Conduct task-based usability sessions with people unfamiliar with the build, preferably litigators.

### Deliverables

- Information architecture and state model
- Updated navigation and case workspace
- Readiness/checklist component
- Advanced processing details area
- Task-based UI automation and usability protocol
- `docs/B-S3-19_lawyer_workflow.md`

### Acceptance gates

- **AC-S3-19.1:** New users identify each screen’s purpose and primary action within the frozen usability threshold.
- **AC-S3-19.2:** Users complete the controlled journey without technical documentation at the frozen success rate.
- **AC-S3-19.3:** Prerequisite, completion, stale, error, and partial-success states are accurate.
- **AC-S3-19.4:** Destructive operations require scoped confirmation and communicate recoverability.
- **AC-S3-19.5:** Long legal text, tables, provenance panels, and keyboard navigation pass accessibility/readability checks.
- **AC-S3-19.6:** Advanced diagnostics remain available without dominating the lawyer workflow.

### Halt if

- UI stage semantics are not backed by persisted state or verifiable readiness gates.

### Done when

The app feels like a case-preparation workspace rather than a pipeline/developer console.

---

## B-S3-20 — Production offline English speech

| Field | Requirement |
|---|---|
| **Goal** | Replace transcript stubs with a measured, bundled, editable offline STT/TTS path for supported hardware. |
| **Closes** | Audit finding #15 |
| **Depends on** | B-S3-19, B-S3-13 |
| **Required reading** | Speech sidecar, Voice/Mock Court views, lexicon, choreography, G-AUDIO protocol |

### Agent mission

- Select/freeze redistributable offline English STT and TTS configurations.
- Capture actual microphone audio with explicit permission/state.
- Stage transcript for correction before it becomes counsel input.
- Preserve audio/transcript/correction provenance according to retention policy.
- Apply legal lexicon corrections without silently changing quoted evidence.
- Measure legal names, citations, amounts, provision numbers, and courtroom terms separately from aggregate WER.
- Keep transcript-stub mode only as an explicit developer/test mode excluded from readiness.
- Gate voice honestly by hardware capability and model availability.

### Deliverables

- Bundled/configured STT/TTS adapters
- Audio permission/capture lifecycle
- Editable staging and correction flow
- G-AUDIO rights-cleared set and report
- Voice latency/RSS/WER report
- `docs/B-S3-20_voice.md`

### Acceptance gates

- **AC-S3-20.1:** Production voice uses real audio; transcript stubs cannot satisfy the gate.
- **AC-S3-20.2:** WER and critical-term error meet frozen thresholds.
- **AC-S3-20.3:** Counsel confirms/edits transcript before commit.
- **AC-S3-20.4:** STT/TTS remain offline and use supported local endpoints only.
- **AC-S3-20.5:** Mic/audio state and retention are visible; cancellation releases resources.
- **AC-S3-20.6:** Unsupported hardware gets a clear text-only fallback.

### Halt if

- Audio rights, model redistribution rights, or WER threshold are unresolved.

### Done when

Voice can participate in controlled testing without simulated transcript input.

---

## B-S3-21 — Tauri CSP, renderer, and active-content hardening

| Field | Requirement |
|---|---|
| **Goal** | Apply a restrictive desktop content-security policy and prevent imported/generated content from becoming executable renderer content. |
| **Closes** | Audit finding #20 |
| **Depends on** | B-S3-20, B-S3-09 |
| **Required reading** | `tauri.conf.json`, capabilities, shell permissions, HTML/render paths, export previews |

### Agent mission

- Define the minimum Tauri CSP required by the built app.
- Remove inline/eval/remote allowances unless individually justified.
- Restrict `connect-src` to required loopback endpoints.
- Audit shell/plugin permissions and remove unused capability.
- Treat pack/document/model output as text; sanitize any necessary rich content.
- Prevent navigation to arbitrary remote origins.
- Add renderer security tests and manual release inspection.

### Deliverables

- Non-null restrictive CSP
- Capability/permission audit
- Sanitization/text-render policy
- Security regression tests
- `docs/B-S3-21_desktop_hardening.md`

### Acceptance gates

- **AC-S3-21.1:** Production CSP is non-null and contains no unjustified wildcard, remote script, or unsafe-eval allowance.
- **AC-S3-21.2:** Malicious HTML/script in documents, packs, model output, and citations renders inert.
- **AC-S3-21.3:** App functionality, exports, source inspection, and approved loopback services work under CSP.
- **AC-S3-21.4:** Tauri capabilities are least-privilege and unused shell permissions are removed.
- **AC-S3-21.5:** Remote navigation and unexpected network connections are blocked and logged safely.

### Halt if

- A required feature depends on broad CSP bypass without an approved design change.

### Done when

The packaged application passes the renderer/active-content suite with its production CSP.

---

## B-S3-22 — Local inference boundary and Stage 3 controlled-beta ship gate

| Field | Requirement |
|---|---|
| **Goal** | Enforce that case context cannot be sent to arbitrary remote “compatible” endpoints and prove all 22 Stage 3 batches cumulatively complete. |
| **Closes** | Audit finding #21 and performs final cumulative closure |
| **Depends on** | B-S3-01 through B-S3-21 |
| **Required reading** | LLM/STT/OCR backend configuration, companion server, CSP, all Stage 3 reports |

### Agent mission — local boundary

- Parse and validate all inference/service endpoints.
- Default allow only loopback addresses and approved local transports.
- Resolve hostname/IP ambiguity safely; defend against non-loopback aliases and redirects.
- Disable redirects from a permitted local endpoint to a remote endpoint.
- Reject credentials embedded in URLs and avoid logging secrets/case context.
- If a future non-local mode is ever supported, require a separate product mode, explicit per-session consent, unmistakable UI, and privacy review; it must remain disabled for controlled beta.
- Apply the same boundary policy to LLM, embeddings, OCR, STT, TTS, updates, and any sidecar that can receive case data.

### Agent mission — final ship gate

Create one command that:

1. Runs production frontend build and Rust tests.
2. Runs every `AC-S3-*` executable gate.
3. Validates contracts and migrations.
4. Runs real/synthetic reports separately.
5. Verifies no unresolved Stage 3 blocker or expired waiver.
6. Verifies required authoritative pack and supported model fingerprints.
7. Executes the controlled case end to end.
8. Produces a signed/fingerprinted readiness report with environment details.
9. Refuses `READY` if any required real-world, security, provenance, latency, or grounding gate is missing.

### Deliverables

- Shared local-endpoint validation module
- Backend allowlist/policy UI
- Redirect/DNS/remote-endpoint tests
- `eval/checks/local_boundary_b_s3_22.py`
- `eval/checks/ship_gate_b_s3_22.py`
- `eval/reports/stage3_ship_gate.json`
- `docs/RELEASE_CHECKLIST_v3.0.md`
- `docs/B-S3-22_ship_gate.md`
- `package.json` commands: `test:s3-boundary`, `ship:stage3`

### Acceptance gates

- **AC-S3-22.1:** Loopback/local configured backends work; public/non-local endpoints, redirects, and aliases are rejected in controlled-beta mode.
- **AC-S3-22.2:** Packet/network monitoring shows no case data leaving approved local interfaces during the full controlled workflow.
- **AC-S3-22.3:** All prior Stage 3 gates pass cumulatively with zero required skips.
- **AC-S3-22.4:** The first controlled matter completes ingestion → OCR → graph review → counsel correction → working library → argument analysis → Mock Court → debrief.
- **AC-S3-22.5:** Every displayed factual/legal proposition in the sampled final session is traceable and verified.
- **AC-S3-22.6:** Planted contradictions, missing evidence, unsupported argument, opponent argument, prompt injection, and absent authority produce the expected behavior.
- **AC-S3-22.7:** 40/100/300-page and live-turn reference measurements meet frozen budgets or features are correctly tier-gated.
- **AC-S3-22.8:** Licence expiry leaves case data readable while preventing unauthorized writes according to policy.
- **AC-S3-22.9:** No placeholder pack, transcript stub, heuristic confidence, deterministic-only Mock Court, or provisional PDF/OCR path can satisfy readiness.

### Final verdict rule

The Stage 3 gate may output only:

- `NOT READY`
- `READY FOR CONTROLLED TESTING`
- `READY FOR LIMITED REAL-CASE INTERNAL TESTING`

`READY FOR EXTERNAL LAWYER BETA` is intentionally out of scope for this playbook and requires a later external-beta security, legal-content, usability, and support audit.

### Halt if

- Any earlier batch is not `PASS`, any required real-world gate is skipped, the authoritative pack/model fingerprint differs from the approved configuration, or the controlled end-to-end case cannot complete with verified provenance. The final gate must return `NOT READY`; it must never waive these conditions automatically.

### Done when

The answer to the following question is evidence-backed **yes**:

> If CaseForge receives the carefully prepared English-language controlled litigation matter and its verified legal pack, is it trustworthy enough to test whether Mock Court improves a lawyer’s preparation?

---

## 4. Mandatory first controlled test case

The final Stage 3 case must include all of the following:

- multiple parties and roles;
- native, scanned, and mixed PDF pages;
- DOCX and image material;
- one low-quality or rotated scan;
- page headings, footnotes, table, signature/stamp, and page numbers;
- clear chronology and deliberately ambiguous dates;
- two genuinely contradictory statements;
- one unsupported allegation;
- one fact supported by multiple documents;
- one irrelevant and one duplicate document;
- monetary figures and provision/citation numbers;
- a procedural event and an order;
- a missing evidentiary link;
- multiple potentially applicable legal provisions;
- one persuasive but unsupported argument;
- one opponent argument the system should discover;
- installed relevant authority;
- one deliberately absent requested authority;
- one superseded or out-of-date authority fixture;
- one custom untrusted source;
- prompt-injection text inside a matter document;
- a counsel correction followed by re-OCR, graph rebuild, document addition, and model/index update;
- at least one text Mock Court session and, when Full-tier voice is in scope, one real-audio session.

The expected-answer file must identify which weakness the Mock Court should discover and which preparation task should result. The human reviewer must judge usefulness, not eloquence.

---

## 5. Cumulative completion matrix

Before starting a batch, the agent marks prior batches `PASS`. At final gate, all rows must be `PASS`.

| Batch | Required evidence | Status |
|---|---|---|
| B-S3-01 | Encoding scan + Unicode round trip + size budget | PASS (2026-08-13) |
| B-S3-02 | Frozen Stage 3 fixtures/metrics | PASS — contract frozen; evidence readiness BLOCKED (2026-08-13) |
| B-S3-03 | Real PDF parser golden report | IMPLEMENTED — formal PASS blocked on human GOLDEN review (2026-08-13) |
| B-S3-04 | Real-scan OCR report | IMPLEMENTED — formal PASS blocked on two-reviewer GOLDEN critical-field labels |
| B-S3-05 | Calibration/critical-field report | IMPLEMENTED — formal PASS blocked on owner-approved ECE/Brier limits and critical-field error costs |
| B-S3-06 | Graph v2 per-concept report | IMPLEMENTED — formal PASS blocked on owner-approved per-concept precision/recall thresholds and representative held-out India/US goldens |
| B-S3-07 | Rerun correction survival matrix | IMPLEMENTED — controlled rerun matrix PASS; correction survival 1.0 |
| B-S3-08 | Legal metadata validation | IMPLEMENTED — v2 source/chunk/pack/citation contracts, checksum lineage, exact metadata filters, legacy development-only isolation |
| B-S3-09 | Malicious-pack suite | IMPLEMENTED — fail-closed contained installer, cumulative exploit fixtures, transactional rollback, verified protection, security report UI |
| B-S3-10 | Verified official controlled pack | IMPLEMENTED — signed narrow India partition/civil-appeal pack, page lineage, freshness UI, placeholder block, repository-gap handling (2026-08-16) |
| B-S3-11 | Hybrid retrieval relevance/performance | IMPLEMENTED — controlled engineering PASS; named legal-editor relevance approval pending (2026-08-16) |
| B-S3-12 | Working-library persistence | IMPLEMENTED — encrypted versioned counsel controls, ambiguity confirmation, rerun/update persistence, downstream provenance selector (2026-08-16) |
| B-S3-13 | Supported-model fitness report | IMPLEMENTED — exact Ollama 0.32.14/Qwen2.5 7B digest freeze, 100% live safety gate, prompt/output provenance, context and size gates (requalified 2026-08-18) |
| B-S3-14 | Proposition/quotation verification | IMPLEMENTED — host-owned structured ledger, exact-excerpt/quotation offsets, permitted-authority policy, fail-closed rendering, adversarial verifier PASS (2026-08-18) |
| B-S3-15 | Mock Court retrieval integration | IMPLEMENTED — host-owned versioned retrieval, readiness/stale-session gates, exact per-turn sources, text+voice grounding, exclusion tests PASS (2026-08-18) |
| B-S3-16 | Grounded local-LLM Mock Court | IMPLEMENTED — allowlisted local-model text+voice turns, structured v2 schema, host verified-only rendering, bounded history/repair/timeout, degraded honesty, 5/5 live controlled turns PASS (2026-08-18) |
| B-S3-17 | Universal provenance inspection | IMPLEMENTED — shared accessible inspector, stable matter/legal locators, explicit counsel/inference/gap states, export/copy provenance retention (2026-08-18) |
| B-S3-18 | Source-specific useful debrief | IMPLEMENTED — verified-ledger Debrief v2, encrypted immutable snapshots/task workflow, source-specific findings, observed session comparison, usefulness/recall gate (2026-08-18) |
| B-S3-19 | Lawyer workflow usability | ENGINEERING IMPLEMENTED — nine-step persisted-state journey, safe next action, advanced diagnostics containment, Mock Court prerequisite routing, destructive-operation confirmation, accessibility hooks, automated gate; frozen five-person usability sign-off pending (2026-08-21) |
| B-S3-20 | Real offline voice metrics | ENGINEERING IMPLEMENTED — real microphone capture, persistent local Faster-Whisper Small, offline Windows SAPI WAV, explicit staging confirmation, encrypted transcript/provenance commit, Mock Court audio proof, text fallback, frozen G-AUDIO evaluator; human-audio WER/critical-term sign-off pending (2026-08-21) |
| B-S3-21 | CSP/renderer hardening | NOT STARTED |
| B-S3-22 | Local boundary + cumulative ship report | NOT STARTED |

---

## 6. Work explicitly deferred beyond Stage 3

Do not expand these 22 batches into:

- Hindi or regional-language OCR;
- cloud case processing or sync;
- firm collaboration;
- mobile full application;
- named-judge analytics;
- verdict, sentence, guilt, or win prediction;
- enterprise document-management integration;
- marketplace;
- broad general-purpose legal chatbot;
- nationwide India and all-US-state corpus breadth before the narrow controlled framework is trustworthy.

Stage 3 succeeds when one narrow English-language, offline, source-grounded preparation workflow is trustworthy enough to test—not when CaseForge appears feature-complete.
