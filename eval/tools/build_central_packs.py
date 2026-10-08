#!/usr/bin/env python3
"""Build one official pack per ministry from an India Code crawl (eval/tools/indiacode_crawl.py) of all Central Acts.

Text basis per Act:
  * India Code section records (official section text, number, heading) when the Act has them;
  * otherwise the official PDF, chunked by page with footnotes, superscript markers and the watermark removed
    (same cleaning as build_criminal_codes_pack.body_pages) — e.g. the Constitution and some older Acts.
The repealed criminal codes and the 2023 Sanhitas are left to caseforge.india.criminal-codes.

Packs are akpkg v2.0.0, OFFICIAL_VERIFIED, unsigned here: `--sign-with <key>` signs locally; in CI the manifests are
signed on the owner's machine (the private key never leaves it) and the signatures injected with `--signatures`.

Usage:
  python eval/tools/build_central_packs.py --crawl <dir> --out <dir> [--sign-with <sk.pem> | --signatures <signatures.json>]
"""
from __future__ import annotations

import argparse
import base64
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from build_criminal_codes_pack import body_pages, canonical, digest, write_json  # noqa: E402

KEY_ID = "caseforge-official-dev-2026"
VERSION = "1.0.0-beta.1"
EXCLUDE_HANDLES = {"123456789/496548", "123456789/496550", "123456789/496549"}  # in caseforge.india.criminal-codes


def slug(s: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", (s or "unassigned").lower()).strip("-")[:60] or "unassigned"


def iso(d: str | None) -> str:
    if not d:
        return "UNKNOWN"
    if re.fullmatch(r"\d{4}-\d{2}-\d{2}", d):
        return d
    m = re.fullmatch(r"(\d{1,2})-(\d{1,2})-(\d{4})", d.strip())
    return f"{m.group(3)}-{int(m.group(2)):02d}-{int(m.group(1)):02d}" if m else "UNKNOWN"


def act_blocks(rec: dict, pdf_dir: Path) -> tuple[list[dict], str]:
    """(blocks with label/text/page/section, basis) for one Act."""
    title = rec["title"].strip()
    if rec["sections"]:
        out = []
        for s in rec["sections"]:
            if not s["number"] or not s["text"]:
                continue
            text = f"{s['number']}. {s['heading']}\n{s['text']}"
            out.append({"section": s["number"], "heading": s["heading"].rstrip(" ."), "page": int(s["page"]) if str(s.get("page") or "").isdigit() else 1,
                        "label": f"[{title} s.{s['number']}]", "text": text})
        return out, "India Code section records"
    pdf = pdf_dir / f"{rec['handle'].split('/')[-1]}.pdf"
    if not pdf.exists():
        return [], "none"
    return [{"section": f"page {p}", "heading": f"PDF page {p}", "page": p, "label": f"[{title} p.{p}]", "text": t}
            for p, t in body_pages(pdf) if len(t) >= 40], "official PDF pages"


def build_pack(pack_id: str, display: str, ministry: str, recs: list[dict], crawl: Path, out: Path) -> dict:
    root = out / pack_id
    (root / "content").mkdir(parents=True, exist_ok=True)
    (root / "metadata").mkdir(parents=True, exist_ok=True)
    sources, ledger, seen = [], [], set()
    authority = "CONSTITUTION" if any(r["kind"] == "CONSTITUTION" for r in recs) else "STATUTE"
    for rec in sorted(recs, key=lambda r: r["title"]):
        blocks, basis = act_blocks(rec, crawl / "pdfs")
        if not blocks:
            continue
        hid = rec["handle"].split("/")[-1]
        content_rel = f"content/{hid}.txt"
        exacts = []
        for b in blocks:
            exact = f"{b['label']}\n{b['text']}"
            if exact in seen:  # identical label+text twice in one Act (duplicate record): keep one
                continue
            seen.add(exact)
            exacts.append(exact)
            checksum = "sha256:" + digest(exact.encode())
            sources.append({
                "schema_version": "2.0.0", "source_id": f"cfsrc:{pack_id}:{checksum.removeprefix('sha256:')[:16]}", "title": rec["title"].strip(),
                "jurisdiction": "India", "jurisdiction_path": ["India", "Central Law", ministry], "authority_type": authority,
                "issuing_authority": "Parliament of India" if authority == "STATUTE" else "Constituent Assembly of India",
                "citation": f"Act No. {rec['act_number']} of {rec['act_year']}" if rec.get("act_number") and rec.get("act_year") else rec["title"].strip(),
                "section": b["section"], "effective_from": iso(rec.get("enforcement_date")) if iso(rec.get("enforcement_date")) != "UNKNOWN" else iso(rec.get("enact_date")),
                "effective_to": "UNKNOWN", "version": f"India Code, crawled {rec['crawled_at'][:10]}", "amendment": "As published on India Code on the crawl date",
                "source_file": content_rel, "page": b["page"], "paragraph": b["section"] if b["section"].startswith("page") else f"Section {b['section']}",
                "heading_path": [rec["title"].strip(), b["heading"]], "exact_text": exact, "checksum": checksum, "pack_id": pack_id,
                "trust_level": "OFFICIAL_VERIFIED", "superseded_status": "SUPERSEDED" if str(rec.get("repealed")).lower() == "true" else "CURRENT",
                "publisher": "India Code, Ministry of Law and Justice, Legislative Department", "source_reference": rec["record_url"],
                "content_kind": "AUTHORITATIVE_TEXT", "derived_from_source_id": None, "holding_tags": [], "proposition_tags": [slug(ministry)],
            })
        (root / content_rel).write_text("\n\n".join(exacts) + "\n", encoding="utf-8", newline="\n")
        pdf = rec.get("pdf") or {}
        ledger.append({"title": rec["title"].strip(), "india_code_record": rec["record_url"], "text_basis": basis, "units": len(exacts),
                       "official_pdf": pdf.get("name"), "official_pdf_url": pdf.get("url"), "official_pdf_sha256": pdf.get("sha256"), "crawled_at": rec["crawled_at"],
                       "permitted_use_basis": "Copyright Act, 1957, section 52(1)(q)(ii)", "non_endorsement": "No Government of India or India Code endorsement is claimed"})
    if not sources:
        return {}
    chunks = [{**s, "chunk_id": f"cfchunk:{pack_id}:{digest((s['source_id'] + '|' + s['checksum']).encode())[:16]}", "source_checksum": s["checksum"],
               "pack_checksum": "PENDING", "temporal_status": "APPLICABLE"} for s in sources]
    write_json(root / "metadata" / "legal_sources.json", sources)
    write_json(root / "metadata" / "legal_chunks.json", chunks)
    write_json(root / "metadata" / "source_ledger.json", ledger)
    write_json(root / "metadata" / "editorial_checklist.json", {"schema_version": "1.0.0", "review_status": "TECHNICALLY_VERIFIED_LEGAL_EDITOR_PENDING",
                                                                  "automated_checks": ["official-api", "pdf-sha256", "lineage", "signature"], "client_release_allowed": False})
    files = []
    for path in sorted(p for p in root.rglob("*") if p.is_file() and p.name not in {"manifest.json", "signature.ed25519"}):
        rel = path.relative_to(root).as_posix()
        role = "AUTHORITATIVE_CONTENT" if rel.startswith("content/") else {"metadata/legal_sources.json": "LEGAL_SOURCES", "metadata/legal_chunks.json": "LEGAL_CHUNKS"}.get(rel, "DERIVED_ARTEFACT")
        files.append({"path": rel, "sha256": "", "bytes": 0, "role": role})
    authoritative = None
    for f in files:
        p = root / f["path"]
        f["sha256"], f["bytes"] = digest(p.read_bytes()), p.stat().st_size
    authoritative = canonical([f for f in files if f["role"] == "AUTHORITATIVE_CONTENT"])
    for c in chunks:
        c["pack_checksum"] = authoritative
    write_json(root / "metadata" / "legal_chunks.json", chunks)
    for f in files:
        p = root / f["path"]
        f["sha256"], f["bytes"] = digest(p.read_bytes()), p.stat().st_size
    crawled = max(r["crawled_at"][:10] for r in recs)
    manifest = {
        "schema_version": "2.0.0", "metadata_version": "2.0.0", "pack_id": pack_id, "pack_kind": "official", "jurisdiction": "India",
        "jurisdiction_path": ["India", "Central Law", ministry], "version": VERSION, "display_name": display,
        "description": f"Official text of the Central Acts administered by the {ministry}, from India Code ({len(ledger)} Acts).",
        "content_sha256": canonical(files), "authoritative_content_sha256": authoritative, "files": files, "headnotes_stripped": True,
        "signature_alg": "Ed25519", "signer_key_id": KEY_ID, "trust_level": "OFFICIAL_VERIFIED", "controlled_beta_eligible": True,
        "sources_file": "metadata/legal_sources.json", "chunks_file": "metadata/legal_chunks.json",
        "publisher": "India Code / Ministry of Law and Justice; CaseForge original matter", "updated_at": crawled, "source_status": "CURRENT",
        "supersedes_version": None, "superseded_by_version": None, "editorial_review_status": "TWO_LEGAL_EDITORS_PENDING",
        "rights_basis": "Copyright Act, 1957, section 52(1)(q)(ii)",
    }
    write_json(root / "manifest.json", manifest)
    return {"pack_id": pack_id, "display_name": display, "ministry": ministry, "acts": len(ledger), "units": len(sources),
            "bytes": sum(f["bytes"] for f in files), "content_sha256": manifest["content_sha256"]}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--crawl", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--sign-with", type=Path, default=None)
    ap.add_argument("--signatures", type=Path, default=None)
    a = ap.parse_args()
    recs = [json.loads(p.read_text(encoding="utf-8")) for p in sorted((a.crawl / "acts").glob("*.json"))]
    groups: dict[str, list[dict]] = defaultdict(list)
    for r in recs:
        if r["kind"] == "REPEALED_CODE" or r["handle"] in EXCLUDE_HANDLES:
            continue
        key = "Constitution of India" if r["kind"] == "CONSTITUTION" else (r.get("ministry") or "Ministry not stated")
        groups[key].append(r)
    built = []
    for ministry, items in sorted(groups.items()):
        pack_id = "caseforge.india.constitution" if ministry == "Constitution of India" else f"caseforge.india.central.{slug(ministry)}"
        display = "The Constitution of India" if ministry == "Constitution of India" else f"India Central Acts - {ministry}"
        info = build_pack(pack_id, display, ministry, items, a.crawl, a.out)
        if info:
            built.append(info)
            print(f"{pack_id}: {info['acts']} Acts, {info['units']} units, {info['bytes'] / 1e6:.1f} MB", flush=True)
    sigs = json.loads(a.signatures.read_text(encoding="utf-8")) if a.signatures else {}
    key = None
    if a.sign_with:
        from cryptography.hazmat.primitives import serialization
        key = serialization.load_pem_private_key(a.sign_with.read_bytes(), password=None)
    for info in built:
        sig = None
        if key is not None:
            sig = base64.b64encode(key.sign(info["content_sha256"].encode("ascii"))).decode("ascii")
        elif info["pack_id"] in sigs and sigs[info["pack_id"]]["content_sha256"] == info["content_sha256"]:
            sig = sigs[info["pack_id"]]["signature"]
        if sig:
            (a.out / info["pack_id"] / "signature.ed25519").write_text(sig + "\n", encoding="ascii", newline="\n")
        info["signed"] = bool(sig)
    write_json(a.out / "packs_index.json", {"packs": built, "total_acts": sum(b["acts"] for b in built), "total_units": sum(b["units"] for b in built),
                                            "total_bytes": sum(b["bytes"] for b in built)})
    print(f"BUILT {len(built)} packs, {sum(b['acts'] for b in built)} Acts, {sum(b['units'] for b in built)} units, {sum(b['bytes'] for b in built) / 1e6:.0f} MB; signed {sum(b['signed'] for b in built)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
