#!/usr/bin/env python3
"""Fetch and derive the reproducible public Stage 3 document fixture.

The original files remain attributable to their official publisher. Generated
derivatives are evaluation-only and are not silently treated as separate real
documents. Network access is needed only for the initial acquisition.
"""

from __future__ import annotations

import hashlib
import io
import json
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

import fitz
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "golden" / "G-DOCS-REAL" / "fixtures" / "uscourts_pro_se_1"
ASSETS = FIXTURE / "assets"
PDF_URL = "https://www.uscourts.gov/sites/default/files/complaint_for_a_civil_case.pdf"
DOCX_URL = "https://www.uscourts.gov/sites/default/files/complaint_for_a_civil_case.docx"


def download(url: str, target: Path) -> None:
    request = urllib.request.Request(url, headers={"User-Agent": "CaseForge-Evaluation/1.0"})
    with urllib.request.urlopen(request, timeout=60) as response:
        target.write_bytes(response.read())


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def asset(path: Path, media_type: str, role: str, derived_from: str | None = None) -> dict:
    return {
        "path": path.relative_to(ROOT).as_posix(),
        "media_type": media_type,
        "sha256": digest(path),
        "bytes": path.stat().st_size,
        "role": role,
        "derived_from": derived_from,
    }


def main() -> int:
    ASSETS.mkdir(parents=True, exist_ok=True)
    native_pdf = ASSETS / "complaint_for_a_civil_case.pdf"
    docx = ASSETS / "complaint_for_a_civil_case.docx"
    download(PDF_URL, native_pdf)
    download(DOCX_URL, docx)

    source = fitz.open(native_pdf)
    rendered: list[Image.Image] = []
    transcript: list[str] = []
    for number, page in enumerate(source, start=1):
        pix = page.get_pixmap(matrix=fitz.Matrix(1.5, 1.5), alpha=False)
        rendered.append(Image.open(io.BytesIO(pix.tobytes("png"))).convert("RGB"))
        transcript.append(f"--- PAGE {number} ---\n{page.get_text('text').rstrip()}\n")

    transcript_path = ASSETS / "complaint_for_a_civil_case.txt"
    transcript_path.write_text("\n".join(transcript), encoding="utf-8", newline="\n")
    png = ASSETS / "page_1.png"
    jpg = ASSETS / "page_1.jpg"
    degraded = ASSETS / "page_1_rotated_degraded.jpg"
    rendered[0].save(png, optimize=True)
    rendered[0].save(jpg, quality=82, optimize=True)
    low = rendered[0].convert("L").resize((max(1, rendered[0].width // 2), max(1, rendered[0].height // 2)))
    low.rotate(2.0, expand=True, fillcolor=245).save(degraded, quality=48, optimize=True)

    scanned_pdf = ASSETS / "complaint_scanned_derivative.pdf"
    rendered[0].save(scanned_pdf, "PDF", resolution=108.0, save_all=True, append_images=rendered[1:])

    mixed_pdf = ASSETS / "complaint_mixed_derivative.pdf"
    mixed = fitz.open()
    mixed.insert_pdf(source, from_page=0, to_page=1)
    scan_bytes = io.BytesIO()
    rendered[2].save(scan_bytes, format="PNG", optimize=True)
    rect = source[2].rect
    page = mixed.new_page(width=rect.width, height=rect.height)
    page.insert_image(page.rect, stream=scan_bytes.getvalue())
    mixed.insert_pdf(source, from_page=3, to_page=len(source) - 1)
    mixed.save(mixed_pdf, garbage=4, deflate=True)

    expected = FIXTURE / "expected_answer.json"
    expected.write_text(json.dumps({
        "schema_version": "1.0.0",
        "status": "DRAFT",
        "source_page_count": len(source),
        "source_page_order": list(range(1, len(source) + 1)),
        "mixed_page_count": len(mixed),
        "scanned_page_count": len(rendered),
        "critical_fields": [],
        "review_note": "Objective page facts are machine-derived and require human fixture review before GOLDEN lock."
    }, indent=2) + "\n", encoding="utf-8", newline="\n")
    source.close()
    mixed.close()

    source_rel = native_pdf.relative_to(ROOT).as_posix()
    manifest = {
        "schema_version": "1.0.0",
        "fixture_id": "USCOURTS-PRO-SE-1-2016",
        "suite_id": "G-DOCS-REAL",
        "status": "READY_FOR_REVIEW",
        "source_class": "REAL_WORLD_RIGHTS_CLEARED",
        "jurisdiction": "United States",
        "purposes": ["page-model", "native-text", "ocr", "mixed-page-routing", "format-import"],
        "source": {
            "title": "Complaint for a Civil Case (Pro Se 1)",
            "publisher": "Administrative Office of the U.S. Courts",
            "source_url": "https://www.uscourts.gov/forms-rules/forms/complaint-a-civil-case",
            "acquired_at": datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
        },
        "rights": {
            "status": "VERIFIED",
            "basis": "Publicly downloadable official U.S. Courts form used for internal evaluation; external redistribution is intentionally not asserted.",
            "confidential": False,
            "distribution_scope": "internal_only",
            "may_redistribute": False
        },
        "assets": [
            asset(native_pdf, "application/pdf", "official-native-source"),
            asset(docx, "application/vnd.openxmlformats-officedocument.wordprocessingml.document", "official-docx-source"),
            asset(transcript_path, "text/plain", "machine-extracted-reference-draft", source_rel),
            asset(png, "image/png", "rendered-page", source_rel),
            asset(jpg, "image/jpeg", "rendered-page-lossy", source_rel),
            asset(degraded, "image/jpeg", "rotated-degraded-derivative", source_rel),
            asset(scanned_pdf, "application/pdf", "image-only-scanned-derivative", source_rel),
            asset(mixed_pdf, "application/pdf", "mixed-native-scanned-derivative", source_rel),
        ],
        "expected_answer": {
            "path": expected.relative_to(ROOT).as_posix(),
            "authored_by": ["fetch_stage3_public_fixtures.py"],
            "status": "DRAFT",
            "reviewed_by": [],
            "adjudicator": None
        },
        "characteristics": ["native", "scanned-derivative", "mixed-derivative", "DOCX", "TXT", "JPG", "PNG", "rotated", "low-quality"],
        "notes": "Derivatives preserve provenance to the official source. This fixture cannot produce PASS evidence until a human locks its expected answer."
    }
    (FIXTURE / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(f"Created {len(manifest['assets'])} assets in {FIXTURE.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
