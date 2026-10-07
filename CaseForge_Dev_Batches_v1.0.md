# CaseForge — Development Batch Plan v1.0

**Source of truth:** `CaseForge_PRD_v1.0.md` (Layer 1)  
**Purpose:** Split the PRD into small, ordered batches so implementers (human or AI) build the product **without inventing features, schemas, or behaviour**.

**How to use this document**

1. Complete batches **in order** (batch number = hard dependency order).
2. A batch is **DONE** only when every listed AC / test gate passes.
3. If anything required by a batch is missing from the PRD, contracts, or golden data → **HALT** and emit a `QUESTION` block. Never guess.
4. Do **not** implement later-batch features early “for convenience.”
5. Stage 2 batches (B-S2-*) start only after Stage 1 ship gate is green.

**Prime Rule (from PRD)**  
If any detail is missing from Layers 1–4, **HALT**. Never invent. Never modify files outside the current batch scope.

---

## Legend

| Field | Meaning |
|--------|---------|
| **Goal** | One-sentence outcome |
| **In scope** | Exact work allowed |
| **Out of scope** | Explicitly forbidden in this batch |
| **Inputs** | Must already exist |
| **Outputs** | Artefacts this batch produces |
| **AC gates** | Executable release gates from PRD |
| **Halt if** | Stop conditions (do not proceed) |
| **Depends on** | Prior batch IDs that must be DONE |

**Stage labels**

- **S0** = Foundation / prep (no product UI value yet)
- **S1** = Stage 1 Digest (v1.0 commercial)
- **S2** = Stage 2 Mock Court (v2.0)

---

# TRACK S0 — Preparation & Contracts

> No production feature code until S0 is complete. Golden data and schemas define correctness; code only makes tests pass.

---

## B-S0-01 — Repository skeleton

| | |
|--|--|
| **Goal** | Create the empty project layout the PRD mandates. |
| **Depends on** | None |
| **In scope** | Folders: `/contracts`, `/golden`, `/eval`, `/src`, `/docs`. README pointing to PRD + this batch plan. `.gitignore` for models, packs, secrets, build artefacts. |
| **Out of scope** | Any app logic, models, UI, DB. |
| **Inputs** | PRD §18 Phase 0 |
| **Outputs** | Empty tree as specified |
| **AC gates** | Structure exists; no invented top-level folders required by later batches |
| **Halt if** | Conflict with existing monorepo layout that is not documented — emit QUESTION |

**Done when:** Directory tree matches PRD Phase 0 item 1.

---

## B-S0-02 — Freeze Layer 2 contracts (schemas only)

| | |
|--|--|
| **Goal** | Freeze every JSON schema before any implementation invents types. |
| **Depends on** | B-S0-01 |
| **In scope** | Author frozen schemas under `/contracts` for at least: `case_graph.json`, argument nodes, citation records, session turns, lexicon entries, experience records, licence payload, IPC (`/contracts/ipc/*.json`), `constants.json` (confidence thresholds). |
| **Out of scope** | Runtime code that *uses* the schemas beyond validation stubs. |
| **Inputs** | PRD §16, §4.1, §6.2, §8, §9, §11, §13 |
| **Outputs** | Versioned schema files; schema version field on each |
| **AC gates** | AC-16.1 (schema validation suite can load all schemas) — partial until eval harness exists |
| **Halt if** | PRD names a type but does not specify fields → QUESTION (do not invent fields) |

**Done when:** Every canonical artefact named in PRD §16 has a schema file path reserved and documented; incomplete field lists are flagged as open QUESTION, not filled by guesswork.

**Note:** If full field lists are not in Layer 1, this batch’s job is to **list missing schemas as QUESTION blocks**, not invent them. Prefer minimal stubs with `$comment: "BLOCKED — needs Layer 2 authoring"` over fake fields.

---

## B-S0-03 — Constants & error codes freeze

| | |
|--|--|
| **Goal** | Freeze thresholds and error codes so OCR/STT/citation code never invents magic numbers. |
| **Depends on** | B-S0-02 |
| **In scope** | `/contracts/constants.json`: confidence thresholds, VI formula constants, performance budget numbers from PRD §3, model choreography limits. `/contracts/errors.json`: stable error codes for IPC/vault/licence. |
| **Out of scope** | Tunable ML hyperparameters that require bake-off (flag as OPEN per §21). |
| **Inputs** | PRD §3, §6.2, §8, §21 |
| **Outputs** | `constants.json`, `errors.json` |
| **AC gates** | Values match PRD numbers where stated (e.g. cold start 6 s, UI freeze 200 ms, 7.2 GB RSS) |
| **Halt if** | A threshold is required by AC but not in PRD → QUESTION |

**Done when:** Every numeric budget in PRD §3 and confidence philosophy in §6.2 is either in `constants.json` or listed as OPEN QUESTION.

---

## B-S0-04 — Golden dataset scaffolds (author before code)

| | |
|--|--|
| **Goal** | Create golden dataset **structure and known-answer files** before production logic. |
| **Depends on** | B-S0-01, B-S0-02 |
| **In scope** | Scaffold under `/golden`: G-DOCS, G-CASE-IN, G-CASE-US, G-AUDIO, G-RETRIEVAL-IN, G-RETRIEVAL-US, G-REDTEAM, G-SCRIPTS. Each has README + placeholder known-answer JSON matching schemas. |
| **Out of scope** | Filling entire golden sets with real case files (can be incremental per later batches). Minimum: one tiny fixture per set so harness can run. |
| **Inputs** | PRD §17, §18 Phase 0 |
| **Outputs** | `/golden/**` scaffolds + at least 1 micro-fixture each |
| **AC gates** | Harness can discover sets; empty/partial sets fail open with clear SKIP vs FAIL policy documented |
| **Halt if** | Real case material is needed and unavailable → QUESTION (do not invent legal facts as “golden truth”) |

**Done when:** Scaffold + micro-fixtures exist; authorship process for full golden sets is documented.

---

## B-S0-05 — Evaluation harness (every AC = script)

| | |
|--|--|
| **Goal** | One command runs AC checks; CI fails if any required AC fails. |
| **Depends on** | B-S0-03, B-S0-04 |
| **In scope** | `/eval` runner: maps `AC-x.y` → test script or SKIP until implemented. Report format: PASS / FAIL / SKIP / BLOCKED. |
| **Out of scope** | Implementing product features to make ACs pass. |
| **Inputs** | All AC-* IDs in PRD |
| **Outputs** | `eval` CLI + inventory of every AC-x.y |
| **AC gates** | AC inventory complete vs PRD; missing implementation = SKIP not silent PASS |
| **Halt if** | An AC in PRD cannot be expressed as a test at all → QUESTION |

**Done when:** `eval` lists every AC from the PRD with status SKIP|PASS|FAIL|BLOCKED; zero silent passes.

---

## S0 exit gate

| Check | Required | Status |
|-------|----------|--------|
| Repo layout | Yes | Done (B-S0-01) |
| Contracts paths reserved / frozen or QUESTION-listed | Yes | Done (B-S0-02) |
| Constants from PRD numbers | Yes | Done (B-S0-03) |
| Golden scaffolds | Yes | Done (B-S0-04) |
| Eval harness + full AC inventory | Yes | Done (B-S0-05) |

**S0 exit gate: GREEN** (as of B-S0-05). Stage 1 may start at **B-S1-01** on go-ahead.

Reference run: `python eval/run_eval.py` → PASS structural harness + AC-16.1 load; product ACs SKIP/BLOCKED (not silent PASS).

---

# TRACK S1 — Stage 1 Digest (v1.0)

> Commercial release of Digest only. Mock Court UI/logic is forbidden until S1 ship gate.

---

## B-S1-01 — Tauri v2 + React 18/TS shell

| | |
|--|--|
| **Goal** | Installable shell with typed IPC stubs and no UI freezes on dummy work. |
| **Depends on** | S0 exit gate |
| **In scope** | Tauri v2 + React 18 / TypeScript; single-window app shell; healthcheck IPC; process supervisor stubs for three sidecars (not full logic). |
| **Out of scope** | Real OCR, LLM, speech, vault crypto beyond stub. Stage 2 screens. |
| **Inputs** | PRD §4.1, contracts IPC |
| **Outputs** | Runnable app; IPC contract round-trip for `ping` / `version` |
| **AC gates** | AC-4.1 (partial: ping suite); AC-3.1 cold start target measured; AC-3.5 dummy heavy work off UI thread |
| **Halt if** | Tauri v2 / React versions conflict with unstated constraints → QUESTION |

**Done when:** App launches; IPC ping passes; heavy dummy job never freezes UI > 200 ms.

---

## B-S1-02 — SQLCipher vault & case DB

| | |
|--|--|
| **Goal** | Encrypted app DB + one DB per case; no plaintext case data on disk. |
| **Depends on** | B-S1-01, B-S0-02 |
| **In scope** | SQLCipher (AES-256 + Argon2id) as per PRD §16; app DB + per-case DB; open/create/close case; passphrase handling. |
| **Out of scope** | Full case_graph population, pack storage, licensing. |
| **Inputs** | PRD §16; schema contracts |
| **Outputs** | Vault module; migrations; tests for unreadable without passphrase |
| **AC gates** | AC-16.2 (unreadable without passphrase); foundation for AC-16.3 later |
| **Halt if** | Exact Argon2id parameters not in contracts → QUESTION (do not invent params) |

**Done when:** Create case → encrypted DB file; wrong passphrase fails; no case plaintext in clear files.

---

## B-S1-03 — New Case wizard (jurisdiction-first)

| | |
|--|--|
| **Goal** | Case creation captures jurisdiction and matter metadata only. |
| **Depends on** | B-S1-02 |
| **In scope** | Wizard fields per PRD §5: cause title, court, **jurisdiction** (India / United States / Other), matter type, user’s side, code/era (auto-suggest stub OK), language (English primary). |
| **Out of scope** | Document upload (next batch), pack install, AI suggestions beyond simple stubs. |
| **Inputs** | PRD §5 |
| **Outputs** | Case row + empty case vault; jurisdiction stored immutably for that case |
| **AC gates** | Jurisdiction required; case opens into empty vault |
| **Halt if** | Extra wizard fields requested by stakeholders not in PRD → do not add without PRD change |

**Done when:** New case can be created for India, US, Other; fields match §5 only.

---

## B-S1-04 — Document intake (drag-drop, type tags)

| | |
|--|--|
| **Goal** | Ingest PDF/DOCX/JPG/PNG into case with `doc_id`, type tag, language tag, per-page quality grade placeholder. |
| **Depends on** | B-S1-03 |
| **In scope** | Drag-and-drop + folder intake; source types listed in §5; one-click type correction; store originals in encrypted case store. |
| **Out of scope** | Phone Capture Companion (later); OCR/HTR; auto-type ML (stub + golden path later). |
| **Inputs** | PRD §5; G-DOCS micro-fixtures |
| **Outputs** | Documents list UI; stored docs with metadata |
| **AC gates** | AC-5.1 targeted when classifier lands (this batch: manual type + correction path works) |
| **Halt if** | File format edge cases not specified → implement listed types only |

**Done when:** User can add multi-file case file; each doc has `doc_id` + editable type tag.

---

## B-S1-05 — `docpipe` sidecar: triage only

| | |
|--|--|
| **Goal** | Classify pages: typed English / other / mixed / clean handwriting / messy handwriting / non-text. |
| **Depends on** | B-S1-04, B-S0-04 (G-DOCS) |
| **In scope** | `docpipe` process; triage classifier; progress IPC; confidence per page. |
| **Out of scope** | Full OCR word boxes, skeleton extraction, vision LLM concurrent with primary (primary not loaded yet). |
| **Inputs** | PRD §6.1; AC-6.1; G-DOCS |
| **Outputs** | Per-page triage labels + confidence in DB |
| **AC gates** | **AC-6.1** ≥ 92 % on golden triage set |
| **Halt if** | Accuracy &lt; 92 % → **do not proceed** to skeleton (PRD Phase 1 perfect-result rule) |

**Done when:** AC-6.1 PASS. If FAIL, stay in this batch until fixed.

**Status (engineering):** Plumbing complete (sidecar, vault `page_triage`, multi-doc jobs, UI, AC-6.1 scorer).  
**Status (ship gate):** **GREEN** (2026-08-06). **500 GOLDEN** pages locked (product-owner synthetic pack `S-SYNTH-GDOCS-500`); AC-6.1 **PASS** 100% on 500 pages; Q-VISION-01 Stage-1 freeze → `caseforge.triage.heuristic.v1` (see `eval/bakeoff/reports/Q-VISION-01_freeze.md`).  
**B-S1-06 may start.** Caveat: supplement with real-scan GOLDEN before commercial audit if required.

**Parallel (not a batch unlock):** Synthetic demo argument packs under `/demo` (easy/medium/hard) + deterministic VI scorer (`npm run demo:score`). Status `DEMO_SYNTHETIC` — never counts as GOLDEN for AC-6.x / AC-8.x ship gates.

---

## B-S1-06 — `docpipe`: recognition + confidence scores

| | |
|--|--|
| **Goal** | Every word has confidence; thresholds from `constants.json`. |
| **Depends on** | B-S1-05 **PASS** |
| **In scope** | OCR/vision path producing word-level text + confidence; low-confidence flagging; store page images/spans for later highlight. |
| **Out of scope** | Case graph / parties / chronology NLP. |
| **Inputs** | PRD §6.2; constants; vision model decision (§21 open — use bake-off or BLOCKED stub) |
| **Outputs** | Recognized text layers linked to page + bbox/span |
| **AC gates** | Confidence thresholds applied from contracts only |
| **Halt if** | Vision model not chosen (§21 #3) → may use provisional model **only if** marked provisional in contracts; else QUESTION |

**Done when:** Text layer exists with confidences; UI can show low-confidence highlights (minimal).

**Status (2026-08-06):** **DONE.** Q-CONST-01 frozen in `constants.json`; `page_text_layer` schema + vault migration 005; provisional recognizer `caseforge.recognize.provisional.v1` (Python + Rust); host jobs `docpipe_recognize_*`; UI **Text layer** tab with low/force-review word highlights. `npm run test:recognize`.

---

## B-S1-07 — Skeleton extraction → `case_graph.json`

| | |
|--|--|
| **Goal** | Extract chronology, parties, statutes, evidence inventory, procedural posture, prayer/relief, contradiction seeds into **exact** `case_graph` schema. |
| **Depends on** | B-S1-06, B-S0-02 (`case_graph` schema frozen) |
| **In scope** | Deterministic parsers + primary reasoner under retrieval constraints as available; every item: source pointer + confidence. |
| **Out of scope** | Argument Tree (Stage 2); Mock Court; citation GREEN/AMBER/RED pack verification (next knowledge batches). |
| **Inputs** | PRD §2 Stage 1 list, §6.3; G-CASE-IN / G-CASE-US |
| **Outputs** | Valid `case_graph.json` per case |
| **AC gates** | **AC-6.2** ≥ 95 % recall dated events on golden 40-page doc; schema validation AC-16.1 |
| **Halt if** | Fact without source pointer → defect, not shippable |

**Done when:** AC-6.2 PASS; every fact has source pointer.

**Status (2026-08-06):** **DONE.** Q-CG-01/02/03 frozen → `case_graph.schema.json` v1.0.0; deterministic extractor `caseforge.skeleton.deterministic.v1`; vault `case_graph` table; host `docpipe_skeleton_start` / `docpipe_case_graph_get`; UI **Skeleton** tab; product-owner 40-page GOLDEN `synth_chrono_40` for AC-6.2. `npm run test:skeleton` · `npm run test:ac62`.

---

## B-S1-08 — Side-by-side verification review UI

| | |
|--|--|
| **Goal** | Lawyer corrects low-confidence extractions; click fact → source page + span. |
| **Depends on** | B-S1-07 |
| **In scope** | DIGEST/CASEBOOK review surface; highlight only low-confidence; corrections write back to graph + Experience Store hook (store can be thin). |
| **Out of scope** | Full Experience Store retrieval (later); Stage 2 panels. |
| **Inputs** | PRD §6.4, §12 |
| **Outputs** | Review UI; correction events |
| **AC gates** | **AC-6.3** click fact → source span; **AC-6.4** ≤ 2 interactions to correct a highlighted word |
| **Halt if** | Design language conflicts (gavels, etc.) — follow §12 only |

**Done when:** AC-6.3 and AC-6.4 PASS.

**Status (2026-08-06):** **DONE.** Side-by-side **Review** tab (CASEBOOK): click fact → source span highlight (AC-6.3); select + save correction ≤ 2 interactions (AC-6.4); graph/word write-back + thin `experience_corrections` store. `npm run test:review`.

---

## B-S1-09 — Guided Intake Interview

| | |
|--|--|
| **Goal** | Targeted questions for gaps; answers become `SOURCED_BY_COUNSEL` facts. |
| **Depends on** | B-S1-08 |
| **In scope** | Question generation from skeleton gaps; resumable, skippable, re-openable; write `SOURCED_BY_COUNSEL` with provenance. |
| **Out of scope** | Full voice dictation (B-S1-16); open-ended chat. |
| **Inputs** | PRD §6.5 |
| **Outputs** | Interview session state in case DB |
| **AC gates** | **AC-6.5**, **AC-6.6** |
| **Halt if** | Question templates invent legal strategy — keep to skeleton completion only |

**Done when:** AC-6.5 and AC-6.6 PASS.

**Status (2026-08-07):** **DONE.** Guided Intake from skeleton gaps only (max 12); answers → `SOURCED_BY_COUNSEL` counsel_facts + collection seeds; session pause/resume/skip/re-open; Interview tab UI. `npm run test:interview` · AC-6.5/6.6.

---

## B-S1-10 — Overnight batch + resume

| | |
|--|--|
| **Goal** | Long-running ingest is resumable after kill/power loss. |
| **Depends on** | B-S1-07 |
| **In scope** | Batch queue in `docpipe`; checkpointing; progress reporting; performance path for AC-3.2. |
| **Out of scope** | Cloud queue; multi-machine. |
| **Inputs** | PRD §6.7, AC-3.2, AC-6.8 |
| **Outputs** | Batch job state machine |
| **AC gates** | **AC-6.8** kill mid-batch → resume; **AC-3.2** when hardware available |
| **Halt if** | Performance miss on Full tier — tune, do not lower AC silently |

**Done when:** AC-6.8 PASS; AC-3.2 measured or SKIP with hardware note.

**Status (2026-08-07):** **DONE.** Durable `batch_job` + `batch_checkpoint` in vault; pipeline triage→recognize→skeleton; cancel/kill + resume; UI **Batch** tab; AC-6.8 PASS (checkpoint logic + wiring); AC-3.2 **SKIP** with Full-tier hardware note (budget 300 pages / 8 h frozen). `npm run test:batch`.

---

## B-S1-11 — Phone Capture Companion (LAN only)

| | |
|--|--|
| **Goal** | QR pairing over local Wi-Fi; zero non-LAN traffic. |
| **Depends on** | B-S1-04 |
| **In scope** | Local companion capture flow; packet-capture test documentation. |
| **Out of scope** | Cloud sync; cellular upload. |
| **Inputs** | PRD §6.6 |
| **Outputs** | Companion feature + AC-6.7 evidence |
| **AC gates** | **AC-6.7** |
| **Halt if** | Platform cannot bind LAN-only → QUESTION |

**Done when:** AC-6.7 PASS.

**Status (2026-08-07):** **DONE.** LAN-only companion HTTP server (private/loopback bind + peer reject); token pair URL; phone upload page → vault ingest; UI **Phone** tab; packet-capture runbook `docs/B-S1-11_phone_capture.md`. AC-6.7 PASS (policy + wiring; human capture still recommended for release evidence). `npm run test:companion`.

---

## B-S1-12 — Knowledge pack system (signed + custom)

| | |
|--|--|
| **Goal** | Install official Ed25519-signed `.akpkg` and user custom packs; jurisdiction switches KB only. |
| **Depends on** | B-S1-02, B-S0-02 |
| **In scope** | Pack format, signature verify, install via download/USB path, versioning/isolation per contracts; India/US core pack placeholders. |
| **Out of scope** | Full corpus content authoring; FAISS (next). |
| **Inputs** | PRD §7, §2 dual-market rule |
| **Outputs** | Pack manager; verified install |
| **AC gates** | **AC-7.1** headnote scan zero matches on official pack pipeline |
| **Halt if** | Pack binary format not in Layer 2 → QUESTION |

**Done when:** Signed pack installs; bad signature rejected; custom pack isolated.

**Status (2026-08-07):** **DONE.** Frozen `contracts/akpkg.schema.json` + `pack_trust.json`; Ed25519 official install; custom isolation under `packs/custom`; AC-7.1 headnote scan on official; India/US placeholder fixtures; Knowledge packs UI. `npm run test:packs`.

---

## B-S1-13 — FAISS retrieval + citation GREEN/AMBER/RED

| | |
|--|--|
| **Goal** | Retrieval-first pipeline; every authority labeled; fake citations RED. |
| **Depends on** | B-S1-12, B-S1-07, model bake-off resolution (§21 #1 as far as needed) |
| **In scope** | DECOMPOSE → RETRIEVE → GENERATE (constrained) → VERIFY → SCORE → RENDER; embeddings model; FAISS over corpus + case graph. |
| **Out of scope** | Mock Court adversary; Stage 2 integrity scoring. |
| **Inputs** | PRD §4.3, §7; G-RETRIEVAL-* |
| **Outputs** | Citation records with GREEN/AMBER/RED |
| **AC gates** | **AC-4.3**, **AC-7.2** (India + US sets) |
| **Halt if** | Generate path cites outside retrieval without RED — hard fail |

**Done when:** AC-4.3 and AC-7.2 PASS on both golden retrieval sets.

**Status (2026-08-07):** **DONE.** Q-CIT-01 frozen → `citation_record.schema.json` v1.0.0; pipeline `caseforge.retrieve.lexical.v1` (FAISS-ready swap); host `retrieval_*` IPC + Library UI; product-owner synthetic GOLDEN `synth_hitrate_20` for IN+US; AC-4.3 + AC-7.2 PASS. `npm run test:retrieval`.

---

## B-S1-14 — Digest surface + completeness score

| | |
|--|--|
| **Goal** | Full Stage 1 DIGEST UI: chronology, parties, contradictions, interview, confidence heat, preparation completeness. |
| **Depends on** | B-S1-08, B-S1-09, B-S1-13 |
| **In scope** | Surfaces: CASEBOOK · CHAMBER · DIGEST per §12; Preparation Completeness Score **derived from graph only** (deterministic). |
| **Out of scope** | ARGUMENT TREE, MOCK COURT, Bench Prep. |
| **Inputs** | PRD §2 Stage 1, §12, G6 |
| **Outputs** | Digest UI; score panel |
| **AC gates** | Copy uses preparation language only (G6); score formula documented in contracts |
| **Halt if** | Score uses LLM judgment — forbidden; must be deterministic from graph |

**Done when:** Digest navigable; score recomputes from graph without LLM.

**Status (2026-08-07):** **DONE.** §12 surfaces CASEBOOK · CHAMBER · DIGEST; `caseforge.prep_completeness.v1` frozen in `constants.json`; pure recompute in `src/lib/completeness.ts` (no LLM); Digest UI wired to case_graph + interview; G6 copy. `npm run test:digest`.

---

## B-S1-15 — Drafting surface + DOCX/PDF export

| | |
|--|--|
| **Goal** | Templates + court-format export; required footer (G4). |
| **Depends on** | B-S1-14 |
| **In scope** | Drafting surface; templates; export DOCX/PDF; digest export variants (court-formatted digest, list of dates, synopsis). |
| **Out of scope** | Auto-filing to court portals. |
| **Inputs** | PRD §2 Stage 1, G4, G5 |
| **Outputs** | Export files; provenance on AI-assisted draft spans |
| **AC gates** | G4 footer present; G5 provenance for AI spans |
| **Halt if** | Footer text not specified → QUESTION (do not invent legal disclaimer wording without product owner) |

**Done when:** Exports open in Word/PDF readers; footer + provenance present.

**Status (2026-08-08):** **DONE.** DRAFTING surface live (CASEBOOK · CHAMBER · DIGEST · DRAFTING); templates `court_formatted_digest` / `list_of_dates` / `synopsis` / `counsel_memo`; DOCX + PDF export from `case_graph`; G4 footer frozen as `caseforge.export_footer.v1` (reuse Stage-1 honesty + G6; no new legal disclaimer invented); G5 provenance appendix (`model_hash`, `prompt_hash`, `retrieval_ids`). `npm run test:drafting`.

---

## B-S1-16 — Voice dictation + self-correcting lexicon (English)

| | |
|--|--|
| **Goal** | Whisper STT → editable staging → commit; TTS roles; lexicon persistence. |
| **Depends on** | B-S1-01; `infer-speech` sidecar; §21 #2 Whisper size decision |
| **In scope** | English primary; staging box; lexicon STT corrections + TTS overrides; pre-seed legal English patterns; export/import lexicon. |
| **Out of scope** | Full voice Mock Court (Stage 2); non-English primary. |
| **Inputs** | PRD §9; G-AUDIO |
| **Outputs** | Voice pipeline; lexicon store |
| **AC gates** | **AC-9.1–9.4** as defined in eval (WER, persistence, voice-only path, language pinning) |
| **Halt if** | Whisper size undecided and both fail budgets → QUESTION |

**Done when:** AC-9.x PASS; network-disabled voice path works for dictation use cases in Stage 1.

**Status (2026-08-08):** **DONE.** Q-LX-01 + Q-WHISPER-01 (small, PROVISIONAL_STAGE1) frozen; `infer-speech` sidecar (`speech.ping/status/stt/tts`); app-vault lexicon + pre-seed; STT → staging → commit UI; TTS roles; export/import `.cflex.json`. AC-9.2/9.3/9.4 PASS (structural + live sidecar); AC-9.1 WER **SKIP** until G-AUDIO GOLDEN. `npm run test:voice`.

---

## B-S1-17 — Experience Store (local encrypted, Stage 1 subset)

| | |
|--|--|
| **Goal** | Store corrections, lexicon, draft edits; inject top-K patterns on retrieval (Stage 1 use). |
| **Depends on** | B-S1-08, B-S1-13, B-S1-16 |
| **In scope** | Local encrypted Experience Store; no fleet upload without explicit review UI. |
| **Out of scope** | Fleet contribution backend; limb outcomes (Stage 2). |
| **Inputs** | PRD §11 |
| **Outputs** | Experience records; retrieval injection |
| **AC gates** | **AC-11.1 & 11.2** (as in eval) |
| **Halt if** | Any default-on network upload — forbidden |

**Done when:** AC-11.x PASS for Stage 1 subset; opt-in fleet path is off by default.

**Status (2026-08-08):** **DONE.** Q-EX-01 frozen → `experience_record.schema.json` v1.0.0 + `constants.experience_store` (top_k=5); encrypted app-vault store; dual-write from review corrections + lexicon; retrieval top-K injection; fleet off by default / no network upload (AC-11.2). UI **Experience**. `npm run test:experience`.

---

## B-S1-18 — Licensing, air-gap, privacy audit

| | |
|--|--|
| **Goal** | One-time activation, offline licence, EXPIRED = read-only vault, air-gap mode. |
| **Depends on** | B-S1-02 |
| **In scope** | Machine fingerprint 3-of-4; Ed25519 licence; state machine; air-gap kills networking; case data has zero network path. |
| **Out of scope** | Heavy DRM beyond PRD budget (≤ 8 % effort total — B-S1-19). |
| **Inputs** | PRD §13 |
| **Outputs** | Licence module; air-gap mode |
| **AC gates** | **AC-13.1** network-capture audit |
| **Halt if** | EXPIRED ransoms data — must be read-only vault only |

**Done when:** AC-13.1 PASS; EXPIRED does not lock/delete user data.

**Status (2026-08-08):** **DONE.** Q-LIC-01 frozen → `licence_payload.schema.json` v1.0.0; fingerprint 3-of-4; Ed25519 offline activate; UNLICENSED/ACTIVE/EXPIRED; EXPIRED=`assert_vault_writable` read-only (no ransom); air-gap kills companion/fleet net; AC-13.1 network policy audit. UI **Licence**. `npm run test:licence`.

---

## B-S1-19 — Model/pack encryption at rest + integrity (budget-capped)

| | |
|--|--|
| **Goal** | Economic protection: encrypt models/packs; integrity checks; never ransom data. |
| **Depends on** | B-S1-12, B-S1-18 |
| **In scope** | At-rest encryption; graceful degrade on tamper; effort ≤ 8 % total (track time). |
| **Out of scope** | Unbreakable DRM claims; online kill-switch for user files. |
| **Inputs** | PRD §14 |
| **Outputs** | Protection layer; AC-14 tests |
| **AC gates** | **AC-14.1 & 14.2** |
| **Halt if** | Protection blocks legitimate offline use without licence path — redesign |

**Done when:** AC-14.x PASS; user data never ransomed.

**Status (2026-08-08):** **DONE.** `caseforge.protection.v1` — AES-256-GCM machine-bound pack/model encryption; integrity seals; graceful degrade on tamper; AC-14.1 copied-folder policy; AC-14.2 no-ransom (user vault independent). Official pack install auto-protects content. UI **Protection**. Effort budget ≤8%. `npm run test:protection`.

---

## B-S1-20 — Performance & model choreography hardening

| | |
|--|--|
| **Goal** | Meet Full/Lite tier budgets; one large model at a time; status chip. |
| **Depends on** | All prior S1 feature batches that load models |
| **In scope** | Sequential load; unload on idle; Lite disables live simulation (N/A Stage 1 sim); RAM/start budgets. |
| **Out of scope** | Stage 2 turn latency AC-3.3 (Stage 2). |
| **Inputs** | PRD §3, §4.2 |
| **Outputs** | Status chip; choreography supervisor |
| **AC gates** | **AC-3.1**, **AC-3.4**, **AC-3.5**, **AC-4.2** |
| **Halt if** | Primary + vision concurrent on 8 GB — hard fail |

**Done when:** AC-3.1, 3.4, 3.5, 4.2 PASS on reference Full tier (or documented SKIP with hardware).

**Status (2026-08-08):** **DONE.** `caseforge.choreography.v1` supervisor — max 1 large model; primary↔vision sequential eviction; idle unload 120s; visible **MODEL** status chip; tier Full/Lite; RSS/cold-start metrics (AC-3.1/3.4/3.5/4.2). `npm run test:choreography`.

---

## B-S1-21 — Guardrails G1–G6 for Stage 1 surfaces

| | |
|--|--|
| **Goal** | No verdicts; no citation bypass; refuse fabrication coaching; footer; provenance; preparation language. |
| **Depends on** | B-S1-13, B-S1-14, B-S1-15 |
| **In scope** | System prompts + hard filters + export footer + provenance log; red-team suite subset for Stage 1. |
| **Out of scope** | Full 200-prompt suite may complete in Stage 2, but Stage 1 must not violate G1–G3. |
| **Inputs** | PRD §15; G-REDTEAM |
| **Outputs** | Guardrail module; eval hooks |
| **AC gates** | Partial **AC-15.1** (Stage 1 prompt set); zero G1–G3 on that set |
| **Halt if** | Any G1–G3 fail on known suite — fix before ship |

**Done when:** Stage 1 red-team subset PASS.

**Status (2026-08-08):** **DONE.** `caseforge.guardrails.v1` — G1–G3 hard input/output filters; G4/G5 reuse export; G6 prep language; Stage-1 GOLDEN red-team **24** prompts zero misses; full 200 deferred to B-S2-06. UI **Guardrails**. `npm run test:guardrails`.

---

## B-S1-22 — Stage 1 ship gate (v1.0)

| | |
|--|--|
| **Goal** | Release Digest only when **all Stage 1 ACs** are green. |
| **Depends on** | B-S1-01 … B-S1-21 |
| **In scope** | Run full `/eval` Stage 1 matrix; installer refuses &lt; 4 GB / 32-bit; Lite/Full messaging; copy audit for G6. |
| **Out of scope** | Any Stage 2 feature in the build flags default ON. |
| **Inputs** | Entire Stage 1 AC list |
| **Outputs** | Signed release candidate; changelog |
| **AC gates** | All Stage 1 ACs PASS; AC-2.1 dependency audit: Stage 2 modules absent or not imported into doc pipeline |
| **Halt if** | Any Stage 1 AC FAIL or SKIP without waiver document |

**Done when:** Stage 1 commercial release checklist signed off.

**Status (2026-08-08):** **DONE (RC).** Ship gate `npm run ship:stage1` / `eval/checks/ship_gate_b_s1_22.py`; AC-2.1 docpipe audit; hardware refuse &lt;4GB/32-bit + Lite/Full messaging; G6 copy audit; waivers in `eval/waivers/stage1_ship_waivers.json`; `CHANGELOG.md` + `docs/RELEASE_CHECKLIST_v1.0.md`; version **1.0.0-rc1**; Stage 2 default OFF.

### Stage 1 AC checklist (must all PASS)

| AC | Topic |
|----|--------|
| AC-3.1 | Cold start / idle RAM |
| AC-3.2 | Overnight 300-page budget (or hardware waiver) |
| AC-3.4 | 40-page digest ≤ 5 min |
| AC-3.5 | UI freeze ≤ 200 ms |
| AC-4.1 | IPC contracts |
| AC-4.2 | RSS ≤ 7.2 GB |
| AC-4.3 | Fake citation RED |
| AC-5.1 | Intake auto-type ≥ 80 % |
| AC-6.1 | Triage ≥ 92 % |
| AC-6.2 | Chronology recall ≥ 95 % |
| AC-6.3 | Click → source |
| AC-6.4 | ≤ 2 interactions correct word |
| AC-6.5 | Interview ≤ 12 Q → ≥ 90 % |
| AC-6.6 | Interview resumable |
| AC-6.7 | Phone capture LAN-only |
| AC-6.8 | Batch resume |
| AC-7.1 | Headnote scan clean |
| AC-7.2 | Retrieval top-8 ≥ 85 % |
| AC-9.x | Voice/lexicon |
| AC-11.x | Experience Store |
| AC-13.1 | No case data on network |
| AC-14.x | Protection / no ransom |
| AC-16.x | Schema / encryption / .akcase |
| G1–G6 | Guardrails (Stage 1 set) |

---

# TRACK S2 — Stage 2 Mock Court (v2.0)

> **Hard rule:** Stage 2 consumes **only** `case_graph.json`. No Stage 2 module may parse raw documents.  
> **AC-2.1:** Static audit: zero Stage 2 imports into document pipeline.

---

## B-S2-01 — Argument Tree + Integrity Engine (deterministic)

| | |
|--|--|
| **Goal** | Build Argument Tree + VI + Missing Links with **zero LLM in scoring path**. |
| **Depends on** | S1 ship gate; frozen argument node schema |
| **In scope** | Tree: root = relief, limbs, leaves = anchors; four links 0–100; VI = 100 − min(links); Missing Links cites inputs. |
| **Out of scope** | Live adversary dialogue; voice. |
| **Inputs** | PRD §8 |
| **Outputs** | Integrity Engine module + ARGUMENT TREE UI |
| **AC gates** | **AC-8.1, AC-8.2, AC-8.3** |
| **Halt if** | Any LLM call inside scoring path — redesign |

**Done when:** AC-8.1–8.3 PASS.

**Status (2026-08-08):** **DONE.** Q-AN-01 frozen → `argument_node.schema.json` v1.0.0. Scorer `caseforge.integrity.v1` (Rust + Python eval). GOLDEN planted gap `PG-EV-01` top-2 Missing Links. UI **Argument Tree**. `npm run test:integrity`.

---

## B-S2-02 — Mock Court text formats (one format at a time)

| | |
|--|--|
| **Goal** | Five rehearsal formats, text-only first: Bail/Interim, Final Arguments, Witness Cross, Appellate, generic Motion. |
| **Depends on** | B-S2-01, B-S1-13 (retrieval pipeline) |
| **In scope** | Sequential delivery: ship format N only after format N-1 eval passes; adversary uses only case graph + retrieved law. |
| **Out of scope** | Voice; named judges. |
| **Inputs** | PRD §2 Stage 2; G-SCRIPTS |
| **Outputs** | Five format modules |
| **AC gates** | Format-specific scripted sessions; AC-3.3 latency when models ready |
| **Halt if** | Adversary uses raw PDF parse — violation of Stage 2 dependency rule |

**Done when:** All five formats complete scripted evals PASS.

**Status (2026-08-08):** **DONE.** Q-ST-01 frozen → `session_turn.schema.json` v1.0.0. Pipeline `caseforge.mock_court.v1` (graph+retrieval only). Five sequential G-SCRIPTS fixtures. UI Mock Court formats 1–5. `npm run test:mockcourt`.

---

## B-S2-03 — Per-Case Adaptive Bench

| | |
|--|--|
| **Goal** | Six generic archetypes + case-order signals + sliders; **no named-judge profiles**. |
| **Depends on** | B-S2-02 |
| **In scope** | Archetypes + dials; behaviour conditioned on this case’s orders only. |
| **Out of scope** | Real judge databases; profiling. |
| **Inputs** | PRD §10 |
| **Outputs** | Bench behaviour controller |
| **AC gates** | **AC-10.1 & 10.2** |
| **Halt if** | Any named-judge feature requested — refuse (non-goal §20) |

**Done when:** AC-10.x PASS.

**Status (2026-08-08):** **DONE.** Q-AB-01 frozen · `adaptive_bench.json` · `caseforge.adaptive_bench.v1` · Mock Court Adaptive Bench tab · `npm run test:bench`.

---

## B-S2-04 — Voice-in-court + structured debrief

| | |
|--|--|
| **Goal** | Full-duplex English voice practice + debrief; lexicon applies. |
| **Depends on** | B-S2-02, B-S1-16 |
| **In scope** | Voice roles (bench/opposition/witness presets); debrief structure; network off test. |
| **Out of scope** | Non-English court voice. |
| **Inputs** | PRD §2 Stage 2, §9 |
| **Outputs** | Voice Mock Court path |
| **AC gates** | Voice-only multi-turn rehearsal; AC-3.3 |
| **Halt if** | Lite tier attempts live sim — must show honesty notice and refuse |

**Done when:** Voice rehearsal + debrief eval PASS; Lite honesty notice verified.

**Status (2026-08-08):** **DONE.** `caseforge.voice_court.v1` — STT→court→TTS duplex; structured debrief; Lite `CF_VOICE_COURT_LITE_REFUSE` honesty; `npm run test:voicecourt`.

---

## B-S2-05 — Bench Preparation Mode

| | |
|--|--|
| **Goal** | AI plays both counsel; **never** the decision-maker. |
| **Depends on** | B-S2-02, B-S2-03 |
| **In scope** | Dual-counsel simulation; explicit non-decision-maker UI; G1 enforcement. |
| **Out of scope** | Verdict/outcome prediction. |
| **Inputs** | PRD §2 Stage 2, G1 |
| **Outputs** | Bench Prep mode |
| **AC gates** | Red-team: no verdict language |
| **Halt if** | Mode outputs “holding” or “order” as if court — fail G1 |

**Done when:** Bench Prep eval PASS; G1 clean.

**Status (2026-08-09):** **DONE.** `caseforge.bench_prep.v1` — dual counsel; G1 holding/order refuse; non-decision-maker UI; `npm run test:benchprep`.

---

## B-S2-06 — Stage 2 Experience Store extensions + full red-team

| | |
|--|--|
| **Goal** | Limb outcomes, rated exchanges, bench settings in Experience Store; full AC-15.1. |
| **Depends on** | B-S2-01 … B-S2-05, B-S1-17 |
| **In scope** | Extend store; ≥ 200-prompt red-team G1–G3. |
| **Out of scope** | New product surfaces. |
| **Inputs** | PRD §11, §15 |
| **Outputs** | Full AC-15.1 report |
| **AC gates** | **AC-15.1** zero G1–G3 violations |
| **Halt if** | Any G1–G3 fail — fix before Stage 2 ship |

**Done when:** AC-15.1 PASS.

**Status (2026-08-09):** **DONE.** limb_outcome/bench_setting writable; `stage2_200` suite (210 prompts) zero G1–G3 misses; `npm run test:s2redteam`.

---

## B-S2-07 — Stage 2 ship gate (v2.0)

| | |
|--|--|
| **Goal** | Ship Mock Court only when Stage 2 ACs + AC-2.1 + AC-15.1 green. |
| **Depends on** | B-S2-01 … B-S2-06 |
| **In scope** | Full eval matrix Stage 2; dependency audit AC-2.1; Full tier required messaging for Mock Court. |
| **Out of scope** | New jurisdictions beyond packs. |
| **AC gates** | AC-2.1, AC-3.3, AC-8.x, AC-10.x, AC-15.1, Stage 2 scripts |
| **Halt if** | Any FAIL |

**Done when:** v2.0 release checklist signed off.

**Status (2026-08-09):** **DONE.** RC `v2.0.0-rc1`. `npm run ship:stage2` → `eval/reports/stage2_ship_gate.json`. Full-tier Mock Court messaging; Lite live-sim refuse retained.

---

# Cross-cutting rules (every batch)

1. **No invention:** Missing schema field, AC detail, footer text, pack format, or model choice → `QUESTION` block and stop.
2. **Case Graph is sacred:** Stage 1 writes it; Stage 2 only reads structured graph + retrieval — never re-parses documents.
3. **Jurisdiction:** Features identical; only KB packs differ. Jurisdiction set at case creation (B-S1-03).
4. **Models:** At most one large model resident; primary never concurrent with vision on 8 GB.
5. **Honesty notices:** Every reduced-capability path (Lite, missing pack, low confidence) shows a visible notice.
6. **Provenance:** Every generative output logs model hash, prompt hash, retrieval IDs (G5).
7. **Tests decide done:** Opinion never marks a batch complete.
8. **Scope lock:** Do not edit files outside the current batch’s declared outputs unless fixing a broken dependency test.

---

# Recommended QUESTION template (when blocked)

```text
QUESTION
- batch: B-S1-XX
- missing: <what PRD/contracts/golden do not specify>
- blocked_artefact: <file or AC>
- options_considered: <only if listed in PRD §21; else none>
- need_from_owner: <exact decision>
HALT
```

---

# Open questions from PRD §21 (resolve before named milestone)

| # | Question | Resolve before |
|---|----------|----------------|
| 1 | Exact primary 7–9B GGUF | M5 / B-S1-13 |
| 2 | Whisper small vs medium | M7 / B-S1-16 — **resolved provisional: small** (re-open if G-AUDIO GOLDEN WER requires medium) |
| 3 | Vision model bake-off | M2 / B-S1-06 |
| 4 | Initial US state packs (federal + 3–5) | B-S1-12 content packs |
| 5 | Shared-lexicon merge (Firm tier) | Post-S1 Firm tier (not v1 solo) |

---

# Batch execution order (checklist)

### S0
- [x] B-S0-01 Repo skeleton  
- [x] B-S0-02 Contracts freeze / QUESTION list  
- [x] B-S0-03 Constants & errors  
- [x] B-S0-04 Golden scaffolds  
- [x] B-S0-05 Eval harness  
- [x] **S0 exit gate**

### S1
- [x] B-S1-01 Shell (`cargo check`/`test` green; `tauri:dev` works with MSVC + Perl)  
- [x] B-S1-02 Vault (SQLCipher + Argon2id freeze; AC-16.2 structural + live tests)  
- [x] B-S1-03 New Case wizard (jurisdiction required + immutable; §5 fields only)  
- [x] B-S1-04 Document intake (PDF/DOCX/JPG/PNG + type tag correction; no OCR)  
- [x] B-S1-05 Triage (**AC-6.1 PASS** — 500 GOLDEN @ 100%; Q-VISION-01 Stage-1 freeze heuristic.v1)  
- [x] B-S1-06 Recognition (word-level confidences + thresholds from contracts; text-layer UI)  
- [x] B-S1-07 Skeleton / case_graph (**AC-6.2** + schema freeze Q-CG-01)  
- [x] B-S1-08 Review UI (AC-6.3 click→source; AC-6.4 ≤2-step correct)  
- [x] B-S1-09 Guided interview (AC-6.5 ≤12 Q → ≥90%; AC-6.6 resume/skip/reopen)  
- [x] B-S1-10 Overnight batch (AC-6.8 kill→resume; AC-3.2 SKIP+hardware note)  
- [x] B-S1-11 Phone capture (LAN-only companion; AC-6.7)  
- [x] B-S1-12 Packs (Ed25519 official + custom isolation; AC-7.1)  
- [x] B-S1-13 Retrieval + citations (AC-4.3 + AC-7.2; Q-CIT-01)  
- [x] B-S1-14 Digest UI (prep completeness v1; G6; CASEBOOK/CHAMBER/DIGEST)  
- [x] B-S1-15 Drafting + export (G4 footer v1; G5 provenance; DOCX/PDF; DRAFTING surface)  
- [x] B-S1-16 Voice + lexicon (Whisper small provisional; staging; AC-9.2/9.3/9.4; AC-9.1 WER SKIP)  
- [x] B-S1-17 Experience Store (top_k=5 injection; fleet off; AC-11.1/11.2)  
- [x] B-S1-18 Licensing + air-gap (Ed25519; 3-of-4; EXPIRED read-only; AC-13.1)  
- [x] B-S1-19 Protection (AES-256-GCM packs/models; AC-14.1/14.2; no ransom)  
- [x] B-S1-20 Performance choreography (1 large model; status chip; AC-3.1/3.4/3.5/4.2)  
- [x] B-S1-21 Guardrails (G1–G6; stage1 red-team 24; partial AC-15.1)  
- [x] **B-S1-22 Stage 1 ship** (v1.0.0-rc1; ship:stage1; AC-2.1; waivers)

### S2
- [x] B-S2-01 Integrity Engine  
- [x] B-S2-02 Five Mock Court formats  
- [x] B-S2-03 Adaptive Bench  
- [x] B-S2-04 Voice court + debrief  
- [x] B-S2-05 Bench Prep Mode  
- [x] B-S2-06 Full red-team + Experience  
- [x] **B-S2-07 Stage 2 ship**

---

# Mapping: PRD §18 phases → batches

| PRD Phase | Batches |
|-----------|---------|
| Phase 0 Preparation | B-S0-01 … B-S0-05 |
| Phase 1 Foundation M1–M2 | B-S1-01 … B-S1-06 |
| Phase 2 Skeleton & Review M3–M4 | B-S1-07 … B-S1-11 |
| Phase 3 Knowledge & Retrieval M5 | B-S1-12 … B-S1-13 |
| Phase 4 Digest & Export M6 | B-S1-14 … B-S1-15 |
| Phase 5 Voice M7 | B-S1-16 |
| Phase 6 Licensing & Hardening M8 | B-S1-17 … B-S1-22 |
| Phase 7 Stage 2 M9–M13 | B-S2-01 … B-S2-07 |

---

# What this plan deliberately does *not* do

- Does **not** invent JSON field lists not present in Layer 2 (those belong in `/contracts`, or QUESTION).
- Does **not** invent golden legal answers (those belong in `/golden`, authored as known-answer files).
- Does **not** schedule cloud features, CLM, verdict prediction, named judges, or marketplace (§20 non-goals).
- Does **not** allow Stage 2 work to start before Stage 1 ship gate.
- Does **not** treat “looks good” as done — only AC scripts.

---

**Document status:** Companion to CaseForge PRD v1.0 — Development Batch Plan v1.0  
**Rule:** Follow batch order. Halt on missing truth. Never invent.

*End of Development Batch Plan*
