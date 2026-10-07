# B-S3-18 — Source-specific Mock Court debrief

Status: **IMPLEMENTED** (2026-08-18)

Debrief v2 converts grounded text and voice rehearsals into traceable findings and preparation tasks. It consumes recorded turn IDs, counsel text, assistant text, the host-verified proposition ledger, retrieved working-set authorities, and the deterministic Integrity missing-links report. It does not ask the model to reconstruct session facts.

## Epistemic boundary

Every finding is marked `OBSERVED`, `INFERENCE`, or `EXPLICIT_GAP`. Each cites one or more turn IDs. Verified matter/legal supports retain stable source locators; an absent source stays visibly a gap. Categories cover weaknesses, contradictions, unanswered questions, factual/evidentiary gaps, legal gaps or misuse, missed authorities, and unsupported persuasion.

The comparison view reports only changes in verified-support rate between recorded sessions. It sets `outcome_prediction=false` and never evaluates likely outcome, guilt, sentence, or judge behavior.

## Preparation tasks and source truth

Debrief reports are immutable encrypted snapshots identified by a source fingerprint. Proposed tasks are copied into a separate encrypted task store. Counsel may accept, edit, dismiss, or complete a task. These operations update only task workflow fields; the original JSON, proposition ledger, sources, and debrief remain unchanged.

The usefulness rubric is frozen in `contracts/debrief_usefulness_rubric.json`. A task must score at least 3/4 for specificity, turn trace, source/gap clarity, and lawyer usability. Generic filler is rejected. The planted-weakness recall threshold is 0.80 and requires controlled human review before release claims.

## Operation

Complete at least one grounded text or voice turn and select **Generate Debrief v2** in Mock Court. Open **Debrief & tasks** to inspect findings, comparison, provenance, and task controls.

Schema: `contracts/mock_court_debrief_v2.schema.json`. Migration: `case_016_mock_court_debrief_v2.sql`. Run `npm run test:s3:18` for the deterministic acceptance gate.
