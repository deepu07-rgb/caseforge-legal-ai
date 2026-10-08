#!/usr/bin/env python3
"""Build the India criminal-codes succession pack (akpkg v2.1): IPC / CrPC / Evidence Act and their 2023 successors
(BNS / BNSS / BSA), section by section, from the official India Code PDFs, plus a STATUTE_SUCCESSION rule table.

The legislative text is the official PDF text (footnotes removed by font size; every section kept verbatim). The
succession table is DEVELOPMENT_ONLY: transition rows cite the repeal-and-savings sections, and the MAPPING rows are
candidates from eval/tools/succession_matcher.py for a named legal editor to confirm. No row carries a sign-off, so
nothing in this pack can make a Code Bridge or audit result verified until an editor signs it.

Usage:
  python eval/tools/build_criminal_codes_pack.py --crawl <indiacode_crawl.py output dir> --key <official_dev.sk.pem>

Text basis per Act: the 2023 Sanhitas/Adhiniyam use India Code's official section records; the repealed codes have no
section records, so their official PDFs are parsed (document-wide body font, footnotes cut at the footnote rule).
"""
from __future__ import annotations

import argparse
import base64
import csv
import hashlib
import json
import re
import sys
from collections import Counter
from pathlib import Path

import pdfplumber
from cryptography.hazmat.primitives import serialization

sys.path.insert(0, str(Path(__file__).resolve().parent))
from succession_matcher import Section, match  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
PACK_ID = "caseforge.india.criminal-codes"
PACK = ROOT / "packs" / "official" / PACK_ID
KEY_ID = "caseforge-official-dev-2026"
VERSION = "1.0.0-beta.1"
JPATH = ["India", "Central Law", "Criminal"]
SWITCH = "2024-07-01"  # commencement of BNS / BNSS / BSA (Gazette notifications of 23 February 2024)

ACTS = {
    "ipc_1860": dict(handle="488475", basis="PDF", code="IPC", title="The Indian Penal Code, 1860", citation="Act No. 45 of 1860", effective_from="1862-01-01", effective_to="2024-06-30",
                     status="SUPERSEDED", start=[r"WHEREAS it is expedient to provide a general Penal Code"]),
    "bns_2023": dict(handle="496548", basis="SECTIONS", code="BNS", title="The Bharatiya Nyaya Sanhita, 2023", citation="Act No. 45 of 2023", effective_from=SWITCH, effective_to="UNKNOWN",
                     status="CURRENT", start=[r"BE it enacted by Parliament"]),
    "crpc_1973": dict(handle="600755", basis="PDF", code="CrPC", title="The Code of Criminal Procedure, 1973", citation="Act No. 2 of 1974", effective_from="1974-04-01", effective_to="2024-06-30",
                      status="SUPERSEDED", start=[r"BE it enacted by Parliament", r"WHEREAS it is expedient to consolidate and amend the law relating to\s+Criminal Procedure"]),
    "bnss_2023": dict(handle="496550", basis="SECTIONS", code="BNSS", title="The Bharatiya Nagarik Suraksha Sanhita, 2023", citation="Act No. 46 of 2023", effective_from=SWITCH, effective_to="UNKNOWN",
                      status="CURRENT", start=[r"BE it enacted by Parliament"]),
    "evidence_1872": dict(handle="488783", basis="PDF", code="IEA", title="The Indian Evidence Act, 1872", citation="Act No. 1 of 1872", effective_from="1872-09-01", effective_to="2024-06-30",
                          status="SUPERSEDED", start=[r"WHEREAS it is expedient to consolidate, define and amend the law of Evidence"]),
    "bsa_2023": dict(handle="496549", basis="SECTIONS", code="BSA", title="The Bharatiya Sakshya Adhiniyam, 2023", citation="Act No. 47 of 2023", effective_from=SWITCH, effective_to="UNKNOWN",
                     status="CURRENT", start=[r"BE it enacted by Parliament"]),
}
# old act, new act, basis date for the governing code, repeal-and-savings section of the new act, its scope (editor to confirm)
PAIRS = [
    ("ipc_1860", "bns_2023", "OFFENCE_DATE", "358", "Offences committed before 1 July 2024 are dealt with under the repealed Indian Penal Code (BNS s.358). Legal editor to confirm."),
    ("crpc_1973", "bnss_2023", "INSTITUTION_DATE", "531", "Appeals, applications, trials, inquiries and investigations pending on 1 July 2024 continue under the Code of Criminal Procedure (BNSS s.531(2)). Proxy: date the proceeding was instituted. Legal editor to confirm."),
    ("evidence_1872", "bsa_2023", "INSTITUTION_DATE", "170", "Proceedings pending before 1 July 2024 continue under the Indian Evidence Act (BSA s.170). Proxy: date the proceeding was instituted. Legal editor to confirm."),
]
ALIASES = [
    {"code": "IPC", "aliases": ["Indian Penal Code", "Penal Code", "I.P.C."]},
    {"code": "CrPC", "aliases": ["Code of Criminal Procedure", "Criminal Procedure Code", "Cr.P.C."]},
    {"code": "IEA", "aliases": ["Indian Evidence Act", "Evidence Act"]},
    {"code": "BNS", "aliases": ["Bharatiya Nyaya Sanhita"]},
    {"code": "BNSS", "aliases": ["Bharatiya Nagarik Suraksha Sanhita"]},
    {"code": "BSA", "aliases": ["Bharatiya Sakshya Adhiniyam"]},
]

HEADING = re.compile(r"^\[?(\d{1,3}[A-Z]{0,3})(?:\.|(?=\s+[“\"]))\s*\[?(\S[^\n]{0,240}?(?:\n[^\n]{0,200}?)?)(?:[.:]?\s*[—–]|\.\s*\]?$|\s+\[?Omitted\.?\]?|\]\s*Rep\.)", re.M)
CHAPTER = re.compile(r"^\s*CHAPTER\s+[IVXLC]+[A-Z]?\b.*$", re.M)
SCHEDULE_START = re.compile(r"^\s*(?:THE\s+)?(?:FIRST\s+)?SCHEDULE\b", re.M)
PAGE_NUMBER = re.compile(r"^\s*\d{1,4}\s*$", re.M)


def digest(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def write_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="\n") as f:
        f.write(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


def canonical(files: list[dict]) -> str:
    return digest(("\n".join(sorted(f"{f['path']}:{f['sha256']}" for f in files)) + "\n").encode())


def body_pages(pdf_path: Path) -> list[tuple[int, str]]:
    """Page text with footnotes and superscript markers removed (characters smaller than the page's body size)."""
    out = []
    with pdfplumber.open(pdf_path) as pdf:
        sizes = Counter(round(c["size"], 1) for pg in pdf.pages for c in pg.chars if c["text"].isalpha())
        doc_body = sizes.most_common(1)[0][0]
        for i, page in enumerate(pdf.pages, 1):
            # Footnotes sit below a short horizontal rule in the lower part of the page; cut there.
            rules = [r for r in page.rects if r["height"] < 2 and 80 < (r["x1"] - r["x0"]) < 300 and r["top"] > page.height * 0.3]
            bottom = min((r["top"] for r in rules), default=page.height)
            region = page.crop((0, 0, page.width, bottom))
            # Some pages are set smaller to fit: measure this page's body size once footnotes are cut away.
            page_sizes = Counter(round(c["size"], 1) for c in region.chars if c["text"].isalpha() and c["size"] < doc_body + 6)
            body = page_sizes.most_common(1)[0][0] if page_sizes else doc_body
            # India Code layout: footnote markers <= body-3pt, small caps ~body-2pt (kept), watermark letters >= body+16pt (dropped).
            kept = region.filter(lambda o: o.get("object_type") != "char" or (o.get("upright", True) and body - 2.4 <= o.get("size", body) <= body + 3.5))
            text = (kept.extract_text(x_tolerance=2, y_tolerance=3) or "").replace("\r\n", "\n").replace("\r", "\n")
            text = PAGE_NUMBER.sub("", text).strip()
            if text:
                out.append((i, text))
    return out


def num_key(n: str) -> tuple[int, str]:
    m = re.match(r"(\d+)([A-Z]*)", n)
    return (int(m.group(1)), m.group(2)) if m else (0, "")


def split_sections(pages: list[tuple[int, str]], spec: dict) -> list[dict]:
    full, offsets = "", []
    for page_no, text in pages:
        offsets.append((len(full), page_no))
        full += text + "\n"
    start = min((m.end() for p in spec["start"] for m in [re.search(p, full)] if m), default=None)
    if start is None:
        raise SystemExit(f"{spec['title']}: enacting formula not found")
    sched = SCHEDULE_START.search(full, start)
    end = sched.start() if sched else len(full)
    sections, last = [], (0, "")
    for m in HEADING.finditer(full, start, end):
        n = m.group(1)
        k = num_key(n)
        # Accept only the next section in sequence (allowing gaps for repealed/omitted numbers).
        if k <= last or k[0] - last[0] > 15:
            continue
        sections.append({"number": n, "heading": m.group(2).strip(" .[]"), "start": m.start()})
        last = k
    for i, s in enumerate(sections):
        stop = sections[i + 1]["start"] if i + 1 < len(sections) else end
        raw = full[s["start"]:stop]
        raw = CHAPTER.sub("", raw)
        # Drop the all-caps chapter title line(s) a chapter heading leaves behind at the end of a section.
        lines = [l for l in raw.rstrip().split("\n")]
        while lines and lines[-1].strip() and lines[-1].strip().upper() == lines[-1].strip() and len(lines) > 1:
            lines.pop()
        s["text"] = re.sub(r"\n{3,}", "\n\n", "\n".join(lines)).strip()
        s["page"] = max(p for off, p in offsets if off <= s["start"])
    return [s for s in sections if len(s["text"]) > 10]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--crawl", type=Path, required=True)
    ap.add_argument("--key", type=Path, required=True)
    ap.add_argument("--worksheet", type=Path, default=ROOT / "docs" / "worksheets" / "Q-S4-02_criminal_succession_candidates.csv")
    a = ap.parse_args()
    records = {slug: json.loads((a.crawl / "acts" / f"{spec['handle']}.json").read_text(encoding="utf-8")) for slug, spec in ACTS.items()}
    ledger = {slug: {**r["pdf"], "url": r["pdf"]["url"], "record_url": r["record_url"], "record_title": r["title"]} for slug, r in records.items()}

    if PACK.exists():
        for child in sorted(PACK.rglob("*"), reverse=True):
            child.unlink() if child.is_file() else child.rmdir()
    (PACK / "content").mkdir(parents=True)
    (PACK / "metadata" / "rule_tables").mkdir(parents=True)

    sources, by_act, report = [], {}, {}
    for slug, spec in ACTS.items():
        pdf = a.crawl / "pdfs" / f"{spec['handle']}.pdf"
        entry = ledger[slug]
        if digest(pdf.read_bytes()) != entry["sha256"]:
            raise SystemExit(f"{slug}: PDF changed since it was fetched")
        if spec["basis"] == "SECTIONS":
            # Official section records: "<number>. <heading>" then the section text, verbatim.
            secs = [{"number": r["number"], "heading": r["heading"].rstrip(" ."), "page": int(r["page"]) if str(r.get("page") or "").isdigit() else 1,
                     "text": f"{r['number']}. {r['heading']}\n{r['text']}"} for r in records[slug]["sections"] if r["number"] and r["text"]]
        else:
            secs = split_sections(body_pages(pdf), spec)
        blocks = []
        for s in secs:
            label = f"[{spec['code']} s.{s['number']}]"
            s["exact"] = f"{label}\n{s['text']}"
            s["label_len"] = len(label) + 1
            blocks.append(s["exact"])
        content = "\n\n".join(blocks) + "\n"
        (PACK / "content" / f"{slug}.txt").write_text(content, encoding="utf-8", newline="\n")
        for s in secs:
            checksum = "sha256:" + digest(s["exact"].encode())
            sid = f"cfsrc:{PACK_ID}:{checksum.removeprefix('sha256:')[:16]}"
            s["source_id"] = sid
            s["chunk_id"] = f"cfchunk:{PACK_ID}:{digest((sid + '|' + checksum).encode())[:16]}"
            sources.append({
                "schema_version": "2.0.0", "source_id": sid, "title": spec["title"], "jurisdiction": "India", "jurisdiction_path": JPATH,
                "authority_type": "STATUTE", "issuing_authority": "Parliament of India", "citation": spec["citation"], "section": s["number"],
                "effective_from": spec["effective_from"], "effective_to": spec["effective_to"], "version": f"India Code PDF fetched {entry['fetched_at'][:10]}",
                "amendment": "As published on India Code on the fetch date; amendments shown in official footnotes are not reproduced",
                "source_file": f"content/{slug}.txt", "page": s["page"], "paragraph": f"Section {s['number']}",
                "heading_path": [spec["title"], f"Section {s['number']} — {s['heading']}"], "exact_text": s["exact"], "checksum": checksum,
                "pack_id": PACK_ID, "trust_level": "OFFICIAL_VERIFIED", "superseded_status": spec["status"],
                "publisher": "India Code, Ministry of Law and Justice, Legislative Department", "source_reference": entry["record_url"],
                "content_kind": "AUTHORITATIVE_TEXT", "derived_from_source_id": None, "holding_tags": [],
                "proposition_tags": [spec["code"].lower(), f"s{s['number'].lower()}"],
            })
        by_act[slug] = secs
        report[slug] = {"sections": len(secs), "first": secs[0]["number"] if secs else None, "last": secs[-1]["number"] if secs else None}
        print(f"{slug}: {len(secs)} sections ({report[slug]['first']}..{report[slug]['last']})", flush=True)

    sources_by_id = {s["source_id"]: s for s in sources}
    if len(sources_by_id) != len(sources):
        raise SystemExit("duplicate source text")
    chunks = [{**s, "chunk_id": next(x["chunk_id"] for secs in by_act.values() for x in secs if x["source_id"] == s["source_id"]),
               "source_checksum": s["checksum"], "pack_checksum": "PENDING", "temporal_status": "APPLICABLE"} for s in sources]

    # --- succession table (DEVELOPMENT_ONLY) ---
    def loc(spec, s, rng=None, label=None):
        out = {"pack_id": PACK_ID, "source_id": s["source_id"], "chunk_id": s["chunk_id"], "char_start": None, "char_end": None,
               "provision_label": label or f"{spec['code']} {s['number']}"}
        if rng:
            out["char_start"], out["char_end"] = rng[0] + s["label_len"], rng[1] + s["label_len"]
        return out

    rows, worksheet = [], []
    for old_slug, new_slug, basis, repeal_no, scope in PAIRS:
        old_spec, new_spec = ACTS[old_slug], ACTS[new_slug]
        new_by_no = {s["number"]: s for s in by_act[new_slug]}
        repeal = new_by_no.get(repeal_no)
        if not repeal:
            raise SystemExit(f"{new_slug}: repeal-and-savings section {repeal_no} not found")
        t_id = f"in.succ.t-{old_spec['code'].lower()}-{new_spec['code'].lower()}"
        rows.append({"rule_id": t_id, "table_kind": "STATUTE_SUCCESSION", "jurisdiction": "India", "effective_from": SWITCH, "effective_to": None,
                     "authorities": [loc(new_spec, repeal)], "legal_editor_signoff": None, "trust_state": "DEVELOPMENT_ONLY",
                     "payload": {"entry_type": "TRANSITION", "before_code": old_spec["code"], "after_code": new_spec["code"], "switch_date": SWITCH,
                                 "governed_by": basis, "scope_note": scope}})
        old = [Section(old_spec["code"], s["number"], s["heading"], s["text"], s["chunk_id"], s["source_id"]) for s in by_act[old_slug]]
        new = [Section(new_spec["code"], s["number"], s["heading"], s["text"], s["chunk_id"], s["source_id"]) for s in by_act[new_slug]]
        old_by_no = {s["number"]: s for s in by_act[old_slug]}
        for c in match(old, new):
            worksheet.append({"old": f"{old_spec['code']} {c.old.number}" if c.old else "", "new": f"{new_spec['code']} {c.new_provision}" if c.new else "",
                              "candidate_class": c.equivalence, "old_in_new": round(c.score_old_in_new, 3), "new_in_old": round(c.score_new_in_old, 3),
                              "heading_similarity": round(c.heading, 3), "note": c.note, "editor_decision": "", "editor_notes": ""})
            if c.equivalence == "REVIEW":
                continue
            auth, pred, succ = [], None, None
            if c.old:
                auth.append(loc(old_spec, old_by_no[c.old.number]))
                pred = {"code": old_spec["code"], "provision": c.old.number}
            if c.new:
                auth.append(loc(new_spec, new_by_no[c.new.number], c.sub_range, f"{new_spec['code']} {c.new_provision}"))
                succ = {"code": new_spec["code"], "provision": c.new_provision}
            if c.equivalence == "OMITTED":
                succ = None
            if c.equivalence == "NEW":
                pred = None
            rid = f"in.succ.{old_spec['code'].lower()}-{c.old.number.lower()}" if c.old else f"in.succ.{new_spec['code'].lower()}-new-{c.new.number.lower()}"
            rows.append({"rule_id": rid, "table_kind": "STATUTE_SUCCESSION", "jurisdiction": "India", "effective_from": SWITCH, "effective_to": None,
                         "authorities": auth, "legal_editor_signoff": None, "trust_state": "DEVELOPMENT_ONLY",
                         "payload": {"entry_type": "MAPPING", "predecessor": pred, "successor": succ, "equivalence": c.equivalence, "transition_rule_ids": [t_id]}})
    table = {"schema_version": "1.0.0", "table_id": "in.criminal-succession.v1", "table_kind": "STATUTE_SUCCESSION", "jurisdiction": "India", "pack_id": PACK_ID,
             "description": "IPC/CrPC/Evidence Act -> BNS/BNSS/BSA. DEVELOPMENT_ONLY: transition rows cite the repeal-and-savings sections; mapping rows are algorithmic candidates (eval/tools/succession_matcher.py) awaiting a named legal editor.",
             "rows": rows}
    write_json(PACK / "metadata" / "rule_tables" / "india_criminal_succession.json", table)
    write_json(PACK / "metadata" / "code_aliases.json", {"schema_version": "1.0.0", "codes": ALIASES})
    a.worksheet.parent.mkdir(parents=True, exist_ok=True)
    with a.worksheet.open("w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(worksheet[0].keys()))
        w.writeheader()
        w.writerows(worksheet)

    write_json(PACK / "metadata" / "legal_sources.json", sources)
    write_json(PACK / "metadata" / "legal_chunks.json", chunks)
    write_json(PACK / "metadata" / "source_ledger.json", [{"title": ACTS[k]["title"], "india_code_record": v["record_url"], "india_code_record_title": v["record_title"],
                                                           "text_basis": "India Code section records" if ACTS[k]["basis"] == "SECTIONS" else "official PDF text", "source_url": v["url"], "official_pdf_sha256": v["sha256"], "bytes": v["bytes"],
                                                           "acquired_at": v["fetched_at"], "issuing_authority": "Parliament of India",
                                                           "publisher": "India Code, Ministry of Law and Justice, Legislative Department",
                                                           "permitted_use_basis": "Copyright Act, 1957, section 52(1)(q)(ii)",
                                                           "non_endorsement": "No Government of India or India Code endorsement is claimed"} for k, v in ledger.items()])  # official PDF + record per Act
    write_json(PACK / "metadata" / "editorial_checklist.json", {
        "schema_version": "1.0.0", "review_status": "TECHNICALLY_VERIFIED_LEGAL_EDITOR_PENDING", "automated_checks": ["official-domain", "pdf-sha256", "section-sequence", "lineage", "signature"],
        "human_legal_editor_1": "PENDING", "human_legal_editor_2": "PENDING", "client_release_allowed": False,
        "succession_rows": {"total": len(rows), "signed": 0, "worksheet": str(a.worksheet.relative_to(ROOT)).replace("\\", "/")},
        "note": "Section texts are official. Succession rows are unsigned candidates and never count as verified until a named legal editor signs them."})

    files = []
    for path in sorted(p for p in PACK.rglob("*") if p.is_file()):
        rel = path.relative_to(PACK).as_posix()
        role = "AUTHORITATIVE_CONTENT" if rel.startswith("content/") else "RULE_TABLE" if rel.startswith("metadata/rule_tables/") else "DERIVED_ARTEFACT"
        role = {"metadata/legal_sources.json": "LEGAL_SOURCES", "metadata/legal_chunks.json": "LEGAL_CHUNKS"}.get(rel, role)
        files.append({"path": rel, "sha256": digest(path.read_bytes()), "bytes": path.stat().st_size, "role": role})
    authoritative = canonical([f for f in files if f["role"] == "AUTHORITATIVE_CONTENT"])
    for c in chunks:
        c["pack_checksum"] = authoritative
    write_json(PACK / "metadata" / "legal_chunks.json", chunks)
    for f in files:
        p = PACK / f["path"]
        f["sha256"], f["bytes"] = digest(p.read_bytes()), p.stat().st_size
    manifest = {
        "schema_version": "2.1.0", "metadata_version": "2.0.0", "pack_id": PACK_ID, "pack_kind": "official", "jurisdiction": "India", "jurisdiction_path": JPATH,
        "version": VERSION, "display_name": "India Criminal Codes (IPC/CrPC/Evidence Act and BNS/BNSS/BSA) - Controlled Beta",
        "description": "Section-level official texts of the repealed and new criminal codes, with an unsigned succession table for the Code Bridge.",
        "content_sha256": canonical(files), "authoritative_content_sha256": authoritative, "files": files, "headnotes_stripped": True,
        "signature_alg": "Ed25519", "signer_key_id": KEY_ID, "trust_level": "OFFICIAL_VERIFIED", "controlled_beta_eligible": True,
        "sources_file": "metadata/legal_sources.json", "chunks_file": "metadata/legal_chunks.json",
        "publisher": "India Code / Ministry of Law and Justice; CaseForge original matter", "updated_at": max(v["fetched_at"][:10] for v in ledger.values()),
        "source_status": "CURRENT", "supersedes_version": None, "superseded_by_version": None, "editorial_review_status": "TWO_LEGAL_EDITORS_PENDING",
        "rights_basis": "Copyright Act, 1957, section 52(1)(q)(ii)",
        "rule_tables_files": ["metadata/rule_tables/india_criminal_succession.json"],
        "currency_date": max(v["fetched_at"][:10] for v in ledger.values()),
    }
    write_json(PACK / "manifest.json", manifest)
    key = serialization.load_pem_private_key(a.key.read_bytes(), password=None)
    sig = key.sign(manifest["content_sha256"].encode("ascii"))
    (PACK / "signature.ed25519").write_text(base64.b64encode(sig).decode("ascii") + "\n", encoding="ascii", newline="\n")
    counts = Counter(r["payload"].get("equivalence", "TRANSITION") for r in rows)
    print(json.dumps({"pack": str(PACK.relative_to(ROOT)), "sections": report, "rows": dict(counts), "review_items": sum(1 for x in worksheet if x["candidate_class"] == "REVIEW"),
                      "bytes": sum(f["bytes"] for f in files), "content_sha256": manifest["content_sha256"]}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
