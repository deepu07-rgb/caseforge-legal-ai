"""Offline PDFium page parser for B-S3-03.

No source or password is written to disk or logs. OCR renders are returned only
when requested and are intended for immediate encrypted persistence/processing.
"""

from __future__ import annotations

import base64
import hashlib
import io
import re
from dataclasses import dataclass
from typing import Any, Callable

import pypdfium2 as pdfium
import pypdfium2.raw as pdfium_c

PARSER_ID = "caseforge.pdfium.page-source.v1"
MAX_PDF_BYTES = 256 * 1024 * 1024
MAX_PAGES = 5000
MAX_RENDER_PIXELS = 40_000_000
OCR_DPI = 300


@dataclass
class PdfParseError(Exception):
    code: str
    state: str
    safe_message: str

    def __str__(self) -> str:
        return f"{self.code}: {self.safe_message}"


def _bbox(left: float, bottom: float, right: float, top: float, width: float, height: float) -> dict[str, float]:
    x = max(0.0, min(1.0, left / width))
    y = max(0.0, min(1.0, (height - top) / height))
    w = max(0.0, min(1.0 - x, (right - left) / width))
    h = max(0.0, min(1.0 - y, (top - bottom) / height))
    return {"x": round(x, 7), "y": round(y, 7), "w": round(w, 7), "h": round(h, 7)}


def _union(boxes: list[tuple[float, float, float, float]], width: float, height: float) -> dict[str, float]:
    return _bbox(min(b[0] for b in boxes), min(b[1] for b in boxes), max(b[2] for b in boxes), max(b[3] for b in boxes), width, height)


def _role(text: str, box: dict[str, float]) -> str:
    value = text.strip()
    lower = value.lower()
    if re.fullmatch(r"(?:page\s*)?\d+(?:\s+of\s+\d+)?", lower) and (box["y"] < 0.12 or box["y"] > 0.82):
        return "page_number"
    if any(word in lower for word in ("signature", "signed by", "date signed")):
        return "signature"
    if any(word in lower for word in ("stamp", "seal of", "filed:")):
        return "stamp"
    if box["y"] > 0.82 and len(value) < 240:
        return "footnote"
    letters = [c for c in value if c.isalpha()]
    if letters and len(value) < 140 and sum(c.isupper() for c in letters) / len(letters) > 0.72:
        return "heading"
    return "paragraph" if value else "unknown"


def _text_spans(text_page: Any, full_text: str, width: float, height: float) -> list[dict[str, Any]]:
    spans: list[dict[str, Any]] = []
    offset = 0
    for chunk in full_text.splitlines(keepends=True):
        visible = chunk.rstrip("\r\n")
        end = offset + len(visible)
        boxes: list[tuple[float, float, float, float]] = []
        for idx in range(offset, end):
            if full_text[idx].isspace():
                continue
            try:
                boxes.append(tuple(float(v) for v in text_page.get_charbox(idx, loose=True)))
            except Exception:
                continue
        if visible.strip() and boxes:
            box = _union(boxes, width, height)
            spans.append({"char_start": offset, "char_end": end, "text": visible, "bbox": box, "role": _role(visible, box)})
        offset += len(chunk)
    if not spans and full_text.strip():
        boxes = []
        for idx, char in enumerate(full_text):
            if char.isspace():
                continue
            try:
                boxes.append(tuple(float(v) for v in text_page.get_charbox(idx, loose=True)))
            except Exception:
                continue
        if boxes:
            box = _union(boxes, width, height)
            spans.append({"char_start": 0, "char_end": len(full_text), "text": full_text, "bbox": box, "role": _role(full_text, box)})
    return spans


def _render(page: Any, width: float, height: float) -> tuple[str, int, int]:
    scale = OCR_DPI / 72.0
    pixels = int(width * scale) * int(height * scale)
    if pixels > MAX_RENDER_PIXELS:
        scale *= (MAX_RENDER_PIXELS / pixels) ** 0.5
    image = page.render(scale=scale, rev_byteorder=True).to_pil()
    output = io.BytesIO()
    image.save(output, format="PNG", optimize=True)
    return base64.b64encode(output.getvalue()).decode("ascii"), image.width, image.height


def _capabilities(spans: list[dict[str, Any]], vector_count: int, image_count: int) -> dict[str, dict[str, str]]:
    roles = {span["role"] for span in spans}
    return {
        "tables": {"status": "heuristic" if vector_count >= 6 else "not_present", "note": "Vector-rule heuristic retained; cell semantics require table extraction." if vector_count >= 6 else "No table-rule signal."},
        "footnotes": {"status": "retained" if "footnote" in roles else "not_present", "note": "Bottom-region text retained with coordinates."},
        "headings": {"status": "retained" if "heading" in roles else "not_present", "note": "Uppercase/short-line heuristic; source text and coordinates retained."},
        "signatures": {"status": "heuristic" if "signature" in roles else "not_present", "note": "Signature labels retained; handwriting interpretation is unsupported."},
        "stamps": {"status": "heuristic" if "stamp" in roles or image_count else "not_present", "note": "Raster content is retained/rendered; stamp semantics require OCR/vision."},
        "page_numbers": {"status": "retained" if "page_number" in roles else "not_present", "note": "Header/footer numeric text retained with coordinates."},
    }


def parse_pdf_bytes(
    content: bytes,
    *,
    doc_id: str = "inline",
    password: str | None = None,
    render_ocr_pages: bool = False,
    resume_from_page: int = 0,
    max_pages_per_call: int | None = None,
    cancel_check: Callable[[int], bool] | None = None,
) -> dict[str, Any]:
    if len(content) > MAX_PDF_BYTES:
        raise PdfParseError("CF_PDF_OVERSIZED", "oversized", f"PDF exceeds {MAX_PDF_BYTES} byte parser limit")
    if b"%PDF-" not in content[:1024]:
        raise PdfParseError("CF_PDF_MALFORMED", "malformed", "PDF header is missing or displaced beyond the supported prefix")
    try:
        document = pdfium.PdfDocument(content, password=password)
    except Exception as exc:
        message = str(exc).lower()
        if "password" in message or "security" in message:
            raise PdfParseError("CF_PDF_PASSWORD_REQUIRED", "encrypted", "PDF is password protected or the supplied password is incorrect") from None
        raise PdfParseError("CF_PDF_MALFORMED", "malformed", "PDFium could not safely open this document") from None

    page_count = len(document)
    if page_count > MAX_PAGES:
        document.close()
        raise PdfParseError("CF_PDF_OVERSIZED", "oversized", f"PDF has {page_count} pages; limit is {MAX_PAGES}")
    start = max(0, int(resume_from_page))
    stop = page_count if max_pages_per_call is None else min(page_count, start + max(0, int(max_pages_per_call)))
    source_hash = hashlib.sha256(content).hexdigest()
    pages: list[dict[str, Any]] = []
    state = "ready"
    next_page: int | None = None
    try:
        for page_index in range(start, stop):
            if cancel_check and cancel_check(page_index):
                state, next_page = "cancelled", page_index
                break
            page = document[page_index]
            width, height = (float(v) for v in page.get_size())
            rotation_raw = int(page.get_rotation())
            rotation = rotation_raw * 90 if rotation_raw in (0, 1, 2, 3) else rotation_raw % 360
            text_page = page.get_textpage()
            native_text = text_page.get_text_range(errors="replace")
            spans = _text_spans(text_page, native_text, width, height)
            blocks: list[dict[str, Any]] = []
            image_boxes: list[dict[str, float]] = []
            vector_count = 0
            for obj in page.get_objects(max_depth=15):
                try:
                    bounds = tuple(float(v) for v in obj.get_bounds())
                    box = _bbox(*bounds, width, height)
                except Exception:
                    continue
                if obj.type == pdfium_c.FPDF_PAGEOBJ_IMAGE:
                    image_boxes.append(box)
                    blocks.append({"block_index": len(blocks), "kind": "image", "bbox": box, "text": None, "char_start": None, "char_end": None})
                elif obj.type == pdfium_c.FPDF_PAGEOBJ_PATH:
                    vector_count += 1
            for span in spans:
                blocks.append({"block_index": len(blocks), "kind": "text", "bbox": span["bbox"], "text": span["text"], "char_start": span["char_start"], "char_end": span["char_end"]})
            text_chars = len(native_text.strip())
            image_coverage = min(1.0, sum(box["w"] * box["h"] for box in image_boxes))
            if text_chars and image_coverage >= 0.15:
                source_kind = "mixed"
            elif text_chars:
                source_kind = "native"
            elif image_coverage >= 0.15:
                source_kind = "scanned"
            else:
                source_kind = "empty"
            ocr_required = source_kind in {"scanned", "mixed"}
            render_base64 = render_sha256 = render_type = None
            rendered = False
            render_size = None
            if ocr_required and render_ocr_pages:
                render_base64, render_width, render_height = _render(page, width, height)
                render_sha256 = hashlib.sha256(base64.b64decode(render_base64)).hexdigest()
                render_type, rendered = "image/png", True
                render_size = {"width": render_width, "height": render_height, "dpi": OCR_DPI}
            pages.append({
                "schema_version": "1.0.0", "doc_id": doc_id, "page_index": page_index,
                "source_kind": source_kind, "state": "ocr_required" if ocr_required else "ready",
                "width_points": width, "height_points": height, "rotation_degrees": rotation,
                "native_text": native_text, "text_spans": spans, "layout_blocks": blocks,
                "ocr": {"required": ocr_required, "reason": f"{source_kind} page requires raster recognition" if ocr_required else None,
                        "rendered": rendered, "render_media_type": render_type, "render_sha256": render_sha256,
                        "render_base64": render_base64, "render_size": render_size},
                "capabilities": _capabilities(spans, vector_count, len(image_boxes)),
                "parser_id": PARSER_ID, "source_sha256": source_hash,
            })
            text_page.close()
            page.close()
        if state == "ready" and stop < page_count:
            state, next_page = "cancelled", stop
    finally:
        document.close()
    return {"schema_version":"1.0.0","parser_id":PARSER_ID,"document_state":state,"source_sha256":source_hash,
            "page_count":page_count,"pages":pages,"checkpoint":{"next_page":next_page,"complete":next_page is None}}
