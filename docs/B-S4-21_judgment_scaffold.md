# B-S4-21 — Judgment Scaffold and the Decision-Free Zone (G8)

The judge's reasoning and decision are written only by the judge, and the tool can prove it. Chambers Edition only (the
**Judgment** screen).

## The scaffold (`src-tauri/src/judgment_scaffold.rs`)

Ten sections (contract: `contracts/judgment_scaffold.schema.json`). Seven can be filled from the record, **deterministically — no
model is involved in any section**, each sentence the record's own words with a locator:

| Section | Filled with |
|---|---|
| Cause title | the matter's cause title, court and number |
| Appearances | sentences of the orders saying who appeared for whom (a name marker, "appeared/present/counsel for", and "for the <party>") |
| Brief facts | the facts the filings **agree on** (admitted facts of the accepted issues, from the Issue Matrix) — never one party's allegations |
| Procedural history | the orders on the record, in date order, as written |
| Submissions | each party's argument paragraphs, as written, parties in the order given |
| Issues | the issues the judge accepted or edited (the judge's own act, marked as such) |
| Evidence summary | evidence items whose label appears verbatim in the document they come from |
| **Analysis, Findings, Operative order** | **nothing. Locked to the judge.** |

The judge can remove any pre-filled sentence and add their own text to any section; their text is stored exactly as written
and a refresh from the record never touches it. The existence of a scaffold locks record-only mode on for the matter (B-S4-17).
Allowed help on the judge's text: the browser's spellcheck, formatting in the export, and the existing citation audit. None of
it generates text. The export names the sections filled from the record and carries the G4 footer and the G5 line (no AI spans).

## The decision-free zone (`src-tauri/src/g8.rs`, `packs/builtin/decision_zone.v1.json`)

- **Requests are refused before any model is called** (`CF_GUARDRAIL_G8_DECISION_ZONE`): requests to write, draft, rewrite,
  complete or recommend an analysis, findings, holding, operative order, decree, verdict or sentence; who should win; whether
  to grant, convict, dismiss; "decide issue 2"; role-play as the judge; "ignore the rules"; continuing a judgment from "we
  therefore hold that". The frozen patterns in `contracts/guardrails_stage4.json` plus the data file's. Reading what an earlier
  court decided ("the findings recorded in the impugned judgment") is not refused. The check is deliberately broad: a refusal is
  the safe side.
- **Decisional statements never render**, even a model-written one with valid record bases (an inference may not say a party
  is liable, a suit is dismissed, "I hold", "a stronger case"). A phrase that only *reports* a party's claim or a cited
  judgment's holding is allowed ("The plaint states that the plaintiff is entitled to a half share"; "The cited judgment held
  that ...") and is withheld the moment the statement adopts it ("... which is correct"). This holds with record-only mode off.
- **No path reaches a model with locked text.** The model is reachable in Chambers only on the record-only path (B-S4-17), which
  takes the question and passages of the record, never the scaffold (which is stored apart from the matter's documents, and the
  export writes no document). The gate scans the source for every call into the model and checks that none is in, or can read,
  scaffold code.

## Evidence

- **G-REDTEAM-S4 Chambers set** (pinned before the classifier, `bbef75b`): 113 requests that must be refused (direct requests
  for each locked section, outcome questions, "decide the issue", role-play and continuation tricks, relief and sentencing),
  32 ordinary record questions that must pass, 40 decisional statements that must never render, 16 record statements that must.
- **G-CHAMBERS judgment_scaffold golden** (same commit): matter with two pleadings (an admitted and a denied paragraph, an
  argument each), an order sheet with counsel's appearances, an exhibit list, issues in every decision state, evidence in the
  graph; the pre-fill is hand-authored; the judge's text for the locked sections (which contains decisions, as a judge may
  write) is stored unchanged.
- Rust tests for all of it; validation that a changed excerpt, a missing locator, a machine sentence in a locked section or a
  missing section is caught; the scaffold validates against the frozen schema.
- `eval/checks/judgment_scaffold_b_s4_21.py`: the static audit of every model call site, the red-team counts, the screen.
- **AC-S4-15.1 PASS. AC-S4-15.2 PASS** (113 ≥ the PROPOSED 100, Q-S4-01). **AC-S4-15.3 PASS.**

## Limits

- The red-team set is mine, written before the classifier but by the same author; a larger independent set (the owner's
  Chambers pilot court) is the real test. The input check is wording-based: an unusual phrasing can slip through it, which is why
  the output filter and the record-only verifier stand behind it and why the model never receives scaffold text in any case.
- Brief facts show only facts the filings agree on and need parties and filings assigned in the Issue Matrix.
- Evidence summary lists only items whose label is found verbatim in their document; others are omitted, not described.
- Appearances are recognised by English wording and a name marker (Mr, Ms, Shri...).
