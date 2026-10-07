# B-S3-04 offline OCR bake-off and freeze

Date: 2026-08-13

## Candidates

| Candidate | Footprint / deployment | Word boxes and confidence | Licence posture | Decision |
|---|---:|---|---|---|
| Tesseract 5.5.3 + tessdata_fast `eng` | 111 MiB vendored Windows runtime | Native TSV output | Apache-2.0 engine/model; dependency notices retained | **Frozen** |
| pytesseract wrapper only | No engine included | Wrapper around Tesseract | Apache-2.0 wrapper | Rejected: adds no recognition engine |
| EasyOCR | Larger Python/Torch/model stack | Supported | Multi-component audit required | Rejected for size/runtime burden |
| PaddleOCR | Larger Python/model stack | Supported | Multi-component audit required | Rejected for size/runtime burden |

The bake-off used the rights-verified internal U.S. Courts fixture and machine-derived page-one reference. That reference is useful for engineering selection but remains `DRAFT`, so it cannot close the formal gate.

## Preprocessing bake-off

Page segmentation modes 3, 6, 11 and 12 were compared on the clean page render with raw, grayscale/denoise and threshold paths at native and 2× scale. The selected path is automatic page segmentation (`psm=3`) plus OSD, small-angle projection deskew, minimum long edge 2200 px, grayscale, autocontrast, median-3 denoise and modest contrast enhancement. It produced clean-page normalized CER/WER below the frozen 0.03/0.05 gates in the repeatable evaluator.

The intentionally rotated/degraded derivative does not meet the frozen degraded threshold. Thresholds were not lowered: the page is marked `requires_review=true`, `automatic_extraction_allowed=false`, and downstream skeleton extraction skips warned/force-review layers.

## Frozen identity

- Recognizer: `caseforge.ocr.tesseract-eng-fast.v1`
- Engine: Tesseract `5.5.3.20260724`
- Model: `eng.traineddata`
- Model SHA-256: `7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2`
- Language: English (`eng`), LSTM OEM 1, PSM 3
- Network: forbidden/not required
- Hardware: sequential pages; Lite 1 thread, Full up to 2 threads; primary/vision concurrency remains prohibited by choreography

## Formal evidence state

`BLOCKED`, not failed and not passed. The real fixture is `READY_FOR_REVIEW`; its expected answer is `DRAFT`, has no critical-field labels, and has no two-reviewer/adjudicator lock. The evaluator records clean/degraded metrics, time and peak engine RSS without representing draft evidence as GOLDEN.
