# G-INDIC

Hindi (Devanagari) ingestion: eight synthetic pages (FIR, order, statement, notice, two degraded copies, a mixed Hindi/English page and an English control) with exact ground truth, for OCR accuracy, script detection and critical-field checks. Translation fixtures (AC-S4-09.2, 09.3) are added with B-S4-15.

| | |
|---|---|
| Source class | SYNTHETIC_CONTROLLED |
| Criteria | AC-S4-09.1, AC-S4-09.2, AC-S4-09.3 |
| Status | draft, unreviewed |

## Provenance

Every fixture here is synthetic and written for the test. No client material, no confidential papers, and no real person's data. The authoring and review state of each
fixture is recorded in `golden/STAGE4_MANIFEST.json` (file, SHA-256, who wrote it, whether it was pinned before the engine, what independent
review exists).

## Fixtures

* `hi_fir_clean_01/page.png` — image; engine batch B-S4-14; pinned before the engine. A page rendered with Windows text shaping from the text in truth.json (eval/tools/make_indic_golden.py); not a real scan.
* `hi_fir_clean_01/truth.json` — mechanical; engine batch B-S4-14; pinned before the engine
* `hi_order_clean_02/page.png` — image; engine batch B-S4-14; pinned before the engine. A page rendered with Windows text shaping from the text in truth.json (eval/tools/make_indic_golden.py); not a real scan.
* `hi_order_clean_02/truth.json` — mechanical; engine batch B-S4-14; pinned before the engine
* `hi_statement_clean_03/page.png` — image; engine batch B-S4-14; pinned before the engine. A page rendered with Windows text shaping from the text in truth.json (eval/tools/make_indic_golden.py); not a real scan.
* `hi_statement_clean_03/truth.json` — mechanical; engine batch B-S4-14; pinned before the engine
* `hi_notice_clean_04/page.png` — image; engine batch B-S4-14; pinned before the engine. A page rendered with Windows text shaping from the text in truth.json (eval/tools/make_indic_golden.py); not a real scan.
* `hi_notice_clean_04/truth.json` — mechanical; engine batch B-S4-14; pinned before the engine
* `hi_fir_degraded_05/page.png` — image; engine batch B-S4-14; pinned before the engine. A page rendered with Windows text shaping from the text in truth.json (eval/tools/make_indic_golden.py); not a real scan.
* `hi_fir_degraded_05/truth.json` — mechanical; engine batch B-S4-14; pinned before the engine
* `hi_order_degraded_06/page.png` — image; engine batch B-S4-14; pinned before the engine. A page rendered with Windows text shaping from the text in truth.json (eval/tools/make_indic_golden.py); not a real scan.
* `hi_order_degraded_06/truth.json` — mechanical; engine batch B-S4-14; pinned before the engine
* `mixed_order_07/page.png` — image; engine batch B-S4-14; pinned before the engine. A page rendered with Windows text shaping from the text in truth.json (eval/tools/make_indic_golden.py); not a real scan.
* `mixed_order_07/truth.json` — mechanical; engine batch B-S4-14; pinned before the engine
* `latin_control_08/page.png` — image; engine batch B-S4-14; pinned before the engine. A page rendered with Windows text shaping from the text in truth.json (eval/tools/make_indic_golden.py); not a real scan.
* `latin_control_08/truth.json` — mechanical; engine batch B-S4-14; pinned before the engine

## How to read the status

Under `golden/POLICY.md` a fixture is `GOLDEN` only after an independent human review and lock. The fixtures in this set were written by a coding
agent at the owner's instruction and committed before the code they test (where the manifest says so), which makes them reliable regression
tests of the software but not independently reviewed labels. The manifest records them as `DRAFT_UNREVIEWED` for that reason.

## Limits

The pages are **rendered**, not scanned: the text was written for the test and drawn with Windows text shaping, so the transcription is exact by construction and needs no human labelling. A rendered page is cleaner and more regular than a photographed FIR, and one typeface family is used, so an error rate measured here is a floor, not what a court's scans will give. Real pages (rights-cleared) and a second labeller are still needed before this set can be locked GOLDEN. Translation fixtures arrive with B-S4-15.


## Changing a fixture

Edit the file, run `python eval/tools/make_stage4_manifest.py`, and describe the change in the commit. The new hash replaces the old one in the
manifest, and any legal-editor sign-off bound to the old hash no longer counts (`docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md`).
