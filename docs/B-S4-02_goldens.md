# B-S4-02 — Stage 4 golden sets and the legal-editor protocol

Playbook goal: author known-answer data for every Stage 4 feature, and say who signs the legal answers. Gate: `eval/checks/goldens_b_s4_02.py`
(report `eval/reports/b_s4_02_goldens.json`). Manifest: `golden/STAGE4_MANIFEST.json`. Protocol: `docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md`.

**Done when:** the harness discovers every Stage 4 set and reports honest PASS / SKIP / BLOCKED. It does. What it reports is mostly *BLOCKED* and *draft*,
and that is the point.

## What this batch did, and what it found

Most of the sets were authored earlier, batch by batch, each committed before the engine it tests. This batch put them under one roof:

* `golden/STAGE4_MANIFEST.json` lists the **twelve** sets and every fixture with its SHA-256, what kind it is (mechanical, legal answer, adversarial,
  placeholder), who wrote it, whether it was pinned before its engine and the first commit that held it, and where its legal sign-off stands.
* A `README.md` in each set declares its purpose, criteria, provenance and status.
* **`G-INDIC` was missing**; it now exists as a scaffold with a placeholder fixture and the reason it is blocked (no scans, no OCR comparison, no OCR
  or translation batch).
* `docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md` says who signs, what they check, how a sign-off is recorded and what makes it count. `eval/signoffs/legal_editors.json`
  exists and is empty.
* The gate checks the lot: sets, READMEs and provenance, hashes (a changed fixture must be re-recorded on purpose), schema-shaped objects inside fixtures,
  a heuristic scan for client material, the planted-case coverage table, and each legal answer's sign-off. The ship gate now counts a legal-editor sign-off
  only if the record names the fixture's **current** hash, a jurisdiction and two different editors who each approved.

## What the harness honestly reports

* **No fixture is `GOLDEN`.** Under `golden/POLICY.md` a fixture is locked only after an independent human review, and a coding agent may not lock one.
  Every fixture here (18 of them) was written by a coding agent at the owner's instruction. Some earlier files carry the word `GOLDEN` in their own metadata;
  the manifest records them as `DRAFT_UNREVIEWED`. They remain sound regression tests of the software; they are not independently reviewed labels.
* **Three legal answers are `BLOCKED`**: `G-CODEBRIDGE`, `G-ADMISS`, `G-DEADLINE` need two named legal editors per jurisdiction (Q-S4-02). Their criteria
  (`AC-S4-04.1`, `05.1`, `06.1`) stay blocked. The fixtures use an invented jurisdiction or development-only rules, so even a sign-off on them would not
  make any real mapping, requirement or window correct; the protocol says how real-jurisdiction fixtures get made.
* **Planted-case coverage** (every case the playbook lists has at least one fixture; which of India / the United States it is carried for is detected from the
  text): fabricated citations, single-word misquotes, wrong pinpoints, witness contradictions and omissions, an unanswered argument, extra-record bait and
  decision-solicitation prompts are covered; the opposing-filing cases and the overruled authority are carried for India only (no US filing or US treatment
  language in them); missing key dates, missing foundation certificates and holiday roll-overs are carried only by synthetic-rule fixtures, so for
  **neither real jurisdiction**; **Devanagari scans are blocked** (nothing authored). This is a gap list for the owner, not a pass.

## What it does not do

* It does not make any legal answer correct; it makes the lack of a sign-off visible and blocks on it.
* It does not author the missing US/India variants or the Devanagari scans. The first need editor-supplied sources; the second needs pages (rights-cleared or
  synthetic renderings) and the OCR comparison decision (Q-S4-04). I did not invent either.
* The client-material check is a heuristic (reserved example domains only; no real-looking mobile numbers); it is not a privacy review.

## To change a fixture

Edit it, run `python eval/tools/make_stage4_manifest.py`, say why in the commit. Its hash changes, and any sign-off bound to the old hash no longer counts.
