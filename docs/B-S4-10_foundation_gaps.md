# B-S4-10 — Admissibility & Foundation Gaps

Shows, for each exhibit in the matter, which foundation elements the law requires before it can be offered and which of
them are on file — for example "this electronic record has no section 63(4) certificate on file" — citing the rule and
the text of the section. These are prompts for counsel, not rulings on admissibility, and **they change no score**.

## How it works (`src-tauri/src/foundation.rs`)

1. **Evidence type.** PROPOSED from the words in the item's label, kind and file name, using the vocabulary in
   `packs/builtin/evidence_types.v1.json` (data, not code): electronic record, copy of a document, expert opinion. Whole
   words only ("digitally" is not "digital"); the type with most keyword hits wins, ties go to the earlier type. Counsel
   confirms or changes it (`foundation_set_type`) and it becomes COUNSEL_CONFIRMED. An item matching no type has no rule.
2. **Rows.** The `ADMISSIBILITY_REQUIREMENT` rows for the type in force on each date checked: today and, if recorded, the
   date proceedings were instituted (Code Bridge key date), so a matter between the old and new code sees both.
3. **Each required element** is
   - PRESENT when counsel linked a document to it (`foundation_link`), when a graph node of a required type (for example a
     witness) is connected to the item by an edge that has not been rejected, or when the only item of that type in the
     matter has a document tagged with one of the element's tags;
   - UNKNOWN when such a tagged document exists but could belong to several items (link it);
   - ABSENT otherwise.
   Records follow the frozen `contracts/foundation_check.schema.json`; every element carries the authority it comes from.
4. **Gaps and the panel.** Each ABSENT or UNKNOWN element is a gap with the rule, its trust state, the authority and the
   cited sentence. `merge_panel` puts ABSENT gaps first in the Missing Links panel (gaps on a counsel-confirmed type before
   gaps on a proposed type); UNKNOWN ones are listed separately for counsel to resolve; the existing link-score entries
   follow in their old order.

## What is deliberately not done

The integrity v2 *score* (a foundation factor in the Evidence link) is frozen in the contract as needing **owner approval
and an `argument_node` schema bump before any scorer code**. This batch writes no scorer code and touches no score; the v1
scorer file is pinned by hash in the gate (AC-S4-05.2). If you approve a rule such as "an absent mandatory element caps the
limb's Evidence link below the Missing Link threshold", it is a small change on top of this.

## Data shipped

`packs/official/caseforge.india.admissibility-rules` (development): verbatim IEA s.65B, s.65, s.45 and BSA s.63, s.60, s.39
(cut from the signed criminal-codes pack) and six `DEVELOPMENT_ONLY` rows, each citing the exact sentence: the certificate
for an electronic record (IEA s.65B(4); BSA s.63(4), which must accompany the record each time it is offered), a ground for
giving a copy instead of the original (IEA s.65; BSA s.60), and a showing that an expert is specially skilled (IEA s.45;
BSA s.39). Effective dates are for a legal editor to confirm. Not covered: other kinds of evidence, case-law gloss (for
example when a section 65B certificate can be dispensed with), state amendments, US rules.

## In the app

Argument Analysis (left menu), above the Missing Links list: the gaps with their rule and cited text; "All exhibits" lets
counsel confirm the kind of each item and link a document to an element. Commands: `foundation_check`,
`foundation_set_type`, `foundation_link` (choices stored in the matter's encrypted vault).

## Evidence

- Golden **G-ADMISS** (11 cases + the panel ordering; pinned before the engine): missing certificate; linked by counsel; a
  single tagged certificate attaching; two records and one certificate needing a link; counsel-confirmed type overriding
  the words; an unclassified item; a copy without a ground; an expert partly founded through the graph; a rejected edge not
  founding an element; an earlier code on an earlier date; both codes when both dates are given.
- `shipped_admissibility_rules_install_cite_the_right_text_and_check_a_matter`: the real pack through the real installer;
  each row's first citation contains the section's sentence; a WhatsApp screenshot with no certificate is a gap under the
  Sakshya Adhiniyam today and under the Evidence Act on an earlier date; a tagged certificate founds it.
- `eval/checks/foundation_b_s4_10.py`: pins (golden, vocabulary, v1 scorer file), every check validated against the frozen
  schema with all three statuses and both type states exercised, static properties.
- **AC-S4-05.2 PASS** (v1 scorer pinned, its tests pass). **AC-S4-05.3 PASS** (every element and gap cites a rule row and an
  authority). **AC-S4-05.1 BLOCKED**: measured against legal-editor-approved rows (Q-S4-02); the engine puts a planted gap
  in the top 3, but the shipped rows are unsigned.
