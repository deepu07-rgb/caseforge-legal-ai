# B-S4-07 — Witness resolution and Statement Matrix

**Status (2026-10-08): IMPLEMENTED.**
- AC-S4-03.2 (matrix cells) PASS.
- Counsel merge/split survives rerun: PASS.
- Statement-header proposals: PASS.
- The matrix validates against the full frozen `contracts/witness_matrix.schema.json`.
- Rust tests 108/108.

## What a lawyer gets

A new primary screen, **Witnesses**.

1. **Statements on record:** *Find witness statements in uploaded documents* reads each document's heading and proposes three things: the witness, the kind of statement, and the date it was recorded.
   - India: FIR, statement to police (s.161 Cr.P.C. / s.180 BNSS), statement before the Magistrate (s.164 / s.183), complaint, examination-in-chief, cross-examination, re-examination, affidavit.
   - US: declaration, interrogatory answers, deposition transcript, trial testimony.

   You correct a row and save it. A saved row is never overwritten by a later re-scan.
2. **Witnesses:** statements are grouped by name, ignoring honorifics and PW/DW numbers.
   - The same name with different fathers or husbands (s/o, w/o) is kept apart.
   - An initial-only name such as "R. Kumar" is never merged automatically.
   - You can merge two witnesses ("same person as…") or split a statement off ("belongs to a different person"). Decisions are recorded in order, can be undone, and are re-applied on every rebuild.
3. **Statement matrix:** for each witness, rows are topics (who, what, when, where, amount, identification, sequence) and columns are statements, earliest first.
   - Every cell is the exact text of the statement.
   - Its location is page and character span, or page:line for deposition transcripts.

## How it works

`src-tauri/src/witness.rs` is deterministic and makes no model calls.
- **Proposals:** header patterns from the statement-kind vocabulary `packs/builtin/statement_kinds.v1.json`. The vocabulary is data, not code; the matrix records `pack_id` and `table_id`.
  - Names come from "Statement of …", "PW-n …", "Informant: …", "DECLARATION/DEPOSITION OF …" and "I, …, declare".
  - Recording dates come from "recorded on", "Date and time of report", "Executed on" and "Taken on".
- **Order:** by date recorded; ties broken by counsel's order number, then by procedural rank (FIR before s.161 before chief before cross).
- **Segmentation:**
  - Narrative statements split into sentences, with abbreviations such as Smt., Rs., p.m., initials and No. handled.
  - Transcript pages, where numbered lines are the majority, split into question-and-answer units with printed line numbers.
  - Header and metadata sentences ("recorded on", "under section", "declare under penalty of perjury") are not testimony and are left out.
- **Facts read per assertion:** dates (Indian d.m.y or US m/d/y), times, amounts, counts of persons, places ("near / at / outside …"), who did what ("Mohan hit"), who was present, sequence ("first … then …"), and material terms (identification, weapons, injury).
- **Topics:** assertions from different statements that share two or more specific words.
- **Storage:** vault table `witness_store` (migration 017) holds `register:<doc>`, `decision:<n>`, `finding:<id>` and `model:<witness>`.
- **Page text:** the document's own text, or the reviewed OCR layer for scans. A text file with form feeds is split into transcript pages.

Discipline: `golden/G-IMPEACH/fixtures/impeach_core.json` was committed (57ec899) **before** this engine and is hash-pinned in the gate.

## Gate

`python eval/checks/witness_matrix_b_s4_07.py --cargo-log <log> --matrix <evidence>/witness_matrix.json` (runs in CI).

## Deviations and limits (v1)

- **Source of witness and statement structure:** v1 reads it from the counsel-reviewed statement register, not from case-graph witness/statement nodes. The current graph extractor only recognises labelled "Witness:" / "Statement:" lines. The register is the authority for counsel's merge/split decisions.
- **Vocabulary:** the statement-kind vocabulary is a built-in data table until rule tables gain a `STATEMENT_KIND` kind. That is a planned `rule_table` 1.1.0 amendment in `contracts/MANIFEST.json`, applied in B-S4-03.
- **Language:** English statements only. Hindi and Devanagari statements arrive with B-S4-14/15.
- **Transcript format:** transcripts must carry printed line numbers at the left margin. Other formats are read as page/span.
