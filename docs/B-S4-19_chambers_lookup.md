# B-S4-19 — Both-sides citation check and record lookup

Two Chambers tools: one action that checks every citation in every party's filings, and a way to ask "where in the record...?"
and get the passages themselves.

## Both-sides citation check (`chambers_audit_all`)

One action runs the Citation Audit (the same engine as B-S4-04, with the currency stamp and treatment notes of B-S4-13) on
every document assigned to a party in the Issue Matrix, and lists the results **by party, in the order the parties are
named**. Each filing shows its citations and quotations with their results, and the party shows totals by result. Nothing
compares or ranks the parties; documents not assigned to a party are listed as not checked. Chambers Edition only; a pilot
court can switch it off (`citation_audit`).

## Record lookup (`src-tauri/src/chambers_lookup.rs`)

The answer to "where did the witness say the car was red?" is the passage, word for word, with its document, page and
paragraph, and the matched words marked. Nothing is summarised, nothing concluded, no model is used.

- **Index:** the matter's own documents only (filings, evidence, orders). Never installed law or anything outside the matter.
  Passages are numbered pleading paragraphs (`p4`) or blocks separated by a blank line (`b2`); a passage over 1,500 characters
  is cut at sentence ends (`p4.2`). Rebuilt when a document is added, removed or changed.
- **Ranking:** BM25 over lightly stemmed words; near-synonyms from `packs/builtin/record_synonyms.v1.json` (data, not code)
  count at half weight; adjacent question words appearing together earn a bonus; a passage must contain at least 40 % of the
  question's distinct words, so a question the record cannot answer returns nothing rather than the nearest noise.
- **Voice:** the existing offline speech path. The transcription is shown in the question box and must be ticked as checked
  before it is used; only real microphone audio is accepted.
- Chambers Edition only; a pilot court can switch it off (`record_lookup`).

## Evidence

- Golden **G-CHAMBERS record_lookup** (pinned before the engine, `2c300d5`): a synthetic record of six documents and 24 questions
  with hand-marked answering passages (direct words, other word forms, names, dates and amounts, a few paraphrases), plus two
  questions the record cannot answer.
- Measured: **24 of 24** answered in the top five; both unanswerable questions return nothing. The engine was written after the
  golden and not tuned to it, but the record is small and synthetic: re-measure on real matter records.
- Every hit is checked to be the document's own text at the stated page and character range, character for character; the hit
  has exactly the fields rank, score, document, filename, page, label, character range, excerpt and highlights; the engine
  contains no model, network or text generation.
- `eval/checks/chambers_lookup_b_s4_19.py`. **AC-S4-13.1 PASS** (threshold PROPOSED 90 %, Q-S4-01; synthetic golden).
  **AC-S4-13.2 PASS.**

## Limits

- Lexical matching: a question that shares no words, word forms or listed near-synonyms with the passage will miss it. The
  synonym list is short and general; extend it as data.
- Scanned documents are searched through their trusted text layer only; pages without text are not indexed.
- No dense (meaning-based) ranking of matter passages yet.
