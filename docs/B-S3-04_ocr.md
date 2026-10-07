# B-S3-04 — Offline English OCR

## Outcome

Scanned PDF, JPG and PNG inputs now produce real local text layers instead of empty placeholders. Tesseract emits word text, engine confidence, reading order and normalized bounding boxes. Native PDF pages keep the PDFium text path; scanned and mixed pages are rendered at 300 DPI and routed to OCR.

The production recognizer is `caseforge.ocr.tesseract-eng-fast.v1`. Its engine/model/licence/preprocessing and frozen quality gates are in `contracts/ocr_config.json`; the bake-off is in `eval/bakeoff/reports/B-S3-04_ocr_bakeoff.md`.

## Safety and provenance

- OCR is local stdio only. No HTTP client or network call exists in the recognition path.
- Images are decoded and processed in memory; no plaintext source/render temporary files are created.
- Each OCR page records source and processed SHA-256, engine/model identity, preprocessing, duration, peak engine RSS, state and review/automation decisions in encrypted vault `notes` JSON.
- Empty pages remain `empty`; failures return stable `CF_OCR_*` errors. Neither is presented as understood.
- Warned pages or pages containing force-review words are excluded from automatic skeleton extraction until corrected/reviewed.
- The UI can reconstruct the deterministic processed page in memory and overlay every OCR word box for source inspection.

## Progress, cancellation and resume

Recognition remains off the UI thread and emits per-document progress. Case/overnight processing is sequential to control memory. Existing durable batch checkpoints resume at the document unit after interruption, and cancellation is observed between units; no primary model and OCR vision job may run concurrently on constrained hardware.

## Verification

```powershell
python eval/checks/ocr_b_s3_04.py
npm run build
cargo test docpipe --lib
python scripts/check_size_budget.py
```

The implementation evaluator must pass. Formal AC 4.1/4.2 remains `BLOCKED` until two reviewers lock the rights-cleared fixture, adjudicate disagreements and label critical dates, amounts, citations, provisions, names and exhibit labels. Thresholds must not be lowered to close that gate.
