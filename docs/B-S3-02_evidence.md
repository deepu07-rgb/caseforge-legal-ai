# B-S3-02 — Real-world golden sets and honest readiness metrics

**Status:** PASS for the batch contract; controlled-beta evidence remains BLOCKED  
**Date:** 2026-08-13

The Stage 3 evidence contract is frozen. It validates rights, provenance,
checksums, expected-answer review, source-class isolation, metric definitions,
environment identity, and missing-evidence states. The runner cannot turn an
unreviewed fixture or absent result into PASS.

The official U.S. Courts Pro Se 1 fixture has native PDF/DOCX plus reproducible
TXT, PNG, JPG, degraded/rotated, scanned-PDF, and mixed-PDF derivatives. It is
`READY_FOR_REVIEW`; client redistribution is not authorized by its manifest.
Complex-layout real documents, authoritative legal repositories, performance
profiles, and human audio remain explicitly blocked.

Commands:

```powershell
npm run stage3:fixtures
npm run test:s3-evidence
npm run report:s3-evidence
npm run test:size
```

The canonical readiness artifact is
`eval/reports/stage3_evidence_report.json`. Continue to B-S3-03 only with the
understanding that parser acceptance cannot be claimed until the document
fixture receives human GOLDEN lock.
