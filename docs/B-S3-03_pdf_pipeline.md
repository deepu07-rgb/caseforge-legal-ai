# B-S3-03 — Production PDF parsing and page model

**Status:** IMPLEMENTED; formal PASS blocked on real-world GOLDEN sign-off  
**Date:** 2026-08-13

The raw-byte `/Page` counter and duplicated text fallback have been removed from
the Rust production path. PDF documents now pass to an offline PDFium sidecar
that returns actual page boundaries, per-page native text, normalized spans,
page dimensions and rotation, layout blocks, content classification, OCR
routing, capability declarations, source hash, and parser identity.

Page kinds are `native`, `scanned`, `mixed`, `empty`, or `unsupported`.
Document errors are explicitly `malformed`, `encrypted`, `oversized`, or
`unsupported`. Passwords are accepted in memory but are neither stored nor
included in errors. OCR routing renders scanned and mixed pages in memory;
native pages avoid rendering. Full OCR recognition is B-S3-04 scope.

The encrypted case database migration `case_011_page_source.sql` persists the
page-source JSON and provenance, while originals stay encrypted in
`documents.content`. The Documents table reports native, scanned, or mixed and
the number of pages requiring OCR.

Parsing supports `resume_from_page`, bounded `max_pages_per_call`, and a
page-boundary cancellation callback. The existing overnight batch continues to
checkpoint after each document unit; parser callers can checkpoint within a
large PDF using `checkpoint.next_page`.

Verification:

```powershell
npm run test:s3-pdf
npm run build
cargo test docpipe --lib
npm run test:s3-encoding
npm run test:size
```

Machine-readable report: `eval/reports/b_s3_03_pdf_pipeline.json`.

The five-page native, image-only, and mixed engineering fixtures preserve exact
order. Native pages have distinct text, the scanned set has no invented native
text, and the mixed third page alone routes to OCR. Schema, spans, normalized
coordinates, render, resume, cancellation, password, malformed and oversized
states are exercised.

The B-S3-02 real fixture is still `READY_FOR_REVIEW`, not human-locked GOLDEN.
These tests close the implementation batch but do not claim controlled-beta
real-world evidence readiness. Complex tables, footnotes, signatures, and stamps
are retained as text, vector, or raster blocks or marked heuristic/unsupported;
their real-corpus acceptance remains blocked pending a reviewed fixture.

The currently installed pinned wrapper plus PDFium binary occupy approximately
6.2 MiB. After removing the regenerable Rust target cache, the measured
development total is 4.501 GiB and the estimated installed-client total remains
4.412 GiB, below the 9 GiB hard ceiling. The final signed installer must repeat
this measurement with the bundled Python runtime and licence notices included.
