#!/usr/bin/env python3
from __future__ import annotations

import base64
import io
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

import jsonschema
from pypdf import PdfReader, PdfWriter

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "sidecars" / "docpipe"))
import pdf_parser
from pdf_parser import PdfParseError, parse_pdf_bytes

ASSETS = ROOT / "golden" / "G-DOCS-REAL" / "fixtures" / "uscourts_pro_se_1" / "assets"
SCHEMA = json.loads((ROOT / "contracts" / "page_source.schema.json").read_text(encoding="utf-8"))


def require(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)


def parse(name: str, **kwargs):
    return parse_pdf_bytes((ASSETS / name).read_bytes(), doc_id=name, **kwargs)


def main() -> int:
    native = parse("complaint_for_a_civil_case.pdf")
    scanned = parse("complaint_scanned_derivative.pdf")
    mixed = parse("complaint_mixed_derivative.pdf")
    require(native["page_count"] == scanned["page_count"] == mixed["page_count"] == 5, "real page count/order failed")
    require([p["page_index"] for p in native["pages"]] == list(range(5)), "page order failed")
    require({p["source_kind"] for p in native["pages"]} == {"native"}, "native classification failed")
    require({p["source_kind"] for p in scanned["pages"]} == {"scanned"}, "scan classification failed")
    require([p["source_kind"] for p in mixed["pages"]] == ["native", "native", "scanned", "native", "native"], "mixed routing failed")
    require(len({p["native_text"] for p in native["pages"]}) == 5, "cross-page text was duplicated")
    require(all(not p["native_text"] for p in scanned["pages"]), "image-only pages invented native text")
    require(all(p["ocr"]["required"] for p in scanned["pages"]), "scanned pages did not route to OCR")
    require(not any(p["ocr"]["required"] for p in native["pages"]), "native pages were unnecessarily routed to OCR")
    for page in native["pages"] + scanned["pages"] + mixed["pages"]:
        jsonschema.validate(page, SCHEMA)
        require(page["width_points"] > 0 and page["height_points"] > 0, "page dimensions missing")
        for span in page["text_spans"]:
            require(page["native_text"][span["char_start"]:span["char_end"]] == span["text"], "text span is on wrong page/offset")
            require(all(0 <= span["bbox"][key] <= 1 for key in ("x", "y", "w", "h")), "span bbox out of range")

    rendered = parse("complaint_scanned_derivative.pdf", render_ocr_pages=True, max_pages_per_call=1)
    require(rendered["document_state"] == "cancelled" and rendered["checkpoint"]["next_page"] == 1, "checkpoint failed")
    png = base64.b64decode(rendered["pages"][0]["ocr"]["render_base64"])
    require(png.startswith(b"\x89PNG") and rendered["pages"][0]["ocr"]["rendered"], "OCR render failed")
    resumed = parse_pdf_bytes((ASSETS / "complaint_scanned_derivative.pdf").read_bytes(), doc_id="resume", resume_from_page=1)
    require([p["page_index"] for p in resumed["pages"]] == [1, 2, 3, 4], "resume page boundary failed")
    cancelled = parse_pdf_bytes((ASSETS / "complaint_for_a_civil_case.pdf").read_bytes(), cancel_check=lambda page: page >= 2)
    require(cancelled["document_state"] == "cancelled" and cancelled["checkpoint"]["next_page"] == 2, "soft cancellation failed")

    try:
        parse_pdf_bytes(b"not a PDF")
        raise AssertionError("malformed PDF was accepted")
    except PdfParseError as exc:
        require(exc.state == "malformed", "malformed state not explicit")

    source = PdfReader(io.BytesIO((ASSETS / "complaint_for_a_civil_case.pdf").read_bytes()))
    writer = PdfWriter(); writer.add_page(source.pages[0]); writer.encrypt("secret-password")
    encrypted = io.BytesIO(); writer.write(encrypted)
    try:
        parse_pdf_bytes(encrypted.getvalue())
        raise AssertionError("encrypted PDF was accepted without password")
    except PdfParseError as exc:
        require(exc.state == "encrypted" and "secret-password" not in str(exc), "encrypted/password safety failed")
    require(parse_pdf_bytes(encrypted.getvalue(), password="secret-password")["page_count"] == 1, "password flow failed")

    prior = pdf_parser.MAX_PDF_BYTES
    try:
        pdf_parser.MAX_PDF_BYTES = 10
        try:
            parse_pdf_bytes((ASSETS / "complaint_for_a_civil_case.pdf").read_bytes())
            raise AssertionError("oversized PDF was accepted")
        except PdfParseError as exc:
            require(exc.state == "oversized", "oversized state not explicit")
    finally:
        pdf_parser.MAX_PDF_BYTES = prior

    rpc = {"jsonrpc":"2.0","id":1,"method":"docpipe.parse_pdf","params":{"content_base64":base64.b64encode((ASSETS / "complaint_mixed_derivative.pdf").read_bytes()).decode(),"doc_id":"rpc","max_pages_per_call":1}}
    output = subprocess.check_output([sys.executable, str(ROOT / "sidecars" / "docpipe" / "docpipe.py")], input=json.dumps(rpc) + "\n", text=True, encoding="utf-8")
    require(json.loads(output)["result"]["pages"][0]["page_index"] == 0, "JSON-RPC parser failed")

    rust = (ROOT / "src-tauri" / "src" / "docpipe" / "triage.rs").read_text(encoding="utf-8")
    require("/Type /Page" not in rust and "joined.clone()" not in rust, "Rust byte heuristic remains")
    require((ROOT / "src-tauri" / "migrations" / "case_011_page_source.sql").is_file(), "page source migration missing")
    ui = (ROOT / "src" / "App.tsx").read_text(encoding="utf-8")
    require("PDF quality" in ui and "ocr_required" in ui, "document-quality UI missing")
    report = {
        "schema_version": "1.0.0", "batch": "B-S3-03",
        "generated_at": datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "implementation_state": "PASS", "real_world_golden_state": "BLOCKED",
        "fixture_status": "READY_FOR_REVIEW",
        "parser": {"id": pdf_parser.PARSER_ID, "pypdfium2": "5.3.0", "pdfium": "145.0.7616.0"},
        "fixtures": {
            "native": {"page_count": native["page_count"], "kinds": [p["source_kind"] for p in native["pages"]]},
            "scanned": {"page_count": scanned["page_count"], "kinds": [p["source_kind"] for p in scanned["pages"]]},
            "mixed": {"page_count": mixed["page_count"], "kinds": [p["source_kind"] for p in mixed["pages"]]},
        },
        "acceptance": {
            "AC-S3-3.1": "ENGINEERING_PASS_GOLDEN_BLOCKED", "AC-S3-3.2": "PASS",
            "AC-S3-3.3": "PASS", "AC-S3-3.4": "PASS", "AC-S3-3.5": "PASS",
            "AC-S3-3.6": "PARTIAL_BLOCKED_REAL_COMPLEX_LAYOUT_FIXTURE"
        },
        "notes": ["No synthetic or READY_FOR_REVIEW fixture is reported as GOLDEN.", "Full OCR recognition is B-S3-04 scope."]
    }
    report_path = ROOT / "eval" / "reports" / "b_s3_03_pdf_pipeline.json"
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
    print("PASS B-S3-03 PDFium page model, layout, OCR routing, failures, checkpoint/cancel, storage, and UI")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
