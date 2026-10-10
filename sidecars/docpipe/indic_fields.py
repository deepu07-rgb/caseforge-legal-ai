"""Critical-field recognition and validation for Hindi (Devanagari) text (B-S4-14).

The English validator (confidence.py) finds dates, amounts, provisions, case numbers and names written the way English documents write them.
This module finds the same kinds when they are written the way a Hindi filing writes them: Devanagari or Latin digits, Hindi month names,
the rupee sign or the words for rupees, amounts in lakh and crore, "dhara" section numbers, labelled case numbers, and names after an honorific.
Only matches that contain a Devanagari character are returned here; the English patterns keep the pure-Latin cases, so English output does not
change.

Nothing here guesses. A numeric date with both day and month up to 12 is "locale-ambiguous" and is held for review, as in English. A number that
mixes digit systems, or sits next to a letter an OCR engine confuses with a digit, is "OCR-confusable" and is held for review. A field that
does not validate is never an automatic input; counsel confirms it.
"""

from __future__ import annotations

import re
from datetime import datetime

_DEV_DIGITS = "०१२३४५६७८९"
_TO_ASCII = {ord(c): str(i) for i, c in enumerate(_DEV_DIGITS)}
DG = "[०-९0-9]"
DEV = "ऀ-ॿ"
HAS_DEV = re.compile("[" + DEV + "]")

MONTHS = {
    "जनवरी": 1, "फरवरी": 2, "मार्च": 3, "अप्रैल": 4, "मई": 5, "जून": 6, "जुलाई": 7, "अगस्त": 8,
    "सितंबर": 9, "सितम्बर": 9, "अक्टूबर": 10, "अक्तूबर": 10, "नवंबर": 11, "नवम्बर": 11, "दिसंबर": 12, "दिसम्बर": 12,
}
_MONTH_ALT = "|".join(sorted(MONTHS, key=len, reverse=True))
_UNIT = r"(?:लाख|करोड़|करोड|हजार|हज़ार)"
_MULT = {"लाख": 100_000, "करोड़": 10_000_000, "करोड": 10_000_000, "हजार": 1_000, "हज़ार": 1_000}
_NUM = rf"{DG}[{_DEV_DIGITS}0-9,]*(?:\.{DG}+)?"
_AMT = rf"{_NUM}(?:\s*{_UNIT})?(?:\s+{_NUM}\s*{_UNIT}){{0,2}}"
_CUR = r"(?:₹|रु\.?|रुपये|रुपए|Rs\.?|INR)"
_NOT_DIGIT_BEFORE = rf"(?<![०-९0-9])"
_NOT_DIGIT_AFTER = rf"(?![०-९0-9])"

_PATTERNS: tuple[tuple[str, re.Pattern[str]], ...] = (
    ("date", re.compile(rf"{_NOT_DIGIT_BEFORE}{DG}{{1,2}}[./-]{DG}{{1,2}}[./-]{DG}{{2,4}}{_NOT_DIGIT_AFTER}")),
    ("date", re.compile(rf"{_NOT_DIGIT_BEFORE}{DG}{{1,2}}\s+(?:{_MONTH_ALT})\s+{DG}{{4}}{_NOT_DIGIT_AFTER}")),
    ("money", re.compile(rf"{_CUR}\s*{_AMT}(?:\s*/-)?")),
    ("money", re.compile(rf"{_NOT_DIGIT_BEFORE}{_AMT}\s*(?:रुपये|रुपए)")),
    # An amount in lakh/crore/hazar with no currency marker. A rupee sign that OCR misread (₹ read as a letter) leaves exactly this, and it must
    # surface as a field held for review rather than vanish; validate() holds it because the marker is missing.
    ("money", re.compile(rf"{_NOT_DIGIT_BEFORE}{_NUM}\s*{_UNIT}(?:\s+{_NUM}\s*{_UNIT}){{0,2}}")),
    ("provision", re.compile(rf"धारा\s*{DG}+(?:\({DG}+\))*(?:\s*[/,-]\s*{DG}+(?:\({DG}+\))*)*")),
    ("case_number", re.compile(rf"(?:वाद\s*संख्या|प्रथम\s*सूचना\s*रिपोर्ट\s*सं(?:ख्या)?[.,:;]?|अपराध\s*क्रमांक|अपराध\s*संख्या|मुकदमा\s*संख्या|केस\s*नंबर)\s*[:.]?\s*{DG}+/{DG}{{2,4}}{_NOT_DIGIT_AFTER}")),
)

_STOP = r"(?:पुत्र|पुत्री|निवासी|ने|को|की|के|से|में|का|और|बनाम|द्वारा|उपस्थित|गवाह|आयु|एवं|तथा)"
_WORD = rf"(?!{_STOP}(?![{DEV}]))[{DEV}]+"
_NAME = re.compile(rf"(?:श्रीमती|सुश्री|कुमारी|श्री|डॉ\.?)[ \t]+{_WORD}(?:[ \t]+{_WORD}){{1,2}}")


def to_ascii_digits(s: str) -> str:
    return s.translate(_TO_ASCII)


def find(text: str) -> list[tuple[str, int, int]]:
    """(kind, start, end) of every Devanagari-flavoured critical field in the text."""
    found: list[tuple[str, int, int]] = []
    for kind, pattern in _PATTERNS:
        for m in pattern.finditer(text):
            if HAS_DEV.search(m.group(0)) or (kind == "date" and re.search("[०-९]", m.group(0))):
                found.append((kind, m.start(), m.end()))
    for m in _NAME.finditer(text):
        found.append(("name", m.start(), m.end()))
    # a match inside a longer match of the same kind is the same field seen twice (an amount with its marker and the bare amount)
    return [f for f in found if not any(g is not f and g[0] == f[0] and g[1] <= f[1] and f[2] <= g[2] and (g[1], g[2]) != (f[1], f[2]) for g in found)]


# ---------- values ----------

def parse_date(value: str) -> str | None:
    """ISO date, day-first, or None when it is not a calendar date. (A numeric date with day and month both up to 12 is still returned
    here; validate() is what holds it for review.)"""
    v = to_ascii_digits(value).strip()
    m = re.fullmatch(r"(\d{1,2})[./-](\d{1,2})[./-](\d{2,4})", v)
    if m:
        d, mo, y = int(m.group(1)), int(m.group(2)), int(m.group(3))
    else:
        m = re.fullmatch(rf"(\d{{1,2}})\s+({_MONTH_ALT})\s+(\d{{4}})", v)
        if not m:
            return None
        d, mo, y = int(m.group(1)), MONTHS[m.group(2)], int(m.group(3))
    y = y + 2000 if y < 100 else y
    try:
        return datetime(y, mo, d).strftime("%Y-%m-%d")
    except ValueError:
        return None


def parse_amount(value: str) -> int | None:
    v = to_ascii_digits(value)
    groups = re.findall(rf"(\d[\d,]*(?:\.\d+)?)(?:\s*({_UNIT}))?", v)
    total = 0.0
    seen = False
    for num, unit in groups:
        try:
            n = float(num.replace(",", ""))
        except ValueError:
            continue
        seen = True
        total += n * _MULT.get(unit, 1)
    return int(round(total)) if seen else None


def parse_sections(value: str) -> list[str]:
    return re.findall(r"\d+(?:\(\d+\))*", to_ascii_digits(value))


def _grouping_ok(num: str) -> bool:
    n = to_ascii_digits(num)
    if "," not in n:
        return True
    whole = n.split(".")[0]
    return bool(re.fullmatch(r"\d{1,2}(?:,\d{2})*,\d{3}", whole) or re.fullmatch(r"\d{1,3}(?:,\d{3})+", whole))


# ---------- validation ----------

def _suspicious(value: str) -> bool:
    return bool(
        re.search(r"[०-९][A-Za-z|]|[A-Za-z|][०-९]", value)
        or re.search(r"[०-९]\s*[0-9]|[0-9]\s*[०-९]", value)
        or re.search(r"[?�{}]", value)
    )


def validate(kind: str, value: str) -> tuple[bool, list[str]]:
    reasons = ["OCR-confusable character pattern detected"] if _suspicious(value) else []
    bad = bool(reasons)
    if kind == "date":
        iso = parse_date(value)
        if iso is None:
            return False, reasons + ["date format could not be validated"]
        a = to_ascii_digits(value)
        m = re.fullmatch(r"(\d{1,2})[./-](\d{1,2})[./-]\d{2,4}", a.strip())
        if m and int(m.group(1)) <= 12 and int(m.group(2)) <= 12:
            return False, reasons + ["numeric date is locale-ambiguous"]
        return (not bad), reasons + [("calendar-valid unambiguous numeric date" if m else "calendar-valid named-month date")]
    if kind == "money":
        amount = parse_amount(value)
        nums = re.findall(rf"{_NUM}", value)
        grouped = all(_grouping_ok(n) for n in nums) if nums else False
        marked = bool(re.search(r"₹|रु|रुपये|रुपए|Rs|INR", value))
        if not marked:
            return False, reasons + ["currency marker missing (an unreadable rupee sign leaves this)"]
        valid = amount is not None and amount > 0 and grouped
        return valid and not bad, reasons + (["currency marker and numeric grouping validated"] if valid else ["invalid currency amount grouping"])
    if kind == "provision":
        valid = bool(re.search(r"\d", to_ascii_digits(value)))
        return valid and not bad, reasons + (["provision prefix and identifier present"] if valid else ["provision identifier incomplete"])
    if kind == "case_number":
        v = to_ascii_digits(value)
        m = re.search(r"(\d+)/(\d{2,4})\s*$", v)
        valid = bool(m) and (len(m.group(2)) == 2 or 1900 <= int(m.group(2)) <= 2100)
        return valid and not bad, reasons + (["case-number label and identifier present"] if valid else ["case-number identifier incomplete"])
    if kind == "name":
        valid = len(value.split()) >= 3
        return valid and not bad, reasons + (["honorific and multi-token name present"] if valid else ["name structure incomplete"])
    return False, reasons + ["unsupported critical-field type"]
