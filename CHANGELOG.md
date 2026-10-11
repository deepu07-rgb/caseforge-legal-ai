# Changelog

## [4.0.0-dev] — 2026-10-11 (expired licence is read-only everywhere)

An expired licence now makes every write to a matter fail with `CF_VAULT_READ_ONLY`, not just three: it puts each open matter database in SQLite's query-only mode from one place. Reading, listing and exporting still work; a valid licence lifts it. Found by driving the real app, where a document upload still worked on an expired licence. Test: `vault::tests::an_expired_licence_makes_every_matter_write_read_only_and_a_valid_one_lifts_it`; the editions gate (AC-S3-22.8) requires it.

## [4.0.0-dev] — 2026-10-11

### Stage 4 — Hindi to English translation and guardrail G10 (B-S4-15)

A lawyer reads the Hindi original and the English side by side, and nothing translated is passed off as verified.

- Translation tab: segment by segment, original beside English, every English segment labelled "machine translation - wording not verified"; resumable overnight batch; stoppable; runs on the primary model's slot.
- Dates, amounts, section numbers and case numbers are set aside before the model sees the text and written back by fixed rules, so the model cannot change a number; a marker it drops or invents flags the field. Names get a suggested spelling and always wait for counsel. Counsel confirms each field against the original (append-only history).
- Guardrail G10 in the host verifier: anything resting on translated text is never GREEN, a quotation of it is never verified, a translated source cannot launder a fact, and the label follows an inference.
- Facts read from a translation are kept as a separate graph, every item TRANSLATED and unconfirmed (new coexisting schema `case_graph_v2_1`); the 2.0.0 schema is untouched.
- Real-model run recorded (qwen2.5:7b): 33 of 34 markers carried intact, the dropped one flagged; prose quality is modest, which is why the original is always shown. Reference translations are the coding agent's.
- AC-S4-09.2 and 09.3 measured PASS, formally BLOCKED (98% limit and translation model are the owner's). `docs/B-S4-15_translation.md`. All 25 Stage 4 batches are now built; the ship verdict is still NOT READY.

### Stage 4 — Hindi (Devanagari) ingestion (B-S4-14)

Scanned Hindi pages are read offline, with a confidence on every word and a script label on every page and line.

- Tesseract OSD picks the reader: Devanagari goes to `hin+eng` (`tessdata_best`, Apache-2.0, 11.9 MB); English pages keep the frozen English path unchanged (same CER/WER as B-S3-04). A rotation is only applied when the detector is confident.
- Bake-off (`eval/bakeoff/indic_ocr_b_s4_14.json`): `hin+eng` best gives CER 0.0056 clean / 0.0024 degraded on the eight synthetic G-INDIC pages (a floor: rendered, not photographed). The error limit is the owner's to freeze (Q-S4-04); AC-S4-09.1 is BLOCKED.
- Critical fields in Devanagari (dates, amounts in lakh/crore, dhara numbers, labelled case numbers, names): a misread one is held for review, never accepted (58 labelled; 4 OCR misreads, all held). Indian grouping `1,25,000` now validates in the English validator too.
- Hindi confidence map (synthetic, held-out ECE 0.018 vs 0.051 raw), page script on the review screen, `docs/B-S4-14_indic_ocr.md`.
- Also pinned for B-S4-15: the translation golden `golden/G-INDIC/fixtures/translation_core.json`.

## [4.0.0-dev] — 2026-10-10 (golden sets and legal-editor protocol, B-S4-02)

All twelve Stage 4 golden sets are now under one manifest with hashes and provenance (`golden/STAGE4_MANIFEST.json`); the missing `G-INDIC` set exists as a blocked scaffold; `docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md` says who signs the legal answers and how a sign-off is recorded and counted. No legal editor has signed, so the code-bridge, foundation and deadline criteria stay blocked, and no fixture is locked GOLDEN. See `docs/B-S4-02_goldens.md`.

## [4.0.0-dev] — 2026-10-10 (Encrypted Bundle Exchange, B-S4-16)

`.cfbundle`: a case-work bundle encrypted to a colleague's key (X25519 + AES-256-GCM), signed by the sender, with irreversible byte-level redaction, hand-off on a USB stick or once across the local network, and import with attribution that never overwrites the recipient's own corrections. New *Share with colleagues* tab in the matter. The LAN-only packet capture has not been made, so that criterion stays blocked. See `docs/B-S4-16_bundles.md`. The Stage 4 ship gate is still NOT READY.

## [4.0.0-dev] — 2026-10-10 (Timed Oral Rehearsal, B-S4-12)

Mock Court gains a timer mode: a host-kept clock with presets, graceful end at the limit, bench interruptions from your own Missing Links (host-worded, not model-worded), and a debrief timeline of which points were reached, when, for how long, and which never were. See `docs/B-S4-12_timed_rehearsal.md`. Verdict of the Stage 4 ship gate is still NOT READY.

## [4.0.0-dev] — 2026-10-10

### Stage 4 — ship gate (B-S4-23), verdict NOT READY

Not a release. The Stage 4 cumulative gate now exists and says so plainly.

- `npm run ship:stage4` (`eval/run_stage4.py`): production build, Rust tests, every CI gate, then the ship gate.
- Full red-team suite run against the real filters: 487 attack probes across G1, G2, G3, G7, G8 (lawyer and Chambers), zero violations.
- Report: `eval/reports/stage4_ship_gate.json` (hash-fixed, unsigned: no release key exists). Checklist: `docs/RELEASE_CHECKLIST_v4.0.md`.
- Why NOT READY: two batches not built (B-S4-14, 15), AC-S4-00.7 fails, no legal-editor sign-off, no network capture,
  controlled matters not end to end. See `docs/B-S4-23_ship_gate.md`.

### Stage 4 — editions (B-S4-22)

Edition entitlement map (development-only), pack subscription separate from the app licence, offline seat transfer.

## [2.0.0-rc1] — 2026-08-09

### Stage 2 Mock Court — release candidate

First **Stage 2** release candidate: Digest Engine + Mock Court (Full tier for live voice).

#### Ship gate

- `npm run ship:stage2` / `npm run test:ship2`
- Report: `eval/reports/stage2_ship_gate.json`
- Checklist: `docs/RELEASE_CHECKLIST_v2.0.md`
- Waivers: `eval/waivers/stage2_ship_waivers.json` (AC-15.1 **not** waived)

#### Stage 2 matrix (all green)

- B-S2-01 Integrity · B-S2-02 formats · B-S2-03 Adaptive Bench  
- B-S2-04 Voice court · B-S2-05 Bench Prep · B-S2-06 Experience + AC-15.1  
- AC-2.1 docpipe dependency audit  

#### Tier messaging

- **Lite:** no live Mock Court voice simulation (honesty refuse)  
- **Full:** Digest + Mock Court text/voice/bench prep  

## [1.1.0-dev] — 2026-08-09

### Stage 2 — B-S2-06 Experience S2 + AC-15.1 full red-team

- Experience Store unlocks **limb_outcome** + **bench_setting** (stage S2)
- Full red-team suite `caseforge.redteam.stage2.v1` (210 prompts) — zero G1–G3 violations
- Fixture `golden/G-REDTEAM/fixtures/stage2_200`; `npm run test:s2redteam`

### Stage 2 — B-S2-05 Bench Preparation Mode

- Dual-counsel simulation (`side_a` / `side_b`); never the decision-maker
- G1 enforcement: refuse holdings, court orders, verdicts
- Non-decision-maker UI banner + G1 red-team suite
- `npm run test:benchprep`

## [1.1.0-dev] — 2026-08-08

### Stage 2 — B-S2-04 Voice-in-court + debrief

- Full-duplex English path: STT (lexicon) → Mock Court → TTS roles
- Structured debrief after multi-turn rehearsal
- Lite tier honesty refuse (`CF_VOICE_COURT_LITE_REFUSE`); AC-3.3 budget
- Mock Court **Voice duplex** mode; `npm run test:voicecourt`

### Stage 2 — B-S2-03 Per-Case Adaptive Bench

- Six generic archetypes + five dial-in sliders (incl. opposition aggression)
- Case-order signals from this case’s `case_graph` only (AC-10.2)
- Hard refuse named-judge fields (AC-10.1 / §20)
- Mock Court **Adaptive Bench** tab; `npm run test:bench`

### Stage 2 — B-S2-02 Mock Court text formats

- Frozen `session_turn.schema.json` v1.0.0 (Q-ST-01)
- Five sequential formats: Bail/Interim → Final Arguments → Witness Cross → Appellate → Motion
- Pipeline `caseforge.mock_court.v1` — case_graph + retrieval only (AC-2.1)
- G-SCRIPTS format fixtures + `npm run test:mockcourt`
- Mock Court UI wired to host graph-only turns

### Stage 2 — B-S2-01 Argument Integrity Engine

- Frozen `argument_node.schema.json` v1.0.0 (Q-AN-01)
- Deterministic scorer `caseforge.integrity.v1` — VI + Missing Links (no LLM)
- Build Argument Tree from `case_graph` only (AC-2.1)
- GOLDEN `G-INTEGRITY` planted-gap fixture (AC-8.1)
- UI: **Argument Tree** nav surface
- Eval: `npm run test:integrity`

## [1.0.0-rc1] — 2026-08-08

### Stage 1 Digest Engine — release candidate

First commercial-track **Digest** release candidate for CaseForge offline legal intelligence (India + United States dual-market architecture).

#### Added

- Encrypted SQLCipher vault (app + per-case), New Case wizard (jurisdiction-first)
- Document intake, docpipe triage/recognition, skeleton → `case_graph`
- Review, guided interview, overnight batch resume, LAN phone capture
- Knowledge packs (Ed25519), retrieval + GREEN/AMBER/RED citations
- Digest surfaces (CASEBOOK · CHAMBER · DIGEST · DRAFTING) + prep completeness score
- DOCX/PDF export with G4 footer and G5 provenance
- Voice dictation + lexicon (English primary)
- Experience Store (local, top-K injection; fleet off by default)
- Offline licensing (3-of-4 fingerprint, EXPIRED = read-only vault)
- Air-gap mode; pack/model AES-256-GCM protection (never ransoms user data)
- Model choreography (one large model; status chip)
- Guardrails G1–G6 + Stage-1 red-team suite (24 prompts)
- Hardware gate: refuse &lt;4 GB / 32-bit; Lite/Full messaging
- Ship gate: `npm run ship:stage1`

#### Stage 2

Mock Court / Argument Tree **not** default-on. Track S2 begins only after this ship gate.

#### Known waivers

See `eval/waivers/stage1_ship_waivers.json` (lab timing ACs, WER, `.akcase`, full 200 red-team).

---

## [0.1.0] — 2026-08

Development batches B-S0 through mid Stage 1 (pre-RC).
