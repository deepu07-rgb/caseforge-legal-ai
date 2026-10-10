# Stage 4 Wave C — pilot-readiness note (Q-S4-10)

For the owner and for the court that will pilot the Chambers Edition. The machine-readable checkpoint is
`eval/reports/stage4_wave_c.json`; this note says what it means. Nothing here is a claim of approval: thresholds are still
PROPOSED, legal editors have not signed any rule table, and the pilot court is not yet named.

## What a judge or clerk gets

A judge-facing edition of the same app, offline, with five screens: **Chambers** (record-only questions over the record),
**Issue Matrix** (each accepted issue side by side: positions, agreed and disputed facts, unanswered arguments, a bench memo),
**Cause List** (one printable page per listed matter), **Judgment** (a scaffold filled from the record where it can be) and the
existing Case Digest, Legal Authorities and Deadlines. Advocacy screens (Mock Court, Opponent's Filing, Witnesses, Argument
Builder and Analysis, Voice) are hidden and refused by the host.

| Promise | How it is enforced | Gate |
|---|---|---|
| Nothing outside the record or installed law is shown as fact | every model statement is verified word for word against a passage and classed RECORD / LAW / INFERENCE; the rest is withheld, counted and logged by hash | B-S4-17, AC-S4-11.1/11.2 |
| The model is reachable only through that path | a gate in front of every model call refuses everything else in Chambers; custom packs never supply law or record | B-S4-17 |
| The tool never writes analysis, findings, holdings or orders, and never says who should win | requests refused before any model call; decisional statements withheld; the three locked sections have no code path to a model; a source scan finds every model call site and checks none can read the scaffold | B-S4-21, AC-S4-15.1/15.2 |
| Every line traces to a document | locators are exact slices of the record, checked in tests; the lookup returns passages verbatim, never prose | AC-S4-13.2, 14.1, 15.3 |
| No ranking or favouring of a party | same rules for each party; a swap test mirrors the result exactly; no ranking words in the engine or screens | AC-S4-12.3 |
| Offline, no telemetry | a Judiciary licence forces the air-gap on and it cannot be switched off; no engine contains a network call | B-S4-17 |

## What was measured, and what it is worth

All measured on **synthetic** records authored for the tests. They show the rules work as written; they do not show how the
tool performs on your court's real filings.

- 210 Rust tests pass; every CI gate passes locally; the frontend type-checks. GitHub Actions has not run them (billing).
- Record lookup: the answering passage was in the top five for 24 of 24 questions (proposed threshold 90 %).
- Cause list: 100 matters rendered in about 0.6 s on the development machine (proposed 60 s on the Full reference machine).
- Decision-free zone: 113 of 113 requests for a decision refused (proposed minimum 100); 32 of 32 ordinary record questions
  passed; 40 of 40 decisional statements withheld. The set was written by the same author as the classifier.
- Issue matrix: planted unanswered arguments listed in 2 of 2 fixtures; admitted/disputed split exact on both.

## One measured failure you should know about

**AC-S4-00.7 FAILS**: on the held-out prompt-injection set the default 7B model scored 7 of 8 (case INJ2-US-1 failed). Chambers
does not rely on the model resisting injection (the host verifies every statement and the model never sees the judge's own text),
but this is a real result, not a pending one.

## What is not ready

- **Wave B is now built** (Devanagari OCR B-S4-14 and Hindi translation B-S4-15, with Timed Oral Rehearsal B-S4-12 and Encrypted Bundle
  Exchange B-S4-16), but not accepted: the Hindi OCR error limit is not frozen (Q-S4-04), the translation model is the default 7B reasoner and
  reads Hindi legal prose only modestly, and both were measured on synthetic pages. For a court that files in Hindi, run a pilot on real scans first.
- **No signed rules.** Limitation, admissibility and code-succession tables are development-only; no legal editor has signed one.
  The Deadlines and Foundation Gaps screens say so. Issue-framing templates are not shipped (they need the text of Order XIV rule 1 CPC and editor sign-off).
- **Editions and licences (B-S4-22)** — updated after the Wave C checkpoint: the mechanism is now built (an edition entitlement
  map, a pack subscription separate from the app licence, offline seat transfer; see `docs/B-S4-22_editions.md`), but what each
  edition includes and its price are still undecided (Q-S4-06), so the map is marked development-only, and a production
  licence-signing ceremony has not happened. Today only development keys exist; a pilot install needs a licence issued
  with `eval/tools/make_licence.py --edition JUDICIARY`, signed by a key that must not be the development key before any real use.
- **Real formats unknown.** The importer reads a CSV or numbered lines, and orders headed "Order dated <date>" / "Order of <date>".
  The court's real cause list and order sheet will need their patterns added (a data/regex change, then re-test).
- **Not measured on your machine:** the real 7B model's reasoning quality, speech-to-text on real audio in chambers, install size
  (the base install is above the 4 GiB cap by an owner-waived margin, W-AC-S4-00.1), memory ceiling (Q-S4-12).

## What the court needs to tell us (Q-S4-10)

1. Which court or chamber, and its policy on AI-assisted tools. Capabilities can be switched off per court without code, in
   `chambers_pilot.json`: `reasoning`, `issue_matrix`, `memo_export`, `citation_audit`, `record_lookup`, `cause_list`. If the
   policy forbids something else, the rule is to disable it, not to work around it.
2. A sample cause list and order sheet in the real format (anonymised is fine) so the importer can be fitted and re-tested.
3. Whether scanned or Hindi filings are in scope (then B-S4-14/15 become blocking).
4. Who may hold the licence and where the machine lives (air-gapped, institution-issued).

## Suggested first-day checks on site

Install offline; activate the Judiciary licence; confirm the Chambers screens appear and the advocacy ones do not; try a
request for a decision (it must be refused with no delay from a model); load one real matter, assign the filings in the Issue
Matrix, run "Citations in every filing", ask three record questions, print one cause-list page and one scaffold, and read the
export footer. Anything surprising should be written down against the section above before the pilot begins.
