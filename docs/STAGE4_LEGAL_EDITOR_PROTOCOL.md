# Stage 4 legal-editor protocol

For the owner and for the people who will act as legal editors. It says who signs, what they check, how a sign-off is recorded, and what a sign-off
does and does not mean. Nothing in it names anyone: **no legal editor has been named (Q-S4-02), so no sign-off exists and every criterion that
rests on a legal answer is `BLOCKED`.**

## Why this exists

Three Stage 4 features give answers that are matters of law, not of software: **Code Bridge** (which section of the new code replaces which section
of the old, from which date), **Admissibility & Foundation Gaps** (which foundation a given kind of evidence needs), and **Deadline & Limitation
Watch** (how long, counted how, with which days excluded). A test can show the software does what the table says; only a lawyer can say the table
is right. The PRD (§13) therefore requires a named legal editor's sign-off per jurisdiction on every legal answer in `G-CODEBRIDGE`, `G-ADMISS` and
`G-DEADLINE`; without it the criteria `AC-S4-04.1`, `AC-S4-05.1` and `AC-S4-06.1` stay `BLOCKED`, never `PASS`.

What exists today is **mechanism, not law.** The fixtures use an invented jurisdiction (“Demoland”) or development-only rows, so that they assert no
real law. A sign-off on them would show that the *shape* is sound; it would not make any real mapping, requirement or window correct. The work an
editor is needed for is the real-jurisdiction tables and fixtures (see “Real jurisdictions” below).

## Who signs

* **Two editors per jurisdiction**, each a person with a named identity, who is qualified to state the law of that jurisdiction (the owner decides
  the qualification; suggested: a practising advocate or attorney admitted there, a retired judge, or a law academic who teaches that area).
* **Independent of the author.** A coding agent writes fixtures and may never lock them (`golden/POLICY.md` §1). An editor does not review a fixture
  they wrote themselves; the second editor is a different person from the first.
* **Per jurisdiction.** An editor who is expert in Indian procedure does not sign a US-state deadline table, and vice versa. A US sign-off names the
  state or the federal system.
* **No conflicts.** An editor who acts in a matter that a fixture is modelled on, or has an interest in a product that depends on the answer, says so
  and does not sign.

## What they review

For each legal-answer fixture in their jurisdiction:

| Set | The editor checks |
|---|---|
| `G-CODEBRIDGE` | every old-section → new-section mapping (identical, modified, split, merged, omitted), the date from which each applies, the temporal rule that routes a matter by its key dates, and the text of each modified provision side by side |
| `G-ADMISS` | for each kind of evidence, the elements of foundation the rule requires, the authority that requires each (statute section or rule number), and that nothing required is missing or invented |
| `G-DEADLINE` | each rule's period and how it is counted (whether the trigger day is excluded, whether holidays roll the last day over, what is excluded such as time to obtain a copy), the authority, the court calendar and holidays, and every expected date |

They check against **primary sources** (statute text, rules of court, official calendars) and record the source. They do not rely on the fixture's
own description, on a summary, or on CaseForge's output.

They do **not** review software, wording of screens, or anything that predicts an outcome; if they see such wording they report it.

## How a sign-off is recorded

In `eval/signoffs/legal_editors.json`, one record per fixture and jurisdiction:

```json
{
  "ac": "AC-S4-06.1",
  "fixture": "G-DEADLINE/fixtures/<file>.json",
  "fixture_sha256": "<the fixture's hash in golden/STAGE4_MANIFEST.json when they reviewed it>",
  "jurisdiction": "<IN | US-NY | ...>",
  "editors": [
    {"name": "…", "role": "…", "registration": "…", "decision": "APPROVED", "reviewed_on": "YYYY-MM-DD", "sources": ["…"], "notes": "…"},
    {"name": "…", "role": "…", "registration": "…", "decision": "APPROVED", "reviewed_on": "YYYY-MM-DD", "sources": ["…"], "notes": "…"}
  ]
}
```

A record **counts** only if all of these hold, and the gates check them:

1. the fixture is listed in `golden/STAGE4_MANIFEST.json` as needing a sign-off, and the record names that fixture and its criterion;
2. `fixture_sha256` equals the fixture's **current** hash — a fixture that has changed since the review has no sign-off until it is reviewed again;
3. at least **two different editors** with a non-empty name, each `APPROVED`;
4. a jurisdiction is named.

A reviewer who finds an error records `CHANGES_REQUIRED` with notes; the fixture is corrected (by the editor's text, encoded by an agent or the
owner), its hash changes, and both editors review again. If the two disagree, the owner names a third editor to adjudicate; the third's decision and
reasons are recorded in `notes`. Nothing is signed by majority of an agent's opinion.

## Real jurisdictions

To make `AC-S4-04.1`, `05.1` and `06.1` measurable on real law:

1. The owner names the jurisdictions the product will claim (India; the US states, Q-S4-11) and the two editors for each.
2. An editor supplies, for their jurisdiction, the **source material**: the statute or rule text and sections, the periods and counting conventions,
   the calendars. They are not asked to write code.
3. A coding agent encodes it as a fixture (and a signed rule table, B-S4-03) marked `DRAFT`, with the editor's sources cited in the fixture.
4. A **second** editor reviews it against the sources; the first reviews the encoding. Both record sign-off as above.
5. Only then does the owner lock it `GOLDEN`; a signed rule table can then leave `DEVELOPMENT_ONLY`.

## What a sign-off means, and does not

It means two named lawyers checked this fixture's legal answers against primary sources for that jurisdiction on that date. It does **not** mean
that CaseForge is right in every matter, that the law has not changed since (rule tables carry a currency date for that reason), that the software
gives legal advice (it does not), or that a lawyer may stop checking the dates, mappings and requirements it shows. Every deadline still carries the
permanent VERIFY label.

## Confidentiality, rights and records

Editors are given synthetic or public-law material only: no client papers. If an editor wants to test against a real matter they do it on their own
copy and tell us only what the law is, not the matter. Sign-off records are kept in the repository (names and registration numbers are the editors'
to consent to; the owner decides whether to keep them in a private repository only, which this one is).

## What the harness does with this

`python eval/checks/goldens_b_s4_02.py` discovers all twelve sets, checks every fixture against the manifest, lists which planted cases each fixture
carries and for which jurisdictions, and reports each legal-answer criterion as `BLOCKED` until a record that counts exists. The ship gate reads the
same records. Nothing here lets a criterion pass because a sign-off field is merely filled in.
