# G-ADMISS

Admissibility & Foundation Gaps: evidence items with required foundation elements present or missing, and the order of the Missing Links panel.

| | |
|---|---|
| Source class | SYNTHETIC_CONTROLLED |
| Criteria | AC-S4-05.1, AC-S4-05.3 |
| Status | draft, unreviewed |

## Provenance

Every fixture here is synthetic and written for the test. No client material, no confidential papers, and no real person's data. The authoring and review state of each
fixture is recorded in `golden/STAGE4_MANIFEST.json` (file, SHA-256, who wrote it, whether it was pinned before the engine, what independent
review exists).

## Fixtures

* `foundation_core.json` — legal answer; engine batch B-S4-10; pinned before the engine. Development rows, not a statement of what any court requires.

## How to read the status

Under `golden/POLICY.md` a fixture is `GOLDEN` only after an independent human review and lock. The fixtures in this set were written by a coding
agent at the owner's instruction and committed before the code they test (where the manifest says so), which makes them reliable regression
tests of the software but not independently reviewed labels. The manifest records them as `DRAFT_UNREVIEWED` for that reason.

## Legal answers need a legal editor

The answers in this set (mappings, foundation rows, dates) are legal answers. The PRD requires a named legal editor's sign-off per jurisdiction; until it exists in `eval/signoffs/legal_editors.json` the criteria that rest on this set are `BLOCKED`, never `PASS`. The fixture uses invented or development-only rules so that it asserts no real law.


## Changing a fixture

Edit the file, run `python eval/tools/make_stage4_manifest.py`, and describe the change in the commit. The new hash replaces the old one in the
manifest, and any legal-editor sign-off bound to the old hash no longer counts (`docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md`).
