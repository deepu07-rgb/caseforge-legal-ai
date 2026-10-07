# B-S3-17 — Universal provenance UX

Status: **IMPLEMENTED** (2026-08-18)

## User contract

Every important proposition on Digest, Drafting, Argument Tree/Integrity, Review, Retrieval, and Mock Court has the same **Where did CaseForge get that?** control. The control identifies one of five origins: matter evidence, legal authority, counsel input, model inference, or unsupported gap. It always prints GREEN, AMBER, RED, or UNSUPPORTED plus an icon and explanatory text; color is supplemental only.

Opening the control displays the stable proposition/source ID, class, verification reason, version/checksum and a source locator. Matter locators retain document, one-based display page, character span, bounding region, and excerpt when recorded. Legal locators retain pack, source, chunk/passage, section/paragraph, checksum, and exact passage when recorded. Missing pointers produce an explicit `gap://` locator and can never look verified.

Counsel-created/corrected text is AMBER `COUNSEL_INPUT`. Model-derived text is AMBER `MODEL_INFERENCE` only when bases are visible; unsupported inferences remain RED/UNSUPPORTED. Legal GREEN remains controlled by the host verifier and cannot be assigned by this UI.

## Accessibility

The inspector is a labelled keyboard button and modal drawer. It announces state and origin in the accessible name, exposes `aria-haspopup`, `aria-expanded`, `role=dialog`, and `aria-modal`, traps Tab/Shift+Tab, closes with Escape, and restores focus to the invoking control. State meaning is conveyed by icon and text as well as border styling.

## Export and copy

Plain text, PDF, and DOCX share `renderPlainText`, so every graph/AI/counsel span retains a machine-readable provenance line containing state, class, origin, stable ID, locator, version, and support IDs. Every output retains the preparation-only footer. “Copy with provenance” appends the same provenance line and preparation disclaimer.

Schema: `contracts/provenance_record.schema.json` (`caseforge.provenance-record.v1`). UI vocabulary/router: `src/lib/provenance.ts`. Shared inspector: `src/components/ProvenanceInspector.tsx`.

## Verification

Run `npm run test:s3:17`. The gate checks all six acceptance criteria, named-surface coverage, stable matter/legal routing, non-color accessibility, export/copy retention, and this operator documentation. Run `npm run build` for TypeScript/production-bundle validation.
