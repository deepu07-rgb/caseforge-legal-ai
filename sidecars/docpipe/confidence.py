"""Calibrated OCR confidence and deterministic legal critical-field checks."""

from __future__ import annotations

import hashlib
import json
import re
from datetime import datetime
from functools import lru_cache
from pathlib import Path
from typing import Any

import indic_fields

ROOT = Path(__file__).resolve().parents[2]
ARTIFACT_PATH = ROOT / "eval" / "calibration" / "confidence_calibration_v1.json"
INDIC_ARTIFACT_PATH = ROOT / "eval" / "calibration" / "confidence_calibration_indic_v1.json"
INDIC_DATASET_PATH = ROOT / "eval" / "calibration" / "indic_ocr_calibration_v1.json"
INDIC_CALIBRATION_ID = "caseforge.tesseract-hin-eng.synthetic-isotonic.v1"
INDIC_ENGINE_CLASS = "tesseract_hin_eng_scan"
DATASET_PATH = ROOT / "eval" / "calibration" / "legal_ocr_calibration_v1.json"
CONSTANTS_PATH = ROOT / "contracts" / "constants.json"
CALIBRATION_ID = "caseforge.tesseract-eng-fast.synthetic-isotonic.v1"
VALIDATOR_ID = "caseforge.critical-fields.deterministic.v1"

PATTERNS: tuple[tuple[str, re.Pattern[str]], ...] = (
    ("date", re.compile(r"\b\d{1,2}\s+(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?)\.?\s+\d{4}\b", re.I)),
    ("date", re.compile(r"\b(?:\d{1,2}[./-]\d{1,2}[./-]\d{2,4}|(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?)\s+\d{1,2},?\s+\d{4})\b", re.I)),
    ("money", re.compile(r"(?<!\w)(?:USD|INR|Rs\.?|₹|\$|£)\s*[0-9OoIl][0-9OoIl,]*(?:\.[0-9OoIl]{1,2})?(?!\w)", re.I)),
    ("citation", re.compile(r"\b(?:[0-9OoIl]{1,4}\s+(?:U\.S\.|F\.?\s*\d[dD]?|S\.\s*Ct\.|AIR|SCC)\s+[0-9OoIl]{1,6}|[A-Z][A-Za-z.&' -]+\s+v\.?\s+[A-Z][A-Za-z.&' -]+,?\s+[0-9OoIl]{1,4}\s+[A-Z.]+\s+[0-9OoIl]{1,6})\b")),
    ("provision", re.compile(r"\b(?:Section|Sec\.?|Rule|Order|Article)\s+(?:[0-9][0-9A-Za-z]*|(?-i:[IVXLC]{1,7}(?![A-Za-z])))(?:\([0-9A-Za-z]+\))*\b", re.I)),
    ("case_number", re.compile(r"\b(?:Case|Civil Case|Criminal Case|Crl\.?|Writ|Appeal|Suit)\s+No\.?\s*[A-Z0-9][A-Z0-9./-]{2,}\b", re.I)),
    ("exhibit_label", re.compile(r"\b(?:Exhibit|Annexure|Schedule)\s*[-:]?\s*[A-Z0-9][A-Z0-9./-]*\b", re.I)),
    ("name", re.compile(r"\b(?:Mr\.|Ms\.|Mrs\.|Dr\.|Justice|Judge)\s+[A-Z][A-Za-z'’-]+(?:\s+[A-Z][A-Za-z'’-]+){1,3}\b")),
)


def _thresholds() -> dict[str, float]:
    values = json.loads(CONSTANTS_PATH.read_text(encoding="utf-8-sig"))["confidence_thresholds"]["values"]
    return {key: float(value) for key, value in values.items()}


@lru_cache(maxsize=1)
def load_calibration() -> dict[str, Any]:
    artifact = json.loads(ARTIFACT_PATH.read_text(encoding="utf-8"))
    actual = hashlib.sha256(DATASET_PATH.read_bytes()).hexdigest()
    if artifact.get("dataset_sha256") != actual:
        raise RuntimeError("CF_CONFIDENCE_DATASET_CHECKSUM: calibration dataset fingerprint mismatch")
    return artifact


@lru_cache(maxsize=1)
def load_indic_calibration() -> dict[str, Any] | None:
    """The Hindi calibration map, bound to its dataset's hash. None when it has not been built (raw confidence only)."""
    if not INDIC_ARTIFACT_PATH.is_file() or not INDIC_DATASET_PATH.is_file():
        return None
    artifact = json.loads(INDIC_ARTIFACT_PATH.read_text(encoding="utf-8"))
    actual = hashlib.sha256(INDIC_DATASET_PATH.read_bytes()).hexdigest()
    if artifact.get("dataset_sha256") != actual:
        raise RuntimeError("CF_CONFIDENCE_DATASET_CHECKSUM: Hindi calibration dataset fingerprint mismatch")
    return artifact


def calibrate(raw: float | None, engine_class: str) -> tuple[float | None, str]:
    if raw is None:
        return None, "missing"
    if engine_class == INDIC_ENGINE_CLASS:
        artifact = load_indic_calibration()
        if artifact is None:
            return None, "raw_only"
    elif engine_class != "tesseract_eng_fast_scan":
        return None, "raw_only"
    else:
        artifact = load_calibration()
    value = min(1.0, max(0.0, float(raw)))
    for item in artifact["bins"]:
        if float(item["raw_min"]) <= value < float(item["raw_max"]):
            return float(item["calibrated"]), "calibrated"
    return None, "raw_only"


def calibrate_words(words: list[dict[str, Any]], engine_class: str) -> None:
    thresholds = _thresholds()
    for word in words:
        raw = word.get("confidence")
        calibrated, state = calibrate(float(raw) if raw is not None else None, engine_class)
        word["raw_engine_confidence"] = raw
        word["calibrated_confidence"] = calibrated
        word["confidence_state"] = state
        effective = calibrated if calibrated is not None else (float(raw) if raw is not None else 0.0)
        word["confidence"] = effective
        word["low_confidence"] = calibrated is None or effective < thresholds["word_highlight_below"]
        word["force_review"] = calibrated is None or effective < thresholds["word_force_review_below"]
        word["suppress_from_auto"] = calibrated is None or effective < thresholds["word_suppress_below"]


def _date_valid(value: str) -> tuple[bool, list[str]]:
    if re.fullmatch(r"\d{1,2}\s+[A-Za-z]+\.?\s+\d{4}", value.strip()):
        for fmt in ("%d %B %Y", "%d %b %Y", "%d %b. %Y"):
            try:
                datetime.strptime(re.sub(r"\s+", " ", value.strip()), fmt)
                return True, ["calendar-valid named-month date"]
            except ValueError:
                continue
        return False, ["calendar-invalid date"]
    normalized = value.replace(".", "/").replace("-", "/")
    if re.fullmatch(r"\d{1,2}/\d{1,2}/\d{2,4}", normalized):
        day, month, year = [int(part) for part in normalized.split("/")]
        if day <= 12 and month <= 12:
            return False, ["numeric date is locale-ambiguous"]
        try:
            datetime(year + 2000 if year < 100 else year, month, day)
            return True, ["calendar-valid unambiguous numeric date"]
        except ValueError:
            return False, ["calendar-invalid date"]
    for fmt in ("%B %d %Y", "%B %d, %Y", "%b %d %Y", "%b %d, %Y"):
        try:
            datetime.strptime(value, fmt)
            return True, ["calendar-valid named-month date"]
        except ValueError:
            continue
    return False, ["date format could not be validated"]


def _validate(kind: str, value: str) -> tuple[bool, list[str]]:
    if indic_fields.HAS_DEV.search(value):
        return indic_fields.validate(kind, value)
    suspicious = bool(re.search(r"[|{}?]", value) or re.search(r"\d[OoIl]|[OoIl]\d|[A-Za-z][01][A-Za-z]", value))
    reasons = ["OCR-confusable character pattern detected"] if suspicious else []
    if kind == "date":
        valid, detail = _date_valid(value)
        return valid and not suspicious, reasons + detail
    if kind == "money":
        digits = re.sub(r"\D", "", value)
        # Indian amounts group as 1,25,000 (lakh and crore); that grouping is right after a rupee marker and wrong after any other
        indian = bool(re.search(r"(?:₹|\bRs\.?|\bINR)", value, re.I)) and all(
            re.fullmatch(r"\d{1,2}(?:,\d{2})*,\d{3}(?:\.\d+)?|\d{1,3}(?:,\d{3})*(?:\.\d+)?|\d+(?:\.\d+)?", n) for n in re.findall(r"\d[\d,]*(?:\.\d+)?", value))
        valid = bool(digits) and (indian or not re.search(r",\d{1,2}(?:\D|$)", value))
        return valid and not suspicious, reasons + (["currency marker and numeric grouping validated"] if valid else ["invalid currency amount grouping"])
    if kind == "citation":
        valid = bool(re.search(r"\d", value) and re.search(r"(?:U\.S\.|SCC|AIR|F\.?\s*\d|S\.\s*Ct\.)", value, re.I))
        return valid and not suspicious, reasons + (["reporter and numeric components present"] if valid else ["citation reporter structure incomplete"])
    if kind == "provision":
        valid = bool(re.search(r"\b(?:Section|Sec|Rule|Order|Article)\b", value, re.I) and re.search(r"\d", value))
        return valid and not suspicious, reasons + (["provision prefix and identifier present"] if valid else ["provision identifier incomplete"])
    if kind == "case_number":
        valid = bool(re.search(r"\d", value) and len(re.sub(r"\s", "", value)) >= 6)
        return valid and not suspicious, reasons + (["case-number label and identifier present"] if valid else ["case-number identifier incomplete"])
    if kind == "exhibit_label":
        valid = bool(re.search(r"(?:Exhibit|Annexure|Schedule)", value, re.I) and re.search(r"[A-Z0-9]", value))
        return valid and not suspicious, reasons + (["exhibit label structure present"] if valid else ["exhibit identifier missing"])
    if kind == "name":
        valid = len(value.split()) >= 3
        return valid and not suspicious, reasons + (["honorific and multi-token name present"] if valid else ["name structure incomplete"])
    return False, reasons + ["unsupported critical-field type"]


def _union_bbox(words: list[dict[str, Any]]) -> dict[str, float] | None:
    boxes = [word.get("bbox") for word in words if word.get("bbox")]
    if not boxes:
        return None
    left, top = min(box["x"] for box in boxes), min(box["y"] for box in boxes)
    right, bottom = max(box["x"] + box["w"] for box in boxes), max(box["y"] + box["h"] for box in boxes)
    return {"x": round(left, 7), "y": round(top, 7), "w": round(right - left, 7), "h": round(bottom - top, 7)}


def validate_critical_fields(text: str, words: list[dict[str, Any]], *, exact_text: bool = False) -> list[dict[str, Any]]:
    """exact_text: the text is the file's own text (a digital PDF, a text file), not an engine's reading. There is no engine confidence to be
    missing and no reading error to guard against, so a field is held only when its own check fails (an impossible or ambiguous date, a malformed
    amount), never merely because it is a date. Word confidence stays "missing", honestly: nothing was read."""
    thresholds = _thresholds()
    found: list[dict[str, Any]] = []
    occupied: set[tuple[int, int, str]] = set()
    spans: list[tuple[str, int, int, str]] = [(kind, m.start(), m.end(), m.group(0)) for kind, pattern in PATTERNS for m in pattern.finditer(text)]
    # Hindi fields (Devanagari or Latin digits, Hindi months, rupee words, dhara numbers, labelled case numbers, names after an honorific)
    spans += [(kind, s, e, text[s:e]) for kind, s, e in indic_fields.find(text)]
    for kind, start, end, matched in spans:
        if True:
            key = (start, end, kind)
            if key in occupied:
                continue
            occupied.add(key)
            source_words = [word for word in words if word["char_start"] < end and word["char_end"] > start]
            values = [word.get("calibrated_confidence") for word in source_words if word.get("calibrated_confidence") is not None]
            raw_values = [word.get("raw_engine_confidence") for word in source_words if word.get("raw_engine_confidence") is not None]
            valid, reasons = _validate(kind, matched)
            calibrated = min(values) if values and len(values) == len(source_words) else None
            raw = min(raw_values) if raw_values and len(raw_values) == len(source_words) else None
            confidence_state = "calibrated" if calibrated is not None else "missing"
            field_confidence = calibrated if valid and calibrated is not None else (min(calibrated or 0.0, 0.49) if calibrated is not None else None)
            if exact_text and calibrated is None:
                field_confidence = 1.0 if valid else 0.49
            force = field_confidence is None or field_confidence < thresholds["item_block_downstream_below"] or not valid
            found.append({
                "field_index": len(found), "field_type": kind, "text": matched,
                "char_start": start, "char_end": end, "bbox": _union_bbox(source_words),
                "raw_engine_confidence": raw, "calibrated_confidence": calibrated,
                "field_confidence": field_confidence, "confidence_state": confidence_state,
                "syntactically_valid": valid, "validation_reasons": reasons,
                "force_review": force, "suppress_from_auto": force,
                "counsel_confirmed": False, "validator_id": VALIDATOR_ID,
            })
    return found
