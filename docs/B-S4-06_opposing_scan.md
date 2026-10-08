# B-S4-06 — Opposing Filing Scan

**Status (2026-10-08): IMPLEMENTED.** AC-S4-02.1, AC-S4-02.3 and AC-S4-02.4 PASS. AC-S4-02.2 is BLOCKED only because its recall threshold awaits owner approval (Q-S4-01); measured 3/3 planted record conflicts on G-OPPOSING, each with two locators. Rust tests 98/98; the scan output validates against the full frozen `contracts/external_filing.schema.json`.

## What a lawyer gets

A new primary screen, **Opponent's Filing**. Upload the other side's reply, written statement or submissions and mark the document as *the other side's filing* (type tag `EXTERNAL_FILING`), or paste the text. Then choose who filed it and **Scan their filing**:

| Section | What it shows |
|---|---|
| Their citations and quotations | The B-S4-04 audit over their filing: misquoted passages with a word diff, citations missing from your installed packs, superseded or inapplicable authority |
| Where they contradict your record | A date, an amount, or a denial that conflicts with your case record. Each item shows **both** places: their paragraph and your record entry, plus whether that record entry was extracted from your documents or confirmed by you |
| Factual claims with no record reference | Factual paragraphs that cite no annexure, exhibit or page |
| Their arguments | Argument paragraphs ("It is submitted that…") that are **not answered** in your Argument Tree, and those that touch a limb you already argue. Buttons open Argument Builder or Mock Court |

## The trust boundary (zero leakage)

Their filing is an *untrusted adverse assertion*, never one of your facts:
- **Case graph:** both case-graph extraction paths drop pages of `EXTERNAL_FILING` documents before extraction (`docpipe::exclude_external_pages`). If the document list cannot be read, the rebuild fails closed (`CF_AUDIT_EXTERNAL_ASSERTION_LEAK`) instead of letting the filing in.
- **Model context and proposition support:** `case_llm_context` (local-model context) and `case_llm_support_sources` (support for your own propositions) skip `EXTERNAL_FILING` documents.
- **Scan is read-only:** it writes nothing to the case graph or Argument Tree (`graph_edge_id` is always null).
- **Late tagging:** if a document is marked as the other side's after the case record was built, the scan reports `stale_graph` and the screen asks you to rebuild the case record.
- **Injection:** the scan is rule-based with no model call, so instructions planted in their text cannot change it. The test removes the planted instruction and requires identical findings.

## How it works

`src-tauri/src/opposition.rs` (deterministic). It is called by the `opposition_scan` Tauri command.
- **Paragraphs:** numbered paragraphs (`1.`, `2.`, …) on each page.
- **Classification:**
  - *argument:* "it is submitted", "barred by", "not maintainable", "estopped", "is entitled" …;
  - *authority:* "reliance is placed", "held that" …;
  - *factual:* contains a date, an amount, or a fact verb such as executed, served, paid, possession, registered …
- **Conflicts:**
  - *DATE:* a date that is not in the chronology, where the matching event (two or more shared specific words) has a different date;
  - *AMOUNT:* an amount that is not in the record, where the related entry has a different amount;
  - *PRESENCE:* a denial that strongly overlaps (three or more shared words) a recorded event.
- **Arguments vs tree:** matched by shared specific words against each limb's label, theory and anchors.
- **Authorities:** `audit::audit` with `subject_kind = EXTERNAL_FILING`.

Discipline: `golden/G-OPPOSING/fixtures/respondent_reply.json` was committed (4563f9e) **before** the engine and is hash-pinned in the gate.

## Gate

`python eval/checks/opposing_scan_b_s4_06.py --cargo-log <cargo test --lib output> --record <evidence>/opposing_scan_record.json`. This runs in CI. The contract test writes the record when `CASEFORGE_EVIDENCE_DIR` is set.

The gate also checks the leak barrier's structure:
- every `extract_case_graph` site is filtered;
- the barrier fails closed;
- both the model-context and the proposition-support feeds exclude external filings;
- the scan contains no graph writes.

## Known limitations (v1) and deviations from the playbook

- **Graph entry deferred:** the playbook lets assertions enter the graph as party-owned `asserts`/`denies` edges. v1 keeps them out of the graph entirely, which is stricter. The planned case_graph v2.1.0 `ADVERSE_ASSERTION` amendment stays *planned*.
- **No model-proposed conflicts yet:** only deterministic date, amount and denial conflicts are detected. NAME conflicts and conflicts that need a model to propose them (verified through B-S3-14) are not built yet.
- **Paragraph structure:** works best with numbered paragraphs. Unnumbered prose is not segmented.
- **Mock Court and Argument Builder buttons:** they only navigate. They do not prefill the opponent's argument.
- **Small fixture:** recall is measured on one small synthetic fixture. Real filings need the legal-editor golden set (Q-S4-02).
