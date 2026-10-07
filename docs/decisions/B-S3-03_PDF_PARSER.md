# B-S3-03 PDF parser dependency decision

**Decision:** `pypdfium2` 5.3.0 with PDFium 145.0.7616.0  
**Parser ID:** `caseforge.pdfium.page-source.v1`  
**Date:** 2026-08-13

PDFium is selected for offline page loading, rendering, text extraction, page
objects, dimensions, and rotation. `pypdfium2` is Apache-2.0 OR BSD-3-Clause;
PDFium uses a BSD-style licence and includes dependencies under additional
open-source licences. This is compatible with the intended proprietary desktop
distribution provided every binary distribution includes the PDFium and bundled
dependency notices from the exact wheel/build.

Packaging must install the pinned `sidecars/docpipe/requirements.txt`, retain
the wheel's licence files and `BUILD_LICENSES`, and reproduce them in the client
notices. A differently built wheel requires a new dependency-licence scan,
including any linked runtime such as libgcc. PyMuPDF is not the selected shipping
backend because its licensing was not approved for this proprietary bundle.

PDFium calls are isolated in the docpipe process. PDFium is not invoked
concurrently from multiple threads. Original bytes remain in the SQLCipher case
vault; parsing sends them through local stdio and creates no plaintext temporary
PDF. OCR page renders are in-memory and must be immediately processed or stored
only in the encrypted vault. Password values are never logged or persisted.

Coordinates are normalized top-left after page rotation per
`contracts/page_source.schema.json`; raw PDF points never leak into consumers
without the declared page width, height, and rotation.
