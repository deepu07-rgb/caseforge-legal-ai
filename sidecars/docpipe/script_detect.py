"""Script detection for pages, regions and lines (B-S4-14, PRD v4.0 section 7.9).

Two sources, both deterministic and offline:
  * the OCR engine's orientation-and-script detector (OSD) tells, before any recognition, which script a page is mostly in, with a
    confidence - used to choose the recognizer;
  * the characters of recognized or native text tell, after recognition, which script each line and the page are in - used to label
    regions and to let a screen show "Hindi", "English" or "mixed".

A line or page is MIXED when the minority script makes up at least 15% of its letters. Only letters count (digits, punctuation and combining
vowel signs do not), so a Hindi sentence that quotes "Rs." or a section number stays DEVANAGARI, and a sentence with a whole English phrase
in it becomes MIXED. Text with no letters has no script (None) and takes its neighbours' label.
"""

from __future__ import annotations

import re
import unicodedata
from typing import Any

DEVANAGARI = "DEVANAGARI"
LATIN = "LATIN"
MIXED = "MIXED"
MINORITY_SHARE = 0.15
OSD_ORIENTATION_MIN_CONFIDENCE = 2.0

_OSD_SCRIPTS = {"devanagari": DEVANAGARI, "latin": LATIN}


def _counts(text: str) -> tuple[int, int]:
    dev = lat = 0
    for ch in text:
        if not unicodedata.category(ch).startswith("L"):
            continue
        cp = ord(ch)
        if 0x0900 <= cp <= 0x097F or 0xA8E0 <= cp <= 0xA8FF:
            dev += 1
        elif cp < 0x250 and ch.isalpha():
            lat += 1
    return dev, lat


def script_of_text(text: str) -> str | None:
    dev, lat = _counts(text)
    total = dev + lat
    if total == 0:
        return None
    if min(dev, lat) / total >= MINORITY_SHARE:
        return MIXED
    return DEVANAGARI if dev > lat else LATIN


def line_regions(text: str) -> list[dict[str, Any]]:
    """One region per non-empty line of the text: its character span and script (None when it has no letters)."""
    out: list[dict[str, Any]] = []
    cursor = 0
    for index, line in enumerate(text.split("\n")):
        start, end = cursor, cursor + len(line)
        cursor = end + 1
        if not line.strip():
            continue
        dev, lat = _counts(line)
        out.append({"line": index, "char_start": start, "char_end": end, "script": script_of_text(line), "devanagari_letters": dev, "latin_letters": lat})
    return out


def describe(text: str) -> dict[str, Any]:
    dev, lat = _counts(text)
    return {"script": script_of_text(text), "devanagari_letters": dev, "latin_letters": lat, "regions": line_regions(text)}


def parse_osd(output: str) -> dict[str, Any]:
    """Reads Tesseract's OSD report. Rotation is only trusted above the orientation confidence floor; a page with few glyphs gets an
    unreliable orientation (a short English page was once turned upside down by a 0.71-confidence guess)."""
    def num(label: str) -> float | None:
        m = re.search(rf"{label}:\s*([0-9.]+)", output)
        return float(m.group(1)) if m else None

    rot = re.search(r"Rotate:\s*(0|90|180|270)", output)
    script = re.search(r"Script:\s*([A-Za-z_]+)", output)
    oc = num("Orientation confidence")
    sc = num("Script confidence")
    rotation = int(rot.group(1)) if rot else 0
    trusted = bool(rot) and oc is not None and oc >= OSD_ORIENTATION_MIN_CONFIDENCE
    return {
        "rotation": rotation if trusted else 0,
        "rotation_suggested": rotation,
        "rotation_trusted": trusted,
        "orientation_confidence": oc,
        "script": _OSD_SCRIPTS.get((script.group(1) if script else "").lower()),
        "script_name": script.group(1) if script else None,
        "script_confidence": sc,
    }
