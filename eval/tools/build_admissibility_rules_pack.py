#!/usr/bin/env python3
"""B-S4-10: development admissibility-foundation rules (Indian Evidence Act, 1872 and Bharatiya Sakshya Adhiniyam, 2023).

Builds packs/official/caseforge.india.admissibility-rules: a self-contained official-format pack (akpkg v2.1) that carries
  * verbatim copies of the sections the rules rest on (IEA s.65B, s.65, s.45 and BSA s.63, s.60, s.39), cut from the signed
    criminal-codes pack's official text, and
  * an ADMISSIBILITY_REQUIREMENT rule table whose every row cites the exact sentence it comes from.
Every row is DEVELOPMENT_ONLY (no legal-editor sign-off). The foundation elements are what the cited sections require a
party to show; they are checks for counsel to resolve, never findings that evidence is or is not admissible.

Usage: python eval/tools/build_admissibility_rules_pack.py --key <official_dev.sk.pem>
"""
from __future__ import annotations

import argparse
import base64
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from sign_pack_manifests import canonical, digest  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
SOURCE_PACK = ROOT / "packs" / "official" / "caseforge.india.criminal-codes"
PACK_ID = "caseforge.india.admissibility-rules"
OUT = ROOT / "packs" / "official" / PACK_ID
TABLE_ID = "in.admissibility.v1"
KEY_ID = "caseforge-official-dev-2026"
TODAY = datetime.now(timezone.utc).date().isoformat()
# Commencement dates are for a legal editor to confirm (Q-S4-02): IEA s.65B was inserted by the Information Technology
# Act, 2000 (in force 17 October 2000); the Bharatiya Sakshya Adhiniyam, 2023 came into force on 1 July 2024.
IEA_FROM_65B, IEA_FROM, BSA_FROM, IEA_TO = "2000-10-17", "1872-09-01", "2024-07-01", "2024-06-30"
LABELS = {"iea65b": "IEA s.65B", "bsa63": "BSA s.63", "iea65": "IEA s.65", "bsa60": "BSA s.60", "iea45": "IEA s.45", "bsa39": "BSA s.39"}


def write_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--key", type=Path, required=True)
    a = ap.parse_args()

    chunks = json.loads((SOURCE_PACK / "metadata" / "legal_chunks.json").read_text(encoding="utf-8"))
    by_label = {}
    for c in chunks:
        m = re.match(r"\[([^\]\n]+)\]\n", c["exact_text"])
        if m:
            by_label[m.group(1)] = (c, c["exact_text"][m.end():].rstrip())
    picked = {k: by_label[v] for k, v in LABELS.items()}

    (OUT / "content").mkdir(parents=True, exist_ok=True)
    content = "\n\n".join(text for _, text in picked.values()) + "\n"
    (OUT / "content" / "evidence_law_extract.txt").write_text(content, encoding="utf-8", newline="\n")
    content_file = {"path": "content/evidence_law_extract.txt", "sha256": digest(content.encode()), "bytes": len(content.encode()), "role": "AUTHORITATIVE_CONTENT"}
    authoritative = canonical([content_file])

    sources, out_chunks, loc = [], [], {}
    for key, (template, text) in picked.items():
        checksum = "sha256:" + digest(text.encode())
        sid = f"cfsrc:{PACK_ID}:{checksum.removeprefix('sha256:')[:16]}"
        section = LABELS[key].split(" ", 1)[1]
        src = {k: v for k, v in template.items() if k not in ("chunk_id", "source_checksum", "pack_checksum", "temporal_status")}
        src.update({"source_id": sid, "section": f"Section {section[2:]}", "source_file": content_file["path"], "page": 1, "paragraph": f"Section {section[2:]}",
                    "heading_path": [template["title"], f"Section {section[2:]}"], "exact_text": text, "checksum": checksum, "pack_id": PACK_ID,
                    "jurisdiction_path": ["India", "Central Law", "Evidence"], "proposition_tags": ["admissibility", "foundation", "evidence"],
                    "version": "India Code official text (verbatim extract of the criminal-codes pack)"})
        sources.append(src)
        cid = f"cfchunk:{PACK_ID}:{digest((sid + '|' + checksum).encode())[:16]}"
        out_chunks.append({**src, "chunk_id": cid, "source_id": sid, "source_checksum": checksum, "pack_checksum": authoritative, "temporal_status": "APPLICABLE"})
        loc[key] = (sid, cid, text)

    def authority(key: str, phrase: str, label: str) -> dict:
        sid, cid, text = loc[key]
        start = text.index(phrase)  # fails loudly if the official text does not say this
        return {"pack_id": PACK_ID, "source_id": sid, "chunk_id": cid, "char_start": start, "char_end": start + len(phrase), "provision_label": label}

    def row(rule_id, etype, frm, to, element, authorities):
        return {"rule_id": rule_id, "table_kind": "ADMISSIBILITY_REQUIREMENT", "jurisdiction": "India", "effective_from": frm, "effective_to": to, "authorities": authorities,
                "legal_editor_signoff": None, "trust_state": "DEVELOPMENT_ONLY", "payload": {"evidence_type": etype, "elements": [element]}}

    def element(eid, label, tags, nodes=()):
        return {"element_id": eid, "label": label, "satisfied_by_node_types": list(nodes), "satisfied_by_type_tags": list(tags)}

    rows = [
        row("in.adm.electronic.iea", "ELECTRONIC_RECORD", IEA_FROM_65B, IEA_TO,
            element("certificate", "A certificate under section 65B(4) identifying the electronic record and the device, signed by a person in a responsible official position", ["CERTIFICATE_65B"]),
            [authority("iea65b", "a certificate doing any of the following things, that is to say,", "Evidence Act s.65B(4)")]),
        row("in.adm.electronic.bsa", "ELECTRONIC_RECORD", BSA_FROM, None,
            element("certificate", "The certificate required by section 63(4) (in the Schedule form, signed by the person in charge of the device and an expert), submitted with the electronic record each time it is offered", ["CERTIFICATE_63"]),
            [authority("bsa63", "a certificate doing any of the following things shall be submitted along with the electronic record at each instance where it is being submitted for admission", "Sakshya Adhiniyam s.63(4)")]),
        row("in.adm.secondary-copy.iea", "SECONDARY_COPY", IEA_FROM, IEA_TO,
            element("ground", "One of the grounds in section 65 is shown for giving a copy instead of the original (for example the original is lost, withheld or cannot easily be moved)", ["ORIGINAL_UNAVAILABLE_NOTE"]),
            [authority("iea65", "evidence may be given of the existence, condition or contents of a document in the following cases:", "Evidence Act s.65")]),
        row("in.adm.secondary-copy.bsa", "SECONDARY_COPY", BSA_FROM, None,
            element("ground", "One of the grounds in section 60 is shown for giving a copy instead of the original (for example the original is lost, withheld or cannot easily be moved)", ["ORIGINAL_UNAVAILABLE_NOTE"]),
            [authority("bsa60", "Secondary evidence may be given of the existence, condition, or contents of a document in the following cases", "Sakshya Adhiniyam s.60")]),
        row("in.adm.expert.iea", "EXPERT_OPINION", IEA_FROM, IEA_TO,
            element("qualification", "It is shown that the witness is specially skilled in the field of the opinion (section 45)", ["EXPERT_QUALIFICATION"], ["witness"]),
            [authority("iea45", "persons specially skilled in such foreign law, science or art", "Evidence Act s.45")]),
        row("in.adm.expert.bsa", "EXPERT_OPINION", BSA_FROM, None,
            element("qualification", "It is shown that the witness is specially skilled in the field of the opinion (section 39)", ["EXPERT_QUALIFICATION"], ["witness"]),
            [authority("bsa39", "persons specially skilled in such foreign law, science or art", "Sakshya Adhiniyam s.39")]),
    ]
    table = {"schema_version": "1.0.0", "table_id": TABLE_ID, "table_kind": "ADMISSIBILITY_REQUIREMENT", "jurisdiction": "India", "pack_id": PACK_ID,
             "description": "Foundation elements for electronic records, copies of documents and expert opinion under the Indian Evidence Act, 1872 and the Bharatiya Sakshya Adhiniyam, 2023. DEVELOPMENT_ONLY: awaiting two legal editors (Q-S4-02). A foundation gap is a prompt for counsel, not a ruling on admissibility. Not covered: other kinds of evidence, case-law gloss (for example on when a section 65B certificate can be dispensed with), state amendments."}
    write_json(OUT / "metadata" / "rule_tables" / "india_admissibility.json", {**table, "rows": rows})
    write_json(OUT / "metadata" / "legal_sources.json", sources)
    write_json(OUT / "metadata" / "legal_chunks.json", out_chunks)

    files = [content_file]
    for rel, role in (("metadata/legal_sources.json", "LEGAL_SOURCES"), ("metadata/legal_chunks.json", "LEGAL_CHUNKS"), ("metadata/rule_tables/india_admissibility.json", "RULE_TABLE")):
        data = (OUT / rel).read_bytes()
        files.append({"path": rel, "sha256": digest(data), "bytes": len(data), "role": role})
    files.sort(key=lambda f: f["path"])
    manifest = {
        "schema_version": "2.1.0", "metadata_version": "2.0.0", "pack_id": PACK_ID, "pack_kind": "official", "jurisdiction": "India",
        "jurisdiction_path": ["India", "Central Law", "Evidence"], "version": "1.0.0-beta.1",
        "display_name": "India Admissibility Foundation Rules (Evidence Act / Sakshya Adhiniyam) - Development",
        "description": "Verbatim IEA s.65B, s.65, s.45 and BSA s.63, s.60, s.39, with an unsigned ADMISSIBILITY_REQUIREMENT rule table for the foundation-gap checks.",
        "content_sha256": canonical(files), "authoritative_content_sha256": authoritative, "headnotes_stripped": True, "signature_alg": "Ed25519", "signer_key_id": KEY_ID,
        "trust_level": "OFFICIAL_VERIFIED", "controlled_beta_eligible": True, "sources_file": "metadata/legal_sources.json", "chunks_file": "metadata/legal_chunks.json",
        "files": files, "publisher": "India Code / Ministry of Law and Justice; CaseForge original matter", "updated_at": TODAY, "source_status": "CURRENT",
        "supersedes_version": None, "superseded_by_version": None, "editorial_review_status": "TWO_LEGAL_EDITORS_PENDING",
        "rights_basis": "Copyright Act, 1957, section 52(1)(q)(ii)", "rule_tables_files": ["metadata/rule_tables/india_admissibility.json"], "currency_date": TODAY,
    }
    write_json(OUT / "manifest.json", manifest)
    from cryptography.hazmat.primitives import serialization

    key = serialization.load_pem_private_key(a.key.read_bytes(), password=None)
    (OUT / "signature.ed25519").write_text(base64.b64encode(key.sign(manifest["content_sha256"].encode("ascii"))).decode("ascii"), encoding="utf-8", newline="\n")
    print(f"{PACK_ID}: {len(rows)} rules, {len(sources)} passages, content_sha256 {manifest['content_sha256'][:16]} (signed with {KEY_ID})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
