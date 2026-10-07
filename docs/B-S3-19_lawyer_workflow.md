# B-S3-19 — Lawyer-oriented end-to-end workflow

## Status

ENGINEERING IMPLEMENTED on 2026-08-21. PENDING HUMAN SIGN-OFF against the frozen usability protocol. The app is ready for controlled pre-beta usability testing; it is not yet valid to claim the human success-rate gates have passed.

## Primary preparation journey

CaseForge now presents one saved-state-driven journey:

1. Add Case Material
2. Review Document Quality
3. Review What CaseForge Understood
4. Complete Missing Facts
5. Case Digest
6. Legal Authorities
7. Argument Analysis
8. Mock Court
9. Preparation Tasks

Each stage shows its purpose, persisted status, supporting detail, and safe action. Completion never comes from merely visiting a screen. The state model uses `EMPTY`, `LOADING`, `PARTIAL`, `FAILED`, `BLOCKED`, `CANCELLED`, `STALE`, `READY`, and `COMPLETE`.

Saved document/page state drives material and quality. Document ingestion time versus the saved graph generation time drives stale understanding. The saved graph and open gaps drive missing-facts and digest readiness. Host-backed legal-working-set readiness, the saved argument tree, and encrypted preparation tasks drive the final stages.

## Safety and recoverability

Mock Court cannot open from the primary journey unless the host reports that grounded retrieval is ready. A blocked attempt names the host-provided reasons and returns counsel to Legal Authorities to fix them. Preparation Tasks opens the saved debrief/task panel directly.

Knowledge-pack removal now requires a scoped confirmation. It states that only the installed local copy is removed, matter files are unchanged, and the pack can be reinstalled. Existing vault reset protection remains deliberately stronger because reset is irreversible.

## Advanced details and accessibility

Page triage, OCR/text layers, case-graph build jobs, batch recovery, phone capture, model details, pipeline identifiers, and secondary application tools remain available under Advanced details. They no longer dominate the lawyer-facing path.

Workflow states include text and symbols rather than colour alone. Controls have visible keyboard focus. Long source excerpts, tables, provenance fields, drafts, and preformatted text wrap without horizontal loss. The journey collapses to a readable two-column arrangement on narrow screens. Source-return and provenance controls remain intact.

## Frozen controlled usability protocol

Use `contracts/lawyer_workflow_usability.json`; do not alter its thresholds during the run.

- Recruit at least five unfamiliar participants, preferably at least three litigators.
- Give each participant a clean matter fixture and only this instruction: “Prepare this matter through the next safe step shown by CaseForge.” Do not provide technical documentation.
- At every screen, ask the participant to state the screen purpose and primary action within 15 seconds. Required aggregate success is at least 80%.
- Observe completion of all nine tasks. Required participant journey-completion rate is at least 80%.
- Include fixtures for empty, loading, partial, failed, blocked, cancelled, stale, ready, and complete states; compare every displayed status with the saved host record.
- Include an unsafe Mock Court attempt, a keyboard-only pass, a long authority/source excerpt, a wide provenance table, knowledge-pack removal cancellation, and confirmed removal/reinstall recovery.
- Permitted critical safety failures: zero. Permitted keyboard blockers: zero.
- Record timing, wrong turns, assistance, state mismatches, accessibility blockers, and participant role. Do not count an assisted task as independently completed.

The automated gate (`npm run test:s3:19`) verifies the frozen journey, persisted signals, state vocabulary, safe Mock Court routing, advanced-details containment, confirmation copy, and accessibility hooks. It does not substitute for five unfamiliar participants.
