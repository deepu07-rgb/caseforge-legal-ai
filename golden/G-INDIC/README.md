# G-INDIC

Hindi (Devanagari) ingestion: scans for OCR accuracy, aligned translation, critical fields after translation. Scaffold only: nothing is authored yet.

| | |
|---|---|
| Source class | UNDECIDED |
| Criteria | AC-S4-09.1, AC-S4-09.2, AC-S4-09.3 |
| Status | BLOCKED (scaffold) |

## Provenance

Every fixture here is (none yet). No client material, no confidential papers, and no real person's data. The authoring and review state of each
fixture is recorded in `golden/STAGE4_MANIFEST.json` (file, SHA-256, who wrote it, whether it was pinned before the engine, what independent
review exists).

## Fixtures

* `devanagari_placeholder/meta.json` — placeholder; engine batch B-S4-14/15; not pinned before the engine. Placeholder so the harness discovers the set. Real scans need either rights-cleared pages or synthetic renderings, and the OCR comparison (Q-S4-04).

## How to read the status

Under `golden/POLICY.md` a fixture is `GOLDEN` only after an independent human review and lock. The fixtures in this set were written by a coding
agent at the owner's instruction and committed before the code they test (where the manifest says so), which makes them reliable regression
tests of the software but not independently reviewed labels. The manifest records them as `DRAFT_UNREVIEWED` for that reason.

## Blocked

Nothing here is authored. Hindi scans need rights-cleared pages or synthetic renderings, a decision on how OCR is compared (Q-S4-04), and the OCR and translation batches (B-S4-14, B-S4-15). Until then the harness reports this set as BLOCKED with the reason `golden_not_authored`.


## Changing a fixture

Edit the file, run `python eval/tools/make_stage4_manifest.py`, and describe the change in the commit. The new hash replaces the old one in the
manifest, and any legal-editor sign-off bound to the old hash no longer counts (`docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md`).
