# B-S4-14 — Hindi (Devanagari) ingestion

**Status:** implemented, gate green (`eval/checks/indic_ocr_b_s4_14.py`). **AC-S4-09.1 is reported BLOCKED, not PASS**: the character-error limit `S4.indic.ocr_cer_max` has no frozen value (the owner sets it after the bake-off, Q-S4-04). The measured error is under the limit the coding agent proposes, but a proposal is not a threshold.

## What it does

A scanned Hindi FIR, order, statement or notice now becomes text with a confidence on every word, the same way an English scan does, and the screens say which script a page is in.

1. **Orientation and script, before reading.** Tesseract's own detector (OSD) looks at the page and reports its rotation and its script. A rotation is only applied when the detector is confident (orientation confidence ≥ 2.0). Below that it is a guess: a short English page was once turned upside down by a 0.71-confidence guess, so a guess is now ignored.
2. **Routing** (`recognize.choose_route`, recorded with every page):
   * Devanagari → the Hindi+English reader (`hin+eng`, so English words, section numbers and `Rs.` inside a Hindi page are read too);
   * Latin, with script confidence ≥ 1.0 → the frozen English reader, unchanged;
   * unclear → the English reader first; if its page quality warns, the Hindi reader also runs, and its result is used **only if it actually finds Hindi text** (at least 20 Devanagari letters and 30% of the letters). Confidence is deliberately not the test: a noisy English page scored higher under the Hindi model while reading worse, which is how this rule was found.
   * If the Hindi model is missing or fails its hash check, nothing is routed to it (the page is read as English and the reason is recorded).
3. **Script of what was read** (`script_detect.py`): every line, and the page, is DEVANAGARI, LATIN or MIXED (a minority script of at least 15% of the letters makes it MIXED; digits and punctuation do not count). Native Hindi text layers get the same label. It is carried in the page's diagnostics (`text_script`, `line_scripts`) and shown on the review screen next to the page number ("Hindi", "English", "Hindi and English"). The frozen triage classes are not touched.
4. **Critical fields** (`indic_fields.py`, used by `confidence.validate_critical_fields`): dates (`१२/०३/२०२४`, `१० मार्च २०२४`), amounts (`₹ १,२५,०००`, `रुपये ७५,०००`, `₹ २ लाख ५० हजार`), sections (`धारा ३०२/३४`), labelled case numbers (`वाद संख्या`, `अपराध क्रमांक`, `प्रथम सूचना रिपोर्ट सं.`) and names after an honorific. Nothing is guessed: a numeric date with day and month both up to 12 is *locale-ambiguous* and held; a number mixing digit systems or sitting next to a letter OCR confuses with a digit is *OCR-confusable* and held; an amount with no currency marker is held (an unreadable `₹` leaves exactly that); an impossible date or a malformed amount is held. Indian grouping (`1,25,000`) is valid after a rupee marker (this also fixed the English validator, which called it invalid).
5. **Confidence** (`eval/calibration/confidence_calibration_indic_v1.json`): same method as B-S3-05 (monotonic map from the engine's raw score to the observed chance the word is right), its own artifact bound to its dataset by hash. Until it exists a Hindi word has raw confidence only and is held for review; the host records which map scored a page (`calibration_id_for`).

## Model choice (bake-off, `eval/bakeoff/indic_ocr_b_s4_14.json`)

Official Tesseract models, Apache-2.0, downloaded once with the owner's approval and vendored. Mean character error on the G-INDIC pages:

| candidate | clean CER | degraded CER | clean WER | s/page | peak RAM |
|---|---|---|---|---|---|
| hin only (fast) | 0.0420 | 0.0024 | 0.063 | 0.45 | 43 MB |
| hin only (best) | 0.0428 | 0.0024 | 0.064 | 0.79 | 54 MB |
| hin+eng (fast) | 0.0081 | 0.0061 | 0.016 | 0.6 | 60 MB |
| **hin+eng (best) — chosen** | **0.0056** | **0.0024** | 0.016 | 1.05 | 73 MB |
| English only (today's path) | 0.7441 | 0.8229 | 0.915 | 0.51 | 52 MB |

Rule (written in `contracts/ocr_config_indic.json` before the run): among Hindi+English candidates, lowest degraded-page CER, ties by clean pages. `tessdata_best` is 11.9 MB against 1.1 MB for `tessdata_fast`; it is the only model added to the repository (`sidecars/docpipe/runtime/tesseract/tessdata/hin.traineddata`, sha256 `bd2e65a2…78d6`). No candidate had a licence problem, so the playbook's halt condition did not apply. The Hindi-only rows show why English words must stay readable on a Hindi page.

## Results on G-INDIC (eight synthetic pages)

* CER: clean pages mean **0.0056**, degraded pages **0.0024**; proposed limits (agent, not frozen) 0.02 clean / 0.05 degraded.
* Script: page script right on 8/8; line script right on 62/62 lines.
* Critical fields (58 labelled): 54 read exactly; the other 4 were misread by OCR (`10/03/2024` → `0/03/2024`, `11 मार्च` → `29 मार्च`, an extra digit in a report number, `₹` read as a letter) and **all 4 were held for review**; none was accepted automatically, none was missed. Exact-or-held 58/58 against a proposed 0.98.
* Thirteen deliberately corrupted values (impossible dates, a letter O for a zero, broken grouping, truncated numbers) are all held.
* Calibration (separate synthetic pages: 15 training, 10 held-out, 2,157 words; different templates, names and places; no line shared with G-INDIC): held-out ECE **0.0180** against 0.0507 uncalibrated; Brier 0.0190 against 0.0175 uncalibrated (calibration is not sharper than the raw score here — stated, not hidden). Word accuracy 96.4% held-out. Only 20 training words fall below raw 0.70, so the low-confidence end of the map is thin.
* English path unchanged: the three English OCR samples give the committed B-S3-04 numbers exactly (CER, WER, word count, mean confidence).

## What this is not

* **Not real scans.** The pages are Hindi text rendered with Windows text shaping (Pillow here has no complex-text layout and drew Devanagari wrongly) in one typeface family (Nirmala UI), and degraded with a fixed recipe. A photographed, stamped, folded FIR will read worse; these error rates are a floor. The 0.2% degraded-page error shows the degradation recipe is mild — harsher pages are in the calibration set, not in the evaluation set.
* **Not a legal-editor-reviewed golden.** All eight fixtures are `DRAFT_UNREVIEWED` (the coding agent may not lock a golden, `golden/POLICY.md`).
* **Calibration data is synthetic**, so the map can be over-confident on real scans. The owner's ECE/Brier limits and per-field error costs are unset, so calibration stays BLOCKED like the English one.
* **Not handwriting, not other Indian scripts, not legacy-font PDFs** (Preeti/Kruti Dev text layers that extract as Latin gibberish are not detected here).
* Translation is B-S4-15.

## Truth conventions fixed after the first run (disclosed)

The golden pages were pinned (`cd65be6`) before the engine. The first gate run exposed two annotation conventions, not OCR behaviour: a case-number truth field carried only the number while the detector (like the English one) includes its label, and one name (`श्री मोहन लाल`) present in the text had not been labelled. The truth fields were corrected (transcriptions are unchanged, so CER is unaffected; the mixed page was re-rendered from the same text and its PNG differs by three bytes) and the manifest re-pinned. The same run changed two validators: the English money validator now accepts Indian grouping after a rupee marker, and the Roman-numeral provision form in the English pattern is case-sensitive so a word like "civil" is not read as a provision number.

## Owner decisions (still open)

* **Q-S4-04** — freeze `S4.indic.ocr_cer_max` (this report informs it; a real-scan measurement would inform it better) and confirm `hin+eng` from `tessdata_best` as the Hindi reader.
* **Q-S4-01** — ECE/Brier limits and per-field error costs for the Hindi calibration; `S4.indic.critical_field_accuracy_min` (proposed 0.98, applies to AC-S4-09.2 after translation).
* Whether to commission real scans (with permission) for a real-scan golden and calibration.

## Reproduce

```
python eval/checks/indic_ocr_b_s4_14.py                        # the gate (Windows OCR runtime)
python eval/tools/make_indic_golden.py                         # regenerate the eight pages (Windows)
python eval/tools/make_indic_calibration.py                    # rebuild the calibration set and map
python eval/tools/bakeoff_indic_ocr.py --fast <hin fast> --best <hin best>
```

Files: `sidecars/docpipe/{recognize,script_detect,indic_fields,confidence,docpipe}.py`, `contracts/ocr_config_indic.json`, `eval/calibration/{confidence_calibration_indic_v1,indic_ocr_calibration_v1}.json`, `eval/bakeoff/indic_ocr_b_s4_14.json`, `eval/reports/b_s4_14_indic_ocr.json`, `src-tauri/src/docpipe/mod.rs` (`calibration_id_for`), `src/lib/review.ts` + `ReviewView.tsx` (page script label). CI: the gate runs in the Windows installer workflow with the bundled Python (the vendored OCR runtime is Windows-only); the packaged-layout smoke test checks the Hindi model's hash.
