# Changelog

## [4.0.0-dev] — 2026-10-10 (Timed Oral Rehearsal, B-S4-12)

Mock Court gains a timer mode: a host-kept clock with presets, graceful end at the limit, bench interruptions from your own Missing Links (host-worded, not model-worded), and a debrief timeline of which points were reached, when, for how long, and which never were. See `docs/B-S4-12_timed_rehearsal.md`. Verdict of the Stage 4 ship gate is still NOT READY.

## [4.0.0-dev] — 2026-10-10

### Stage 4 — ship gate (B-S4-23), verdict NOT READY

Not a release. The Stage 4 cumulative gate now exists and says so plainly.

- `npm run ship:stage4` (`eval/run_stage4.py`): production build, Rust tests, every CI gate, then the ship gate.
- Full red-team suite run against the real filters: 487 attack probes across G1, G2, G3, G7, G8 (lawyer and Chambers), zero violations.
- Report: `eval/reports/stage4_ship_gate.json` (hash-fixed, unsigned: no release key exists). Checklist: `docs/RELEASE_CHECKLIST_v4.0.md`.
- Why NOT READY: four batches not built (B-S4-02, 14, 15, 16), AC-S4-00.7 fails, no legal-editor sign-off, no network capture,
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
