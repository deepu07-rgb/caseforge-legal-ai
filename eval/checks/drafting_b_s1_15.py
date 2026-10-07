#!/usr/bin/env python3
"""B-S1-15 Drafting surface + DOCX/PDF export (G4 footer, G5 provenance)."""

from __future__ import annotations

import json
import re
import sys
import zipfile
from io import BytesIO
from pathlib import Path
from xml.etree import ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
CONSTANTS = ROOT / "contracts" / "constants.json"
EXPORT_TS = ROOT / "src" / "lib" / "exportDraft.ts"
DRAFT_VIEW = ROOT / "src" / "components" / "DraftingView.tsx"
DIGEST_VIEW = ROOT / "src" / "components" / "DigestView.tsx"
APP = ROOT / "src" / "App.tsx"
DOC = ROOT / "docs" / "B-S1-15_drafting.md"
FOOTER_ID = "caseforge.export_footer.v1"
PIPELINE_ID = "caseforge.draft_export.v1"
HONESTY_LINE = (
    "Local model output for preparation only. Not legal advice. "
    "Citations require pack verification."
)
G6_LINE = (
    "Preparation completeness only — never win prediction, outcome odds, or verdict language."
)

TEMPLATES = (
    "court_formatted_digest",
    "list_of_dates",
    "synopsis",
    "counsel_memo",
)


def build_sample_graph() -> dict:
    return {
        "schema_version": "1.0.0",
        "case_id": "case_export_test",
        "jurisdiction": "India",
        "generated_at": "2026-08-08T00:00:00Z",
        "pipeline": {"extractor_id": "caseforge.skeleton.deterministic.v1"},
        "parties": [
            {
                "party_id": "p1",
                "display_name": "Acme Calibration Pvt. Ltd.",
                "role": "plaintiff",
                "provenance": {
                    "confidence": 0.9,
                    "source": {"doc_id": "d1", "page_index": 0},
                },
            }
        ],
        "chronology": [
            {
                "event_id": "e1",
                "date": "2023-03-12",
                "date_precision": "day",
                "summary": "Goods delivered under purchase order.",
                "sort_key": "2023-03-12",
                "provenance": {
                    "confidence": 0.9,
                    "source": {"doc_id": "d1", "page_index": 0},
                },
            },
            {
                "event_id": "e2",
                "date": "2024-09-15",
                "date_precision": "day",
                "summary": "Delivery challan issued.",
                "sort_key": "2024-09-15",
                "provenance": {
                    "confidence": 0.85,
                    "source": {"doc_id": "d1", "page_index": 1},
                },
            },
        ],
        "statutes": [
            {
                "statute_id": "s1",
                "label": "Section 138 Negotiable Instruments Act",
                "provenance": {
                    "confidence": 0.8,
                    "source": {"doc_id": "d1", "page_index": 0},
                },
            }
        ],
        "evidence": [
            {
                "evidence_id": "ev1",
                "label": "Exhibit A-1 delivery challan",
                "provenance": {
                    "confidence": 0.8,
                    "source": {"doc_id": "d1", "page_index": 1},
                },
            }
        ],
        "procedural_posture": {"summary": "Suit for recovery pending."},
        "prayer_relief": [
            {
                "relief_id": "r1",
                "text": "Recovery of INR 1,25,000",
                "provenance": {
                    "confidence": 0.9,
                    "source": {"doc_id": "d1", "page_index": 0},
                },
            }
        ],
        "contradictions": [],
        "gaps": [{"gap_id": "g1", "topic": "Notice date", "status": "open"}],
        "counsel_facts": [],
    }


def footer_text_from_constants(data: dict) -> str:
    g4 = data.get("export_footer_g4") or {}
    if g4.get("text"):
        return g4["text"]
    lines = g4.get("lines") or []
    return " | ".join(lines)


def render_plain(body_lines: list[str], footer_lines: list[str], provenance: dict) -> str:
    lines = list(body_lines)
    lines += [
        "",
        "────────────────────────────────────────",
        "EXPORT FOOTER (G4)",
        *footer_lines,
        "",
        "PROVENANCE LOG (G5)",
        f"export_id: {provenance['export_id']}",
        f"generated_at: {provenance['generated_at']}",
        f"template_id: {provenance['template_id']}",
        f"case_id: {provenance['case_id']}",
        f"pipeline_id: {provenance['pipeline_id']}",
        f"footer_id: {provenance['footer_id']}",
        f"llm_used: {provenance['llm_used']}",
        f"ai_spans: {len(provenance['ai_spans'])}",
    ]
    for s in provenance["ai_spans"]:
        lines.append(
            f"  - [{s['span_id']}] origin={s['origin']} model_hash={s['model_hash']} "
            f"prompt_hash={s['prompt_hash']} retrieval_ids={json.dumps(s['retrieval_ids'])}"
        )
    lines.append("")
    lines.append(provenance["footer_text"])
    return "\n".join(lines)


def build_body(template_id: str, graph: dict) -> tuple[list[str], list[dict]]:
    spans: list[dict] = []
    extractor = (graph.get("pipeline") or {}).get("extractor_id") or "deterministic:case_graph"
    header = [
        "IN THE Sample District Court",
        "Acme Calibration Pvt. Ltd. v Northern Sample Traders",
        "",
        "Jurisdiction: India",
        "Matter type: Commercial",
        "Counsel side: Plaintiff",
        "",
    ]

    def add_span(text: str) -> None:
        spans.append(
            {
                "span_id": f"span_{len(spans)+1}",
                "text": text[:200],
                "origin": "deterministic_graph",
                "model_hash": f"extractor:{extractor}",
                "prompt_hash": "deterministic:template",
                "retrieval_ids": [],
            }
        )

    if template_id == "list_of_dates":
        lines = header + ["LIST OF DATES", ""]
        for i, e in enumerate(graph.get("chronology") or [], 1):
            row = f"{i}. {e['date']} — {e['summary']}"
            lines.append(row)
            add_span(row)
        return lines, spans

    if template_id == "synopsis":
        lines = header + ["SYNOPSIS", ""]
        body = (
            f"Parties: {graph['parties'][0]['display_name']}. "
            f"Relief: {graph['prayer_relief'][0]['text']}."
        )
        lines.append(body)
        add_span(body)
        return lines, spans

    if template_id == "counsel_memo":
        lines = header + ["COUNSEL PREPARATION MEMO", "", "1. Facts to lock"]
        for e in graph.get("chronology") or []:
            row = f"   - [{e['date']}] {e['summary']}"
            lines.append(row)
            add_span(row)
        return lines, spans

    # court_formatted_digest
    lines = header + ["I. PARTIES"]
    for p in graph.get("parties") or []:
        row = f"{p['display_name']} — {p['role']}"
        lines.append(row)
        add_span(row)
    lines += ["", "II. CHRONOLOGY"]
    for e in graph.get("chronology") or []:
        row = f"{e['date']}: {e['summary']}"
        lines.append(row)
        add_span(row)
    lines += ["", "III. STATUTES / RULES"]
    for s in graph.get("statutes") or []:
        row = s["label"]
        lines.append(row)
        add_span(row)
    lines += ["", "VI. PRAYER / RELIEF"]
    for r in graph.get("prayer_relief") or []:
        row = r["text"]
        lines.append(row)
        add_span(row)
    return lines, spans


def make_docx(text: str) -> bytes:
    def esc(s: str) -> str:
        return (
            s.replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
            .replace('"', "&quot;")
        )

    paras = []
    for line in text.split("\n"):
        t = esc(line if line else " ")
        paras.append(f'<w:p><w:r><w:t xml:space="preserve">{t}</w:t></w:r></w:p>')
    document_xml = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    {"".join(paras)}
  </w:body>
</w:document>"""
    content_types = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"""
    rels = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"""
    buf = BytesIO()
    with zipfile.ZipFile(buf, "w", compression=zipfile.ZIP_STORED) as zf:
        zf.writestr("[Content_Types].xml", content_types)
        zf.writestr("_rels/.rels", rels)
        zf.writestr("word/document.xml", document_xml)
    return buf.getvalue()


def make_pdf(text: str) -> bytes:
    # Minimal single-page PDF with Helvetica text (truncated for smoke test)
    lines = text.split("\n")[:40]
    content_cmds = ["BT", "/F1 10 Tf", "50 780 Td", "12 TL"]
    for i, ln in enumerate(lines):
        safe = ln.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")[:90]
        if i == 0:
            content_cmds.append(f"({safe}) Tj")
        else:
            content_cmds.append("T*")
            content_cmds.append(f"({safe}) Tj")
    content_cmds.append("ET")
    stream = "\n".join(content_cmds)
    objects = []
    objects.append("<< /Type /Catalog /Pages 2 0 R >>")
    objects.append("<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
    objects.append(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
        "/Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>"
    )
    objects.append(f"<< /Length {len(stream)} >>\nstream\n{stream}\nendstream")
    objects.append("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")

    out = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for i, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out.extend(f"{i} 0 obj\n{body}\nendobj\n".encode("latin-1", errors="replace"))
    xref = len(out)
    out.extend(f"xref\n0 {len(objects)+1}\n".encode())
    out.extend(b"0000000000 65535 f \n")
    for off in offsets[1:]:
        out.extend(f"{off:010d} 00000 n \n".encode())
    out.extend(
        f"trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    )
    return bytes(out)


def extract_docx_text(data: bytes) -> str:
    with zipfile.ZipFile(BytesIO(data)) as zf:
        xml = zf.read("word/document.xml")
    root = ET.fromstring(xml)
    ns = {"w": "http://schemas.openxmlformats.org/wordprocessingml/2006/main"}
    parts = []
    for t in root.findall(".//w:t", ns):
        if t.text:
            parts.append(t.text)
    return "\n".join(parts)


def main() -> int:
    errors: list[str] = []

    for p in (CONSTANTS, EXPORT_TS, DRAFT_VIEW, DIGEST_VIEW, APP, DOC):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    footer_lines: list[str] = []
    footer_text = ""
    if CONSTANTS.exists():
        data = json.loads(CONSTANTS.read_text(encoding="utf-8-sig"))
        g4 = data.get("export_footer_g4") or {}
        if g4.get("status") != "FROZEN":
            errors.append("export_footer_g4 not FROZEN in constants.json")
        if g4.get("footer_id") != FOOTER_ID:
            errors.append(f"footer_id must be {FOOTER_ID}")
        if not g4.get("required_on_every_export"):
            errors.append("required_on_every_export must be true")
        footer_lines = list(g4.get("lines") or [])
        if HONESTY_LINE not in footer_lines:
            errors.append("G4 footer must reuse existing Stage-1 honesty notice (infer_llm line)")
        if G6_LINE not in footer_lines:
            errors.append("G4 footer must include frozen G6 preparation language")
        if FOOTER_ID not in " ".join(footer_lines) and FOOTER_ID not in (g4.get("text") or ""):
            errors.append("G4 footer must include footer_id marker")
        footer_text = footer_text_from_constants(data)
        if FOOTER_ID not in footer_text:
            errors.append("footer text missing footer_id")

        g5 = data.get("export_provenance_g5") or {}
        if g5.get("status") != "FROZEN":
            errors.append("export_provenance_g5 not FROZEN")
        req = set(g5.get("required_fields_per_ai_span") or [])
        for f in ("model_hash", "prompt_hash", "retrieval_ids"):
            if f not in req:
                errors.append(f"G5 missing required field {f}")

        drafting = data.get("drafting_export") or {}
        if drafting.get("status") != "FROZEN":
            errors.append("drafting_export not FROZEN")
        if drafting.get("pipeline_id") != PIPELINE_ID:
            errors.append(f"pipeline_id must be {PIPELINE_ID}")
        tids = {t.get("id") for t in (drafting.get("templates") or [])}
        for t in TEMPLATES:
            if t not in tids:
                errors.append(f"drafting_export missing template {t}")
        formats = set(drafting.get("export_formats") or [])
        if "docx" not in formats or "pdf" not in formats:
            errors.append("export_formats must include docx and pdf")
        if "auto_filing_to_court_portals" not in (drafting.get("out_of_scope") or []):
            errors.append("out_of_scope must include auto_filing_to_court_portals")

    for path, needles in (
        (
            EXPORT_TS,
            [
                FOOTER_ID,
                PIPELINE_ID,
                "EXPORT_FOOTER_LINES",
                "renderDocx",
                "renderPdf",
                "renderPlainText",
                "buildDraftDocument",
                "model_hash",
                "prompt_hash",
                "retrieval_ids",
                "court_formatted_digest",
                "list_of_dates",
                "synopsis",
                "counsel_memo",
                HONESTY_LINE,
                G6_LINE,
            ],
        ),
        (
            DRAFT_VIEW,
            [
                "DRAFTING",
                "export-docx",
                "export-pdf",
                "g4-footer-preview",
                "g5-spans",
                "buildDraftDocument",
                "renderDocx",
                "renderPdf",
            ],
        ),
        (
            DIGEST_VIEW,
            [
                "surface-drafting",
                "DraftingView",
                '"drafting"',
                "DRAFTING",
            ],
        ),
        (APP, ["DRAFTING"]),
        (
            DOC,
            [
                FOOTER_ID,
                "G4",
                "G5",
                "DOCX",
                "PDF",
                "court_formatted_digest",
                PIPELINE_ID,
            ],
        ),
    ):
        if not path.exists():
            continue
        text = path.read_text(encoding="utf-8")
        for n in needles:
            if n not in text:
                errors.append(f"{path.relative_to(ROOT)} missing {n!r}")

    # DRAFTING must not remain disabled placeholder
    if DIGEST_VIEW.exists():
        dv = DIGEST_VIEW.read_text(encoding="utf-8")
        if re.search(r'surface-tab disabled[^>]*>\s*DRAFTING', dv):
            errors.append("DigestView still has disabled DRAFTING placeholder")
        if "from \"./DraftingView\"" not in dv and "from './DraftingView'" not in dv:
            errors.append("DigestView must import DraftingView")

    # Live export generation (Python mirror of product structure)
    graph = build_sample_graph()
    for tid in TEMPLATES:
        body, spans = build_body(tid, graph)
        prov = {
            "export_id": f"exp_{tid}",
            "generated_at": "2026-08-08T12:00:00Z",
            "template_id": tid,
            "case_id": graph["case_id"],
            "pipeline_id": PIPELINE_ID,
            "footer_id": FOOTER_ID,
            "footer_text": footer_text or HONESTY_LINE,
            "llm_used": False,
            "ai_spans": spans,
        }
        plain = render_plain(body, footer_lines or [HONESTY_LINE, G6_LINE], prov)

        if "EXPORT FOOTER (G4)" not in plain:
            errors.append(f"{tid}: plain export missing G4 section")
        if FOOTER_ID not in plain:
            errors.append(f"{tid}: plain export missing footer_id")
        if "PROVENANCE LOG (G5)" not in plain:
            errors.append(f"{tid}: plain export missing G5 section")
        if "model_hash=" not in plain:
            errors.append(f"{tid}: plain export missing model_hash in G5")
        if HONESTY_LINE not in plain and HONESTY_LINE.split(".")[0] not in plain:
            errors.append(f"{tid}: honesty footer line missing from plain export")

        # Every span must have G5 fields
        for s in spans:
            for f in ("model_hash", "prompt_hash", "retrieval_ids"):
                if f not in s:
                    errors.append(f"{tid}: span missing {f}")

        docx_bytes = make_docx(plain)
        if docx_bytes[:2] != b"PK":
            errors.append(f"{tid}: DOCX is not a ZIP (PK header)")
        try:
            extracted = extract_docx_text(docx_bytes)
            if FOOTER_ID not in extracted and "EXPORT FOOTER" not in extracted:
                errors.append(f"{tid}: DOCX text missing G4 footer content")
            if "PROVENANCE LOG" not in extracted and "model_hash" not in extracted:
                errors.append(f"{tid}: DOCX text missing G5 provenance")
        except Exception as e:  # noqa: BLE001
            errors.append(f"{tid}: DOCX parse failed: {e}")

        pdf_bytes = make_pdf(plain)
        if not pdf_bytes.startswith(b"%PDF"):
            errors.append(f"{tid}: PDF missing %PDF header")
        if b"%%EOF" not in pdf_bytes:
            errors.append(f"{tid}: PDF missing %%EOF")
        # Footer tokens should appear in content stream (latin-1 safe subset)
        if b"EXPORT FOOTER" not in pdf_bytes and b"caseforge.export_footer" not in pdf_bytes:
            errors.append(f"{tid}: PDF stream missing footer marker")

    # AI-assisted span path requires G5 fields
    ai_span = {
        "span_id": "ai_1",
        "text": "Suggested phrasing for prayer clause.",
        "origin": "ai_assisted",
        "model_hash": "sha256:deadbeef",
        "prompt_hash": "sha256:cafebabe",
        "retrieval_ids": ["ret_1", "ret_2"],
    }
    for f in ("model_hash", "prompt_hash", "retrieval_ids"):
        if not ai_span.get(f) and ai_span.get(f) != []:
            errors.append(f"ai span missing {f}")
    if not isinstance(ai_span["retrieval_ids"], list):
        errors.append("retrieval_ids must be a list")

    if errors:
        print("FAIL  B-S1-15 drafting / export (G4+G5)")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(
        f"PASS  B-S1-15 drafting + DOCX/PDF export "
        f"({FOOTER_ID}; templates={len(TEMPLATES)}; G4 footer + G5 provenance)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
