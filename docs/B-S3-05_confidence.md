# B-S3-05 — Calibrated confidence and critical-field validation

## Outcome

OCR confidence is no longer inferred from token shape. Tesseract's raw word confidence is preserved separately, a versioned calibration map produces the displayed/trust confidence, and absence is represented as `missing` rather than zero or a fabricated score.

The deterministic validator identifies names, dates, monetary amounts, reporter citations, provisions, case numbers and exhibit labels. Each field records its exact character span, union source-image box, raw and calibrated confidence, validation reasons, review/block flags, counsel-confirmation state and validator identity.

## Trust boundary

- OCR words below frozen word thresholds retain their text for review but cannot enter automatic extraction.
- Invalid, missing-confidence or below-threshold critical fields are replaced with whitespace before skeleton extraction. The rest of a sound page remains usable.
- Empty/low-quality pages remain fully blocked.
- Corrections do not silently become counsel confirmation. Confirmation remains an explicit state and is never inferred.
- The former Rust/Python surface-form scorer is not called by either production document-recognition entrypoint.

## Calibration

`eval/calibration/legal_ocr_calibration_v1.json` is a synthetic controlled development corpus, separated into non-overlapping training and held-out IDs. It is not legal evidence and is not the real-scan evaluation fixture. The versioned artifact binds to its SHA-256 and reports reproducible held-out metrics:

- ECE: `0.08125`
- Brier score: `0.04530625`

The artifact remains `PROPOSED_AWAITING_OWNER_APPROVAL`. The playbook requires owner-approved ECE/Brier limits and critical-field error costs, but none exist in the frozen contracts. Existing owner-approved thresholds continue unchanged: critical-field accuracy `≥0.99`, downstream item block `<0.55`, date strict review `<0.85`, and word/page thresholds from `constants.json`.

## Persistence and UI

Migration 012 stores raw confidence, calibrated confidence, confidence state, counsel-confirmation state, calibration ID and critical-field JSON inside the encrypted per-case SQLCipher database. Review displays raw OCR, calibrated confidence, validation reasons, counsel status and the exact normalized source region.

## Verification

```powershell
python eval/checks/confidence_b_s3_05.py
python eval/checks/ocr_b_s3_04.py
npm run build
cargo check --lib
cargo test docpipe --lib
python scripts/check_size_budget.py
```

Implementation may be green while formal AC-S3-5.1 remains `BLOCKED`; it must not be called passed until the product owner freezes the missing limits/error costs and a qualifying GOLDEN corpus is available.
