# B-S4-04 — Citation & Quote Audit

**Status (2026-10-08): IMPLEMENTED.** AC-S4-01.1 and AC-S4-01.2 PASS; AC-S4-01.3 BLOCKED only because its recall threshold awaits owner approval (Q-S4-01) — measured 10/10 planted citations on G-AUDIT. Rust tests 88/88; frontend build clean.

## What a lawyer gets

In **Drafting**, under the editable draft: **Audit citations & quotes**. Every citation and every quotation in the draft is checked against the legal packs installed on this computer:

| Result | Meaning |
|---|---|
| Verified | Official, applicable, current source; a quotation matches the source word for word (line breaks ignored) |
| Misquoted | The cited source is installed but the quoted words differ — shows the nearest real passage and a word-by-word diff |
| Not in installed packs | A gap in *your* library — never a claim the authority does not exist |
| Superseded / Not applicable or status unknown / Custom source only | From the source's legal metadata; never shown as controlling |

Pinpoints are checked too: "para 12" or "section 4" must exist in the installed text of that authority (e.g. "section 99 of the Partition Act, 1893" → pinpoint not found).

## How it works

`src-tauri/src/audit.rs` (deterministic, no model) — `audit_draft` Tauri command over `retrieval::build_corpus` (all installed packs).
- **Extraction:** SCC / SCC OnLine / AIR / INSC / bracketed neutral citations / U.S., S. Ct., F.2d-4th, F. Supp.; statutes as "section N of the X Act, YYYY", "X Act, YYYY, s. N" and bare "X Act, YYYY"; plus literal matches of every installed source's citation string (e.g. "Act No. 4 of 1893", a firm memo id).
- **Resolution:** pages of the same work (pack + title + citation) are one authority; the most restrictive status across pages applies.
- **Quotations:** text in “…” or "…" (≥ 20 chars), attributed to the nearest citation before it (≤ 300 chars) or just after (≤ 120); otherwise searched across all installed text.
- **Misquote diff:** nearest same-length passage by shared words, ties broken by word-for-word alignment.
- Output follows the frozen `contracts/audit_report.schema.json` (checked by a Rust test reading the contract).

Discipline: `golden/G-AUDIT/fixtures/audit_core.json` was committed (5902203) **before** the engine. Writing it first caught two real bugs: per-page sources split one statute into three authorities (pinpoint missed), and a shifted passage was chosen for the misquote diff.

## Gate

`python eval/checks/citation_audit_b_s4_04.py --cargo-log <cargo test --lib output>` (also run in CI).

## Known limitations (v1)

- Case names cited **without** a reporter citation, "supra"/"ibid." and short forms are not resolved; footnote citations in DOCX/PDF files are not read yet (v1 audits the Drafting surface text).
- Only the installed India pack is real (three statutes) — most case-law citations will show "Not in installed packs" until judgment packs exist.
- Treatment ("overruled", "distinguished") and currency dates arrive with B-S4-13; the Audit Certificate with B-S4-05.
