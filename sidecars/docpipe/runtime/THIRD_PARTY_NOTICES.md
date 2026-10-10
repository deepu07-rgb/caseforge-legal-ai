# Offline OCR runtime notices

The `tesseract/` directory contains Tesseract OCR 5.5.3 and its Windows runtime dependencies.

- Tesseract source: Apache License 2.0. The bundled license is at `tesseract/doc/LICENSE`.
- Leptonica: BSD 2-Clause.
- `eng.traineddata`: tessdata_fast English model, Apache License 2.0; frozen SHA-256 is recorded in `contracts/ocr_config.json`.
- `hin.traineddata`: tessdata_best Hindi model, Apache License 2.0, from the official `tesseract-ocr/tessdata_best` repository; its SHA-256 is recorded in `contracts/ocr_config_indic.json` and checked before every use (B-S4-14).
- Remaining DLLs retain their upstream licences. Redistribution must preserve the notices shipped in the runtime and must be re-audited before a public/client build.

CaseForge performs OCR locally and does not call a network service.
