# B-S4-20 — Daily Cause-List Brief

One page per listed matter, every line traceable to an order or document. Chambers Edition only (the **Cause List** screen).

## How it works (`src-tauri/src/cause_list.rs`)

1. **Import.** A cause list comes from a CSV (columns serial, case number, cause title; a header row is recognised), from a
   PDF/text file whose matters are numbered lines ("1. Ravi Kumar v. Anil Kumar CS 112/2023"), or is typed in. Nothing is sent
   anywhere; there is no court-portal integration.
2. **Matching.** A case number that belongs to exactly one matter decides. Otherwise cause titles are compared by shared words
   (v, vs, of, and, others... ignored): one clearly best matter (at least 0.8, and 0.2 ahead of the next) is *matched*; two or
   more plausible matters (at least 0.5) are *ambiguous* and wait for the judge to choose; nothing close is *no local record*.
   A choice is accepted only if it is one of the candidates.
3. **The brief** is built only from the orders on the record. An order is a block of a document that begins "Order dated <date>"
   or "Order of <date>" (and the blocks after it up to the next heading); dates may be written "12 March 2024", "March 12, 2024",
   "12.03.2024" (day first, month first for a US matter) or ISO.
   - **Stage:** the newest order that uses stage wording (`packs/builtin/case_stages.v1.json`, data, not code) names the stage,
     and the stage cites that order.
   - **Last order and its date:** the newest order, whole.
   - **Directions in the last order:** each sentence that directs something ("shall", "directed to", "must"...). Each is
     *shown done on the record* only when a later **dated** sentence in a document that is not an order sheet uses a completion
     verb (deposited, filed, produced...), is not negated, is dated on or after the order and shares at least three content
     words with the direction; otherwise it is *not shown on the record as done*, never "not done".
   - **Adjournments:** every order that adjourns the matter, counted; who sought it appears only where the order says ("at the
     request of the defendant", "on the application of the prosecution"); otherwise the order is said not to say.
   - **Applications:** each application number ("IA No. 45/2024") whose latest mention in the orders is not a disposal
     (allowed, dismissed, rejected, withdrawn...).
4. Every field is either FOUND with a locator (document, page, character range, exact excerpt) or NOT_FOUND_IN_RECORD with
   nothing. No model is used; nothing is summarised.
5. **Print:** one page per matter with the export footer. Matters whose vault is not open cannot be read; the screen names them.

## Evidence

- Golden **G-CAUSELIST** (pinned before the engine, `fd0c765`): the same list as CSV and as numbered lines; five synthetic
  matters: a civil one with four orders, an unfulfilled direction, an application still pending, three adjournments (two with the
  requester stated) and a trap (a pleading saying the defendant "failed to produce"); a criminal one whose direction a later
  document shows done and whose application was dismissed; two matters a listing cannot tell apart (before and after the judge
  chooses); a listing with no local record; a matter with no orders. Expectations are hand-authored.
- Rust tests: import; matching and every brief field against the golden (including that each locator is the record's exact text);
  every field cites or says not found; negated, undated, earlier or thinly related sentences do not show a direction done; dates
  and sentence splitting; a 100-matter list rendered within the budget; no model or network.
- `eval/checks/cause_list_b_s4_20.py`: pins, both brief documents validated against the frozen schema, static properties.
- **AC-S4-14.1 PASS. AC-S4-14.2 PASS** against the PROPOSED 60 s, measured on the test machine: 100 matters in about 0.6 s; re-measure on the Full
  reference machine.

## Limits

- The pilot court's real cause-list and order-sheet formats are not available (Q-S4-10, a QUESTION): orders are recognised by the
  heading above, lists by CSV or numbered lines. A different layout needs its heading or line pattern added.
- Scanned order sheets are read through their trusted text layer only.
- Stage, direction and completion wording is English; adding a court's own wording is a data change for stages and a code change for directions.
