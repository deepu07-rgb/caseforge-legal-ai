#!/usr/bin/env python3
"""Build the B-S3-10 controlled India partition/civil-appeal pack.

Inputs are the exact official India Code PDFs acquired into tmp/pdfs. The client pack contains
selected page text, CaseForge-authored applicability notes, provenance, and update metadata. It
does not contain India Code logos, page images, or website layout assets.
"""
from __future__ import annotations

import base64
import hashlib
import json
from pathlib import Path

import pdfplumber
from cryptography.hazmat.primitives import serialization

ROOT = Path(__file__).resolve().parents[2]
PDF_ROOT = ROOT / "tmp" / "pdfs"
PACK = ROOT / "packs" / "official" / "caseforge.india.partition.highcourt.civil-appeal"
PACK_ID = "caseforge.india.partition.highcourt.civil-appeal"
KEY_ID = "caseforge-official-dev-2026"
ACQUIRED = "2026-08-16"

SPECS = [
    {
        "file": "partition_act_1893.pdf",
        "sha256": "080054eb4b50690727288384c95e4eed520e421c516bcb30d1f2eef5b9a9b4bb",
        "pages": [1, 2, 3],
        "slug": "partition_act_1893",
        "title": "The Partition Act, 1893",
        "citation": "Act No. 4 of 1893",
        "section": "Sections 1-10",
        "effective_from": "1893-03-09",
        "url": "https://www.indiacode.nic.in/bitstream/123456789/5657/1/189304.pdf",
        "tags": ["partition", "sale-instead-of-division", "shareholder", "dwelling-house"],
    },
    {
        "file": "cpc_1908.pdf",
        "sha256": "81a542d54d30d22a580b4d0ffc27cc82982714e2c8fe739634536df6babda26c",
        "pages": [68, 69, 70, 218, 219, 220, 221, 222, 223, 224, 225],
        "slug": "code_of_civil_procedure_1908",
        "title": "The Code of Civil Procedure, 1908",
        "citation": "Act No. 5 of 1908",
        "section": "Sections 96-104 and Order XLI (selected official PDF pages)",
        "effective_from": "1909-01-01",
        "url": "https://www.indiacode.nic.in/bitstream/123456789/2191/1/aA1908-05.pdf",
        "tags": ["civil-appeal", "first-appeal", "second-appeal", "order-xli", "appellate-court"],
    },
    {
        "file": "limitation_act_1963.pdf",
        "sha256": "1ad1f2964881272f5ae8769b238ea6ee2bb3e126edbae39dd6bb00794d9f2d82",
        "pages": [3, 4, 5, 6, 7, 22],
        "slug": "limitation_act_1963",
        "title": "The Limitation Act, 1963",
        "citation": "Act No. 36 of 1963",
        "section": "Sections 1-15 and Schedule Articles 115-117 (selected official PDF pages)",
        "effective_from": "1964-01-01",
        "url": "https://www.indiacode.nic.in/bitstream/123456789/1565/5/A1963-36.pdf",
        "tags": ["limitation", "appeal", "condonation", "time-exclusion", "article-116"],
    },
]


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_json(path: Path, value: object) -> None:
    with path.open("w", encoding="utf-8", newline="\n") as stream:
        stream.write(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


def canonical(files: list[dict]) -> str:
    body = "\n".join(sorted(f"{f['path']}:{f['sha256']}" for f in files)) + "\n"
    return digest(body.encode("utf-8"))


def source_id(checksum: str) -> str:
    return f"cfsrc:{PACK_ID}:{checksum.removeprefix('sha256:')[:16]}"


def chunk_id(sid: str, checksum: str) -> str:
    return f"cfchunk:{PACK_ID}:{digest((sid + '|' + checksum).encode('utf-8'))[:16]}"


def extract_pages(path: Path, pages: list[int]) -> dict[int, str]:
    out: dict[int, str] = {}
    with pdfplumber.open(path) as pdf:
        for page_number in pages:
            text = pdf.pages[page_number - 1].extract_text(x_tolerance=2, y_tolerance=3) or ""
            text = text.replace("\r\n", "\n").replace("\r", "\n").strip()
            if len(text) < 80:
                raise SystemExit(f"insufficient extracted text: {path.name} page {page_number}")
            out[page_number] = text
    return out


def reset_output() -> None:
    if PACK.exists():
        for child in sorted(PACK.rglob("*"), reverse=True):
            if child.is_file():
                child.unlink()
            elif child.is_dir():
                child.rmdir()
    (PACK / "content").mkdir(parents=True, exist_ok=True)
    (PACK / "metadata").mkdir(parents=True, exist_ok=True)


def main() -> int:
    reset_output()
    sources: list[dict] = []
    chunks: list[dict] = []
    ledgers: list[dict] = []

    for spec in SPECS:
        pdf_path = PDF_ROOT / spec["file"]
        if not pdf_path.is_file() or digest(pdf_path.read_bytes()) != spec["sha256"]:
            raise SystemExit(f"official PDF missing or changed: {pdf_path}")
        extracted = extract_pages(pdf_path, spec["pages"])
        content_path = PACK / "content" / f"{spec['slug']}.txt"
        blocks = [f"[Official PDF page {page}]\n{text}" for page, text in extracted.items()]
        with content_path.open("w", encoding="utf-8", newline="\n") as stream:
            stream.write("\n\n".join(blocks) + "\n")

        for page, exact_text in extracted.items():
            checksum = "sha256:" + digest(exact_text.encode("utf-8"))
            sid = source_id(checksum)
            common = {
                "schema_version": "2.0.0",
                "source_id": sid,
                "title": spec["title"],
                "jurisdiction": "India",
                "jurisdiction_path": ["India", "Central Law", "High Court", "Civil Appeal"],
                "authority_type": "STATUTE",
                "issuing_authority": "Parliament of India",
                "citation": spec["citation"],
                "section": spec["section"],
                "effective_from": spec["effective_from"],
                "effective_to": "UNKNOWN",
                "version": f"India Code consolidated PDF acquired {ACQUIRED}",
                "amendment": "Consolidated text; amendments and state variations are shown in official footnotes",
                "source_file": f"content/{spec['slug']}.txt",
                "page": page,
                "paragraph": f"Official PDF page {page}",
                "heading_path": [spec["title"], spec["section"], f"PDF page {page}"],
                "exact_text": exact_text,
                "checksum": checksum,
                "pack_id": PACK_ID,
                "trust_level": "OFFICIAL_VERIFIED",
                "superseded_status": "CURRENT",
                "publisher": "India Code, Ministry of Law and Justice, Legislative Department",
                "source_reference": spec["url"],
                "content_kind": "AUTHORITATIVE_TEXT",
                "derived_from_source_id": None,
                "holding_tags": [],
                "proposition_tags": spec["tags"],
            }
            sources.append(common)
            chunks.append(
                {
                    **common,
                    "chunk_id": chunk_id(sid, checksum),
                    "source_checksum": checksum,
                    "pack_checksum": "PENDING",
                    "temporal_status": "APPLICABLE",
                }
            )
        ledgers.append(
            {
                "title": spec["title"],
                "issuing_authority": "Parliament of India",
                "publisher": "India Code, Ministry of Law and Justice, Legislative Department",
                "source_url": spec["url"],
                "acquired_at": ACQUIRED,
                "official_pdf_sha256": spec["sha256"],
                "selected_pdf_pages": spec["pages"],
                "version_status": "CURRENT_AS_PUBLISHED_ON_ACQUISITION_DATE",
                "permitted_use_basis": "Copyright Act, 1957, section 52(1)(q)(ii); reproduced with CaseForge original matter",
                "non_endorsement": "No Government of India or India Code endorsement is claimed",
            }
        )

    commentary = {
        "schema_version": "1.0.0",
        "original_matter_owner": "CaseForge",
        "scope": "India central-law baseline for High Court civil appeals in property partition disputes",
        "use_notes": [
            "This pack is an aid to source inspection, not legal advice or a substitute for the official text.",
            "Confirm the particular High Court's rules, state amendments, forum competence, and current notifications.",
            "Personal-law entitlement, agricultural or revenue land, and state-specific procedure are deliberately absent.",
            "A missing authority must be reported as a repository gap; model memory must not fill it.",
        ],
        "rights_basis": "Indian Copyright Act, 1957, section 52(1)(q)(ii), with this original commentary and pack metadata",
        "source_attribution": "Legislative text sourced from India Code, Ministry of Law and Justice, Legislative Department",
    }
    update_manifest = {
        "schema_version": "1.0.0",
        "pack_version": "1.0.0-beta.1",
        "published_at": ACQUIRED,
        "source_last_checked": ACQUIRED,
        "next_review_due": "2026-09-16",
        "supersedes_version": None,
        "superseded_by_version": None,
        "status": "CURRENT",
        "update_rule": "Recheck official URLs and PDF checksums before every controlled case and at least every 31 days",
    }
    checklist = {
        "schema_version": "1.0.0",
        "review_status": "PRODUCT_OWNER_SCOPE_APPROVED_TECHNICALLY_VERIFIED",
        "product_owner_scope_approved_at": ACQUIRED,
        "automated_checks": ["official-domain", "pdf-sha256", "page-extraction", "lineage", "signature"],
        "human_legal_editor_1": "PENDING",
        "human_legal_editor_2": "PENDING",
        "client_release_allowed": False,
        "note": "Controlled development pack; two named legal-editor approvals are required before client release.",
    }
    goldens = [
        {"query": "When may the court order sale instead of physical division in a partition suit?", "expected": "Partition Act section 2", "must_gap": False},
        {"query": "What is the route of an appeal from an original decree?", "expected": "Code of Civil Procedure section 96", "must_gap": False},
        {"query": "What must accompany a delayed appeal?", "expected": "Order XLI rule 3A", "must_gap": False},
        {"query": "What limitation entry applies to an appeal to a High Court under the CPC?", "expected": "Limitation Act Schedule article 116", "must_gap": False},
        {"query": "What filing rule applies in the Bombay High Court?", "expected": "REPOSITORY_GAP: state/specific High Court rules absent", "must_gap": True},
        {"query": "What share does a Muslim heir receive in this partition?", "expected": "REPOSITORY_GAP: personal-law sources absent", "must_gap": True},
    ]

    metadata = {
        "legal_sources.json": sources,
        "legal_chunks.json": chunks,
        "source_ledger.json": ledgers,
        "editorial_commentary.json": commentary,
        "update_manifest.json": update_manifest,
        "editorial_checklist.json": checklist,
        "retrieval_goldens.json": goldens,
    }
    for name, value in metadata.items():
        write_json(PACK / "metadata" / name, value)

    files: list[dict] = []
    for path in sorted(p for p in PACK.rglob("*") if p.is_file()):
        rel = path.relative_to(PACK).as_posix()
        role = "AUTHORITATIVE_CONTENT" if rel.startswith("content/") else "DERIVED_ARTEFACT"
        if rel == "metadata/legal_sources.json":
            role = "LEGAL_SOURCES"
        elif rel == "metadata/legal_chunks.json":
            role = "LEGAL_CHUNKS"
        files.append({"path": rel, "sha256": digest(path.read_bytes()), "bytes": path.stat().st_size, "role": role})
    authoritative_hash = canonical([f for f in files if f["role"] == "AUTHORITATIVE_CONTENT"])
    for chunk in chunks:
        chunk["pack_checksum"] = authoritative_hash
    write_json(PACK / "metadata" / "legal_chunks.json", chunks)
    for entry in files:
        path = PACK / entry["path"]
        entry["sha256"] = digest(path.read_bytes())
        entry["bytes"] = path.stat().st_size

    manifest = {
        "schema_version": "2.0.0",
        "metadata_version": "2.0.0",
        "pack_id": PACK_ID,
        "pack_kind": "official",
        "jurisdiction": "India",
        "jurisdiction_path": ["India", "Central Law", "High Court", "Civil Appeal"],
        "version": "1.0.0-beta.1",
        "display_name": "India High Court Partition Civil Appeal - Controlled Beta",
        "description": "Narrow central-law repository for the approved controlled case; state and personal-law sources deliberately absent.",
        "content_sha256": canonical(files),
        "authoritative_content_sha256": authoritative_hash,
        "files": files,
        "headnotes_stripped": True,
        "signature_alg": "Ed25519",
        "signer_key_id": KEY_ID,
        "trust_level": "OFFICIAL_VERIFIED",
        "controlled_beta_eligible": True,
        "sources_file": "metadata/legal_sources.json",
        "chunks_file": "metadata/legal_chunks.json",
        "publisher": "India Code / Ministry of Law and Justice; CaseForge original matter",
        "updated_at": ACQUIRED,
        "source_status": "CURRENT",
        "supersedes_version": None,
        "superseded_by_version": None,
        "editorial_review_status": "TWO_LEGAL_EDITORS_PENDING",
        "rights_basis": "Copyright Act, 1957, section 52(1)(q)(ii)",
    }
    write_json(PACK / "manifest.json", manifest)

    key_path = ROOT / "eval" / "tools" / ".pack_keys" / "official_dev.sk.pem"
    if not key_path.is_file():
        raise SystemExit("development signing key missing; run eval/tools/make_akpkg.py key ceremony first")
    key = serialization.load_pem_private_key(key_path.read_bytes(), password=None)
    signature = key.sign(manifest["content_sha256"].encode("ascii"))
    with (PACK / "signature.ed25519").open("w", encoding="ascii", newline="\n") as stream:
        stream.write(base64.b64encode(signature).decode("ascii") + "\n")
    print(f"BUILT {PACK}")
    print(f"sources={len(sources)} chunks={len(chunks)} files={len(files)} bytes={sum(f['bytes'] for f in files)}")
    print(f"content_sha256={manifest['content_sha256']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
