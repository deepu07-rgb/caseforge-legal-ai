# G-CHAMBERS

Chambers Edition: record-only classification (G7), the issue matrix and its fallback pairing, record lookup, and the judgment scaffold.

| | |
|---|---|
| Source class | SYNTHETIC_CONTROLLED |
| Criteria | AC-S4-11.1, AC-S4-11.2, AC-S4-12.1, AC-S4-12.2, AC-S4-13.1, AC-S4-13.2, AC-S4-15.1, AC-S4-15.3 |
| Status | draft, unreviewed |

## Provenance

Every fixture here is synthetic and written for the test. No client material, no confidential papers, and no real person's data. The authoring and review state of each
fixture is recorded in `golden/STAGE4_MANIFEST.json` (file, SHA-256, who wrote it, whether it was pinned before the engine, what independent
review exists).

## Fixtures

* `record_only_core.json` — mechanical; engine batch B-S4-17; pinned before the engine
* `issue_matrix_core.json` — mechanical; engine batch B-S4-18; pinned before the engine
* `issue_matrix_fallback.json` — mechanical; engine batch B-S4-18; pinned before the engine
* `record_lookup_core.json` — mechanical; engine batch B-S4-19; pinned before the engine
* `judgment_scaffold_core.json` — mechanical; engine batch B-S4-21; pinned before the engine

## How to read the status

Under `golden/POLICY.md` a fixture is `GOLDEN` only after an independent human review and lock. The fixtures in this set were written by a coding
agent at the owner's instruction and committed before the code they test (where the manifest says so), which makes them reliable regression
tests of the software but not independently reviewed labels. The manifest records them as `DRAFT_UNREVIEWED` for that reason.

## Changing a fixture

Edit the file, run `python eval/tools/make_stage4_manifest.py`, and describe the change in the commit. The new hash replaces the old one in the
manifest, and any legal-editor sign-off bound to the old hash no longer counts (`docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md`).
