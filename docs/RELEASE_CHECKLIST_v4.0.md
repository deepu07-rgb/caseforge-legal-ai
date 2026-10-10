# CaseForge Stage 4 — Release checklist v4.0

**Batch:** B-S4-23. **Date of this version:** 2026-10-10. **Verdict at this date: NOT READY** (`eval/reports/stage4_ship_gate.json`).
A box is ticked only where the gate or a test has shown it. Nothing here is signed.

## Gate

- [ ] `npm run ship:stage4` exits 0 (READY FOR CONTROLLED TESTING or better) — **currently NOT READY**
- [x] Full red-team suite: zero G1, G2, G3, G7, G8 violations, 487 attack probes (PROPOSED minimum 300, unapproved)
- [x] Production build (`tsc -b && vite build`); the Tauri installer bundle is not built by the gate
- [x] Rust tests pass (234 passed, 0 failed, 1 ignored)
- [ ] Zero required skips — one ignored test (`retrieval::store::tests::b_s4_29_scale_benchmark`) needs a recorded reason
- [x] Every CI gate passes (`ci_gates`)
- [x] No waiver on G1, G7, G8 or the red-team
- [x] Contracts load; migrations consecutive and embedded
- [x] No network primitive in any Stage 4 engine (static); the phone companion is refused under air-gap
- [ ] Measured network capture across the Stage 4 workflows (AC-13.1 / AC-S3-22.2) — **not run**

## Batches

- [x] B-S4-00, 00b, 01, 03–13, 17–23 built
- [ ] B-S4-02 twelve golden sets and the legal-editor protocol
- [ ] B-S4-14 Devanagari triage and OCR
- [ ] B-S4-15 Aligned offline translation and G10
- [ ] B-S4-16 Encrypted Bundle Exchange

## Acceptance

- [ ] Every AC-S4-* measured PASS — 50 of 66 today; AC-S4-00.7 fails; 7 not measurable here; 7 belong to unbuilt batches
- [ ] Every AC-S4-* formally accepted (thresholds approved by the owner, sign-offs, no open owner question)
- [ ] Two legal editors' sign-off for every legal-answer criterion (`eval/signoffs/legal_editors.json` does not exist)
- [ ] Base install within the 4 GiB cap — over the cap by owner waiver W-AC-S4-00.1 (5.08 GiB), reported as WAIVED

## Controlled matters

- [ ] IN matter with a Hindi document and a code-bridge transition (needs B-S4-14 and B-S4-15)
- [ ] US matter with deposition impeachment and a deadline
- [ ] Chambers session

The gate composes these from the per-engine goldens; none has been run end to end through the screens.

## Owner decisions still open

Q-S4-01 thresholds · Q-S4-02 legal editors · Q-S4-04 Indic OCR bake-off · Q-S4-06 edition contents and prices · Q-S4-07 seat-transfer
limit · Q-S4-10 Chambers pilot court · production licence-signing key · release signing key.

## Sign-off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Product Owner | | | |
| Legal editor 1 | | | |
| Legal editor 2 | | | |
| QA | | | |

**Rule:** do not release Stage 4 while `eval/reports/stage4_ship_gate.json` says anything other than READY FOR CONTROLLED TESTING or
better, and do not use it on real cases below READY FOR LIMITED REAL-CASE INTERNAL TESTING. External beta needs a separate audit.
