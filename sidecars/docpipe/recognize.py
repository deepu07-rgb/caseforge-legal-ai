"""Offline OCR (English, and Hindi with English) and native text-layer construction (B-S3-04, B-S4-14).

Tesseract runs as a local subprocess over stdin/stdout. Source bytes, rendered
pages, and processed images are never written to disk. Every OCR word carries
the engine confidence and a normalized source-page bounding box.
"""

from __future__ import annotations

import base64
import hashlib
import io
import json
import os
import re
import shutil
import subprocess
import threading
import time
from pathlib import Path
from typing import Any, Callable

from PIL import Image, ImageEnhance, ImageFilter, ImageOps

import script_detect
from confidence import CALIBRATION_ID, INDIC_CALIBRATION_ID, INDIC_ENGINE_CLASS, calibrate_words, validate_critical_fields
from pdf_parser import PdfParseError, parse_pdf_bytes
from triage import pages_from_document

ROOT = Path(__file__).resolve().parents[2]
CONFIG_PATH = ROOT / "contracts" / "ocr_config.json"
INDIC_CONFIG_PATH = ROOT / "contracts" / "ocr_config_indic.json"
CONSTANTS_PATH = ROOT / "contracts" / "constants.json"
RECOGNIZER_ID = "caseforge.ocr.tesseract-eng-fast.v1"
INDIC_RECOGNIZER_ID = "caseforge.ocr.tesseract-hin-eng-best.v1"
ROUTE_ENGLISH, ROUTE_INDIC, ROUTE_AUTO = "english", "indic", "auto"
WORD_RE = re.compile(r"\S+")

_FALLBACK = {
    "word_highlight_below": 0.75,
    "word_force_review_below": 0.5,
    "word_suppress_below": 0.25,
    "page_mean_warn_below": 0.7,
}


class OcrError(RuntimeError):
    def __init__(self, code: str, safe_message: str):
        super().__init__(f"{code}: {safe_message}")
        self.code = code
        self.safe_message = safe_message


def load_thresholds() -> dict[str, float]:
    if not CONSTANTS_PATH.is_file():
        return dict(_FALLBACK)
    data = json.loads(CONSTANTS_PATH.read_text(encoding="utf-8-sig"))
    values = ((data.get("confidence_thresholds") or {}).get("values") or {})
    return {key: float(values.get(key, fallback)) for key, fallback in _FALLBACK.items()}


def load_ocr_config() -> dict[str, Any]:
    if not CONFIG_PATH.is_file():
        raise OcrError("CF_OCR_CONFIG_MISSING", "frozen OCR configuration is unavailable")
    return json.loads(CONFIG_PATH.read_text(encoding="utf-8"))


def load_indic_config() -> dict[str, Any] | None:
    """The Hindi OCR configuration, or None when this build does not carry it."""
    if not INDIC_CONFIG_PATH.is_file():
        return None
    return json.loads(INDIC_CONFIG_PATH.read_text(encoding="utf-8"))


def indic_model_ready(config: dict[str, Any] | None = None) -> bool:
    """True when the Hindi model is present and matches its recorded hash. A model that fails the check is never used."""
    config = config or load_indic_config()
    if not config:
        return False
    try:
        model = find_tesseract().parent / "tessdata" / config["model"]["filename"]
    except OcrError:
        return False
    return model.is_file() and hashlib.sha256(model.read_bytes()).hexdigest() == config["model"]["sha256"].lower()


def recognize_text(text: str, *, thresholds: dict[str, float] | None = None) -> dict[str, Any]:
    """Legacy unit-test helper; production document routing uses native spans or OCR."""
    values = thresholds or load_thresholds()
    words = []
    for index, match in enumerate(WORD_RE.finditer(text or "")):
        token = match.group(0)
        confidence = 0.28 if "???" in token else 1.0
        words.append({
            "word_index": index, "text": token, "confidence": confidence,
            "char_start": match.start(), "char_end": match.end(),
            "low_confidence": confidence < values["word_highlight_below"],
            "force_review": confidence < values["word_force_review_below"],
            "suppress_from_auto": confidence < values["word_suppress_below"], "bbox": None,
        })
    mean = sum(word["confidence"] for word in words) / len(words) if words else 0.0
    return {"full_text": text or "", "mean_confidence": mean,
            "page_quality_warn": not words or mean < values["page_mean_warn_below"], "words": words}


def _runtime_root() -> Path:
    return Path(__file__).resolve().parent / "runtime" / "tesseract"


def find_tesseract() -> Path:
    configured = os.environ.get("CASEFORGE_TESSERACT")
    candidates = [Path(configured)] if configured else []
    candidates += [_runtime_root() / ("tesseract.exe" if os.name == "nt" else "tesseract")]
    located = shutil.which("tesseract")
    if located:
        candidates.append(Path(located))
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()
    raise OcrError("CF_OCR_RUNTIME_MISSING", "the bundled offline OCR runtime is unavailable")


def runtime_manifest() -> dict[str, Any]:
    config = load_ocr_config()
    executable = find_tesseract()
    model = executable.parent / "tessdata" / config["model"]["filename"]
    if not model.is_file():
        raise OcrError("CF_OCR_MODEL_MISSING", "the frozen English OCR model is unavailable")
    checksum = hashlib.sha256(model.read_bytes()).hexdigest()
    expected = config["model"]["sha256"].lower()
    if checksum != expected:
        raise OcrError("CF_OCR_MODEL_CHECKSUM", "the English OCR model failed integrity verification")
    version = subprocess.run(
        [str(executable), "--version"], capture_output=True, text=True, timeout=10, check=False
    ).stdout.splitlines()[0].strip()
    return {
        "recognizer_id": RECOGNIZER_ID,
        "engine_version": version,
        "model_sha256": checksum,
        "language": config["language"],
        "network_required": False,
    }


def _png_bytes(image: Image.Image) -> bytes:
    output = io.BytesIO()
    image.save(output, format="PNG", optimize=True)
    return output.getvalue()


def _osd(image: Image.Image, executable: Path) -> dict[str, Any]:
    """Orientation and script of the page. Rotation is trusted only above the confidence floor; with too few glyphs the report is a guess,
    and a short English page was once turned upside down by a 0.71-confidence one."""
    osd_image = image.copy()
    osd_image.thumbnail((1200, 1200))
    try:
        proc = subprocess.run(
            [str(executable), "stdin", "stdout", "-l", "osd", "--psm", "0"],
            input=_png_bytes(osd_image), capture_output=True, timeout=15, check=False,
        )
    except subprocess.TimeoutExpired:
        return script_detect.parse_osd("")
    text = proc.stdout.decode("utf-8", errors="replace") + "\n" + proc.stderr.decode("utf-8", errors="replace")
    return script_detect.parse_osd(text)


def _orientation(image: Image.Image, executable: Path) -> int:
    """Return the clockwise correction; 0 when the engine is not confident."""
    return int(_osd(image, executable)["rotation"])


def choose_route(osd: dict[str, Any], indic_ready: bool) -> tuple[str, str]:
    """Which recognizer a page gets, and why. Deterministic and recorded with the page."""
    config = load_indic_config() or {}
    floor = float((config.get("routing") or {}).get("latin_script_confidence_min", 1.0))
    if not indic_ready:
        return ROUTE_ENGLISH, "hindi model not installed"
    if osd.get("script") == script_detect.DEVANAGARI:
        return ROUTE_INDIC, "osd detected Devanagari"
    if osd.get("script") == script_detect.LATIN and (osd.get("script_confidence") or 0.0) >= floor:
        return ROUTE_ENGLISH, "osd detected Latin"
    return ROUTE_AUTO, "osd script unclear"


def _deskew_angle(image: Image.Image) -> float:
    """Small-angle projection-profile deskew; deterministic and dependency-light."""
    sample = ImageOps.grayscale(image)
    # 400 px is enough for a projection profile and bounds CPU on 300-DPI batches.
    sample.thumbnail((400, 400))
    sample = ImageOps.autocontrast(sample).point(lambda p: 0 if p < 190 else 255)
    best_angle, best_score = 0.0, -1.0
    for tenths in range(-30, 31, 5):
        angle = tenths / 10.0
        rotated = sample.rotate(angle, expand=False, fillcolor=255)
        pixels = list(rotated.getdata())
        width, height = rotated.size
        rows = [sum(1 for x in range(width) if pixels[y * width + x] < 128) for y in range(height)]
        mean = sum(rows) / max(len(rows), 1)
        score = sum((value - mean) ** 2 for value in rows)
        if score > best_score:
            best_angle, best_score = angle, score
    return best_angle if abs(best_angle) >= 0.4 else 0.0


def preprocess_image(image: Image.Image, executable: Path) -> tuple[Image.Image, dict[str, Any]]:
    original_size = image.size
    osd = _osd(image, executable)
    orientation = int(osd["rotation"])
    if orientation:
        image = image.rotate(-orientation, expand=True, fillcolor="white")
    deskew = _deskew_angle(image)
    if deskew:
        image = image.rotate(deskew, expand=True, fillcolor="white")
    scale = max(1.0, 2200.0 / max(image.width, image.height))
    if scale > 1.01:
        image = image.resize((round(image.width * scale), round(image.height * scale)), Image.Resampling.LANCZOS)
    processed = ImageOps.grayscale(image)
    processed = ImageOps.autocontrast(processed, cutoff=1)
    processed = processed.filter(ImageFilter.MedianFilter(size=3))
    processed = ImageEnhance.Contrast(processed).enhance(1.15)
    return processed, {
        "orientation_correction_degrees": orientation,
        "deskew_degrees": deskew,
        "osd": {k: osd[k] for k in ("script", "script_name", "script_confidence", "orientation_confidence", "rotation_suggested", "rotation_trusted")},
        "original_size": {"width": original_size[0], "height": original_size[1]},
        "processed_size": {"width": processed.width, "height": processed.height},
        "steps": ["osd", "small-angle-deskew", "min-long-edge-2200", "grayscale", "autocontrast", "median-3", "contrast-1.15"],
    }


def _parse_tsv(tsv: str, width: int, height: int, thresholds: dict[str, float]) -> tuple[str, list[dict[str, Any]]]:
    selected: list[dict[str, Any]] = []
    lines = tsv.splitlines()
    headers = lines[0].split("\t") if lines else []
    for line in lines[1:]:
        values = line.split("\t", 11)
        if len(headers) != 12 or len(values) != 12:
            continue
        row = dict(zip(headers, values))
        text = (row.get("text") or "").strip()
        try:
            if int(row.get("level") or 0) != 5 or not text:
                continue
            raw_conf = float(row.get("conf") or -1)
            if raw_conf < 0:
                continue
            selected.append({**row, "text": text, "confidence": min(1.0, max(0.0, raw_conf / 100.0))})
        except (TypeError, ValueError):
            continue

    full_parts: list[str] = []
    words: list[dict[str, Any]] = []
    previous_line: tuple[str, str, str, str] | None = None
    cursor = 0
    for index, row in enumerate(selected):
        line = tuple(row.get(key, "") for key in ("page_num", "block_num", "par_num", "line_num"))
        separator = "\n" if previous_line is not None and line != previous_line else (" " if full_parts else "")
        full_parts.append(separator)
        cursor += len(separator)
        token = row["text"]
        start, end = cursor, cursor + len(token)
        full_parts.append(token)
        cursor = end
        confidence = float(row["confidence"])
        left, top = int(row["left"]), int(row["top"])
        box_width, box_height = int(row["width"]), int(row["height"])
        words.append({
            "word_index": index, "text": token, "confidence": confidence,
            "char_start": start, "char_end": end,
            "low_confidence": confidence < thresholds["word_highlight_below"],
            "force_review": confidence < thresholds["word_force_review_below"],
            "suppress_from_auto": confidence < thresholds["word_suppress_below"],
            "bbox": {
                "x": round(left / width, 7), "y": round(top / height, 7),
                "w": round(box_width / width, 7), "h": round(box_height / height, 7),
            },
        })
        previous_line = line
    return "".join(full_parts), words


def _rss_bytes(pid: int) -> int:
    if os.name == "nt":
        try:
            import ctypes
            from ctypes import wintypes
            class Counters(ctypes.Structure):
                _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD),
                            ("PeakWorkingSetSize", ctypes.c_size_t), ("WorkingSetSize", ctypes.c_size_t),
                            ("QuotaPeakPagedPoolUsage", ctypes.c_size_t), ("QuotaPagedPoolUsage", ctypes.c_size_t),
                            ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t), ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                            ("PagefileUsage", ctypes.c_size_t), ("PeakPagefileUsage", ctypes.c_size_t)]
            handle = ctypes.windll.kernel32.OpenProcess(0x0410, False, pid)
            counters = Counters(); counters.cb = ctypes.sizeof(counters)
            ok = ctypes.windll.psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb)
            ctypes.windll.kernel32.CloseHandle(handle)
            return int(counters.WorkingSetSize) if ok else 0
        except Exception:
            return 0
    try:
        return int(Path(f"/proc/{pid}/status").read_text().split("VmRSS:", 1)[1].split()[0]) * 1024
    except Exception:
        return 0


def _run_engine(command: list[str], payload: bytes, timeout: int) -> tuple[subprocess.CompletedProcess[bytes], int]:
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    result: dict[str, Any] = {}
    def communicate() -> None:
        try:
            result["streams"] = process.communicate(input=payload, timeout=timeout)
        except subprocess.TimeoutExpired:
            process.kill()
            result["streams"] = process.communicate()
            result["timeout"] = True
    worker = threading.Thread(target=communicate, daemon=True)
    worker.start()
    peak_rss = 0
    while worker.is_alive():
        peak_rss = max(peak_rss, _rss_bytes(process.pid))
        worker.join(0.02)
    stdout, stderr = result.get("streams", (b"", b""))
    if result.get("timeout"):
        raise OcrError("CF_OCR_TIMEOUT", "offline OCR exceeded the per-page time limit")
    return subprocess.CompletedProcess(command, process.returncode or 0, stdout, stderr), peak_rss


INDIC_MIN_LETTERS = 20
INDIC_MIN_SHARE = 0.30


def _reads_devanagari(text: str) -> bool:
    dev, lat = script_detect._counts(text)
    return dev >= INDIC_MIN_LETTERS and dev / max(1, dev + lat) >= INDIC_MIN_SHARE


def _recognize_pass(route: str, processed: Image.Image, processed_bytes: bytes, executable: Path, thresholds: dict[str, float]) -> dict[str, Any]:
    """One engine run. route is ROUTE_ENGLISH (the frozen English path) or ROUTE_INDIC (Hindi with English)."""
    if route == ROUTE_INDIC:
        config = load_indic_config() or {}
        engine_class, recognizer_id, calibration_id = INDIC_ENGINE_CLASS, INDIC_RECOGNIZER_ID, INDIC_CALIBRATION_ID
    else:
        config = load_ocr_config()
        engine_class, recognizer_id, calibration_id = "tesseract_eng_fast_scan", RECOGNIZER_ID, CALIBRATION_ID
    command = [str(executable), "stdin", "stdout", "-l", config["language"], "--oem", str(config["oem"]), "--psm", str(config["psm"]), "tsv"]
    proc, peak_rss = _run_engine(command, processed_bytes, int(config["timeout_seconds_per_page"]))
    if proc.returncode != 0:
        raise OcrError("CF_OCR_ENGINE_FAILED", "offline OCR could not recognize this page")
    full_text, words = _parse_tsv(proc.stdout.decode("utf-8", errors="replace"), processed.width, processed.height, thresholds)
    raw_mean = sum(word["confidence"] for word in words) / len(words) if words else 0.0
    calibrate_words(words, engine_class)
    return {"route": route, "recognizer_id": recognizer_id, "calibration_id": calibration_id, "language": config["language"],
            "full_text": full_text, "words": words, "peak_rss": peak_rss, "raw_mean": raw_mean}


def recognize_image_bytes(content: bytes, *, page_index: int = 0) -> dict[str, Any]:
    started = time.perf_counter()
    source_sha = hashlib.sha256(content).hexdigest()
    executable = find_tesseract()
    manifest = runtime_manifest()
    try:
        image = Image.open(io.BytesIO(content))
        image.load()
        processed, provenance = preprocess_image(image.convert("RGB"), executable)
    except OcrError:
        raise
    except Exception as exc:
        raise OcrError("CF_OCR_IMAGE_INVALID", "the image could not be decoded") from exc
    processed_bytes = _png_bytes(processed)
    thresholds = load_thresholds()
    indic_ready = indic_model_ready()
    route, route_reason = choose_route(provenance["osd"], indic_ready)

    def page_warn(attempt: dict[str, Any]) -> bool:
        words = attempt["words"]
        mean = sum(word["confidence"] for word in words) / len(words) if words else 0.0
        return not words or mean < thresholds["page_mean_warn_below"]

    attempts: list[dict[str, Any]] = []
    if route == ROUTE_INDIC:
        attempts.append(_recognize_pass(ROUTE_INDIC, processed, processed_bytes, executable, thresholds))
    else:
        attempts.append(_recognize_pass(ROUTE_ENGLISH, processed, processed_bytes, executable, thresholds))
        if route == ROUTE_AUTO and page_warn(attempts[0]):
            attempts.append(_recognize_pass(ROUTE_INDIC, processed, processed_bytes, executable, thresholds))
    chosen = attempts[0]
    if len(attempts) > 1 and _reads_devanagari(attempts[1]["full_text"]):
        # An unclear page is only given to the Hindi reader when that reader actually finds Hindi text in it. A noisy English page can score a
        # higher engine confidence under the Hindi model while reading worse, so confidence is not the test; the script of the text is.
        chosen = attempts[1]
    words, full_text = chosen["words"], chosen["full_text"]
    critical_fields = validate_critical_fields(full_text, words)
    mean = sum(word["confidence"] for word in words) / len(words) if words else 0.0
    critical_block = any(field["suppress_from_auto"] for field in critical_fields)
    warn = not words or mean < thresholds["page_mean_warn_below"]
    state = "success" if words else "empty"
    text_script = script_detect.describe(full_text)
    diagnostics = {
        **manifest, **provenance, "state": state, "source_kind": "raster",
        "source_sha256": source_sha, "processed_sha256": hashlib.sha256(processed_bytes).hexdigest(),
        "duration_ms": round((time.perf_counter() - started) * 1000),
        "peak_engine_rss_bytes": max(a["peak_rss"] for a in attempts),
        "calibration_id": chosen["calibration_id"], "critical_field_count": len(critical_fields),
        "route": chosen["route"], "route_reason": route_reason, "language": chosen["language"],
        "attempts": [{"route": a["route"], "raw_mean_confidence": round(a["raw_mean"], 4), "word_count": len(a["words"])} for a in attempts],
        "text_script": text_script["script"],
        "line_scripts": [{"line": r["line"], "script": r["script"]} for r in text_script["regions"]],
        "requires_review": warn or critical_block, "automatic_extraction_allowed": bool(words) and not warn and not critical_block,
    }
    if chosen["route"] == ROUTE_INDIC:
        diagnostics["indic_model_sha256"] = (load_indic_config() or {})["model"]["sha256"]
    return {
        "page_index": page_index, "recognizer_id": chosen["recognizer_id"], "full_text": full_text,
        "mean_confidence": mean, "page_quality_warn": warn, "words": words, "critical_fields": critical_fields,
        "notes": json.dumps(diagnostics, sort_keys=True, separators=(",", ":")),
    }


def processed_preview_bytes(content: bytes) -> bytes:
    """Recreate the deterministic OCR coordinate surface for visual inspection."""
    try:
        image = Image.open(io.BytesIO(content))
        image.load()
        processed, _ = preprocess_image(image.convert("RGB"), find_tesseract())
        return _png_bytes(processed)
    except Exception as exc:
        raise OcrError("CF_OCR_IMAGE_INVALID", "the image preview could not be prepared") from exc


def _bbox_for_native_word(start: int, end: int, spans: list[dict[str, Any]]) -> dict[str, Any] | None:
    for span in spans:
        if start >= int(span.get("char_start", -1)) and end <= int(span.get("char_end", -1)):
            return span.get("bbox")
    return None


def recognize_native_page(page: dict[str, Any]) -> dict[str, Any]:
    text = str(page.get("native_text") or page.get("text") or "")
    spans = page.get("text_spans") or []
    words = []
    for index, match in enumerate(WORD_RE.finditer(text)):
        words.append({
            "word_index": index, "text": match.group(0), "confidence": 1.0,
            "raw_engine_confidence": None, "calibrated_confidence": None,
            "confidence_state": "missing",
            "char_start": match.start(), "char_end": match.end(), "low_confidence": False,
            "force_review": False, "suppress_from_auto": False,
            "bbox": _bbox_for_native_word(match.start(), match.end(), spans),
        })
    state = "success" if words else "empty"
    notes = json.dumps({
        "state": state, "source_kind": "native", "engine_confidence_present": False,
        "requires_review": not words, "automatic_extraction_allowed": bool(words),
        "source_sha256": page.get("source_sha256"), "processed_sha256": None,
        "text_script": script_detect.script_of_text(text),
    }, sort_keys=True, separators=(",", ":"))
    critical_fields = validate_critical_fields(text, words, exact_text=True)
    return {
        "page_index": int(page.get("page_index") or 0), "recognizer_id": "caseforge.pdfium.native-text.v1",
        "full_text": text, "mean_confidence": 1.0 if words else 0.0,
        # A native page is not "poor" because it contains a date. Found by running the real app: every native page with a date, an amount or a
        # section number was blocked from the case graph as if it were bad OCR, and the Digest chronology came back empty. A field that fails its own
        # check is still held (see validate_critical_fields); the page is only warned when it has no text at all.
        "page_quality_warn": not words, "words": words,
        "critical_fields": critical_fields, "notes": notes,
    }


def recognize_pages(pages: list[dict[str, Any]], *, thresholds: dict[str, float] | None = None) -> list[dict[str, Any]]:
    """Compatibility RPC: OCR pages containing render/image bytes; preserve native pages."""
    del thresholds
    out = []
    for page in pages:
        encoded = ((page.get("ocr") or {}).get("render_base64") or page.get("image_base64"))
        if encoded:
            out.append(recognize_image_bytes(base64.b64decode(encoded), page_index=int(page.get("page_index") or 0)))
        else:
            out.append(recognize_native_page(page))
    return out


def recognize_document(*, content_base64: str | None = None, text: str | None = None, media_type: str | None = None, filename: str | None = None, cancel_check: Callable[[int], bool] | None = None) -> list[dict[str, Any]]:
    if content_base64:
        try:
            content = base64.b64decode(content_base64, validate=True)
        except Exception as exc:
            raise OcrError("CF_OCR_INPUT_INVALID", "document content is not valid base64") from exc
        kind = (media_type or "").lower()
        suffix = Path(filename or "").suffix.lower()
        if kind == "application/pdf" or suffix == ".pdf" or content.startswith(b"%PDF"):
            parsed = parse_pdf_bytes(content, doc_id=filename or "inline", render_ocr_pages=True, cancel_check=cancel_check)
            layers = []
            for page in parsed["pages"]:
                if cancel_check and cancel_check(int(page["page_index"])):
                    break
                encoded = (page.get("ocr") or {}).get("render_base64")
                layers.append(recognize_image_bytes(base64.b64decode(encoded), page_index=int(page["page_index"])) if encoded else recognize_native_page(page))
            return layers
        if kind.startswith("image/") or suffix in {".png", ".jpg", ".jpeg", ".tif", ".tiff", ".bmp"}:
            return [recognize_image_bytes(content, page_index=0)]
    pages = pages_from_document(content_base64=content_base64, text=text, media_type=media_type, filename=filename)
    return [recognize_native_page(page) for page in pages]
