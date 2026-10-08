# B-S4-08 — Contradiction engine and Impeachment Kit

**Status (2026-10-08): IMPLEMENTED.**
- AC-S4-03.2, AC-S4-03.3 and AC-S4-03.4 PASS.
- AC-S4-03.1 is BLOCKED only because the recall and precision thresholds await owner approval (Q-S4-01). On G-IMPEACH the measured result is 12 of 12 planted contradictions found and no false positives, so recall and precision are both 1.0.
- Kits validate against the full frozen `contracts/impeachment_kit.schema.json`, including its OMISSION and MODEL_VERIFIED rules.

## What a lawyer gets

On **Witnesses**, each witness gets an **Impeachment kit**: a list of points, ordered as a suggested confrontation sequence.

- **Contradictions:** in date, time, place, number of persons, who did it, who was present, order of events, or amount. Each point shows the earlier and the later statement, each quoted exactly with its page (and line for transcripts).
- **Omissions / improvements:** a material fact in testimony that no earlier statement mentions, for example identifying the accused, a weapon, or an injury. Each is labelled *for your judgement*, with a note on which statements are silent.
- **Suggested questions:** commit, credit, confront ("Today you say …", "You gave your statement to police …", "In it you said …"). These are suggestions for counsel, never what the witness should say.
- **Accept / Not useful:** your choice is kept across rebuilds. Rejected points are left out of the export.
- **Export DOCX / PDF:** includes the G4 footer and G5 provenance. Any point from the local AI is listed as an AI-assisted span.
- **Rehearse in Mock Court:** opens Mock Court in the Witness Cross-Examination format with the first three confrontations pre-filled as your draft.
- **Look for contradictions in meaning (local AI):** the model proposes conflicts such as "red truck" vs "blue truck". A proposal is shown **only** if both quotations match two different statements of the witness word for word; everything else is discarded and counted.

## How it works

`src-tauri/src/impeach.rs`:
- **Deterministic layer:** each later statement is compared with the **most recent earlier statement** that speaks to the same matter (two or more shared specific words).
  - Times are compared even without shared words when each statement gives only one time, as in a deposition answer.
  - Times count as different at 45 minutes or more apart.
  - "Near the temple" and "near the Hanuman temple" are the same place, because one contains the other.
  - "Rs 5,000" and "Rs. 5000/-" are the same amount.
  - "Two or three persons" is a range.
- **Omissions:** only testimony kinds (chief, cross, deposition, trial) are checked against prior-statement kinds (FIR, police, Magistrate, affidavit, declaration, deposition), using the vocabulary's flags.
- **Model layer:** the request goes through the existing `infer_llm` contract with `structured_propositions`. Each candidate is a pair of `FACTUAL_BASIS` propositions (`cN-a` earlier, `cN-b` later), checked by the B-S3-14 verifier (`proposition::verify`). Pairs are discarded when:
  - either quote is not exact;
  - both quotes come from the same statement;
  - a statement is unknown;
  - a quote crosses a page break;
  - the pair duplicates a finding already shown.

  Raw model output is stored and **re-verified on every rebuild**. Only quotes render; model prose never does.
- **Confrontation order:** who did it → presence → omissions → date → time → place → sequence → count → amount → meaning. Ties are ordered by position in the later statement.
- **G3:** witness-coaching requests are refused. Covered requests: change, align or script testimony, "tell/coach/train the witness to say…", and "testify that … even though…". Legitimate impeachment preparation is allowed, for example confronting with a prior statement or preparing a witness for the stress of cross. Covered by `golden/G-REDTEAM-S4/fixtures/witness_coaching.json`: 10 refuse, 8 allow.

## Gate

`python eval/checks/impeachment_b_s4_08.py --cargo-log <log> --kits <evidence>/impeachment_kits.json` (runs in CI).

## Limits (v1)

- **Small fixture:** recall and precision were measured on one small synthetic fixture. Real statements need the legal-editor golden set (Q-S4-02), and real FIRs and depositions will show misses (paraphrased places, pronouns instead of names). That is why AC-S4-03.1 waits for thresholds measured on editor-built data.
- **Name matching:** the "who did it" and "who was present" checks rely on proper names. "He", "they" and "the accused" are not resolved.
- **Mock Court hand-off:** the kit pre-fills counsel's draft. The Mock Court reply engine itself does not yet read the kit.
- **Coaching suite scope:** the witness-coaching suite is a slice of G-REDTEAM-S4. The full suite is authored in B-S4-02 and run in B-S4-23.
