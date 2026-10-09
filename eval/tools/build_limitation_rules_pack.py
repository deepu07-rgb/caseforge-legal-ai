#!/usr/bin/env python3
"""B-S4-11: development limitation rules for civil appeals and related applications (Limitation Act, 1963).

Builds packs/official/caseforge.india.limitation-rules: a self-contained official-format pack (akpkg v2.1) that carries
  * verbatim copies of the exact official passages the rules rest on (sections 4 and 12 and Schedule Articles 116-122),
    cut from the signed controlled partition pack's official PDF text, and
  * a LIMITATION_PERIOD rule table whose every row cites the exact line of those passages it comes from.
Every row is DEVELOPMENT_ONLY (no legal-editor sign-off): the engine shows such dates as reference only, never verified.

Usage: python eval/tools/build_limitation_rules_pack.py --key <official_dev.sk.pem>
"""
from __future__ import annotations

import argparse
import base64
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from sign_pack_manifests import canonical, digest  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
SOURCE_PACK = ROOT / "packs" / "official" / "caseforge.india.partition.highcourt.civil-appeal"
PACK_ID = "caseforge.india.limitation-rules"
OUT = ROOT / "packs" / "official" / PACK_ID
TABLE_ID = "in.limitation.civil-appeals.v1"
KEY_ID = "caseforge-official-dev-2026"
TODAY = datetime.now(timezone.utc).date().isoformat()

COPY_EXCLUSION = {"exclusion_id": "copy-obtaining-time", "condition_fact_type": "COPY_OBTAINING_TIME",
                  "description": "Time requisite for obtaining a copy of the decree or order appealed from (section 12(2)); the day the order was pronounced is already excluded.",
                  "authority_index": 2}


def write_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")


def passages() -> dict[str, tuple[str, str, str]]:
    """key -> (text, section label, heading) cut verbatim from the partition pack's official text."""
    chunks = json.loads((SOURCE_PACK / "metadata" / "legal_chunks.json").read_text(encoding="utf-8"))
    whole = "\n".join(c["exact_text"] for c in chunks if "Limitation Act, 1963" in c["title"])

    def cut(start: str, end: str | None) -> str:
        i = whole.index(start)
        j = whole.index(end, i) if end else whole.rindex("\n", i)
        return whole[i:j].rstrip()

    return {
        "s4": (cut("4. Expiry of prescribed period when court is closed.", "5. Extension of prescribed period in certain cases."), "Section 4", "Expiry of prescribed period when court is closed"),
        "s12": (cut("12. Exclusion of time in legal proceedings.", "13. Exclusion of time in cases where leave"), "Section 12", "Exclusion of time in legal proceedings"),
        "a116": (cut("116. Under the Code of Civil Procedure,", "THIRD DIVISION"), "Schedule, Articles 116-117", "Appeals under the Code of Civil Procedure and from High Court decrees"),
        "a118": (cut("118. For leave to appear and defend a", None), "Schedule, Articles 118-122", "Applications in specified cases"),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--key", type=Path, required=True)
    a = ap.parse_args()
    template = next(s for s in json.loads((SOURCE_PACK / "metadata" / "legal_sources.json").read_text(encoding="utf-8")) if "Limitation Act, 1963" in s["title"])
    texts = passages()

    (OUT / "content").mkdir(parents=True, exist_ok=True)
    content = "\n\n".join(t for t, _, _ in texts.values()) + "\n"
    (OUT / "content" / "limitation_act_1963_extract.txt").write_text(content, encoding="utf-8", newline="\n")
    content_file = {"path": "content/limitation_act_1963_extract.txt", "sha256": digest(content.encode()), "bytes": len(content.encode()), "role": "AUTHORITATIVE_CONTENT"}
    authoritative = canonical([content_file])

    sources, chunks, loc = [], [], {}
    for key, (text, section, heading) in texts.items():
        checksum = "sha256:" + digest(text.encode())
        sid = f"cfsrc:{PACK_ID}:{checksum.removeprefix('sha256:')[:16]}"
        src = {**template, "source_id": sid, "section": section, "effective_from": "1963-10-05", "effective_to": "UNKNOWN", "jurisdiction_path": ["India", "Central Law", "Limitation"],
               "version": "India Code consolidated PDF acquired 2026-08-16; verbatim extract of the controlled partition pack", "source_file": content_file["path"], "page": 1,
               "paragraph": section, "heading_path": ["The Limitation Act, 1963", section, heading], "exact_text": text, "checksum": checksum, "pack_id": PACK_ID,
               "proposition_tags": ["limitation", "deadline", "computation-of-period"]}
        sources.append(src)
        cid = f"cfchunk:{PACK_ID}:{digest((sid + '|' + checksum).encode())[:16]}"
        chunks.append({**src, "chunk_id": cid, "source_id": sid, "source_checksum": checksum, "pack_checksum": authoritative, "temporal_status": "APPLICABLE"})
        loc[key] = (sid, cid, text)

    def authority(key: str, line: str, label: str) -> dict:
        sid, cid, text = loc[key]
        start = text.index(line)
        return {"pack_id": PACK_ID, "source_id": sid, "chunk_id": cid, "char_start": start, "char_end": start + len(line), "provision_label": label}

    s12_1 = authority("s12", "the day from which such period is to be reckoned, shall be excluded.", "Limitation Act s.12(1)")
    s12_2 = authority("s12", "the time requisite for obtaining a copy of the decree, sentence or order appealed from or sought to be", "Limitation Act s.12(2)")
    s4 = authority("s4", "expires on a day when the court is closed", "Limitation Act s.4")

    def row(rule_id, label, event, days, key, line, article, copy):
        entry = authority(key, line, f"Limitation Act Schedule, {article}")
        return {
            "rule_id": rule_id, "table_kind": "LIMITATION_PERIOD", "jurisdiction": "India", "effective_from": "1964-01-01", "effective_to": None,
            "authorities": [entry, s12_1, s12_2, s4] if copy else [entry, s12_1, s4], "legal_editor_signoff": None, "trust_state": "DEVELOPMENT_ONLY",
            "payload": {"deadline_label": label, "trigger_event_type": event, "applies_to_matter_types": ["CIVIL"], "period": {"value": days, "unit": "DAYS"},
                        "counting": {"exclude_trigger_day": True, "roll_forward_if_court_closed": True, "calendar_table_id": None},
                        "exclusions": [COPY_EXCLUSION] if copy else []},
        }

    rows = [
        row("in.limitation.cpc.appeal.high-court", "Appeal to a High Court from a decree or order", "DECREE_OR_ORDER", 90, "a116", "(a) to a High Court from any Ninety days. The date of the decree or order.", "Art. 116(a)", True),
        row("in.limitation.cpc.appeal.other-court", "Appeal to a court other than a High Court from a decree or order", "DECREE_OR_ORDER", 30, "a116", "(b) to any other court from any Thirty days. The date of the decree or order.", "Art. 116(b)", True),
        row("in.limitation.hc.appeal.same-court", "Appeal from a High Court decree or order to the same Court", "DECREE_OR_ORDER", 30, "a116", "117. From a decree or order of any High Thirty days. The date of the decree or order.", "Art. 117", True),
        row("in.limitation.summary-suit.leave-to-defend", "Application for leave to appear and defend a summary suit", "SUMMONS_SERVED", 10, "a118", "118. For leave to appear and defend a Ten days. When the summons is served.", "Art. 118", False),
        row("in.limitation.cpc.legal-representative", "Application to bring the legal representative of a deceased party on record", "PARTY_DEATH", 90, "a118", "120. Under the Code of Civil Ninety days. The date of death of the plaintiff,", "Art. 120", False),
        row("in.limitation.cpc.set-aside-abatement", "Application to set aside an abatement", "ABATEMENT", 60, "a118", "121. Under the same Code for an order Sixty days. The date of abatement.", "Art. 121", False),
        row("in.limitation.restore-dismissed-for-default", "Application to restore a matter dismissed for default", "DISMISSAL_FOR_DEFAULT", 30, "a118", "122. To restore a suit or appeal or Thirty days. The date of dismissal.", "Art. 122", False),
    ]
    table = {"schema_version": "1.0.0", "table_id": TABLE_ID, "table_kind": "LIMITATION_PERIOD", "jurisdiction": "India", "pack_id": PACK_ID,
             "description": "Limitation periods for civil appeals and related applications from the Schedule to the Limitation Act, 1963, counted under sections 4 and 12. DEVELOPMENT_ONLY: awaiting two legal editors (Q-S4-02). Not covered: other Articles, state amendments, special-law periods."}
    write_json(OUT / "metadata" / "rule_tables" / "india_limitation_civil_appeals.json", {**table, "rows": rows})
    write_json(OUT / "metadata" / "legal_sources.json", sources)
    write_json(OUT / "metadata" / "legal_chunks.json", chunks)

    files = [content_file]
    for rel, role in (("metadata/legal_sources.json", "LEGAL_SOURCES"), ("metadata/legal_chunks.json", "LEGAL_CHUNKS"), ("metadata/rule_tables/india_limitation_civil_appeals.json", "RULE_TABLE")):
        data = (OUT / rel).read_bytes()
        files.append({"path": rel, "sha256": digest(data), "bytes": len(data), "role": role})
    files.sort(key=lambda f: f["path"])
    manifest = {
        "schema_version": "2.1.0", "metadata_version": "2.0.0", "pack_id": PACK_ID, "pack_kind": "official", "jurisdiction": "India",
        "jurisdiction_path": ["India", "Central Law", "Limitation"], "version": "1.0.0-beta.1",
        "display_name": "India Limitation Rules for Civil Appeals (Limitation Act, 1963) - Development",
        "description": "Verbatim sections 4 and 12 and Schedule Articles 116-122 of the Limitation Act, 1963, with an unsigned LIMITATION_PERIOD rule table for the Deadlines view.",
        "content_sha256": canonical(files), "authoritative_content_sha256": authoritative, "headnotes_stripped": True, "signature_alg": "Ed25519", "signer_key_id": KEY_ID,
        "trust_level": "OFFICIAL_VERIFIED", "controlled_beta_eligible": True, "sources_file": "metadata/legal_sources.json", "chunks_file": "metadata/legal_chunks.json",
        "files": files, "publisher": "India Code / Ministry of Law and Justice; CaseForge original matter", "updated_at": TODAY, "source_status": "CURRENT",
        "supersedes_version": None, "superseded_by_version": None, "editorial_review_status": "TWO_LEGAL_EDITORS_PENDING",
        "rights_basis": "Copyright Act, 1957, section 52(1)(q)(ii)", "rule_tables_files": ["metadata/rule_tables/india_limitation_civil_appeals.json"], "currency_date": TODAY,
    }
    write_json(OUT / "manifest.json", manifest)
    from cryptography.hazmat.primitives import serialization

    key = serialization.load_pem_private_key(a.key.read_bytes(), password=None)
    (OUT / "signature.ed25519").write_text(base64.b64encode(key.sign(manifest["content_sha256"].encode("ascii"))).decode("ascii"), encoding="utf-8", newline="\n")
    print(f"{PACK_ID}: {len(rows)} rules, {len(sources)} passages, content_sha256 {manifest['content_sha256'][:16]} (signed with {KEY_ID})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
