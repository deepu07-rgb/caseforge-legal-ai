#!/usr/bin/env python3
"""
Product-owner synthetic GOLDEN pack for B-S1-13 (AC-4.3 + AC-7.2).

Authorizes S-SYNTH-GRETRIEVAL corpus + queries (not real law).
Writes golden/G-RETRIEVAL-IN|US fixtures with status=GOLDEN.
Also expands pack placeholder content for install-path demos.
"""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NOW = datetime.now(timezone.utc).strftime("%Y-%m-%d")
OWNER = "CaseForge Product Owner"
PACK = "S-SYNTH-GRETRIEVAL-v1"

INDIA_AUTH = [
    ("[2020] INSC DEMO 1", "consideration under the contract was admitted", "contract consideration"),
    ("[2019] INSC DEMO 2", "specific performance of sale agreement", "specific performance"),
    ("[2018] INSC DEMO 3", "arbitration agreement must be in writing", "arbitration agreement writing"),
    ("[2017] INSC DEMO 4", "limitation period for suit on promissory note", "limitation promissory note"),
    ("[2016] INSC DEMO 5", "injunction against breach of negative covenant", "injunction negative covenant"),
    ("[2015] INSC DEMO 6", "bail conditions must be reasonable", "bail conditions reasonable"),
    ("[2014] INSC DEMO 7", "natural justice requires opportunity of hearing", "natural justice hearing"),
    ("[2013] INSC DEMO 8", "writ jurisdiction under Article 226", "writ jurisdiction Article 226"),
    ("[2012] INSC DEMO 9", "consumer complaint maintainability", "consumer complaint maintainability"),
    ("[2011] INSC DEMO 10", "section 138 negotiable instruments cheque bounce", "negotiable instruments cheque"),
    ("[2010] INSC DEMO 11", "partition of ancestral property", "partition ancestral property"),
    ("[2009] INSC DEMO 12", "temporary injunction balance of convenience", "temporary injunction convenience"),
    ("[2008] INSC DEMO 13", "locus standi in public interest litigation", "locus standi public interest"),
    ("[2007] INSC DEMO 14", "res judicata bars re-litigation of same issue", "res judicata re-litigation"),
    ("[2006] INSC DEMO 15", "damages for breach of employment contract", "damages employment contract"),
    ("[2005] INSC DEMO 16", "stamp duty on conveyance of immovable property", "stamp duty conveyance"),
    ("[2004] INSC DEMO 17", "registration of sale deed mandatory", "registration sale deed"),
    ("[2003] INSC DEMO 18", "quashing of FIR under section 482 CrPC", "quashing FIR CrPC"),
    ("[2002] INSC DEMO 19", "maintenance under section 125 CrPC", "maintenance section 125"),
    ("[2001] INSC DEMO 20", "anticipatory bail exceptional circumstances", "anticipatory bail exceptional"),
]

US_AUTH = [
    ("560 U.S. DEMO 1", "statutory text as enacted by Congress controls", "statutory text Congress"),
    ("559 U.S. DEMO 2", "summary judgment requires no genuine dispute of material fact", "summary judgment material fact"),
    ("558 U.S. DEMO 3", "qualified immunity shields officials unless clearly established", "qualified immunity clearly established"),
    ("557 U.S. DEMO 4", "commerce clause reaches interstate economic activity", "commerce clause interstate"),
    ("556 U.S. DEMO 5", "fourth amendment reasonableness of search warrants", "fourth amendment search warrants"),
    ("555 U.S. DEMO 6", "due process requires notice and opportunity to be heard", "due process notice opportunity"),
    ("554 U.S. DEMO 7", "standing requires injury in fact fairly traceable", "standing injury in fact"),
    ("553 U.S. DEMO 8", "res judicata and claim preclusion bars", "claim preclusion res judicata"),
    ("552 U.S. DEMO 9", "arbitration agreements under the FAA are enforceable", "FAA arbitration enforceable"),
    ("551 U.S. DEMO 10", "class certification under Rule 23 commonality", "class certification Rule 23"),
    ("550 U.S. DEMO 11", "first amendment free speech public forum", "first amendment free speech"),
    ("549 U.S. DEMO 12", "equal protection rational basis review", "equal protection rational basis"),
    ("548 U.S. DEMO 13", "habeas corpus successive petition limits", "habeas corpus successive"),
    ("547 U.S. DEMO 14", "securities fraud requires material misstatement", "securities fraud material misstatement"),
    ("546 U.S. DEMO 15", "antitrust rule of reason analysis", "antitrust rule of reason"),
    ("545 U.S. DEMO 16", "patent obviousness under section 103", "patent obviousness section 103"),
    ("544 U.S. DEMO 17", "copyright fair use transformative purpose", "copyright fair use transformative"),
    ("543 U.S. DEMO 18", "ERISA fiduciary duty of loyalty", "ERISA fiduciary duty"),
    ("542 U.S. DEMO 19", "Title VII disparate treatment proof", "Title VII disparate treatment"),
    ("541 U.S. DEMO 20", "sovereign immunity and congressional abrogation", "sovereign immunity abrogation"),
]


def write_corpus(root: Path, jurisdiction: str, rows: list[tuple[str, str, str]]) -> None:
    content = root / "corpus" / "content"
    content.mkdir(parents=True, exist_ok=True)
    for i, (cite, holding, _q) in enumerate(rows, 1):
        body = (
            f"[CASEFORGE_SYNTH_NOT_LAW pack={PACK}]\n"
            f"Neutral citation: {cite}\n"
            f"Court: synthetic {jurisdiction}\n"
            f"The Court held that {holding}.\n"
            f"This excerpt is product-owner synthetic GOLDEN material for retrieval metrics only.\n"
        )
        (content / f"auth_{i:02d}.txt").write_text(body, encoding="utf-8", newline="\n")


def write_fixture(
    set_id: str,
    jurisdiction: str,
    rows: list[tuple[str, str, str]],
    planted: str,
) -> None:
    base = ROOT / "golden" / set_id / "fixtures" / "synth_hitrate_20"
    write_corpus(base, jurisdiction, rows)
    queries = []
    for i, (cite, holding, terms) in enumerate(rows, 1):
        queries.append(
            {
                "query_id": f"{set_id.lower()}-q{i:02d}",
                "query": f"{terms} {holding.split()[0]} neutral citation",
                "known_relevant_authorities": [f"Neutral citation: {cite}", cite],
                "must_appear_in_top_k": 8,
            }
        )
    known = {
        "schema_version": "1.0.0",
        "authoritative": False,
        "status": "GOLDEN",
        "fixture_id": f"{set_id}/synth_hitrate_20",
        "jurisdiction": jurisdiction,
        "product_owner_pack": PACK,
        "authored_by": OWNER,
        "locked_at": NOW,
        "top_k": 8,
        "ac_7_2_threshold": 0.85,
        "corpus_root": "corpus",
        "queries": queries,
        "planted_fake_citations": [
            {
                "citation_string": planted,
                "must_verification_status": "RED",
                "exclude_from_downstream_unless_user_confirms": True,
                "notes": "AC-4.3 negative — not a real authority.",
            }
        ],
        "outside_retrieve_rule": {
            "description": "Authority not in RETRIEVE set must be RED (G2)",
            "sample_authority": "COMPLETELY_UNKNOWN_AUTHORITY_ZZZ_999",
            "must_verification_status": "RED",
        },
        "eval_policy": "SCORE_WHEN_STATUS_GOLDEN",
        "index_id": "caseforge.retrieve.lexical.v1",
    }
    (base / "known_answer.json").write_text(
        json.dumps(known, indent=2) + "\n", encoding="utf-8", newline="\n"
    )
    meta = {
        "fixture_id": f"{set_id}/synth_hitrate_20",
        "set_id": set_id,
        "fixture_version": "1.0.0",
        "status": "GOLDEN",
        "authoritative": False,
        "batch": "B-S1-13",
        "primary_acs": ["AC-4.3", "AC-7.2"],
        "jurisdiction": jurisdiction,
        "description": (
            f"Product-owner synthetic GOLDEN retrieval set ({jurisdiction}). "
            "Not law. AC-7.2 top-8 hit rate + AC-4.3 planted RED."
        ),
        "inputs": {"corpus": "corpus/content", "queries": "known_answer.json"},
        "known_answer": "known_answer.json",
        "authored_by": OWNER,
        "reviewed_by": OWNER,
        "locked_at": NOW,
        "product_owner_pack": PACK,
    }
    (base / "meta.json").write_text(
        json.dumps(meta, indent=2) + "\n", encoding="utf-8", newline="\n"
    )
    # Keep micro_001 as scaffold negative still present; promote note in README handled separately.
    print(f"wrote {base.relative_to(ROOT)}")


def expand_pack_placeholders() -> None:
    india = ROOT / "packs" / "fixtures" / "caseforge.core.india.placeholder" / "content"
    us = ROOT / "packs" / "fixtures" / "caseforge.core.us.placeholder" / "content"
    if india.is_dir():
        for i, (cite, holding, _) in enumerate(INDIA_AUTH[:5], 1):
            (india / f"synth_auth_{i:02d}.txt").write_text(
                f"[CASEFORGE_PACK_PLACEHOLDER_NOT_LAW]\nNeutral citation: {cite}\n"
                f"The Court held that {holding}.\n",
                encoding="utf-8",
                newline="\n",
            )
    if us.is_dir():
        for i, (cite, holding, _) in enumerate(US_AUTH[:5], 1):
            (us / f"synth_auth_{i:02d}.txt").write_text(
                f"[CASEFORGE_PACK_PLACEHOLDER_NOT_LAW]\nNeutral citation: {cite}\n"
                f"The Court held that {holding}.\n",
                encoding="utf-8",
                newline="\n",
            )
    print("expanded pack placeholders (5 each)")


def main() -> int:
    write_fixture(
        "G-RETRIEVAL-IN",
        "India",
        INDIA_AUTH,
        "PLANTED_FAKE v. Placeholder, (2099) 1 SYNTH 1 (Neverland HC)",
    )
    write_fixture(
        "G-RETRIEVAL-US",
        "United States",
        US_AUTH,
        "PLANTED_FAKE v. Placeholder, 999 F. Fake 1 (99th Cir. 2099)",
    )
    expand_pack_placeholders()
    audit = {
        "pack": PACK,
        "owner": OWNER,
        "locked_at": NOW,
        "sets": ["G-RETRIEVAL-IN/synth_hitrate_20", "G-RETRIEVAL-US/synth_hitrate_20"],
        "acs": ["AC-4.3", "AC-7.2"],
        "note": "Synthetic GOLDEN authorized for Stage-1 retrieval metrics; not legal authority.",
    }
    out = ROOT / "golden" / "G-RETRIEVAL-IN" / "PRODUCT_OWNER_LOCK_AUDIT.json"
    out.write_text(json.dumps(audit, indent=2) + "\n", encoding="utf-8", newline="\n")
    print("DONE B-S1-13 synthetic GOLDEN")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
