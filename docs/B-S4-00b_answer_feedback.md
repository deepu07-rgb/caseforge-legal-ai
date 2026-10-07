# B-S4-00b — Per-answer feedback in Mock Court

**Status (2026-10-08): IMPLEMENTED — all five gates PASS** (Rust tests 85/85; `npm run build` clean).

## What a lawyer sees

Under every answer in Mock Court, an **Answer check** box, computed *before* the next challenge:

| Check | Statuses | Basis |
|---|---|---|
| Dates in the answer | Supported by record · **Conflicts with record** (names the event and the recorded date) · Not found in record | case-graph chronology, matched by date and by the words around it |
| Amounts (Rs/INR/₹/$/USD, Indian digit grouping) | Supported by record · Not found in record | chronology, evidence, contradictions |
| Authorities cited | In working set · **Not verified** | counsel-confirmed working set; reporter citations (SCC, AIR, INSC, U.S., F.) and "X Act, 1893, s. 4" style statutes |
| What the answer left unused | Record contradiction on the challenged point · relevant working-set authority | only relative to the actual previous challenge |
| Engagement | Engages / Partly / Does not engage the challenge's specifics — with the term counts | shared specific terms and dates |

Every line names its record reference and page. There is **no model call, no score, no "right/wrong" and no outcome language** (prohibited words are enforced by the tests).

## How it works

`src-tauri/src/answer_feedback.rs` (deterministic). `mock_court_turn_for_case` grades counsel's text against the previous rendered challenge (kept per session in host memory, like the turn history), then generates the next challenge and remembers it. The result travels in `GroundedTurnResponse.answer_feedback` and is rendered by `AnswerFeedbackPanel` under counsel's message.

Discipline: contract `contracts/answer_feedback.schema.json` and the golden fixture `golden/G-ANSWER-FEEDBACK/fixtures/partition_appeal.json` (four turns: good answer, wrong date + fake citation + ignored challenge, working-set statute + unknown amount, opening statement) were written and pinned **before** the engine.

## Gates

`python eval/checks/answer_feedback_b_s4_00b.py --cargo-log <cargo test --lib output>` — CI runs it from its own cargo log.

| AC | Result |
|---|---|
| 00b.1 golden turns reproduce exactly | PASS |
| 00b.2 planted wrong date → CONFLICTS_WITH_RECORD on the right event | PASS |
| 00b.3 working-set vs. outside authority | PASS |
| 00b.4 model-free, no outcome/prohibited language, record refs | PASS |
| 00b.5 returned with every grounded turn, rendered under counsel's message | PASS |

## Known limitations (v1)

- Engagement is a word/date-overlap heuristic shown with its counts — a prompt to reflect, not a grade.
- Amounts are checked for presence only (no "conflicting amount" detection yet); dates without a year, relative dates ("two days later") and numeric dates like 05/01/2021 (ambiguous dd/mm vs mm/dd) are not parsed.
- Witness-statement matching, legal-element coverage and model-written suggestions are out of scope here (later: B-S4-07/08 matrix, verified model suggestions through the B-S3-14 ledger).
- Exercised by unit tests and wiring checks; not yet used by a lawyer in the desktop app.
