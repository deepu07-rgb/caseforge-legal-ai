"""Aligned Hindi-to-English translation, segment by segment (B-S4-15, PRD v4.0 section 7.9, guardrail G10).

The model writes English prose. It does not write the facts a lawyer relies on. Before a segment goes to the model, every critical field in it
(date, amount, section number, case number, name) is replaced by a marker such as [F1]; the host puts the English form back afterwards, from
the source value, by fixed rules (dates spelled with the month name, amounts in lakh/crore grouping, "धारा ३०२/३४" as "Section 302/34", a labelled
case number as "Case No. ..."). So a number cannot be changed by the model. If the model drops, repeats or invents a marker, the field is flagged
and the segment is flagged; nothing is silently accepted. Names are always left for counsel to confirm (a suggested spelling is shown).

Every segment carries: the original text and its character span in the page text, the OCR confidence of that span, the English text, the model
and prompt that made it, provenance TRANSLATED, quotation_verifiable false, and its critical fields with counsel_confirmed false until counsel
confirms each one. Segments with no Devanagari letter are not translated (they are already English).

This module has no network access and does not start a model. The caller (the host, through the model sidecar) supplies the translated text; the
tests supply a fixed translator.
"""

from __future__ import annotations

import hashlib
import json
import re
import unicodedata
from datetime import datetime
from pathlib import Path
from typing import Any, Callable

import indic_fields
import script_detect
from confidence import PATTERNS as EN_PATTERNS

ROOT = Path(__file__).resolve().parents[2]
CONFIG_PATH = ROOT / "contracts" / "translation_config.json"
SCHEMA_VERSION = "1.0.0"
SENTENCE_MARKS = "।.?!:"
WRAP_SHARE = 0.85
HAS_DEV = indic_fields.HAS_DEV
LABEL = "machine translation - wording not verified"
MONTH_NAMES = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"]

KIND_OF = {"date": "DATE", "money": "AMOUNT", "provision": "SECTION_NUMBER", "case_number": "CASE_NUMBER", "name": "NAME"}
HONORIFICS = {"श्रीमती": "Smt.", "सुश्री": "Ms.", "कुमारी": "Kumari", "श्री": "Shri", "डॉ": "Dr.", "डॉ.": "Dr."}
CASE_LABELS = [
    (re.compile(r"प्रथम\s*सूचना\s*रिपोर्ट\s*सं(?:ख्या)?[.,:;]?"), "FIR No."),
    (re.compile(r"अपराध\s*क्रमांक"), "Crime No."),
    (re.compile(r"अपराध\s*संख्या"), "Crime No."),
    (re.compile(r"वाद\s*संख्या"), "Case No."),
    (re.compile(r"मुकदमा\s*संख्या"), "Case No."),
    (re.compile(r"केस\s*नंबर"), "Case No."),
]


def load_config() -> dict[str, Any]:
    return json.loads(CONFIG_PATH.read_text(encoding="utf-8"))


# ---------------------------------------------------------------- segmentation

def segment(text: str) -> list[dict[str, Any]]:
    """The rule written in golden/G-INDIC/fixtures/translation_core.json. Each segment: source text and its character span in `text`."""
    spans: list[tuple[int, int]] = []
    cursor = 0
    for raw in text.split("\n"):
        start, end = cursor, cursor + len(raw)
        cursor = end + 1
        if raw.strip():
            spans.append((start, end))
    if not spans:
        return []
    longest = max(e - s for s, e in spans)
    paragraphs: list[tuple[int, int]] = []
    open_start: int | None = None
    for s, e in spans:
        if open_start is None:
            open_start = s
        line = text[s:e]
        if line.rstrip()[-1] in SENTENCE_MARKS or (e - s) < WRAP_SHARE * longest:
            paragraphs.append((open_start, e))
            open_start = None
    if open_start is not None:
        paragraphs.append((open_start, spans[-1][1]))
    out: list[dict[str, Any]] = []
    for ps, pe in paragraphs:
        piece_start = ps
        for m in re.finditer(r"(?<=।) ", text[ps:pe]):
            out.append({"char_start": piece_start, "char_end": ps + m.start()})
            piece_start = ps + m.end()
        out.append({"char_start": piece_start, "char_end": pe})
    out = _split_long(text, out, int(load_config()["segmentation"]["max_segment_chars"]))
    result = []
    for seg in out:
        raw = text[seg["char_start"]:seg["char_end"]]
        # source text is the page text with line breaks inside a joined paragraph read as single spaces
        result.append({**seg, "source_text": re.sub(r"\n", " ", raw), "translate": bool(HAS_DEV.search(raw))})
    return result


def _split_long(text: str, segs: list[dict[str, int]], limit: int) -> list[dict[str, int]]:
    """A piece longer than the limit (a long run with no sentence mark) is cut at the last comma or space before the limit, so a long paragraph is
    never sent to the model in one piece."""
    out: list[dict[str, int]] = []
    for seg in segs:
        s, e = seg["char_start"], seg["char_end"]
        while e - s > limit:
            window = text[s:s + limit]
            cut = max(window.rfind(", "), window.rfind("\n"), window.rfind(" "))
            cut = cut + 1 if cut > limit // 2 else limit
            out.append({"char_start": s, "char_end": s + cut})
            s = s + cut
            while s < e and text[s] in " \n":
                s += 1
        out.append({"char_start": s, "char_end": e})
    return [x for x in out if text[x["char_start"]:x["char_end"]].strip()]


# ---------------------------------------------------------------- critical fields

def find_fields(source: str) -> list[dict[str, Any]]:
    """Critical fields in a segment, left to right, none inside another."""
    found: list[tuple[str, int, int]] = []
    for kind, pattern in EN_PATTERNS:
        if kind in KIND_OF:
            found += [(KIND_OF[kind], m.start(), m.end()) for m in pattern.finditer(source)]
    found += [(KIND_OF[k], s, e) for k, s, e in indic_fields.find(source) if k in KIND_OF]
    found = sorted(set(found), key=lambda f: (f[1], -(f[2] - f[1])))
    kept: list[tuple[str, int, int]] = []
    for f in found:
        if any(k[1] <= f[1] and f[2] <= k[2] for k in kept):
            continue
        if any(f[1] < k[2] and k[1] < f[2] for k in kept):
            continue  # overlapping, not contained: keep the earlier, longer one
        kept.append(f)
    return [{"field_index": i + 1, "field_kind": k, "source_char_start": s, "source_char_end": e, "source_value": source[s:e]} for i, (k, s, e) in enumerate(kept)]


def _indian_group(n: int) -> str:
    s = str(n)
    if len(s) <= 3:
        return s
    head, tail = s[:-3], s[-3:]
    parts: list[str] = []
    while len(head) > 2:
        parts.insert(0, head[-2:])
        head = head[:-2]
    if head:
        parts.insert(0, head)
    return ",".join(parts + [tail])


def _ascii(s: str) -> str:
    return indic_fields.to_ascii_digits(s)


def render_date(value: str) -> tuple[str | None, str | None]:
    """(English form, reason it needs counsel's eye). A date that is not a calendar date has no English form."""
    v = value.strip()
    if re.fullmatch(r"\d{1,2}\s+[A-Za-z]+\.?\s+\d{4}", v):
        return re.sub(r"\s+", " ", v), None
    iso = indic_fields.parse_date(v)
    if iso is None:
        return None, "not a calendar date"
    y, m, d = (int(x) for x in iso.split("-"))
    numeric = re.fullmatch(r"([0-9०-९]{1,2})[./-]([0-9०-९]{1,2})[./-]([0-9०-९]{2,4})", v)
    if numeric and int(_ascii(numeric.group(1))) <= 12 and int(_ascii(numeric.group(2))) <= 12:
        return _ascii(v), "day/month order cannot be told from the numbers"
    return f"{d} {MONTH_NAMES[m - 1]} {y}", None


def render_amount(value: str) -> tuple[str | None, str | None]:
    v = value.strip()
    latin_marker = re.match(r"(Rs\.?|INR)\s*", v, re.I)
    if latin_marker:
        return f"{latin_marker.group(1)} {_ascii(v[latin_marker.end():]).strip()}".replace("Rs  ", "Rs. ").replace("Rs. .", "Rs."), None
    if not re.search(r"₹|रु|रुपये|रुपए", v):
        return None, "no currency marker"
    amount = indic_fields.parse_amount(v)
    if amount is None or amount <= 0:
        return None, "amount could not be read"
    return f"₹{_indian_group(amount)}", None


def render_section(value: str) -> tuple[str | None, str | None]:
    v = value.strip()
    if v.startswith("धारा"):
        rest = _ascii(v[len("धारा"):]).strip()
        return (f"Section {rest}" if rest else None), (None if rest else "no section number")
    return re.sub(r"\s+", " ", v), None


def render_case_number(value: str) -> tuple[str | None, str | None]:
    v = value.strip()
    for pattern, english in CASE_LABELS:
        m = pattern.match(v)
        if m:
            number = _ascii(v[m.end():]).strip(" :.-")
            return (f"{english} {number}" if number else None), (None if number else "no case number")
    return re.sub(r"\s+", " ", v), None


_LEXICON = {
    "सिंह": "Singh", "कुमार": "Kumar", "कुमारी": "Kumari", "शर्मा": "Sharma", "वर्मा": "Verma", "गुप्ता": "Gupta", "यादव": "Yadav", "मिश्रा": "Mishra",
    "पांडेय": "Pandey", "तिवारी": "Tiwari", "देवी": "Devi", "प्रसाद": "Prasad", "लाल": "Lal", "चंद्र": "Chandra", "राम": "Ram", "खान": "Khan",
    "अहमद": "Ahmad", "पटेल": "Patel", "जोशी": "Joshi", "दुबे": "Dubey", "सक्सेना": "Saxena", "कपूर": "Kapoor", "मोहन": "Mohan", "सीता": "Sita",
    "गीता": "Geeta", "विजय": "Vijay", "रमेश": "Ramesh", "सुरेश": "Suresh", "अजय": "Ajay",
}
_VOWELS = {"अ": "a", "आ": "a", "इ": "i", "ई": "i", "उ": "u", "ऊ": "u", "ऋ": "ri", "ए": "e", "ऐ": "ai", "ओ": "o", "औ": "au"}
_MATRAS = {"ा": "a", "ि": "i", "ी": "i", "ु": "u", "ू": "u", "ृ": "ri", "े": "e", "ै": "ai", "ो": "o", "ौ": "au", "ॉ": "o"}
_CONS = {
    "क": "k", "ख": "kh", "ग": "g", "घ": "gh", "ङ": "ng", "च": "ch", "छ": "chh", "ज": "j", "झ": "jh", "ञ": "n", "ट": "t", "ठ": "th", "ड": "d", "ढ": "dh", "ण": "n",
    "त": "t", "थ": "th", "द": "d", "ध": "dh", "न": "n", "प": "p", "फ": "ph", "ब": "b", "भ": "bh", "म": "m", "य": "y", "र": "r", "ल": "l", "व": "v", "श": "sh",
    "ष": "sh", "स": "s", "ह": "h",
}


def romanize_word(word: str) -> str:
    """A suggested English spelling of a Devanagari name word: a small lexicon of conventional spellings, otherwise a plain scheme that drops the
    final 'a' and the 'a' between two vowelled syllables, as Hindi is spoken. It is a suggestion shown to counsel, never a fact."""
    word = unicodedata.normalize("NFC", word).replace("़", "")
    if word in _LEXICON:
        return _LEXICON[word]
    # syllables: (latin text, bare) - bare is a consonant that still carries the inherent 'a'
    syl: list[tuple[str, bool]] = []
    i = 0
    while i < len(word):
        ch = word[i]
        if ch in _CONS:
            base = _CONS[ch]
            nxt = word[i + 1] if i + 1 < len(word) else ""
            if nxt == "्":  # virama: the consonant joins the next one
                syl.append((base, False))
                i += 2
            elif nxt in _MATRAS:
                syl.append((base + _MATRAS[nxt], False))
                i += 2
            else:
                syl.append((base, True))
                i += 1
        elif ch in _VOWELS:
            syl.append((_VOWELS[ch], False))
            i += 1
        elif ch in "ंँ":
            syl.append(("n", False))
            i += 1
        elif ch == "ः":
            syl.append(("h", False))
            i += 1
        else:
            i += 1
    n = len(syl)
    out = []
    for idx, (latin, bare) in enumerate(syl):
        if not bare:
            out.append(latin)
            continue
        if idx == n - 1:
            out.append(latin)  # a final inherent 'a' is not spoken
            continue
        next_bare_final = syl[idx + 1][1] and idx + 1 == n - 1
        out.append(latin + ("a" if idx == 0 or next_bare_final or not syl[idx - 1] else ""))
    return "".join(out).capitalize()


def render_name(value: str) -> tuple[str | None, str | None]:
    words = value.replace(" ", " ").split()
    out = []
    for w in words:
        out.append(HONORIFICS[w] if w in HONORIFICS else romanize_word(w))
    text = " ".join(out)
    return text, "a name is always confirmed by counsel"


RENDERERS = {"DATE": render_date, "AMOUNT": render_amount, "SECTION_NUMBER": render_section, "CASE_NUMBER": render_case_number, "NAME": render_name}


# ---------------------------------------------------------------- the model's side of the exchange

def mask(source: str, fields: list[dict[str, Any]]) -> str:
    """The segment as the model sees it: each critical field replaced by its marker."""
    out, cursor = [], 0
    for f in fields:
        out.append(source[cursor:f["source_char_start"]])
        out.append(f"[F{f['field_index']}]")
        cursor = f["source_char_end"]
    out.append(source[cursor:])
    return "".join(out)


def prompt_text() -> str:
    cfg = load_config()
    p = ROOT / cfg["prompt"]["file"]
    data = p.read_bytes()
    if hashlib.sha256(data).hexdigest() != cfg["prompt"]["sha256"]:
        raise RuntimeError("CF_TRANSLATE_PROMPT_CHECKSUM: the translation prompt does not match its recorded hash")
    return data.decode("utf-8")


# The marker as given is [F1]. A model sometimes drops the brackets or swaps them for parentheses; that is still the marker (it is ours, and
# only a number we issued counts when bare), so it is read as one rather than lost. An unknown number in brackets is an invented marker.
_MARK = re.compile(r"\[F(\d+)\]|\(F(\d+)\)|(?<![A-Za-z0-9])F(\d+)(?![A-Za-z0-9])")


def _num(m: re.Match[str]) -> int:
    return int(m.group(1) or m.group(2) or m.group(3))


def fill(masked_output: str, fields: list[dict[str, Any]]) -> dict[str, Any]:
    """Puts each field's English form back and judges whether the model carried every marker intact."""
    counts: dict[int, int] = {}
    known = {f["field_index"] for f in fields}
    for m in _MARK.finditer(masked_output):
        k = _num(m)
        if m.group(3) is not None and k not in known:
            continue  # a bare "F7" we never issued is ordinary text, not a marker
        counts[k] = counts.get(k, 0) + 1
    invented = sorted(k for k in counts if k not in known)
    rendered: dict[int, tuple[str | None, str | None]] = {f["field_index"]: RENDERERS[f["field_kind"]](f["source_value"]) for f in fields}
    states: dict[int, str] = {}
    for f in fields:
        c = counts.get(f["field_index"], 0)
        states[f["field_index"]] = "OK" if c == 1 else ("NOT_CARRIED" if c == 0 else "REPEATED")

    def sub(m: re.Match[str]) -> str:
        k = _num(m)
        if k not in known:
            return m.group(0) if m.group(3) is not None else "[unresolved field]"
        english, _reason = rendered[k]
        return english if english is not None else f"[unreadable: {next(f['source_value'] for f in fields if f['field_index'] == k)}]"

    text = _MARK.sub(sub, masked_output).strip()
    flagged = bool(invented) or any(s != "OK" for s in states.values())
    return {"text": text, "field_states": states, "invented": invented, "segment_flagged": flagged, "rendered": rendered}


def translate_segment(source: str, translator: Callable[[str], str]) -> dict[str, Any]:
    fields = find_fields(source)
    masked = mask(source, fields)
    output = translator(masked)
    return {"fields": fields, "masked": masked, "model_output": output, **fill(output, fields)}


def info() -> dict[str, Any]:
    cfg = load_config()
    return {"model": cfg["model"]["name"], "model_digest": cfg["model"]["digest"], "prompt_version": cfg["prompt"]["version"], "prompt_sha256": cfg["prompt"]["sha256"],
            "label": cfg["label"], "generation": cfg["generation"], "choreography": cfg["choreography"], "guardrail": cfg["guardrail"]}


def prepare(*, doc_id: str, page_index: int, text: str, words: list[dict[str, Any]] | None = None) -> list[dict[str, Any]]:
    """Everything the host needs to translate one page: its segments, the markers, the text the model should see, and the OCR confidence of each."""
    out = []
    for i, seg in enumerate(segment(text)):
        if not seg["translate"]:
            out.append({**seg, "seg_index": i, "fields": [], "masked": "", "ocr_confidence": None})
            continue
        fields = find_fields(seg["source_text"])
        out.append({**seg, "seg_index": i, "fields": fields, "masked": mask(seg["source_text"], fields), "ocr_confidence": _ocr_confidence(seg, words)})
    return out


def _ocr_confidence(seg: dict[str, Any], words: list[dict[str, Any]] | None) -> dict[str, Any]:
    inside = [w for w in (words or []) if w["char_start"] < seg["char_end"] and w["char_end"] > seg["char_start"]]
    raws = [w["raw_engine_confidence"] for w in inside if w.get("raw_engine_confidence") is not None]
    cals = [w["calibrated_confidence"] for w in inside if w.get("calibrated_confidence") is not None]
    return {"raw": round(min(raws), 4) if raws else None, "calibrated": round(min(cals), 4) if cals else None}


def finish(*, doc_id: str, page_index: int, prepared: dict[str, Any], model_output: str, calibration_id: str | None = None) -> dict[str, Any]:
    """The stored record for one segment, from the model's output."""
    result = {"fields": prepared["fields"], "masked": prepared["masked"], "model_output": model_output, **fill(model_output, prepared["fields"])}
    cfg = load_config()
    conf = prepared.get("ocr_confidence") or {"raw": None, "calibrated": None}
    return build_record(doc_id=doc_id, page_index=page_index, seg_index=prepared["seg_index"], seg=prepared, result=result, words=None,
                        model={"model_id": cfg["model"]["name"], "digest": cfg["model"]["digest"]}, page_script=None, calibration_id=calibration_id, ocr=conf)


def build_record(*, doc_id: str, page_index: int, seg_index: int, seg: dict[str, Any], result: dict[str, Any], words: list[dict[str, Any]] | None,
                 model: dict[str, str], page_script: str | None, calibration_id: str | None, ocr: dict[str, Any] | None = None) -> dict[str, Any]:
    """{"segment": a record that satisfies contracts/translation_segment.schema.json exactly, "review": what the screen needs beside it}. Nothing here is a fact."""
    s, e = seg["char_start"], seg["char_end"]
    conf = ocr if ocr is not None else _ocr_confidence({"char_start": s, "char_end": e}, words)
    fields = []
    field_flags: dict[str, list[str]] = {}
    for f in result["fields"]:
        english, reason = result["rendered"][f["field_index"]]
        state = result["field_states"][f["field_index"]]
        fields.append({
            "field_id": f"f{f['field_index']}", "field_kind": f["field_kind"], "source_char_start": seg["char_start"] + f["source_char_start"], "source_char_end": seg["char_start"] + f["source_char_end"],
            "source_value": f["source_value"], "translated_value": english if english is not None else "", "counsel_confirmed": False, "confirmed_value": None,
        })
        field_flags[f"f{f['field_index']}"] = [r for r in (reason, None if state == "OK" else f"the model did not carry the marker intact ({state})") if r]
    segment_record = {
        "schema_version": SCHEMA_VERSION, "segment_id": f"{doc_id}:{page_index}:{seg_index}", "doc_id": doc_id, "page_index": page_index,
        "source_script": script_detect.script_of_text(seg["source_text"]) or "DEVANAGARI", "source_language": "hi", "source_text": seg["source_text"],
        "source_char_start": s, "source_char_end": e,
        "ocr_confidence": {"raw": conf["raw"], "calibrated": conf["calibrated"], "calibration_id": calibration_id if conf["calibrated"] is not None else None},
        "target_language": "en", "target_text": result["text"], "model": model, "provenance_origin": "TRANSLATED", "quotation_verifiable": False, "critical_fields": fields,
    }
    review = {"label": LABEL, "segment_flagged": result["segment_flagged"], "invented_markers": result["invented"], "field_flags": field_flags,
              "unconfirmed_fields": [x["field_id"] for x in fields if not x["counsel_confirmed"]]}
    return {"segment": segment_record, "review": review}
