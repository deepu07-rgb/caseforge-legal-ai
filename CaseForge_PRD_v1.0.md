# CaseForge — Product Requirement Document v1.0

**Offline Legal Intelligence for Litigators**  
**Dual-Market Architecture (India + United States + Global Expansion Ready)**  
**Stage 1: The Digest Engine · Stage 2: The Mock Court**

**Document Status:** Normative Execution Package – Layer 1  
**Version:** 1.0  
**Date:** 29 July 2026  
**Previous name:** Aakhada (retired – Hindi origin, not suitable for global positioning)

---

## Document Contract

This PRD is **Layer 1** of a four-layer Execution Package. It is written so that low-reasoning implementers (human developers or AI coding agents such as Grok Build) can construct the product with zero invention.

- **Layer 1** — this PRD: what to build, why, binding acceptance criteria (AC-x.y), and **detailed step-by-step construction guidance**.
- **Layer 2** — Contracts: frozen JSON schemas, module interfaces, function signatures, error codes (live in `/contracts`).
- **Layer 3** — Work Orders: atomic tickets with exact scope, inputs, outputs, behaviour, forbidden actions, and tests.
- **Layer 4** — Golden Dataset + Test Suites: pre-written tests with known-answer files. Agents make tests pass; they do not interpret intent.

**Prime Rule:**  
If any detail is missing from Layers 1–4, **HALT** and emit a `QUESTION` block.  
Never guess. Never invent. Never modify files outside the current work-order scope.

Sections marked **[NORMATIVE]** are binding.  
Sections marked **[STRATEGY]** are context.  
Every **AC-x.y** is an executable release gate.

---

## 0. Why the Name Changed

“Aakhada” is a Hindi word. For a product that targets both the Indian and United States markets (and later other common-law jurisdictions), an English, globally pronounceable, professional name is required.  

**New name: CaseForge**

- Conveys the core value: forging a strong case through rigorous preparation.
- Works in English-speaking markets (USA, UK, Canada, Australia, Singapore, etc.).
- Neutral, modern, and free of cultural or linguistic friction.
- Easy to trademark and domain-secure.

All previous references to “Aakhada” are retired. The product is CaseForge.

---

## 1. Product Truth [STRATEGY — binding on all marketing and UI copy]

**What CaseForge is NOT**  
An expert lawyer in a box. A 7–9B parameter model on consumer hardware is never marketed as a substitute for legal judgment.

**What CaseForge IS**  
A system that makes the lawyer measurably better prepared through four honest mechanisms:

1. **Perfect memory of this case** — the Digest: every fact, date, party, and contradiction extracted from the paperbook or case file, source-linked to the exact page image or PDF span.
2. **Deterministic weak-link detection** — graph algorithms (not neural guesswork) over the claim → fact → evidence → law chain.
3. **Verified citations** — nothing reaches the lawyer unlabeled; every authority resolves to local corpus text or is flagged GREEN / AMBER / RED.
4. **A relentless sparring partner** — an adversary that attacks using only the case’s own facts and retrieved law under courtroom-format pressure.

**One-line pitch (global):**  
> CaseForge reads your entire case file overnight, remembers everything, shows you where your case is weakest, and then argues against you until it isn’t — all on your own machine, with client papers never touching the internet.

UI copy, onboarding, and sales material must always frame outcomes as **preparation completeness**, never win prediction.  
[Guardrail G6]

---

## 2. Two-Stage Product Definition [NORMATIVE]

### Stage 1 — CaseForge Digest (v1.0, first commercial release)

Ingest a messy or clean case file → produce a verified, source-linked Case Digest:

- Chronology of dated events (each linked to page image or PDF span)
- Parties & roles map
- Statutes / rules invoked (with jurisdiction-aware mapping tables)
- Evidence inventory, procedural posture, prayer / relief extraction
- Contradiction Report
- Citation verification of every authority (GREEN / AMBER / RED)
- Drafting surface + templates + court-format export (DOCX / PDF)
- English voice dictation with self-correcting lexicon
- Export: court-formatted digest, list of dates, synopsis

**Commercial reality:** Stage 1 alone is valuable. Juniors and small firms currently spend 2–4 days per matter reconstructing exactly this material by hand. Hallucination risk is near zero because every output is an extraction with a source pointer.

### Stage 2 — CaseForge Mock Court (v2.0)

Built exclusively on the structured Case Graph produced by Stage 1.

- Argument Tree + Argument Integrity Engine (Vulnerability Index + Missing Links panel)
- Five rehearsal formats: Bail / Interim-Motion, Final Arguments, Witness Cross-Examination, Appellate, and generic Motion practice
- Per-Case Adaptive Bench + opposition aggression control
- Full-duplex English voice argument practice + structured debrief
- Bench Preparation Mode (AI plays both counsel; never the decision-maker)

**Dependency rule [NORMATIVE]**  
Stage 2 consumes **only** the structured Case Graph (`case_graph.json`).  
No Stage 2 module may parse raw documents directly.  
**AC-2.1:** Static dependency audit shows zero imports from Stage 2 modules into the document pipeline.

**Dual-market / global rule [NORMATIVE]**  
All features and functions are identical for every jurisdiction.  
The **only** difference is the knowledge base (official packs + any user-uploaded custom packs).  
Jurisdiction is selected at case creation. The knowledge base is switched or extended by the user.

---

## 3. Target Hardware & Performance Budget [NORMATIVE]

| Tier | Spec | Experience |
|------|------|------------|
| **Full** (required for Mock Court) | 8 GB RAM, Intel i3-8th-gen / i5 / Apple M1 or better, 4 GB free disk + packs, Windows 10+ / macOS 12+ | Full experience. Sequential model loading. |
| **Lite** | 4–6 GB RAM | Digest vault, drafting, citation checking, lexicon voice dictation. No live simulation. |
| **Not supported** | < 4 GB or 32-bit OS | Installer refuses with clear message. |

**Performance budgets** (Full tier, reference ≈ i5-8250U):

- **AC-3.1:** Cold start ≤ 6 s; idle RAM ≤ 400 MB (models unloaded).
- **AC-3.2:** Overnight batch of 300-page mixed case file ≤ 8 hours.
- **AC-3.3:** Mock Court turn latency ≤ 15 s for ~300-token adversary response (including sequential model swap).
- **AC-3.4:** Digest of 40-page typed chargesheet or clean US complaint ≤ 5 minutes foreground.
- **AC-3.5:** No UI thread freeze > 200 ms. All heavy work in Rust sidecar with progress reporting.

---

## 4. System Architecture [NORMATIVE]

### 4.1 Shell & Process Topology

- **Shell:** Tauri v2 (Rust core) + React 18 / TypeScript UI. Single installer per OS.
- **Three supervised sidecar processes** (spawned on demand, killed when idle):
  1. `infer-llm` — llama.cpp (pinned commit) or equivalent GGUF runtime. Loads **exactly one** GGUF at a time.
  2. `infer-speech` — whisper.cpp + high-quality local English TTS.
  3. `docpipe` — OCR / vision / HTR pipeline + overnight batch queue.
- **IPC:** Typed JSON-RPC over stdio. Schemas frozen in `/contracts/ipc/*.json`.
- **AC-4.1:** Full contract round-trip test suite must pass.

### 4.2 Model Inventory (offline only, versioned, updatable via signed packs)

| Role | Class | Notes |
|------|-------|-------|
| Primary Reasoner / Adversary / Critic | Strongest open-weight 7–9B instruct model that fits sequential loading on 8 GB (Llama-3.1/3.3-8B-Instruct class or current best equivalent) Q4_K_M / Q5_K_M | ~4.0–4.8 GB disk, ~5.0–5.8 GB RAM |
| Fallback / Lite | Strong 3B-class instruct | Used when primary cannot fit |
| Vision / HTR | Strong 3B-class vision-language model | Used only by docpipe, never concurrent with primary |
| STT | Whisper small or medium (English-optimised) | |
| TTS | High-quality local English voices | Distinct presets for bench / opposition / witness |
| Embeddings | Strong English (+ multilingual capable) model | bge-m3 class or equivalent |

**8 GB choreography rule [NORMATIVE]:** At most one large model resident at any time. Visible status chip required.

**AC-4.2:** Worst-case scripted session never exceeds 7.2 GB total RSS.

### 4.3 Retrieval-First Pipeline [NORMATIVE]

Every generative turn:  
**DECOMPOSE → RETRIEVE (FAISS over corpus + case graph + Experience Store) → GENERATE (constrained to retrieved material) → VERIFY (citation GREEN/AMBER/RED) → SCORE → RENDER**

**AC-4.3:** Planted fake citation always renders RED and is excluded from downstream reasoning unless user confirms.

---

## 5. Case Intake [NORMATIVE]

- Wizard fields: cause title, court, **jurisdiction** (India / United States / Other), matter type, user’s side, code/era (auto-suggested), language (English primary).
- Sources: PDF, DOCX, JPG/PNG, Phone Capture Companion.
- Every document gets `doc_id`, type tag, language tag, per-page quality grade.
- **AC-5.1:** 60-file intake achieves ≥ 80 % correct auto-type suggestions on golden set; one-click correction.

---

## 6. Document Understanding Pipeline [NORMATIVE]

**Philosophy:** Extract the load-bearing ~20 % (the skeleton), know what could not be read, and ask. Never pretend.

### 6.1 Triage
Fast classifier → typed English / other / mixed / clean handwriting / messy handwriting / non-text.  
**AC-6.1:** ≥ 92 % accuracy on 500-page golden triage set.

### 6.2 Recognition with confidence
Every word has a confidence score. Thresholds live in `/contracts/constants.json`.

### 6.3 Skeleton Extraction
Primary reasoner + deterministic parsers → Case Graph. Every item carries source pointer + confidence.  
**AC-6.2:** ≥ 95 % recall of known dated events on golden 40-page document.  
**AC-6.3:** Click any fact → source page with span highlighted.

### 6.4 Verification Review
Side-by-side surface. Only low-confidence items highlighted. Corrections feed Experience Store.  
**AC-6.4:** ≤ 2 interactions to correct a highlighted word.

### 6.5 Guided Intake Interview
Targeted questions for unreadable content or skeleton gaps. Answers become `SOURCED_BY_COUNSEL` facts.  
**AC-6.5:** ≤ 12 questions complete skeleton to ≥ 90 % of known-answer graph.  
**AC-6.6:** Resumable, skippable, re-openable.

### 6.6 Phone Capture Companion
Local Wi-Fi only, QR pairing, zero non-LAN traffic.  
**AC-6.7:** Packet capture confirms zero non-LAN traffic.

### 6.7 Overnight Batch
Resumable after power loss.  
**AC-6.8:** Kill mid-batch → restart resumes correctly.

---

## 7. Knowledge Base [NORMATIVE]

**Core dual-market / global principle:**  
Identical features and functions for every user.  
The **only** difference is the knowledge base the user installs or uploads.

### Official Packs (Ed25519-signed `.akpkg`)
- **India Core + Domain Packs** (Criminal, Civil, Property, Banking, Corporate, Family)
- **United States Core + Domain Packs** (Federal + major states initially)
- Raw judgment text + neutral citation only. Headnotes stripped.
- **AC-7.1:** Automated headnote scan returns zero matches.
- Installable via download or USB. Quarterly deltas. Require valid licence.

### User-Uploadable Custom Packs
Any user may upload acts, rules, judgments, templates, internal precedents.  
Treated identically for retrieval. Versioning and isolation rules in `/contracts`.

**AC-7.2:** Retrieval returns known-relevant authority in top-8 for ≥ 85 % of golden queries (separate India / US sets).

---

## 8. Argument Integrity Engine [NORMATIVE — Stage 2]

Deterministic only.

- Argument Tree (root = relief, limbs = arguments, leaves = anchors)
- Four links per limb (Fact / Evidence / Law / Exposure), each 0–100
- Vulnerability Index = 100 − min(links)
- Missing Links panel always cites its inputs

**AC-8.1:** Planted gap appears in top-2 Missing Links.  
**AC-8.2:** VI recomputation ≤ 5 s.  
**AC-8.3:** Scoring is pure deterministic code; no LLM inside scoring path.

---

## 9. Voice & Self-Correcting Lexicon [NORMATIVE]

English primary.  
STT (Whisper) → editable staging box → commit.  
TTS with distinct role voices.  
Lexicon stores STT corrections and TTS pronunciation overrides permanently and applies them forever. Pre-seeded with legal English + statute names + proper-name patterns. Exportable/importable.

**AC-9.1–9.4:** WER, correction persistence, full voice-only rehearsal, language pinning — all must pass.

---

## 10. Per-Case Adaptive Bench [NORMATIVE]

No named-judge profiles ever.  
Behaviour = six generic archetypes + signals extracted from this case’s own orders + user dial-in sliders.  
**AC-10.1 & 10.2** must pass.

---

## 11. Experience Store [NORMATIVE]

All local and encrypted. Accumulates corrections, lexicon entries, rated exchanges, draft edits, limb outcomes, bench settings.  
Retrieval injects top-K relevant past patterns.  
Optional anonymised fleet contribution only after explicit user review.

**AC-11.1 & 11.2** must pass.

---

## 12. Surfaces & Design Language [NORMATIVE]

Ink/Parchment palette, professional typography, no gavels.  
Stage 1: CASEBOOK · CHAMBER · DIGEST · DRAFTING.  
Stage 2: ARGUMENT TREE · MOCK COURT · Bench Prep Mode.  
Every reduced-capability path has a visible honesty notice.

---

## 13. Licensing, Activation, Privacy [NORMATIVE]

One-time activation with machine fingerprint (3-of-4 tolerance).  
Ed25519-signed licence. Offline validation.  
EXPIRED = read-only vault; user data never ransomed.  
Air-gap mode kills all networking.  
Case data has zero network code path.

**AC-13.1:** Network-capture audit shows zero user-document content leaving the machine.

---

## 14. Code & Asset Protection [NORMATIVE + STRATEGY]

Honest model: absolute unbreakability is impossible on attacker-owned hardware.  
Strategy = economic (cracking costs more than a licence + cracked copies decay).  
Rust core, encrypted-at-rest models/packs, runtime integrity checks, value decay via signed updates.  
Protection work ≤ 8 % of total effort.  
**AC-14.1 & 14.2** must pass. Data is never ransomed.

---

## 15. Guardrails [NORMATIVE]

G1 No verdicts/guilt/sentencing predictions.  
G2 No citation bypassing retrieval.  
G3 Refuse fabrication/perjury-coaching.  
G4 Export footer required.  
G5 Provenance log for every AI output.  
G6 Preparation language only.  

**AC-15.1:** ≥ 200-prompt red-team suite → zero G1–G3 violations.

---

## 16. Layer 2 — Contracts [NORMATIVE]

SQLCipher (AES-256 + Argon2id). One DB per case + one app DB.  
Canonical schemas (full files in `/contracts`):

- `case_graph.json` (the hand-off artefact between Stage 1 and Stage 2)
- argument nodes, citation records, session turns, lexicon entries, experience records, licence payload

**AC-16.1–16.3** must pass (schema validation, unreadable without passphrase, `.akcase` round-trip).

---

## 17. Layer 4 — Golden Dataset & Test Strategy [NORMATIVE]

Golden datasets are authored **before** code and define correctness.  
Separate India and United States golden sets for triage, chronology, retrieval, audio, and red-team.  
Every AC maps to an executable test in `/eval`.  
An agent’s work order is DONE only when its listed tests pass.

---

## 18. Detailed Step-by-Step Construction Guidance (How to Achieve Perfect Results)

This section is written for AI coding agents and human implementers. Follow the sequence exactly.

### Phase 0 — Preparation (before any code)

1. Create the repository structure:
   ```
   /contracts
   /golden
   /eval
   /src
   /docs
   ```
2. Freeze all JSON schemas in `/contracts` first. No code may invent types.
3. Author the golden datasets (G-DOCS, G-CASE India, G-CASE US, G-AUDIO, G-RETRIEVAL, G-REDTEAM, G-SCRIPTS) **before** writing production code.
4. Write the evaluation harness that runs every AC-x.y as a script. CI must fail if any AC fails.

### Phase 1 — Foundation (M1–M2)

**Goal:** Solid shell, encrypted vault, case intake, and reliable document triage/OCR.

1. Implement Tauri + React shell with strict IPC contracts.
2. Implement SQLCipher vault and case database.
3. Build the New Case wizard (including jurisdiction selector).
4. Build drag-and-drop + folder intake with type tagging.
5. Implement `docpipe` sidecar with triage classifier and confidence scoring.
6. Make all heavy work asynchronous with progress reporting. Never block the UI thread > 200 ms.
7. Run G-DOCS triage tests until AC-6.1 passes.

**Perfect-result rule:** If triage accuracy is below 92 %, do not proceed. Fix the classifier first.

### Phase 2 — Skeleton & Review (M3–M4)

1. Implement skeleton extraction (chronology, parties, statutes, relief, contradictions) into the exact `case_graph.json` schema.
2. Build the side-by-side review surface (page image + extraction).
3. Implement Guided Intake Interview (resumable, skippable, voice-capable).
4. Implement Phone Capture Companion (local Wi-Fi only).
5. Run AC-6.2 through AC-6.8 until every one passes.

**Perfect-result rule:** Every extracted fact must have a clickable source pointer. If a fact has no source, it is a defect.

### Phase 3 — Knowledge & Retrieval (M5)

1. Build the pack system (official signed packs + user-uploadable custom packs).
2. Implement on-device FAISS indexing.
3. Implement the full retrieval-first pipeline with GREEN/AMBER/RED citation verification.
4. Run AC-4.3 and AC-7.2 until they pass on both India and US golden retrieval sets.

**Perfect-result rule:** A fabricated citation must always be RED and excluded unless the user explicitly confirms it.

### Phase 4 — Digest Surface & Export (M6)

1. Build the full Digest UI (chronology, parties, contradictions, interview panel, confidence heat).
2. Build the drafting surface and court-format DOCX/PDF export.
3. Implement the Preparation Completeness Score derived from the graph.
4. Ship Stage 1 (v1.0) only after all Stage 1 ACs pass.

### Phase 5 — Voice & Lexicon (M7)

1. Integrate Whisper + local TTS.
2. Build the editable staging box and lexicon system.
3. Pre-seed the lexicon and implement permanent correction application.
4. Run AC-9.x until voice-only 10-turn rehearsal succeeds with network disabled.

### Phase 6 — Licensing, Protection, Hardening (M8)

1. Implement licence activation, fingerprinting, offline validation, and state machine.
2. Encrypt models and packs at rest.
3. Implement anti-debug tripwires that degrade gracefully (never lock user data).
4. Run network-capture audit (AC-13.1) and copied-folder test (AC-14.1).

**Ship Stage 1 only when every Stage 1 AC is green.**

### Phase 7 — Stage 2 (M9–M13)

1. Build Argument Tree and deterministic Integrity Engine first (AC-8.x).
2. Add text-only Mock Court formats one by one.
3. Add Adaptive Bench.
4. Add voice-in-court and debrief.
5. Add Bench Prep Mode and final red-team hardening.
6. Ship Stage 2 only when AC-15.1 and all Stage 2 ACs pass.

### Continuous Rules for Perfect Results

- Never allow the primary model and vision model to be resident simultaneously on 8 GB machines.
- Every generative output must carry a provenance log (model hash, prompt hash, retrieval IDs).
- Every AC is a script. Opinion is never used to declare “done”.
- If an agent is unsure, it must emit a QUESTION block and stop.
- The Case Graph is the single source of truth between Stage 1 and Stage 2. Protect it.

---

## 19. Pricing & GTM [STRATEGY]

- Digest (Stage 1) is the accessible wedge for solos and chambers.
- Pro adds Stage 2.
- Firm / Institutional / Enterprise add shared archives, priority updates, air-gap SLA, Bench Prep.
- India first for commercial validation, then United States as privacy-first offline offering, then other common-law jurisdictions via additional packs.

---

## 20. Non-goals

No cloud case management, no CLM, no generalist chatbot, no verdict prediction, no marketplace, no named-judge profiling, no automatic decision-making.

---

## 21. Open Questions (to be resolved before the relevant milestone)

1. Exact primary 7–9B GGUF (bake-off on golden legal-reasoning set before M5).
2. Whisper small vs medium (decide on G-AUDIO data).
3. Vision model bake-off before M2 freeze.
4. Initial US state packs (federal + 3–5 major states).
5. Shared-lexicon merge policy (Firm tier).

---

**Document v1.0 — CaseForge**  

This is the binding Product Requirement Document.  
All features and functions are identical across jurisdictions.  
The only difference a user experiences is the knowledge base they choose to install or upload.  

Follow the step-by-step construction sequence in §18 exactly.  
Every AC is an executable gate.  
Never invent.

*End of Product Requirement Document*
