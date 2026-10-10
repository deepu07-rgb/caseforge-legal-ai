# G-AUDIT

Citation & Quote Audit and the Audit Certificate: planted fabricated citations, single-word misquotes and wrong pinpoints in an Indian and a US draft.

| | |
|---|---|
| Source class | SYNTHETIC_CONTROLLED |
| Criteria | AC-S4-01.1, AC-S4-01.2, AC-S4-01.3, AC-S4-01.4, AC-S4-01.5 |
| Status | draft, unreviewed |

## Provenance

Every fixture here is synthetic and written for the test. No client material, no confidential papers, and no real person's data. The authoring and review state of each
fixture is recorded in `golden/STAGE4_MANIFEST.json` (file, SHA-256, who wrote it, whether it was pinned before the engine, what independent
review exists).

## Fixtures

* `audit_core.json` — mechanical; engine batch B-S4-04; pinned before the engine
* `certificate_reference.json` — mechanical; engine batch B-S4-05; not pinned before the engine. A certificate and draft that the independent offline verifier must accept; added with the engine.

## How to read the status

Under `golden/POLICY.md` a fixture is `GOLDEN` only after an independent human review and lock. The fixtures in this set were written by a coding
agent at the owner's instruction and committed before the code they test (where the manifest says so), which makes them reliable regression
tests of the software but not independently reviewed labels. The manifest records them as `DRAFT_UNREVIEWED` for that reason.

## Changing a fixture

Edit the file, run `python eval/tools/make_stage4_manifest.py`, and describe the change in the commit. The new hash replaces the old one in the
manifest, and any legal-editor sign-off bound to the old hash no longer counts (`docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md`).
