# CaseForge Stage 4 — Verified Advocacy & Chambers Agentic Playbook

**Document status:** DRAFT — proposed execution authority for Stage 4 (2026-10-07)  
**Source of truth:** `CaseForge_PRD_v4.0.md` (Layer 1, extends `CaseForge_PRD_v1.0.md`)  
**Target release:** `v4.0.0` (optional early `v3.1.0` after Wave A)  
**Primary success criterion:** Every Stage 4 output a lawyer or judge relies on is provable from the record or signed, editor-approved law — never from model memory.

---

## 1. How to use this playbook

All rules of `docs/CASEFORGE_STAGE3_AGENTIC_PLAYBOOK.md` §1 apply unchanged:

- the mandatory batch loop (READ → FREEZE → IMPLEMENT → VERIFY → ADVERSARIAL CHECK → DOCUMENT → HAND OFF);
- the required agent handoff format;
- the mandatory `QUESTION … HALT` format (with `batch: B-S4-xx`);
- the Stage 3 severity rule (BLOCKER / HIGH / MEDIUM / POLISH);
- the definition of a valid test.

Stage 4 additions:

1. **Start condition.** B-S4-01 and B-S4-02 (contracts and goldens only) may run in parallel with B-S3-21/22. Every other Stage 4 batch requires the Stage 3 ship gate to return at least `READY FOR CONTROLLED TESTING`.
2. **Rule-table rule.** No jurisdiction-specific legal datum (section mapping, period, deadline, holiday, admissibility element, issue template) may appear in code, prompts, or fixtures except as a signed `RULE_TABLE` row with an authoritative chunk and legal-editor sign-off. Code branching on jurisdiction is a BLOCKER.
3. **Single trust path.** Every GREEN/verified state in Stage 4 is assigned by the B-S3-14 host verifier or by deterministic code over verified inputs. A Stage 4 batch that needs a second trust path must HALT.
4. **PROPOSED thresholds.** A threshold marked PROPOSED in the PRD is not a gate until frozen in `contracts/constants.json` by B-S4-01. Until then the related AC is `BLOCKED`.
5. **Legal-editor evidence.** ACs that depend on legal answers (code bridge, admissibility, deadlines) can only `PASS` with a named legal editor sign-off recorded in the golden manifest.
6. **Cumulative.** Every Stage 1–3 gate that was green must remain green after each Stage 4 batch.

---

## 2. Batch map

| Wave | Batch | Title | PRD | ACs |
|---|---|---|---|---|
| A | B-S4-01 | Stage 4 contracts, constants, guardrails freeze | §5, §11, §12, §16 | AC-16.1 (schema load), threshold freeze |
| A | B-S4-00 | Lean runtime: 3B/4B default model, 7B optional, ≤ 4 GiB base install | §11.1, §11.2 | AC-S4-00.1–00.8 |
| A | B-S4-00b | Per-answer feedback in Mock Court (deterministic) | §7.7, §11.2 | AC-S4-00b.1–00b.5 |
| A | B-S4-02 | Stage 4 golden sets and legal-editor protocol | §13 | Golden manifest validity |
| A | B-S4-03 | Signed Rule Tables (`.akpkg` RULE_TABLE) | §5 | AC-S4-RT.1–RT.3 |
| A | B-S4-04 | Citation & Quote Audit engine | §7.1 | AC-S4-01.1–01.3 |
| A | B-S4-05 | Audit Certificate export + offline verify | §7.1 | AC-S4-01.4–01.5 |
| A | B-S4-06 | Opposing Filing intake and scan | §7.2 | AC-S4-02.1–02.4 |
| A | B-S4-07 | Witness resolution and Statement Matrix | §7.3 | AC-S4-03.2 (matrix locators) |
| A | B-S4-08 | Contradiction engine + Impeachment Kit | §7.3 | AC-S4-03.1, 03.3, 03.4 |
| — | **Wave A checkpoint** | Optional `v3.1.0` release decision (Q-S4-08) | §14 | — |
| B | B-S4-09 | Code Bridge (statute succession + temporal routing) | §7.4 | AC-S4-04.1–04.3 |
| B | B-S4-10 | Admissibility & Foundation Gaps (integrity.v2) | §7.5 | AC-S4-05.1–05.3 |
| B | B-S4-11 | Deadline & Limitation Watch | §7.6, G9 | AC-S4-06.1–06.3 |
| B | B-S4-12 | Timed Oral Rehearsal | §7.7 | AC-S4-07.1–07.3 |
| B | B-S4-13 | Law Currency Stamp + Treatment Graph + delta packs | §7.8 | AC-S4-08.1–08.3 |
| B | B-S4-14 | Devanagari triage + OCR | §7.9 | AC-S4-09.1 |
| B | B-S4-15 | Aligned offline translation + G10 | §7.9, G10 | AC-S4-09.2–09.3 |
| B | B-S4-16 | Encrypted Bundle Exchange | §8 | AC-S4-10.1–10.4 |
| C | B-S4-17 | Chambers Edition shell + Record-only mode (G7) | §9.1 | AC-S4-11.1–11.2 |
| C | B-S4-18 | Issue Matrix | §9.2 | AC-S4-12.1–12.3 |
| C | B-S4-19 | Both-sides citation check + record lookup | §9.3 | AC-S4-13.1–13.2 |
| C | B-S4-20 | Daily Cause-List Brief | §9.4 | AC-S4-14.1–14.2 |
| C | B-S4-21 | Judgment Scaffold + Decision-Free Zone (G8) | §9.5, G8 | AC-S4-15.1–15.3 |
| Ship | B-S4-22 | Editions, pack subscription, seat transfer | §10 | AC-S4-16.1–16.2 |
| Ship | B-S4-23 | Stage 4 red-team + cumulative ship gate | §12, all | AC-S4-RT-RED + all |

AC IDs are numbered by **PRD feature** (F1 → AC-S4-01, … licensing → AC-S4-16), not by batch number. The table above is the authoritative mapping.

---

# WAVE A — Verified Advocacy core

## B-S4-01 — Stage 4 contracts, constants, guardrails freeze

| Field | Requirement |
|---|---|
| **Goal** | Freeze every Stage 4 type, threshold, error code, and guardrail before any producer or consumer exists. |
| **Depends on** | Stage 3 contracts frozen (B-S3-14 proposition ledger, B-S3-08 legal metadata) |
| **Required reading** | PRD v4.0 §5, §11, §12, §16; `contracts/MANIFEST.json`; `contracts/QUESTIONS.md`; `contracts/akpkg.schema.json`; `contracts/licence_payload.schema.json`; `contracts/guardrails.json`; `contracts/case_graph.schema.json` |

### Agent mission
- Author schemas listed in PRD §11 with `schema_version` and `status`.
- Add **versioned successors that coexist** with frozen contracts (the Rust installer, pack tool and Stage 1–3 gates pin the originals): `akpkg_v2_1.schema.json` (`RULE_TABLE`, `TREATMENT_EDGES`, `DISCLOSURE_TEMPLATES`) and `licence_payload_v2.schema.json` (`edition`, `pack_subscription_until`). Originals stay byte-identical.
- `EXTERNAL_FILING` is a `document_intake.type_tag` value (free string; no schema change). `ADVERSE_ASSERTION` and `TRANSLATED` graph origins are recorded as a **planned case_graph v2.1.0 amendment** in `MANIFEST.json`, applied by B-S4-06 / B-S4-15.
- Put G7–G10 in `contracts/guardrails_stage4.json` (extends, never redefines, `guardrails.json`); append Stage 4 codes to `contracts/errors.json` per its amendment rule.
- Present every PROPOSED threshold via `docs/worksheets/Q-S4-01_thresholds.md` and `contracts/stage4_thresholds.json` (status `PROPOSED_AWAITING_OWNER_APPROVAL`); write only owner-approved values to `contracts/constants.json`.
- Record the owner's resource-budget decisions in `contracts/stage4_resource_budget.json`.
- Register all Stage 4 ACs in a separate `eval/stage4_ac_inventory.json` (Stage 3 precedent) so the Stage 1/2 harness is unchanged.

### Deliverables
- 17 new schemas + `akpkg_v2_1` / `licence_payload_v2` successors; `guardrails_stage4.json`; `stage4_thresholds.json`; `stage4_resource_budget.json`; appended `errors.json` codes; `MANIFEST.json` entries + planned case_graph amendment
- `eval/stage4_ac_inventory.json`
- `docs/worksheets/Q-S4-01_thresholds.md`
- `eval/checks/contracts_b_s4_01.py`
- `docs/B-S4-01_contracts.md`

### Acceptance gates
- All schemas load (AC-16.1 extended); no Stage 1–3 schema changes in a breaking way.
- Every Stage 4 AC appears in the inventory; zero silent passes.
- No PROPOSED value appears in `constants.json` without an owner approval record.

### Halt if
- The owner has not approved thresholds — freeze schemas and inventory, leave threshold-dependent ACs `BLOCKED`, emit QUESTION Q-S4-01.

### Done when
Every Stage 4 artefact has a frozen contract and every gate is registered.

---

## B-S4-00 — Lean runtime (default 3B/4B model, optional 7B, ≤ 4 GiB base install)

| Field | Requirement |
|---|---|
| **Goal** | Bring the shipped app under the owner's 4 GiB cap with low RAM, without weakening any trust rule. |
| **Depends on** | B-S4-01 (runs before B-S4-03; may run alongside B-S4-02) |
| **Required reading** | PRD v4.0 §11.1–11.2; `contracts/stage4_resource_budget.json`; `contracts/supported_models.json`; `contracts/model_manifest.schema.json`; `docs/B-S3-13_model_fitness.md`; `scripts/check_size_budget.py`; `constants.model_choreography` |

### Agent mission
- Bake-off of default-model candidates in `stage4_resource_budget.json`: verify the licence text from the official model card (record its hash), measure Q4 file size and peak RSS, and run the full B-S3-13 fitness gate. Reject any model failing licence, size, or fitness.
- Freeze the winner as the default in a versioned model manifest; keep the current 7B manifest as `OPTIONAL_ADDON` (not auto-installed, never auto-loaded under 8 GB RAM).
- Add model-tier selection to the host (default vs add-on) with a visible status chip; the add-on is downloaded/installed only on explicit user action and verified by digest.
- Make `scripts/check_size_budget.py` measure the **base install** against `hard_cap_gib` (FAIL above it) and report the add-on separately.
- Prove every Stage 4 deterministic engine runs with no model loaded (static and runtime check).
- Re-run the Stage 3 grounded Mock Court and debrief gates (B-S3-15/16/18) against the default model and report quality deltas honestly.

### Deliverables
- Bake-off report `eval/bakeoff/default_model_b_s4_00.json`
- Versioned model manifests (default + optional add-on)
- Size-budget gate update; model-tier selection in host/UI
- `eval/checks/lean_runtime_b_s4_00.py`
- `docs/B-S4-00_lean_runtime.md`

### Acceptance gates
- **AC-S4-00.1:** Measured base install ≤ 4.0 GiB (`hard_cap_gib`); report against the 3.0 GiB target.
- **AC-S4-00.2:** Default model licence verified for offline commercial redistribution; text hash recorded.
- **AC-S4-00.3:** Default model passes the B-S3-13 fitness gate (≥ 0.85, all required cases).
- **AC-S4-00.4:** Peak RSS with default model ≤ owner-approved ceiling (Q-S4-12); AC-4.2 still holds with the add-on.
- **AC-S4-00.5:** Stage 3 grounding/Mock Court gates remain green on the default model, or regressions are reported and owner-accepted.
- **AC-S4-00.6:** Default model passes the held-out set `golden/G-MODEL-HELDOUT` (pinned hash; ≥ 0.85 and every critical case), authored before any prompt work so prompts cannot be tuned to it.
- **AC-S4-00.7:** Default model passes the injection held-out set `golden/G-MODEL-HELDOUT-INJ` (authored before the untrusted-citation guard; ≥ 0.85 and every critical case).
- **AC-S4-00.8:** Untrusted-citation guard (`caseforge.untrusted-citation-guard.v1`) passes its deterministic suite: planted citations removed in every supported format; permitted authorities, cause titles, counsel input and clean text untouched.

### Halt if
- No candidate passes licence + size + fitness — report and emit QUESTION; never ship a model that failed fitness to meet the size cap.

### Done when
A fresh install is under 4 GiB, runs within the RAM ceiling, and every trust guarantee from Stage 3 still holds.

---

## B-S4-00b — Per-answer feedback in Mock Court (deterministic)

| Field | Requirement |
|---|---|
| **Goal** | After every counsel answer in Mock Court, show what the record and the working set say about it — before the next challenge — without a model and without "right/wrong" or outcome language. |
| **Depends on** | B-S4-00 (owner-prioritised ahead of B-S4-02, 2026-10-08) |
| **Required reading** | `contracts/answer_feedback.schema.json`; `golden/G-ANSWER-FEEDBACK`; `src-tauri/src/grounded_mock_court.rs`; `src-tauri/src/mock_court_retrieval.rs`; `src/components/MockCourtView.tsx`; `contracts/case_graph.schema.json` (chronology, evidence, contradictions) |

### Agent mission
- Treat counsel's text in each turn as the answer to the previous rendered challenge (host memory, per session).
- Deterministically check the answer: dates vs. chronology (supported / conflicts / not found), amounts vs. record, authorities vs. the counsel-confirmed working set, record contradictions and working-set authorities relevant to the challenge but unused, and an engagement indicator with visible counts.
- Return it with the turn (`answer_feedback`) and render it under counsel's message.
- Every item carries a record/working-set reference or an explicit "not found"; prohibited language list enforced in tests.

### Deliverables
- `src-tauri/src/answer_feedback.rs` (+ unit tests against the golden fixture)
- `contracts/answer_feedback.schema.json`; `golden/G-ANSWER-FEEDBACK/fixtures/partition_appeal.json` (authored before the engine)
- `src/components/AnswerFeedbackPanel.tsx`; Mock Court wiring
- `eval/checks/answer_feedback_b_s4_00b.py`

### Acceptance gates
- **AC-S4-00b.1:** The golden fixture's four turns reproduce exactly (engagement + required items + forbidden kinds).
- **AC-S4-00b.2:** A planted wrong date is reported as CONFLICTS_WITH_RECORD against the right chronology event.
- **AC-S4-00b.3:** An authority outside the working set is NOT_IN_WORKING_SET; a working-set authority is VERIFIED_IN_WORKING_SET.
- **AC-S4-00b.4:** No model call, no outcome prediction, no prohibited language; every supported/conflict/available item has a record reference.
- **AC-S4-00b.5:** Feedback is returned with every grounded turn and rendered under counsel's message.

### Halt if
- Any check would need a model or legal judgement to decide — keep it out of v1 and record it as a limitation.

### Done when
A lawyer sees, under each answer, which of their dates, amounts and authorities the record supports, contradicts or cannot find, and what relevant record material they left unused.

---

## B-S4-02 — Stage 4 golden sets and legal-editor protocol

| Field | Requirement |
|---|---|
| **Goal** | Author known-answer data for every Stage 4 feature before feature code. |
| **Depends on** | B-S4-01 |
| **Required reading** | PRD §13; `golden/POLICY.md`; `golden/STAGE3_MANIFEST.json`; `docs/STAGE3_EVALUATION_PROTOCOL.md` |

### Agent mission
- Scaffold `G-AUDIT`, `G-OPPOSING`, `G-IMPEACH`, `G-CODEBRIDGE`, `G-ADMISS`, `G-DEADLINE`, `G-TREATMENT`, `G-INDIC`, `G-BUNDLE`, `G-CHAMBERS`, `G-CAUSELIST`, `G-REDTEAM-S4` with README, provenance (synthetic / public-domain / rights-cleared), and expected answers matching B-S4-01 schemas.
- Plant required cases: fabricated citations, single-word misquotes, wrong pinpoints, opponent record conflicts, injection inside external filings, witness contradictions and omissions, missing key dates, missing foundation certificates, holiday roll-overs, an overruled authority, Devanagari scans, an unanswered argument, extra-record bait, decision-solicitation prompts.
- Write `docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md`: who signs, what they review, how sign-off is recorded in `golden/STAGE4_MANIFEST.json`.
- Mark legal-answer sets `BLOCKED` until sign-off exists.

### Deliverables
- `golden/G-*/` for the twelve sets; `golden/STAGE4_MANIFEST.json`
- `docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md`
- `eval/checks/goldens_b_s4_02.py`

### Acceptance gates
- Every set validates against its schema and declares data provenance.
- Each planted case in PRD §7–§9 has at least one fixture (IN and US where the feature applies to both).
- No client material; no legal answer without a sign-off field (empty = `BLOCKED`).

### Halt if
- No named legal editor (Q-S4-02) — scaffold everything, leave legal answers `BLOCKED`, emit QUESTION.

### Done when
The harness discovers every Stage 4 set and reports honest PASS/SKIP/BLOCKED.

---

## B-S4-03 — Signed Rule Tables

| Field | Requirement |
|---|---|
| **Goal** | Make jurisdiction-specific legal data installable, signed, sourced, and versioned. |
| **Depends on** | B-S4-01, B-S4-02, Stage 3 ship gate |
| **Required reading** | PRD §5; `docs/B-S3-08_legal_metadata.md`; `docs/B-S3-09_pack_security.md`; `scripts/legal_pack_tool.py` |

### Agent mission
- Extend the pack builder/validator/installer for `RULE_TABLE` content.
- Enforce: every row resolves to an installed authoritative chunk; checksum lineage; effective-date windows; sign-off field.
- Rows without sign-off install as `DEVELOPMENT_ONLY`.
- Add a host API: `rule_table.query(table_kind, jurisdiction, as_of_date)` returning rows + authority locators.
- Add a static audit for jurisdiction branching in Stage 4 modules.
- Extend malicious-pack suite (B-S3-09) with rule-table exploits (dangling chunk IDs, overlapping windows, forged sign-off).

### Deliverables
- Pack tool + installer changes; migration for rule-table storage
- `eval/checks/rule_tables_b_s4_03.py`
- `docs/B-S4-03_rule_tables.md`

### Acceptance gates
- AC-S4-RT.1, AC-S4-RT.2, AC-S4-RT.3.
- B-S3-09 malicious-pack suite remains green with new fixtures added.

### Halt if
- No official pack contains the authority text needed to ground a fixture row — emit QUESTION; do not ground rows on custom packs.

### Done when
Rule data can be installed and queried with provenance, and nothing unsigned can turn GREEN.

---

## B-S4-04 — Citation & Quote Audit engine

| Field | Requirement |
|---|---|
| **Goal** | Audit every citation and quotation in a user draft against installed law. |
| **Depends on** | B-S4-03 |
| **Required reading** | PRD §7.1; `docs/B-S3-14_grounding_verifier.md`; `docs/B-S3-11_hybrid_retrieval.md`; `contracts/citation_record.schema.json`; `src/lib/exportDraft.ts`; `src-tauri/src/proposition.rs` |

### Agent mission
- Deterministic citation extractor (case names, neutral citations, reporter cites, statute/section/rule references) with draft offsets; extractor patterns are pack data where jurisdiction-specific.
- Quote extractor (quoted spans + block quotes) with attribution to the nearest citation.
- Resolve via hybrid retrieval; verify via B-S3-14 exact-substring verifier; pinpoint check against page lineage.
- Classify per PRD §7.1; produce `audit_report` JSON.
- AUDIT surface: list grouped by class, click → draft span and source span side by side; misquote diff view.
- Works for DOCX and PDF inputs and for the Drafting surface directly.

### Deliverables
- `src-tauri/src/audit.rs` (or equivalent); AUDIT view component
- `eval/checks/citation_audit_b_s4_04.py`
- `docs/B-S4-04_citation_audit.md`

### Acceptance gates
- AC-S4-01.1, AC-S4-01.2, AC-S4-01.3 on G-AUDIT (IN and US reported separately).
- Zero model calls in the classification path (static check).

### Halt if
- A citation format in G-AUDIT has no extraction rule in any pack — QUESTION, do not hard-code.

### Done when
A lawyer can see, for every citation and quotation in a draft, exactly what verified and what did not.

---

## B-S4-05 — Audit Certificate export and offline verify

| Field | Requirement |
|---|---|
| **Goal** | Produce a tamper-evident, honestly scoped certificate of the audit. |
| **Depends on** | B-S4-04 |
| **Required reading** | PRD §7.1, §1; `contracts/licence_trust.json`; `docs/B-S1-15_drafting.md`; `docs/B-S3-17_provenance_ux.md` |

### Agent mission
- Per-install Ed25519 key generated in the vault; key model per Q-S4-09.
- Certificate PDF + JSON per `audit_certificate.schema.json`; includes draft hash, pack currency dates, model fingerprint, per-item table, scope statement.
- Optional disclosure paragraph from `DISCLOSURE_TEMPLATE` pack rows; user must edit/confirm before inclusion.
- `caseforge verify <certificate>` CLI and in-app verify for received certificates; fully offline.
- Prohibited-phrase lint on certificate text.

### Deliverables
- Certificate generator + verifier; UI export button
- `eval/checks/audit_certificate_b_s4_05.py`
- `docs/B-S4-05_audit_certificate.md`

### Acceptance gates
- AC-S4-01.4, AC-S4-01.5.
- Certificate generation adds G4 footer and G5 provenance.

### Halt if
- Q-S4-09 unresolved — implement per-install key only and mark institution keys `BLOCKED`.

### Done when
A third party can verify, offline, that a certificate is unaltered and see exactly what it does and does not claim.

---

## B-S4-06 — Opposing Filing intake and scan

| Field | Requirement |
|---|---|
| **Goal** | Turn the opponent's filing into an audited list of weaknesses without contaminating matter facts. |
| **Depends on** | B-S4-04 |
| **Required reading** | PRD §7.2; `docs/B-S3-06_case_graph_v2.md`; `docs/B-S3-07_correction_authority.md`; Stage 3 injection fixtures in `golden/G-INJECTION` |

### Agent mission
- Intake role `EXTERNAL_FILING` with filing party; assertions enter graph only as party-owned `asserts`/`denies`.
- Run B-S4-04 audit over their citations.
- Record-conflict detector: assertion vs counsel-confirmed/document-extracted facts; deterministic for dates/amounts/names, model-proposed otherwise and verified via B-S3-14.
- Unsupported-assertion and new-argument detection against the user's Argument Tree.
- OPPOSITION surface with "add to tree" and "rehearse in Mock Court" actions.
- Leak audit: no external assertion reachable from Digest/Drafting/Mock Court fact sources.

### Deliverables
- Intake, graph, detector, UI changes; migration
- `eval/checks/opposing_scan_b_s4_06.py`
- `docs/B-S4-06_opposing_scan.md`

### Acceptance gates
- AC-S4-02.1 – AC-S4-02.4.

### Halt if
- Graph v2 cannot represent party-owned assertions without schema change not frozen in B-S4-01 — QUESTION.

### Done when
A lawyer loads the other side's brief and gets their bad citations, their record conflicts, and their new arguments — with zero leakage into the lawyer's own facts.

---

## B-S4-07 — Witness resolution and Statement Matrix

| Field | Requirement |
|---|---|
| **Goal** | Build a per-witness, topic-aligned view of every statement on record. |
| **Depends on** | B-S4-03 |
| **Required reading** | PRD §7.3; `contracts/case_graph.schema.json` (`witness`, `statement`, `witness_to_statement`, `alias_of`); `docs/B-S3-07_correction_authority.md` |

### Agent mission
- Witness alias resolution proposals; counsel confirm/split/merge (authority hierarchy).
- Statement-kind vocabulary loaded from pack data (FIR, police statement, magistrate statement, deposition, declaration, examination-in-chief, cross, etc.).
- Locators: page/span for documents; page:line for transcripts.
- Topic segmentation into assertions (who/what/when/where/amount/identification), each with exact excerpt.
- WITNESSES surface: matrix of witness × topic × statement.

### Deliverables
- Matrix builder; WITNESSES view; migration
- `eval/checks/witness_matrix_b_s4_07.py`
- `docs/B-S4-07_witness_matrix.md`

### Acceptance gates
- AC-S4-03.2 for the matrix: every cell has exact excerpt + valid locator.
- Counsel merge/split survives rerun (B-S3-07 survival matrix extended).

### Halt if
- Transcript page:line structure cannot be recovered from G-IMPEACH inputs — QUESTION on supported transcript formats.

### Done when
Every witness's statements are lined up by topic, each cell traceable to its source.

---

## B-S4-08 — Contradiction engine and Impeachment Kit

| Field | Requirement |
|---|---|
| **Goal** | Detect contradictions and omissions across a witness's statements and export a cross-examination kit. |
| **Depends on** | B-S4-07 |
| **Required reading** | PRD §7.3; `docs/B-S3-14_grounding_verifier.md`; `docs/B-S2-02_mock_court.md` (Witness Cross format); `contracts/guardrails.json` |

### Agent mission
- Deterministic layer: date/time/amount/count/place/person/presence/sequence conflicts.
- Model layer: semantic conflict candidates; each must verify with exact excerpts from both statements or is discarded.
- `OMISSION` detection (later statement adds a material fact absent earlier), labelled as counsel-judgement candidate.
- Impeachment Kit export (DOCX/PDF) with G4/G5; suggested confrontation order.
- Feed kit into Mock Court Witness Cross format.
- Red-team: witness-coaching / testimony-alteration prompts refused (G3).

### Deliverables
- Contradiction engine; kit exporter; Mock Court integration
- `eval/checks/impeachment_b_s4_08.py`
- `docs/B-S4-08_impeachment_kit.md`

### Acceptance gates
- AC-S4-03.1, AC-S4-03.3, AC-S4-03.4.

### Halt if
- Owner has not frozen precision/recall thresholds — report measured values, status `BLOCKED`.

### Done when
A lawyer exports a cross-examination kit in which every contradiction is quoted and located on both sides.

---

## Wave A checkpoint

Run all Stage 1–3 gates + B-S4-01…08. Produce `eval/reports/stage4_wave_a.json`. Owner decides Q-S4-08 (ship `v3.1.0` Advocate/Pro with F1–F3, or continue). If shipping, add `docs/RELEASE_CHECKLIST_v3.1.md` and a scoped red-team subset of G-REDTEAM-S4 for F1–F3 with zero G1–G3 violations.

---

# WAVE B — Law currency, India language, collaboration

## B-S4-09 — Code Bridge (statute succession)

| Field | Requirement |
|---|---|
| **Goal** | Annotate every statutory reference with its successor/predecessor and compute the governing code from case dates. |
| **Depends on** | B-S4-03 (and Wave A checkpoint) |
| **Required reading** | PRD §7.4; `STATUTE_SUCCESSION` rule rows in installed packs; `contracts/case_wizard.schema.json` (code/era) |

### Agent mission
- Equivalence classes `IDENTICAL / RENUMBERED / MODIFIED / NEW / OMITTED / UNKNOWN` from rule rows only.
- Temporal routing using rule-table transition rows + case key dates; missing/ambiguous date → `NEEDS_COUNSEL_INPUT` with a guided-interview question.
- Annotations in Digest, Drafting, AUDIT (adds `VERIFIED_WITH_NOTE`), retrieval results.
- `MODIFIED` → side-by-side old/new text from authoritative chunks.

### Deliverables
- Bridge service; UI annotations; interview hook
- `eval/checks/code_bridge_b_s4_09.py`
- `docs/B-S4-09_code_bridge.md`

### Acceptance gates
- AC-S4-04.1 (legal-editor signed), AC-S4-04.2, AC-S4-04.3.

### Halt if
- Signed `STATUTE_SUCCESSION` rows absent — feature renders `UNKNOWN` everywhere; AC `BLOCKED`; QUESTION.

### Done when
No statutory reference renders without its succession status and governing-code basis.

---

## B-S4-10 — Admissibility & Foundation Gaps

| Field | Requirement |
|---|---|
| **Goal** | Surface missing evidentiary foundations deterministically in the Argument Integrity Engine. |
| **Depends on** | B-S4-03 |
| **Required reading** | PRD §7.5; `docs/B-S2-01_integrity.md`; `contracts/argument_node.schema.json`; `ADMISSIBILITY_REQUIREMENT` rows |

### Agent mission
- Evidence-type classification of evidence/exhibit nodes (counsel-confirmable).
- Foundation element status `PRESENT / ABSENT / UNKNOWN` with locators.
- `caseforge.integrity.v2` scorer: foundation factor into Evidence link; pure deterministic; v1 retained and selectable for old sessions.
- Missing Links panel shows rule row + authority.

### Deliverables
- integrity.v2; migration; UI
- `eval/checks/foundation_b_s4_10.py`
- `docs/B-S4-10_foundation_gaps.md`

### Acceptance gates
- AC-S4-05.1 – AC-S4-05.3; AC-8.1–8.3 still pass under v1 and v2.

### Halt if
- Evidence-type vocabulary not in pack data — QUESTION.

### Done when
"This electronic record has no certificate on file" (or its US equivalent) appears as a cited Missing Link.

---

## B-S4-11 — Deadline & Limitation Watch

| Field | Requirement |
|---|---|
| **Goal** | Compute deadlines from the verified chronology with full reasoning shown and VERIFY labels. |
| **Depends on** | B-S4-03 |
| **Required reading** | PRD §7.6, G9; `LIMITATION_PERIOD`, `PROCEDURAL_DEADLINE`, `COURT_CALENDAR` rows; Digest chronology code |

### Agent mission
- Deterministic date engine: periods (days/months/years), inclusive/exclusive rules, holiday/vacation roll-over, exclusions only when the triggering fact exists in the graph.
- Uncertain inputs → date range + reason.
- DEADLINES surface; local reminders; `.ics` export (no network).
- G9 enforcement: every deadline carries rule + authority + trigger locator + VERIFY.

### Deliverables
- Date engine; DEADLINES view; ICS exporter
- `eval/checks/deadlines_b_s4_11.py`
- `docs/B-S4-11_deadlines.md`

### Acceptance gates
- AC-S4-06.1 (legal-editor signed), AC-S4-06.2, AC-S4-06.3.
- Property tests: leap years, month-end, consecutive holidays.

### Halt if
- Court calendar for a fixture court/year missing — range output only; QUESTION Q-S4-11.

### Done when
Every deadline shown can be re-derived by a lawyer from the displayed rule, trigger, and calendar.

---

## B-S4-12 — Timed Oral Rehearsal

| Field | Requirement |
|---|---|
| **Goal** | Rehearse argument under a real time limit with grounded bench interruptions and a timeline debrief. |
| **Depends on** | Wave A checkpoint |
| **Required reading** | PRD §7.7; `docs/B-S3-16_grounded_mock_court.md`; `docs/B-S3-18_debrief.md`; `contracts/adaptive_bench.json` |

### Agent mission
- Timer mode with presets; interruption-rate control mapped to bench archetypes.
- Interruptions sourced from Missing Links / weakest limbs via grounded path.
- Debrief v2 timeline extension (schema bump frozen in B-S4-01).
- Works in text and voice (Full tier).

### Deliverables
- Mock Court timer; debrief timeline; UI
- `eval/checks/timed_rehearsal_b_s4_12.py`
- `docs/B-S4-12_timed_rehearsal.md`

### Acceptance gates
- AC-S4-07.1 – AC-S4-07.3; AC-3.3 latency budget still met.

### Halt if
- Turn latency makes short presets unusable on the reference machine — report and tier-gate, do not fake.

### Done when
A lawyer sees which limbs they never reached in a 5-minute slot.

---

## B-S4-13 — Law Currency Stamp, Treatment Graph, delta packs

| Field | Requirement |
|---|---|
| **Goal** | Show how current every authority is and what installed judgments say about it. |
| **Depends on** | B-S4-03 |
| **Required reading** | PRD §7.8; `docs/B-S3-10_first_legal_repository.md`; `scripts/legal_pack_tool.py`; citation rendering components |

### Agent mission
- Currency date on every citation render (shared component).
- Pack-build treatment extraction → `TREATMENT_EDGE` with exact citing paragraph; verified as derived content.
- Treatment shown as notes; never changes supersession status; never "good law".
- Delta pack pipeline (build, sign, install, rollback) at cadence per Q-S4-05.
- UI-copy lint for prohibited phrases.

### Deliverables
- Treatment extractor in pack tool; delta pipeline; UI
- `eval/checks/treatment_b_s4_13.py`
- `docs/B-S4-13_currency_treatment.md`

### Acceptance gates
- AC-S4-08.1 – AC-S4-08.3; B-S3-09 suite green for delta packs.

### Halt if
- Delta distribution channel undecided — build/install from file only; QUESTION.

### Done when
No citation renders without a currency date, and an overruling in the corpus is visible where it matters.

---

## B-S4-14 — Devanagari triage and OCR

| Field | Requirement |
|---|---|
| **Goal** | Read Hindi documents offline with calibrated confidence. |
| **Depends on** | Wave A checkpoint |
| **Required reading** | PRD §7.9; `sidecars/docpipe/*`; `docs/B-S3-04_ocr.md`; `docs/B-S3-05_confidence.md`; `contracts/ocr_config.json` |

### Agent mission
- Script detection per page/region in triage.
- Model bake-off (Q-S4-04): accuracy on G-INDIC, licence for offline commercial use, size, RAM under choreography.
- Integrate chosen engine; calibrate confidence; critical-field validation for Devanagari digits and dates.
- Size budget (`scripts/check_size_budget.py`) respected or explicitly raised by owner.

### Deliverables
- docpipe changes; OCR config bump; bake-off report `eval/bakeoff/indic_ocr_b_s4_14.json`
- `eval/checks/indic_ocr_b_s4_14.py`
- `docs/B-S4-14_indic_ocr.md`

### Acceptance gates
- AC-S4-09.1 at owner-frozen CER; calibration metrics per B-S3-05 method.

### Halt if
- No candidate model has a licence permitting offline commercial redistribution — QUESTION.

### Done when
A Hindi FIR scan yields text with honest per-word confidence.

---

## B-S4-15 — Aligned offline translation and G10

| Field | Requirement |
|---|---|
| **Goal** | Translate Hindi segments to English with alignment, without letting translation masquerade as verified source. |
| **Depends on** | B-S4-14 |
| **Required reading** | PRD §7.9, G10; `contracts/translation_segment.schema.json`; `docs/B-S3-14_grounding_verifier.md`; `docs/B-S1-20_choreography.md` |

### Agent mission
- Translation model under one-large-model choreography; never concurrent with primary reasoner.
- Segment alignment stored; side-by-side UI.
- `TRANSLATED` provenance on extracted facts; critical fields require counsel confirmation.
- Verifier: translated text can never be a verified quotation (G10).
- Overnight batch support (resumable).

### Deliverables
- Translation sidecar capability; UI; verifier rule
- `eval/checks/translation_b_s4_15.py`
- `docs/B-S4-15_translation.md`

### Acceptance gates
- AC-S4-09.2, AC-S4-09.3; AC-4.2 RSS ceiling holds.

### Halt if
- Translation model would exceed RAM budget on Full tier — QUESTION (tier-gate options).

### Done when
A lawyer reads the Hindi original and English side by side, and nothing translated is passed off as verbatim.

---

## B-S4-16 — Encrypted Bundle Exchange

| Field | Requirement |
|---|---|
| **Goal** | Let seniors, juniors, and co-counsel exchange case work offline, encrypted, with provenance. |
| **Depends on** | Wave A checkpoint |
| **Required reading** | PRD §8; `docs/B-S1-11_phone_capture.md`; `docs/B-S3-07_correction_authority.md`; `contracts/bundle_manifest.schema.json` |

### Agent mission
- Recipient key exchange via QR/file; bundle encryption + sender signature.
- Content selection and irreversible redaction (byte-level absence check).
- USB file and LAN transfer (Companion LAN rules).
- Import with attribution; authority-hierarchy merge; never overwrite recipient corrections.

### Deliverables
- Bundle writer/reader; UI; migration
- `eval/checks/bundle_b_s4_16.py`
- `docs/B-S4-16_bundles.md`

### Acceptance gates
- AC-S4-10.1 – AC-S4-10.4.

### Halt if
- Any design requires a relay/server — HALT; out of scope.

### Done when
A junior can hand a senior a complete, encrypted, attributable case bundle on a USB stick.

---

## Wave B checkpoint

All Stage 1–3 gates + B-S4-01…16 green. Report `eval/reports/stage4_wave_b.json`.

---

# WAVE C — Chambers Edition

## B-S4-17 — Chambers Edition shell and Record-only mode (G7)

| Field | Requirement |
|---|---|
| **Goal** | Ship a judge-facing edition whose every output is confined to the record and installed law. |
| **Depends on** | Wave B checkpoint |
| **Required reading** | PRD §9, §9.1, G7; `docs/B-S2-05_bench_prep.md`; `src/lib/lawyerWorkflow.ts`; `contracts/licence_payload.schema.json` |

### Agent mission
- `JUDICIARY` entitlement gates CHAMBERS surface; hides advocacy-only controls.
- Record-only classifier on every generated sentence: `RECORD / LAW / INFERENCE_FROM_RECORD / EXTRA_RECORD`; block `EXTRA_RECORD`.
- Lock record-only on once a scaffold exists.
- Force air-gap and zero telemetry in Judiciary edition.
- Pilot constraints from Q-S4-10 captured as configuration, not code.

### Deliverables
- Edition gating; G7 enforcement; CHAMBERS shell
- `eval/checks/chambers_record_only_b_s4_17.py`
- `docs/B-S4-17_chambers_record_only.md`

### Acceptance gates
- AC-S4-11.1, AC-S4-11.2.

### Halt if
- Pilot court's AI policy forbids a capability in this batch — QUESTION; disable, do not work around.

### Done when
In Chambers Edition nothing from outside the record can render as fact.

---

## B-S4-18 — Issue Matrix

| Field | Requirement |
|---|---|
| **Goal** | Give the judge a neutral, side-by-side view of each issue with admitted, disputed, and unanswered points. |
| **Depends on** | B-S4-17, B-S4-06 |
| **Required reading** | PRD §9.2; graph `issue`, `asserts`, `denies`, `argument` structures; `ISSUE_FRAMING_TEMPLATE` rows |

### Agent mission
- Proposed issues from pleadings + templates; judge accept/edit/merge/reject (judge-accepted only proceed).
- Per issue: positions, evidence, authorities with audit status, admitted vs disputed facts, unanswered arguments (deterministic query).
- Neutral rendering; no ranking, scoring, or suggested answer.
- Export as bench memo with G4/G5.

### Deliverables
- Issue matrix service; UI; exporter
- `eval/checks/issue_matrix_b_s4_18.py`
- `docs/B-S4-18_issue_matrix.md`

### Acceptance gates
- AC-S4-12.1 – AC-S4-12.3.

### Halt if
- Party-owned assertions from B-S4-06 cannot represent both parties symmetrically — QUESTION.

### Done when
A judge sees, per issue, what is agreed, what is disputed, and what one side never answered.

---

## B-S4-19 — Both-sides citation check and record lookup

| Field | Requirement |
|---|---|
| **Goal** | Audit every party's citations in one action and let the judge find anything in the record by question. |
| **Depends on** | B-S4-17, B-S4-04 |
| **Required reading** | PRD §9.3; `docs/B-S3-11_hybrid_retrieval.md`; `docs/B-S3-20_voice.md` |

### Agent mission
- Batch F1 audit over all filings grouped by party.
- Record lookup over matter index only; returns ranked excerpts + locators; no generated prose (static check).
- Voice query via existing offline STT staging path.

### Deliverables
- Batch audit; lookup service; UI
- `eval/checks/chambers_lookup_b_s4_19.py`
- `docs/B-S4-19_chambers_lookup.md`

### Acceptance gates
- AC-S4-13.1, AC-S4-13.2.

### Halt if
- Lookup quality below threshold with no index fix — report, `BLOCKED`.

### Done when
"Where in the record…?" is answered with excerpts and page references, nothing else.

---

## B-S4-20 — Daily Cause-List Brief

| Field | Requirement |
|---|---|
| **Goal** | Produce a one-page, record-cited brief for every matter on today's list. |
| **Depends on** | B-S4-17 |
| **Required reading** | PRD §9.4; graph `order`, `procedure`, `hearing`, `application` nodes |

### Agent mission
- Cause-list import (PDF/CSV/manual); matching to local matters with confirmation for ambiguous matches.
- Per-matter brief fields per PRD; each cites a locator or says "not found in record".
- Adjournment count and requesting party only from orders on record.
- Batch render within budget.

### Deliverables
- Importer; brief generator; print/export
- `eval/checks/cause_list_b_s4_20.py`
- `docs/B-S4-20_cause_list.md`

### Acceptance gates
- AC-S4-14.1, AC-S4-14.2.

### Halt if
- Pilot court's cause-list format unavailable — use G-CAUSELIST fixtures; QUESTION for real format.

### Done when
A judge prints one page per listed matter, every line traceable to an order or document.

---

## B-S4-21 — Judgment Scaffold and Decision-Free Zone (G8)

| Field | Requirement |
|---|---|
| **Goal** | Pre-fill the record-based parts of a judgment and make the decision parts structurally impossible to generate. |
| **Depends on** | B-S4-18 |
| **Required reading** | PRD §9.5, G8; `docs/B-S1-15_drafting.md`; `contracts/judgment_scaffold.schema.json` |

### Agent mission
- Scaffold sections per PRD; pre-fill only permitted sections, record-only, every sentence located.
- Analysis/findings/operative order: no model invocation path exists (enforced in host, not just UI); refuse prompts (G8).
- Allowed assistance on judge-typed text: spellcheck, formatting, F1 citation audit.
- Export with machine-assisted-section footer.
- Chambers red-team set (≥ owner-frozen count).

### Deliverables
- Scaffold editor; host-level lock; exporter
- `eval/checks/judgment_scaffold_b_s4_21.py`
- `docs/B-S4-21_judgment_scaffold.md`

### Acceptance gates
- AC-S4-15.1 – AC-S4-15.3.

### Halt if
- Any surface could route locked-section text to a model — BLOCKER; HALT until removed.

### Done when
The judge's reasoning and decision are written only by the judge, and the tool can prove it.

---

## Wave C checkpoint

All prior gates + B-S4-17…21 green. Report `eval/reports/stage4_wave_c.json`. Pilot-readiness note for Q-S4-10 court.

---

# SHIP

## B-S4-22 — Editions, pack subscription, seat transfer

| Field | Requirement |
|---|---|
| **Goal** | Enforce Stage 4 editions offline and separate app licence from pack subscription without ever ransoming data. |
| **Depends on** | Wave C checkpoint |
| **Required reading** | PRD §10; `docs/B-S1-18_licence.md`; `contracts/licence_payload.schema.json`; `eval/tools/make_licence.py` |

### Agent mission
- Edition entitlements for every Stage 4 surface (single entitlement map in contracts).
- `pack_subscription_until` handling: installed packs usable at last currency date with "updates expired" notice.
- Offline seat transfer with per-year limit (Q-S4-07).
- Judiciary: forced air-gap, zero telemetry.
- Licence tooling updates for all editions.

### Deliverables
- Licence changes; entitlement map; UI notices
- `eval/checks/editions_b_s4_22.py`
- `docs/B-S4-22_editions.md`

### Acceptance gates
- AC-S4-16.1, AC-S4-16.2; Stage 3 AC-S3-22.8 still holds.

### Halt if
- Pricing/edition boundaries undecided (Q-S4-06) — implement mechanism with placeholder entitlement map marked `DEVELOPMENT_ONLY`; QUESTION.

### Done when
Each edition shows exactly what it paid for, offline, and expiry never locks a user out of their own work.

---

## B-S4-23 — Stage 4 red-team and cumulative ship gate

| Field | Requirement |
|---|---|
| **Goal** | Prove Stage 4 is safe and complete, cumulatively with Stages 1–3. |
| **Depends on** | B-S4-01 … B-S4-22 |
| **Required reading** | All Stage 4 reports; `docs/B-S3-22_ship_gate.md`; `eval/run_stage3.py` |

### Agent mission
- Full G-REDTEAM-S4 suite (≥ owner-frozen count) across lawyer and Chambers surfaces.
- One command `npm run ship:stage4`: production build, Rust tests, all Stage 1–4 gates, contract/migration validation, legal-editor sign-off presence, network capture, size budget, signed readiness report.
- End-to-end controlled matters: one IN (with Hindi document and code-bridge transition) and one US (with deposition impeachment and deadline), plus one Chambers session.
- Verdicts allowed: `NOT READY` · `READY FOR CONTROLLED TESTING` · `READY FOR LIMITED REAL-CASE INTERNAL TESTING` (external beta requires separate audit, as Stage 3).

### Deliverables
- `eval/checks/ship_gate_b_s4_23.py`; `eval/run_stage4.py`
- `eval/reports/stage4_ship_gate.json`
- `docs/RELEASE_CHECKLIST_v4.0.md`; `docs/B-S4-23_ship_gate.md`
- `CHANGELOG.md` entry; `package.json` scripts `test:s4:*`, `ship:stage4`

### Acceptance gates
- AC-S4-RT-RED: zero G1, G2, G3, G7, G8 violations.
- Every AC-S4-* PASS with zero required skips; every legal-answer AC carries editor sign-off.
- AC-13.1 / AC-S3-22.2 network audits pass across Stage 4 workflows.

### Halt if
- Any earlier batch not PASS, any sign-off missing, any waiver on G1/G7/G8 — gate returns `NOT READY`; no automatic waivers.

### Done when
The answer is evidence-backed **yes** to:

> Can a lawyer rely on CaseForge to audit a filing, attack the other side's filing, and impeach a witness — and can a judge rely on it to organise the record — with every claim provable and no decision ever made by the machine?

---

## 3. Cumulative completion matrix

| Batch | Required evidence | Status |
|---|---|---|
| B-S4-01 | Contracts + inventory + threshold approval | IMPLEMENTED — contracts frozen; formal PASS blocked on Q-S4-01 threshold approval (2026-10-08) |
| B-S4-00b | Answer-feedback golden + Rust tests + UI | IMPLEMENTED — 5/5 gates PASS; Rust 85/85 (2026-10-08) |
| B-S4-00 | Base install ≤ 4 GiB + default-model fitness | PARTIAL — tier infra, honest size gate, 7B requalified on Ollama 0.35.1; qwen3:4b rejected, qwen3:4b-instruct HALT (critical prompt-injection fail); base install 5.08 GiB still over cap (2026-10-08) |
| B-S4-02 | Twelve golden sets + editor protocol | NOT STARTED |
| B-S4-03 | Rule-table install/query + exploit suite | NOT STARTED |
| B-S4-04 | G-AUDIT report (IN/US) | IMPLEMENTED — AC-S4-01.1/01.2 PASS (Rust 88/88); AC-S4-01.3 BLOCKED on Q-S4-01 threshold (measured 10/10 on G-AUDIT) (2026-10-08) |
| B-S4-05 | Certificate verify/tamper report | IMPLEMENTED — AC-S4-01.4/01.5 PASS; Rust 93/93; independent stdlib verifier (2026-10-08) |
| B-S4-06 | G-OPPOSING report + leak audit | IMPLEMENTED — AC-S4-02.1/02.3/02.4 PASS; AC-S4-02.2 BLOCKED on Q-S4-01 threshold (measured 3/3 on G-OPPOSING); Rust 98/98; leak barrier fail-closed (2026-10-08) |
| B-S4-07 | Matrix locator report | NOT STARTED |
| B-S4-08 | G-IMPEACH precision/recall + G3 red-team | NOT STARTED |
| B-S4-09 | G-CODEBRIDGE (signed) | NOT STARTED |
| B-S4-10 | G-ADMISS + integrity v1/v2 parity | NOT STARTED |
| B-S4-11 | G-DEADLINE (signed) + property tests | NOT STARTED |
| B-S4-12 | Timer/debrief timeline + latency | NOT STARTED |
| B-S4-13 | G-TREATMENT + delta pack suite | NOT STARTED |
| B-S4-14 | Indic OCR bake-off + CER | NOT STARTED |
| B-S4-15 | Translation critical-field + G10 | NOT STARTED |
| B-S4-16 | Bundle crypto/redaction/network | NOT STARTED |
| B-S4-17 | G7 record-only suite | NOT STARTED |
| B-S4-18 | G-CHAMBERS issue matrix | NOT STARTED |
| B-S4-19 | Lookup top-5 + no-prose check | NOT STARTED |
| B-S4-20 | G-CAUSELIST + render budget | NOT STARTED |
| B-S4-21 | G8 static + red-team | NOT STARTED |
| B-S4-22 | Edition/expiry matrix | NOT STARTED |
| B-S4-23 | Cumulative ship report | NOT STARTED |

---

## 4. Work explicitly deferred beyond Stage 4

- Indian scripts other than Devanagari; non-English reasoning or Mock Court
- Any online court portal, e-filing, docket, or cause-list scraping
- Cloud sync, relay servers, multi-user live editing
- Named-judge analytics; party scoring; outcome, bail, or sentence recommendation
- Treatment data licensed from commercial citators
- Mobile full application
- Nationwide India / all-US-state breadth before the narrow signed rule tables are proven
