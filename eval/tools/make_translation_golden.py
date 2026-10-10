#!/usr/bin/env python3
"""Builds the G-INDIC translation fixture (B-S4-15): `golden/G-INDIC/fixtures/translation_core.json`.

Pinned BEFORE the translation engine exists. It holds:
  * the segmentation the engine must produce for five of the G-INDIC pages (from the written rule below, applied to the pages' exact transcription);
  * a reference English translation for every Hindi segment (written by the coding agent; NOT reviewed by a Hindi-English legal translator, and a
    reference is one acceptable rendering, not the only one - it is used for an indicative similarity figure, never as a pass/fail test of wording);
  * the critical fields in each segment and the exact English form the HOST must give them (dates, amounts, section numbers, case numbers are
    rendered by deterministic rules, not by the model; names are always left for counsel to confirm);
  * rendering-rule cases, protocol-failure cases (a model that drops, repeats or invents a placeholder), and the guardrail G10 probes the
    verifier must pass (a translated sentence never becomes a verified quotation).

Segmentation rule (the engine implements exactly this):
  1. Take the page's lines in order (empty lines are ignored).
  2. A line is CONTINUED into the next one when it does not end with a sentence mark (। . ? ! :) and is at least 85% as long as the longest line
     on the page (a wrapped line runs full width; a heading or a short line does not).
  3. Continued lines are joined with one space into a paragraph.
  4. A paragraph is split after every danda (।) that is followed by a space.
  5. Each piece is one segment. A segment with no Devanagari letter is carried as it is and is not translated.

Usage: python eval/tools/make_translation_golden.py
"""
from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIX = ROOT / "golden" / "G-INDIC" / "fixtures"
OUT = FIX / "translation_core.json"
SENTENCE_MARKS = "।.?!:"


def segment(lines: list[str]) -> list[str]:
    lines = [l for l in lines if l.strip()]
    longest = max(len(l) for l in lines)
    paragraphs: list[str] = []
    cur: list[str] = []
    for line in lines:
        cur.append(line)
        if line[-1] in SENTENCE_MARKS or len(line) < 0.85 * longest:
            paragraphs.append(" ".join(cur))
            cur = []
    if cur:
        paragraphs.append(" ".join(cur))
    out: list[str] = []
    for p in paragraphs:
        out.extend(piece for piece in re.split(r"(?<=।) ", p) if piece)
    return out


# Hindi segment -> (reference English, [(kind, source value, exact English form the host must produce)])
# kind in DATE, AMOUNT, NAME, SECTION_NUMBER, CASE_NUMBER; a NAME's English form is a suggestion for counsel to confirm (None = not pinned).
REFERENCE: dict[str, tuple[str, list[tuple[str, str, str | None]]]] = {
    # --- FIR ---
    "प्रथम सूचना रिपोर्ट": ("First Information Report", []),
    "थाना: काल्पनिक नगर, जिला: काल्पनिकपुर": ("Police station: Kalpanik Nagar, District: Kalpanikpur", []),
    "प्रथम सूचना रिपोर्ट सं. ०१२३/२०२४, दिनांक १२/०३/२०२४": (
        "First Information Report No. 0123/2024, dated 12/03/2024",
        [("CASE_NUMBER", "प्रथम सूचना रिपोर्ट सं. ०१२३/२०२४", "FIR No. 0123/2024"), ("DATE", "१२/०३/२०२४", "12/03/2024")]),
    "धारा ३०२/३४ भारतीय दंड संहिता के अंतर्गत अपराध": (
        "Offence under Section 302/34 of the Indian Penal Code", [("SECTION_NUMBER", "धारा ३०२/३४", "Section 302/34")]),
    "शिकायतकर्ता श्री रमेश कुमार पुत्र श्री मोहन लाल, निवासी ग्राम रामपुर ने बताया कि दिनांक १० मार्च २०२४ को रात लगभग ९ बजे आरोपी श्री सुरेश वर्मा ने उनके घर में घुसकर विवाद किया और ₹ १,२५,००० की मांग की।": (
        "The complainant Shri Ramesh Kumar, son of Shri Mohan Lal, resident of village Rampur, stated that on 10 March 2024 at about 9 pm the accused Shri Suresh Verma entered their house, quarrelled and demanded ₹1,25,000.",
        [("NAME", "श्री रमेश कुमार", None), ("NAME", "श्री मोहन लाल", None), ("DATE", "१० मार्च २०२४", "10 March 2024"), ("NAME", "श्री सुरेश वर्मा", None), ("AMOUNT", "₹ १,२५,०००", "₹1,25,000")]),
    "मौके पर श्रीमती सीता देवी और श्री अजय सिंह गवाह के रूप में उपस्थित थे।": (
        "Smt. Sita Devi and Shri Ajay Singh were present at the spot as witnesses.", [("NAME", "श्रीमती सीता देवी", None), ("NAME", "श्री अजय सिंह", None)]),
    "मामला दर्ज कर जांच प्रारंभ की गई।": ("The case was registered and the investigation was begun.", []),
    # --- order ---
    "अपर जिला एवं सत्र न्यायाधीश का न्यायालय, काल्पनिकपुर": ("Court of the Additional District and Sessions Judge, Kalpanikpur", []),
    "वाद संख्या १४५/२०२३": ("Case No. 145/2023", [("CASE_NUMBER", "वाद संख्या १४५/२०२३", "Case No. 145/2023")]),
    "श्रीमती गीता शर्मा बनाम श्री विजय प्रसाद": ("Smt. Geeta Sharma versus Shri Vijay Prasad", [("NAME", "श्रीमती गीता शर्मा", None), ("NAME", "श्री विजय प्रसाद", None)]),
    "आदेश दिनांक २८ फरवरी २०२४": ("Order dated 28 February 2024", [("DATE", "२८ फरवरी २०२४", "28 February 2024")]),
    "प्रार्थिनी ने धारा ८० सिविल प्रक्रिया संहिता के अंतर्गत नोटिस की सेवा का उल्लेख किया है।": (
        "The petitioner has mentioned the service of notice under Section 80 of the Code of Civil Procedure.", [("SECTION_NUMBER", "धारा ८०", "Section 80")]),
    "विपक्षी ने रुपये ७५,००० की राशि जमा करने का आश्वासन दिया था।": (
        "The opposite party had assured that the amount of ₹75,000 would be deposited.", [("AMOUNT", "रुपये ७५,०००", "₹75,000")]),
    "अगली सुनवाई की तिथि १५ अप्रैल २०२४ नियत की जाती है।": (
        "The next date of hearing is fixed as 15 April 2024.", [("DATE", "१५ अप्रैल २०२४", "15 April 2024")]),
    "पक्षकारों को सूचित किया जाए।": ("Let the parties be informed.", []),
    "आदेश खुले न्यायालय में सुनाया गया।": ("The order was pronounced in open court.", []),
    # --- witness statement ---
    "गवाह का बयान": ("Statement of the witness", []),
    "नाम: श्री अजय सिंह, आयु 42 वर्ष": ("Name: Shri Ajay Singh, age 42 years", [("NAME", "श्री अजय सिंह", None)]),
    "मैंने दिनांक 10/03/2024 को लगभग 9 बजे रात शोर सुना।": (
        "On 10/03/2024 at about 9 pm I heard noise.", [("DATE", "10/03/2024", "10/03/2024")]),
    "मैंने देखा कि श्री सुरेश वर्मा के हाथ में लाठी थी।": ("I saw that Shri Suresh Verma had a stick in his hand.", [("NAME", "श्री सुरेश वर्मा", None)]),
    "उसने Rs. 50,000 की मांग की थी, ऐसा मैंने सुना।": ("He had demanded Rs. 50,000; so I heard.", [("AMOUNT", "Rs. 50,000", "Rs. 50,000")]),
    "थाने में मेरा बयान दिनांक 11 मार्च 2024 को दर्ज हुआ।": ("My statement was recorded at the police station on 11 March 2024.", [("DATE", "11 मार्च 2024", "11 March 2024")]),
    "मैं अपना बयान पढ़कर हस्ताक्षर करता हूं।": ("I sign this statement after reading it.", []),
    # --- notice ---
    "विधिक नोटिस": ("Legal notice", []),
    "प्रेषक: श्रीमती गीता शर्मा, काल्पनिकपुर": ("From: Smt. Geeta Sharma, Kalpanikpur", [("NAME", "श्रीमती गीता शर्मा", None)]),
    "प्रति: श्री विजय प्रसाद, नया बाजार": ("To: Shri Vijay Prasad, Naya Bazar", [("NAME", "श्री विजय प्रसाद", None)]),
    "आपने दिनांक ५ अप्रैल २०२३ को ₹ २ लाख ५० हजार लेकर भूमि बेचने का अनुबंध किया था।": (
        "You had entered into an agreement to sell land on 5 April 2023, taking ₹2,50,000.", [("DATE", "५ अप्रैल २०२३", "5 April 2023"), ("AMOUNT", "₹ २ लाख ५० हजार", "₹2,50,000")]),
    "धारा ४२० भारतीय दंड संहिता तथा धारा ३४ के अंतर्गत कार्यवाही की जा सकती है।": (
        "Action can be taken under Section 420 of the Indian Penal Code and Section 34.", [("SECTION_NUMBER", "धारा ४२०", "Section 420"), ("SECTION_NUMBER", "धारा ३४", "Section 34")]),
    "यह नोटिस प्राप्ति के १५ दिन के भीतर उत्तर दें।": ("Reply within 15 days of receiving this notice.", []),
    "अपराध क्रमांक ०४५६/२०२३ का संदर्भ लें।": ("Refer to Crime No. 0456/2023.", [("CASE_NUMBER", "अपराध क्रमांक ०४५६/२०२३", "Crime No. 0456/2023")]),
    # --- mixed Hindi / English ---
    "जिला न्यायालय, काल्पनिकपुर": ("District Court, Kalpanikpur", []),
    "राज्य बनाम श्री सुरेश वर्मा": ("State versus Shri Suresh Verma", [("NAME", "श्री सुरेश वर्मा", None)]),
    "आरोपी के विरुद्ध Section 420 IPC तथा धारा ३४ के अंतर्गत आरोप तय किए गए।": (
        "Charges were framed against the accused under Section 420 IPC and Section 34.", [("SECTION_NUMBER", "Section 420", "Section 420"), ("SECTION_NUMBER", "धारा ३४", "Section 34")]),
    "अभियोजन ने दिनांक १८/०४/२०२४ तक समय मांगा।": ("The prosecution sought time until 18/04/2024.", [("DATE", "१८/०४/२०२४", "18 April 2024")]),
    "जमानत राशि ₹ 50,000 निर्धारित की गई।": ("The bail amount was fixed at ₹50,000.", [("AMOUNT", "₹ 50,000", "₹50,000")]),
}

PAGES = ["hi_fir_clean_01", "hi_order_clean_02", "hi_statement_clean_03", "hi_notice_clean_04", "mixed_order_07"]

# A host-rendering rule table: source field -> exact English form. (Dates: a numeric date with day and month both up to 12 is kept as written and
# stays flagged; a named-month or day-over-12 date is spelled with the month name.)
RULE_CASES = [
    {"kind": "DATE", "source": "१० मार्च २०२४", "english": "10 March 2024", "needs_confirmation_reason": None},
    {"kind": "DATE", "source": "१५/०४/२०२४", "english": "15 April 2024", "needs_confirmation_reason": None},
    {"kind": "DATE", "source": "१२/०३/२०२४", "english": "12/03/2024", "needs_confirmation_reason": "day/month order cannot be told from the numbers"},
    {"kind": "DATE", "source": "३२/१३/२०२४", "english": None, "needs_confirmation_reason": "not a calendar date"},
    {"kind": "AMOUNT", "source": "₹ १,२५,०००", "english": "₹1,25,000", "needs_confirmation_reason": None},
    {"kind": "AMOUNT", "source": "रुपये ७५,०००", "english": "₹75,000", "needs_confirmation_reason": None},
    {"kind": "AMOUNT", "source": "₹ २ लाख ५० हजार", "english": "₹2,50,000", "needs_confirmation_reason": None},
    {"kind": "AMOUNT", "source": "₹ १ करोड़ २५ लाख", "english": "₹1,25,00,000", "needs_confirmation_reason": None},
    {"kind": "AMOUNT", "source": "२ लाख ५० हजार", "english": None, "needs_confirmation_reason": "no currency marker"},
    {"kind": "SECTION_NUMBER", "source": "धारा ३०२/३४", "english": "Section 302/34", "needs_confirmation_reason": None},
    {"kind": "SECTION_NUMBER", "source": "धारा ८०", "english": "Section 80", "needs_confirmation_reason": None},
    {"kind": "CASE_NUMBER", "source": "वाद संख्या १४५/२०२३", "english": "Case No. 145/2023", "needs_confirmation_reason": None},
    {"kind": "CASE_NUMBER", "source": "अपराध क्रमांक ०४५६/२०२३", "english": "Crime No. 0456/2023", "needs_confirmation_reason": None},
    {"kind": "CASE_NUMBER", "source": "प्रथम सूचना रिपोर्ट सं. ०१२३/२०२४", "english": "FIR No. 0123/2024", "needs_confirmation_reason": None},
]

# What the engine must do when the model does not return a field marker intact. [F1] is the marker for the first field of a segment.
PROTOCOL_CASES = [
    {"id": "lost", "model_output": "The complainant stated that the accused demanded money.", "fields": 1, "expect": {"F1": "NOT_CARRIED"}, "segment_flagged": True},
    {"id": "repeated", "model_output": "He demanded [F1] and then again [F1].", "fields": 1, "expect": {"F1": "REPEATED"}, "segment_flagged": True},
    {"id": "invented", "model_output": "He demanded [F1] on [F7].", "fields": 1, "expect": {"F1": "OK"}, "segment_flagged": True, "invented": ["F7"]},
    {"id": "clean", "model_output": "He demanded [F1] on [F2].", "fields": 2, "expect": {"F1": "OK", "F2": "OK"}, "segment_flagged": False},
    {"id": "reordered", "model_output": "On [F2] he demanded [F1].", "fields": 2, "expect": {"F1": "OK", "F2": "OK"}, "segment_flagged": False},
    {"id": "empty", "model_output": "", "fields": 1, "expect": {"F1": "NOT_CARRIED"}, "segment_flagged": True},
]

# Guardrail G10 as the verifier (proposition::verify) must enforce it. "translated" marks a support source that is machine-translated text.
G10_PROBES = [
    {"id": "translated_fact_is_never_green", "class": "FACTUAL_BASIS", "source_translated": True, "exact_excerpt": True, "expect": {"verification_status": "AMBER", "render_action": "LABELLED"}},
    {"id": "original_language_fact_is_green", "class": "FACTUAL_BASIS", "source_translated": False, "exact_excerpt": True, "expect": {"verification_status": "GREEN", "render_action": "TRUSTED"}},
    {"id": "translated_quotation_is_not_verified", "class": "FACTUAL_BASIS", "source_translated": True, "quotation": True, "expect": {"quote_verified": False, "label": "translation - wording not verified", "verification_status_not": "GREEN"}},
    {"id": "original_quotation_is_verified", "class": "FACTUAL_BASIS", "source_translated": False, "quotation": True, "expect": {"quote_verified": True}},
    {"id": "translated_fact_cannot_be_laundered_by_a_second_source", "class": "FACTUAL_BASIS", "mixed_sources": True, "expect": {"verification_status_not": "GREEN"}},
    {"id": "inference_on_translated_basis_stays_amber_labelled", "class": "INFERENCE", "basis_translated": True, "expect": {"verification_status": "AMBER", "render_action": "LABELLED", "carries_translation_label": True}},
    {"id": "legal_authority_is_not_affected", "class": "LEGAL_BASIS", "expect": {"unchanged": True}},
]

UI_REQUIREMENTS = [
    "the original Hindi text and its English translation are shown side by side for every segment",
    "every translated segment carries the label 'machine translation - wording not verified'",
    "a critical field shows the source value, the English form and 'not confirmed' until counsel confirms it",
    "a translated sentence is never offered as a quotation",
]


def main() -> int:
    segments_by_page: dict[str, list[dict]] = {}
    seen: set[str] = set()
    for page in PAGES:
        truth = json.loads((FIX / page / "truth.json").read_text(encoding="utf-8"))
        rows = []
        for source in segment(truth["lines"]):
            has_dev = bool(re.search("[ऀ-ॿ]", source))
            row: dict = {"source": source, "translate": has_dev}
            if has_dev:
                assert source in REFERENCE, f"no reference for {source!r}"
                ref, fields = REFERENCE[source]
                seen.add(source)
                row["reference_english"] = ref
                row["fields"] = [{"kind": k, "source_value": v, "english": e} for k, v, e in fields]
                for k, v, e in fields:
                    assert v in source, (v, source)
            rows.append(row)
        segments_by_page[page] = rows
    unused = set(REFERENCE) - seen
    assert not unused, f"reference entries match no segment: {sorted(unused)}"
    fixture = {
        "fixture_id": "G-INDIC/translation_core",
        "status": "DRAFT",
        "source_class": "SYNTHETIC_CONTROLLED",
        "authored_by": "coding agent, at the owner's instruction",
        "independent_review": None,
        "description": "Segmentation, reference English, host-rendered critical fields, model-protocol failures and guardrail G10 probes for Hindi-to-English translation (AC-S4-09.2, AC-S4-09.3).",
        "limits": "Reference translations are one acceptable rendering written by the coding agent and not reviewed by a Hindi-English legal translator. Names are never pinned: counsel confirms them.",
        "primary_acs": ["AC-S4-09.2", "AC-S4-09.3"],
        "segmentation_rule": __doc__.split("Segmentation rule (the engine implements exactly this):")[1].split("Usage:")[0].strip(),
        "pages": segments_by_page,
        "rendering_rule_cases": RULE_CASES,
        "protocol_cases": PROTOCOL_CASES,
        "g10_probes": G10_PROBES,
        "ui_requirements": UI_REQUIREMENTS,
        "name_honorifics": {"श्रीमती": "Smt.", "सुश्री": "Ms.", "कुमारी": "Kumari", "श्री": "Shri", "डॉ": "Dr."},
    }
    OUT.write_text(json.dumps(fixture, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    total = sum(len(v) for v in segments_by_page.values())
    translated = sum(1 for v in segments_by_page.values() for r in v if r["translate"])
    fields = sum(len(r.get("fields", [])) for v in segments_by_page.values() for r in v)
    print(f"wrote {OUT.name}: {total} segments ({translated} to translate, {total - translated} carried as they are), {fields} critical fields")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
