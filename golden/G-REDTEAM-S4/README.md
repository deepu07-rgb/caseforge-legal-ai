# G-REDTEAM-S4

Stage 4 red-team: witness-coaching requests (G3), Chambers requests for a decision and decisional statements (G8), and statements that reach outside the record (G7).

| | |
|---|---|
| Source class | ADVERSARIAL_SYNTHETIC |
| Criteria | AC-S4-03.4, AC-S4-12.3, AC-S4-15.2, AC-S4-RT-RED |
| Status | draft, unreviewed |

## Provenance

Every fixture here is synthetic and written for the test. No client material, no confidential papers, and no real person's data. The authoring and review state of each
fixture is recorded in `golden/STAGE4_MANIFEST.json` (file, SHA-256, who wrote it, whether it was pinned before the engine, what independent
review exists).

## Fixtures

* `witness_coaching.json` — adversarial; engine batch B-S4-08; pinned before the engine
* `chambers_decision_zone.json` — adversarial; engine batch B-S4-21; pinned before the engine
* `chambers_extra_record.json` — adversarial; engine batch B-S4-23; not pinned before the engine. Written after the record-only classifier, by the same author: a probing set, not an independent one.

## How to read the status

Under `golden/POLICY.md` a fixture is `GOLDEN` only after an independent human review and lock. The fixtures in this set were written by a coding
agent at the owner's instruction and committed before the code they test (where the manifest says so), which makes them reliable regression
tests of the software but not independently reviewed labels. The manifest records them as `DRAFT_UNREVIEWED` for that reason.

## Changing a fixture

Edit the file, run `python eval/tools/make_stage4_manifest.py`, and describe the change in the commit. The new hash replaces the old one in the
manifest, and any legal-editor sign-off bound to the old hash no longer counts (`docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md`).
