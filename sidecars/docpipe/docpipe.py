#!/usr/bin/env python3
"""CaseForge docpipe sidecar — triage (B-S1-05) + recognition (B-S1-06).

JSON-RPC 2.0 over stdio (one request/response per line).
Methods: docpipe.ping, docpipe.version, docpipe.triage_pages, docpipe.recognize_pages
"""

from __future__ import annotations

import json
import base64
import sys
from typing import Any

import translate
from pdf_parser import PARSER_ID, PdfParseError, parse_pdf_bytes
from confidence import INDIC_CALIBRATION_ID
from recognize import INDIC_RECOGNIZER_ID, OcrError, RECOGNIZER_ID, indic_model_ready, load_thresholds, processed_preview_bytes, recognize_document, recognize_pages, runtime_manifest
from skeleton import EXTRACTOR_ID, assert_every_extracted_has_source, extract_from_pages
from triage import (
    CLASSIFIER_ID,
    TRIAGE_CLASSES,
    pages_from_document,
    triage_pages,
)

BATCH = "B-S3-06"
VERSION = "1.2.0"


def ok(id_: Any, result: Any) -> dict[str, Any]:
    return {"jsonrpc": "2.0", "result": result, "id": id_}


def err(id_: Any, code: int, message: str, data: Any = None) -> dict[str, Any]:
    e: dict[str, Any] = {"code": code, "message": message}
    if data is not None:
        e["data"] = data
    return {"jsonrpc": "2.0", "error": e, "id": id_}


def handle(req: dict[str, Any]) -> dict[str, Any]:
    if req.get("jsonrpc") != "2.0":
        return err(req.get("id"), -32600, "Invalid Request: jsonrpc must be 2.0")
    method = req.get("method")
    params = req.get("params") or {}
    if isinstance(params, list):
        params = params[0] if params else {}
    rid = req.get("id")

    if method == "docpipe.ping":
        return ok(
            rid,
            {"ok": True, "pong": "pong", "classifier_id": CLASSIFIER_ID, "batch": BATCH},
        )

    if method == "docpipe.version":
        thr = load_thresholds()
        return ok(
            rid,
            {
                "product": "CaseForge",
                "sidecar": "docpipe",
                "version": VERSION,
                "batch": BATCH,
                "classifier_id": CLASSIFIER_ID,
                "recognizer_id": RECOGNIZER_ID,
                "extractor_id": EXTRACTOR_ID,
                "triage_classes": list(TRIAGE_CLASSES),
                "scope": "triage_recognition_skeleton",
                "provisional": False,
                "pdf_parser_id": PARSER_ID,
                "confidence_thresholds": thr,
                "ocr_runtime": runtime_manifest(),
                "calibration_id": "caseforge.tesseract-eng-fast.synthetic-isotonic.v1",
                "indic_ocr": {
                    "recognizer_id": INDIC_RECOGNIZER_ID,
                    "ready": indic_model_ready(),
                    "calibration_id": INDIC_CALIBRATION_ID,
                    "scripts": ["DEVANAGARI", "LATIN", "MIXED"],
                },
                "critical_field_validator_id": "caseforge.critical-fields.deterministic.v1",
                "note": "B-S3-06: calibrated trusted text into normalized graph v2 with complete provenance, ambiguity-aware dates, semantic contradictions, and v1 read projections.",
            },
        )

    if method == "docpipe.parse_pdf":
        encoded = params.get("content_base64")
        if not isinstance(encoded, str) or not encoded:
            return err(rid, -32602, "Invalid params: content_base64 required", {"code":"CF_PDF_INPUT_REQUIRED","state":"unsupported"})
        try:
            content = base64.b64decode(encoded, validate=True)
            result = parse_pdf_bytes(
                content,
                doc_id=str(params.get("doc_id") or params.get("filename") or "inline"),
                password=params.get("password"),
                render_ocr_pages=bool(params.get("render_ocr_pages", False)),
                resume_from_page=int(params.get("resume_from_page") or 0),
                max_pages_per_call=params.get("max_pages_per_call"),
            )
            return ok(rid, result)
        except (ValueError, TypeError):
            return err(rid, -32602, "Invalid base64 or paging parameters", {"code":"CF_PDF_INPUT_INVALID","state":"unsupported"})
        except PdfParseError as exc:
            return err(rid, -32030, exc.safe_message, {"code":exc.code,"state":exc.state})

    if method == "docpipe.triage_pages":
        pages = params.get("pages")
        if pages is None and (
            params.get("content_base64") is not None
            or params.get("text") is not None
            or params.get("filename") is not None
        ):
            try:
                pages = pages_from_document(
                    content_base64=params.get("content_base64"),
                    text=params.get("text"),
                    media_type=params.get("media_type"),
                    filename=params.get("filename"),
                )
            except PdfParseError as exc:
                return err(rid, -32030, exc.safe_message, {"code":exc.code,"state":exc.state})
        if not isinstance(pages, list) or not pages:
            return err(
                rid,
                -32602,
                "Invalid params: pages[] or document content required",
                {"code": "CF_DOCPIPE_TRIAGE_FAILED"},
            )
        results = triage_pages(pages)
        return ok(
            rid,
            {
                "pages": results,
                "classifier_id": CLASSIFIER_ID,
                "page_count": len(results),
            },
        )

    if method == "docpipe.recognize_pages":
        pages = params.get("pages")
        if pages is None and (
            params.get("content_base64") is not None
            or params.get("text") is not None
            or params.get("filename") is not None
        ):
            try:
                pages = pages_from_document(
                    content_base64=params.get("content_base64"),
                    text=params.get("text"),
                    media_type=params.get("media_type"),
                    filename=params.get("filename"),
                )
            except PdfParseError as exc:
                return err(rid, -32030, exc.safe_message, {"code":exc.code,"state":exc.state})
        if not isinstance(pages, list) or not pages:
            return err(
                rid,
                -32602,
                "Invalid params: pages[] or document content required",
                {"code": "CF_DOCPIPE_RECOGNIZE_FAILED"},
            )
        results = recognize_pages(pages)
        return ok(
            rid,
            {
                "pages": results,
                "recognizer_id": RECOGNIZER_ID,
                "page_count": len(results),
                "confidence_thresholds": load_thresholds(),
            },
        )

    if method == "docpipe.recognize_document":
        try:
            results = recognize_document(
                content_base64=params.get("content_base64"),
                text=params.get("text"),
                media_type=params.get("media_type"),
                filename=params.get("filename"),
            )
        except OcrError as exc:
            return err(rid, -32040, exc.safe_message, {"code": exc.code, "state": "failed"})
        except PdfParseError as exc:
            return err(rid, -32030, exc.safe_message, {"code": exc.code, "state": exc.state})
        if not results:
            return err(rid, -32040, "recognition returned no pages", {"code": "CF_OCR_EMPTY_DOCUMENT", "state": "empty"})
        return ok(rid, {
            "pages": results,
            "recognizer_id": RECOGNIZER_ID,
            "page_count": len(results),
            "confidence_thresholds": load_thresholds(),
        })

    if method == "docpipe.render_page":
        encoded = params.get("content_base64")
        page_index = int(params.get("page_index") or 0)
        if not isinstance(encoded, str) or not encoded:
            return err(rid, -32602, "Invalid params: content_base64 required", {"code": "CF_OCR_INPUT_REQUIRED"})
        try:
            content = base64.b64decode(encoded, validate=True)
            media_type = str(params.get("media_type") or "").lower()
            filename = str(params.get("filename") or "")
            if media_type.startswith("image/") or filename.lower().endswith((".png", ".jpg", ".jpeg", ".tif", ".tiff", ".bmp")):
                if page_index != 0:
                    return err(rid, -32602, "Image documents contain one page", {"code": "CF_OCR_PAGE_RANGE"})
                preview = processed_preview_bytes(content)
                return ok(rid, {"page_index": 0, "media_type": "image/png", "image_base64": base64.b64encode(preview).decode("ascii")})
            parsed = parse_pdf_bytes(content, doc_id=filename or "inline", render_ocr_pages=True, resume_from_page=page_index, max_pages_per_call=1)
            if not parsed["pages"]:
                return err(rid, -32602, "Page is outside the document", {"code": "CF_OCR_PAGE_RANGE"})
            page = parsed["pages"][0]
            render = page.get("ocr") or {}
            if not render.get("render_base64"):
                return err(rid, -32041, "This native page has no OCR raster", {"code": "CF_OCR_PREVIEW_NOT_REQUIRED"})
            preview = processed_preview_bytes(base64.b64decode(render["render_base64"]))
            return ok(rid, {"page_index": page_index, "media_type": "image/png", "image_base64": base64.b64encode(preview).decode("ascii"), "render_size": render.get("render_size")})
        except (ValueError, TypeError):
            return err(rid, -32602, "Invalid page render request", {"code": "CF_OCR_INPUT_INVALID"})
        except PdfParseError as exc:
            return err(rid, -32030, exc.safe_message, {"code": exc.code, "state": exc.state})

    if method == "docpipe.extract_skeleton":
        case_id = params.get("case_id") or "unknown"
        jurisdiction = params.get("jurisdiction") or "Other"
        pages_in = params.get("pages")
        if not isinstance(pages_in, list) or not pages_in:
            # Accept single text document
            if params.get("text") is not None:
                pages_in = [
                    {
                        "doc_id": params.get("doc_id") or "doc_inline",
                        "page_index": 0,
                        "text": params.get("text"),
                        "mean_confidence": 0.8,
                    }
                ]
            else:
                return err(
                    rid,
                    -32602,
                    "Invalid params: pages[] or text required",
                    {"code": "CF_DOCPIPE_SKELETON_FAILED"},
                )
        graph = extract_from_pages(
            case_id=str(case_id),
            jurisdiction=str(jurisdiction),
            pages=pages_in,
            classifier_id=CLASSIFIER_ID,
            recognizer_id=RECOGNIZER_ID,
        )
        defects = assert_every_extracted_has_source(graph)
        if defects:
            return err(
                rid,
                -32000,
                "case_graph source pointer defects",
                {"code": "CF_DOCPIPE_SKELETON_FAILED", "defects": defects},
            )
        return ok(
            rid,
            {
                "case_graph": graph,
                "extractor_id": EXTRACTOR_ID,
                "chronology_count": len(graph.get("chronology") or []),
            },
        )

    if method in ("docpipe.translate_info", "docpipe.translate_prepare", "docpipe.translate_finish"):
        try:
            if method == "docpipe.translate_info":
                return ok(rid, translate.info())
            if method == "docpipe.translate_prepare":
                text = params.get("text")
                if not isinstance(text, str):
                    return err(rid, -32602, "Invalid params: text required", {"code": "CF_TRANSLATE_BAD_PARAMS"})
                return ok(rid, {"segments": translate.prepare(doc_id=str(params.get("doc_id") or ""), page_index=int(params.get("page_index") or 0), text=text, words=params.get("words")), **translate.info()})
            prepared = params.get("prepared")
            if not isinstance(prepared, dict) or not isinstance(params.get("model_output"), str):
                return err(rid, -32602, "Invalid params: prepared and model_output required", {"code": "CF_TRANSLATE_BAD_PARAMS"})
            return ok(rid, translate.finish(doc_id=str(params.get("doc_id") or ""), page_index=int(params.get("page_index") or 0), prepared=prepared, model_output=params["model_output"], calibration_id=params.get("calibration_id")))
        except RuntimeError as exc:
            return err(rid, -32050, str(exc), {"code": str(exc).split(":", 1)[0]})
    return err(rid, -32601, f"Method not found: {method}")


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="strict")
    if hasattr(sys.stdin, "reconfigure"):
        sys.stdin.reconfigure(encoding="utf-8", errors="strict")
    if len(sys.argv) >= 3 and sys.argv[1] == "--once":
        try:
            request = json.loads(sys.argv[2])
            response = handle(request)
        except json.JSONDecodeError as exc:
            response = err(None, -32700, f"Parse error: {exc}")
        sys.stdout.write(json.dumps(response, ensure_ascii=False) + "\n")
        sys.stdout.flush()
        return 0
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except json.JSONDecodeError as e:
            sys.stdout.write(json.dumps(err(None, -32700, f"Parse error: {e}")) + "\n")
            sys.stdout.flush()
            continue
        resp = handle(req)
        sys.stdout.write(json.dumps(resp, ensure_ascii=False) + "\n")
        sys.stdout.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
