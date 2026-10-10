# B-S4-15 — Aligned offline translation and guardrail G10

**Status:** implemented, gate green (`eval/checks/translation_b_s4_15.py`). **AC-S4-09.2 and AC-S4-09.3 are measured PASS and formally BLOCKED**: the 98% limit (`S4.indic.critical_field_accuracy_min`) is PROPOSED, not approved, and the translation model is an open owner decision (Q-S4-04).

## What a lawyer gets

On the **Hindi to English** tab of a matter, *Translate* turns the recognized Hindi text into English, segment by segment, and shows the Hindi original and the English side by side. Each English segment is labelled **"machine translation - wording not verified"**. There is no way to copy the English as a quotation: to quote, quote the Hindi.

Dates, amounts, section numbers, case numbers and names are listed under each segment with the value from the original, the English form, and **"not confirmed"**. The lawyer types the value as the original gives it and confirms it (or withdraws that later). The machine's value is kept beside the lawyer's; every confirmation or withdrawal is an event in an append-only history.

*Translate* with no document chosen translates every document that has Hindi text, one after another, and can be stopped; what was done is kept and the next run continues from there (an overnight batch). A segment already translated from the same text is not asked of the model again; if counsel corrects a word in the text, only the segments that changed are redone.

## How it works

1. **Segments** (`translate.segment`): the page's lines are joined into sentences by one written rule (a full-width line that has no sentence mark carries on; a danda ends a sentence). Each segment keeps its place in the page. Pure-English lines are carried as they are, not translated.
2. **Fields are set aside, then written back by the host.** Before the model sees a segment, each critical field is replaced by a marker (`[F1]`). The model writes English prose around the markers; the host puts the English form back by fixed rules: `१० मार्च २०२४` → `10 March 2024`, `₹ २ लाख ५० हजार` → `₹2,50,000`, `धारा ३०२/३४` → `Section 302/34`, `वाद संख्या १४५/२०२३` → `Case No. 145/2023`. A numeric date whose day and month are both up to 12 stays as written and flagged (`12/03/2024` could be either). A name gets a *suggested* spelling and is always left for counsel. So **the model cannot change a number**.
3. **When the model does not give a marker back intact** (dropped, repeated, or invented), the field and its segment are flagged. The tolerant reading accepts `F1` or `(F1)` for `[F1]`, since a model sometimes drops the brackets; an invented `[F7]` is flagged.
4. **The model** is the default local model (`qwen2.5:7b`) with a fixed prompt (hash recorded in `contracts/translation_config.json`), greedy decoding, no case context. It takes the **primary model's slot** in the choreography, so it is never resident beside the vision model or a reasoning turn, and no second large model is added. The prompt carries a short glossary of court and police terms, a rule to transliterate (not translate) place names, and a rule not to guess a person's gender.
5. **Records** satisfy the frozen `contracts/translation_segment.schema.json` exactly (original text and span, OCR confidence, model and digest, `provenance_origin: TRANSLATED`, `quotation_verifiable: false`, fields with `counsel_confirmed`). The review flags (label, flagged, why) are stored beside the record, not inside it. Stored in the vault (`translation_store`, migration 18), encrypted with the case.
6. **Facts read from a translation** (`translation_graph`): the deterministic extractor runs over the English and the result is kept as a **separate** graph in which every item is `TRANSLATED`, not confirmed, held below the downstream-trust line (confidence ≤ 0.49) and pointing at the page, not at a character span (the spans would refer to the English). It is shown on the tab and is **not merged into the matter's own case graph**. The `TRANSLATED` origin lives in the new coexisting schema `contracts/case_graph_v2_1.schema.json`; the pinned 2.0.0 schema is untouched.

## Guardrail G10 in the verifier (`proposition::verify`)

A support source can now be marked `translated`. Then, in the host, whatever the model says:

* a factual proposition resting on translated text is **never GREEN** (AMBER, rendered `[TRANSLATION - wording not verified] …`);
* a quotation of translated text is an exact substring of the *translation* but is **never verified** (`quote_verified: false`, and `all_quotes_exact` turns false);
* a translated source cannot launder a fact by sitting beside an original-language source;
* an inference built on a translated basis stays AMBER and carries the label;
* legal authority is unaffected.

Translated pages reach the model and the verifier through `case_llm_support_sources`, marked translated. Original-language text keeps GREEN as before.

## Results

* **Segmentation**: 37/37 segments of the five pages equal the pinned golden. **Fields**: 22/22 non-name fields render as the golden says; 12 names all left for counsel with a plain-letter suggestion. 14 rendering-rule cases and 6 model-protocol failure cases pass.
* **Three model behaviours through the whole pipeline**: markers returned intact — every field exact; model ignores the markers (writes its own numbers) — every field flagged, none of its numbers used; model corrupts markers at random — every field exact or flagged. **Wrong and not flagged: 0** in all three. Accuracy "exact or flagged" is 100% against the proposed 98%.
* **The real model** (`eval/evidence/translation_live_b_s4_15.json`, bound to the prompt, model and golden by hash): 35 segments, 33 of 34 markers carried intact (97.1%); the one that was not (the model left "Section 80" out of "notice … under the Code of Civil Procedure") was flagged; all 22 non-name fields equal to the golden; mean chrF against the references 0.74 (indicative; no limit is set on it).
* **G10**: seven verifier tests, eight translation-store tests (translate and resume, stop and continue, changed source replaced, confirm/withdraw with history, translated sources reach the verifier, facts marked TRANSLATED through the real extractor, no network). The graph read from a translation validates against `case_graph_v2_1`.

## What this is not

* **The guarantee is about fields, not prose.** A 7B general model translates Hindi legal prose modestly. Live examples: "I read my statement and put my signature on it" for "I sign this statement after reading it"; "The order was heard in the open court" where the original says *pronounced*; "agreement … to sell the land to ₹2,50,000" where the price was *taken*. None of these changes a field, which is why the screen puts the original beside the English and labels every segment. A first run with a plainer prompt also produced "Rules Court and Panchkula" for a court heading and "age amount" for *bail amount*; the glossary removed those, which shows the model's limit is real.
* **References are the coding agent's**, not a legal translator's, and the golden is `DRAFT_UNREVIEWED`. The 22 labelled fields are on synthetic pages.
* **Not handwriting, not other Indian scripts, not legacy-font PDFs.** Names get a suggested spelling from a small lexicon and plain rules (e.g. "Bhupendr" for भूपेंद्र); counsel confirms every one.
* **Chambers edition**: the model call honours B-S4-17 (a judiciary licence reaches the model only through record-only reasoning), so a Chambers licence cannot run translation. That may be wrong for judges who receive Hindi filings; it is an owner question below.
* **Translation reads the recognized text.** If OCR misread a word, the translation of that word is wrong too; the OCR confidence of each segment is shown, and a corrected word is retranslated.

## Owner decisions

* **Q-S4-04** — is `qwen2.5:7b` with this prompt acceptable as the Hindi translator, or should a dedicated translation model be bought in (an Indic machine-translation model would very likely read better, but needs a download — which I will not do without your say-so — and a runtime other than Ollama, and so another bake-off on the same golden)? Approve or freeze the 98% limit (Q-S4-01).
* Whether a Chambers licence may translate record text (it is record, not extra-record, but it is a model call).
* A Hindi-English legal translator to review the references and the glossary would turn this golden from `DRAFT_UNREVIEWED` into something a limit can be set against.

## Reproduce

```
python eval/checks/translation_b_s4_15.py --cargo-log cargo-test.log --evidence-dir evidence   # the gate
python eval/tools/make_translation_golden.py                                                   # the golden
python eval/tools/translate_golden_live.py                                                     # the real-model run (needs Ollama)
cargo test --manifest-path src-tauri/Cargo.toml --lib translation:: proposition::
```

Files: `sidecars/docpipe/translate.py`, `sidecars/docpipe/docpipe.py` (`docpipe.translate_*`), `sidecars/infer_llm/infer_llm.py` (`llm.translate`), `contracts/{translation_config.json,prompts/translate_hi_en_v1.txt,case_graph_v2_1.schema.json}`, `src-tauri/src/{translation.rs,proposition.rs}`, `src-tauri/migrations/case_018_translation_store.sql`, `src/components/TranslationPanel.tsx`, `eval/{checks,tools}/…`.
