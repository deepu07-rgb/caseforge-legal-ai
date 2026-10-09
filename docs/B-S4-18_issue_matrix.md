# B-S4-18 — Issue Matrix

A neutral, side-by-side view for the judge of each issue they have accepted: what each party has pleaded, which facts the
filings agree on, which they dispute, and which arguments one side never answered, each pointing to where it is on the
record. It ranks nothing, scores nothing and suggests no answer to any issue. The same rules run for every party.

## How it works (`src-tauri/src/issue_matrix.rs`)

Everything is deterministic — no model, no network — so text inside a filing (including instructions aimed at AI systems)
cannot steer it.

1. **Issues.** Proposed from the installed `ISSUE_FRAMING_TEMPLATE` rows for the matter type and from `issue` nodes in the
   case graph; the judge can also write their own. Nothing proposed is used until the judge **accepts**, **edits**,
   **merges** (into an accepted issue) or **rejects** it. Only ACCEPTED and EDITED issues get content; the others carry
   empty positions, facts and arguments, as the contract requires. An issue others are merged into cannot be rejected
   until they are sent back.
2. **Filings.** The documents counsel assigns to a party (an opponent's filing tagged `EXTERNAL_FILING` is read through the
   existing leak-barrier path; nothing is written to the case graph). Each filing is split into numbered paragraphs by the
   same splitter the Opposing Filing Scan uses, and each paragraph is a fact, an argument or neither.
3. **Positions.** A paragraph belongs to an issue when it shares at least 2 keywords with the issue text; issues merged
   into it add their paragraphs. Every party always gets a position, in the order given, so both sides are always shown.
   Each position lists the party's own paragraphs (quoted whole, with a locator), the evidence they refer to ("Annexure
   P-1", "Exhibit P-2", located in the paragraph), and the audit result of the citations inside them (the filing is audited
   against the installed law with the same engine as the Citation Audit). Sentence classes are `RECORD` (a statement of
   fact), `LAW` (carries an audited citation) and `INFERENCE_FROM_RECORD` (a contention); they say what kind of statement it
   is, never whether it is true.
4. **Agreed and disputed facts.** A paragraph that names "paragraph N" of the other side's filing responds to it: it admits
   it ("admitted", "not disputed"…) or disputes it ("denied", "does not admit"…). A response that does both is listed as
   disputed. Where nothing is cross-referenced, fact paragraphs of different parties are paired when they share at least 3
   keywords: one denies and the other does not → both are disputed; neither denies → both are agreed. Only paragraphs of fact
   count.
5. **Unanswered arguments.** An argument is unanswered by a party that has filed something when none of that party's
   paragraphs refers to it and none of their paragraphs on the same issue shares at least 3 keywords with it. A party with
   no assigned filing is never shown as having failed to answer; the screen says no filing is assigned.
6. **Bench memo.** Plain text with the accepted issues only, each statement with its document, page and character range,
   the G4 footer (identical to `contracts/constants.json`) and a G5 provenance line. Nothing in it is machine-written text;
   it says so.

## In the app

Left menu → **Issue Matrix**: assign filings to parties; accept, reword, merge or reject proposed issues, or write your
own; for each accepted issue read both positions, the agreed and disputed facts and the unanswered arguments;
**Download bench memo (.txt)**. Commands: `issue_matrix_build`, `issue_matrix_add`, `issue_matrix_decide`,
`issue_matrix_set_filing`, `issue_matrix_set_matter_type`, `issue_matrix_memo` (choices stored in the matter's encrypted
vault).

## Limits (stated plainly)

- Matching is by shared wording and explicit "paragraph N" references. A response that answers without using the other
  side's words and without naming the paragraph is missed, so "not answered" means "no answer found by these rules" — the
  judge reads the paragraph the locator points to. Ranges ("paragraphs 3 to 5") and numbered sub-paragraphs are not read.
- Only paragraph-numbered pleadings are read. Narrative briefs without numbered paragraphs yield no positions.
- The kind of statement is decided by the same words as the Opposing Filing Scan (see its limits).
- Edition gating (showing this only in the Chambers Edition) waits for B-S4-17; until then it is a menu item for every user.
- No `ISSUE_FRAMING_TEMPLATE` pack is shipped yet. The engine reads any installed template table; a development pack needs
  the text of Code of Civil Procedure Order XIV rule 1 (framing of issues) as its cited authority, which is not in an
  installed pack, and two legal editors' sign-off (Q-S4-02). Until then proposals come from the pleadings' issue nodes and
  the judge's own issues.

## Evidence

- Golden **G-CHAMBERS** (pinned before the engine, `6c78a28` and the following commit): *core* — a partition suit with
  cross-referenced pleadings: a fact admitted by reference, a fact denied by reference, an argument for an account of rents
  and an argument of suppression that the other side never answers, a template issue, a graph issue, an edited issue, a
  judge-authored issue, a proposed issue, a rejected issue and a merged issue; *fallback* — a sale suit with no
  cross-references: the sale deed stated by one side and denied by the other (disputed), the payment of consideration stated
  by both (agreed), and one unanswered argument.
- Property tests: positions, evidence, authorities and sentence classes from the paragraphs; merging; a party with no
  filing; swapping which party filed which pleading mirrors the result exactly (neutrality); refusals with reasons;
  proposals skipping decided issues; templates read from installed tables for the matter type; bench memo vocabulary and
  footer; audit positions mapped back to pages.
- `eval/checks/issue_matrix_b_s4_18.py`: pins, every matrix validated against the frozen `contracts/issue_matrix.schema.json`
  (every party shown on every accepted issue; constant neutrality block), static properties (no model or network, no graph or
  score writes, no ranking or favouring words in the engine or the screen).
- **AC-S4-12.1 PASS** (planted unanswered argument listed in 100 % of fixtures). **AC-S4-12.2 PASS** against the golden
  (exact agreement); the 95 % threshold is PROPOSED (Q-S4-01) and the golden is synthetic, so re-measure on judge-annotated
  pleadings. **AC-S4-12.3 PASS** (party-swap mirror test, no ranking words, constant neutrality block).
