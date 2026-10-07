#!/usr/bin/env python3
"""Generate product-owner synthetic G-CASE-IN GOLDEN pack for AC-6.2 chronology recall.

40 short pages; 20 known dated events. Not client law. Locks GOLDEN under product-owner path.
"""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIX = ROOT / "golden" / "G-CASE-IN" / "fixtures" / "synth_chrono_40"

# 20 known dates the extractor should recover (ISO day form after normalization)
KNOWN_DATES = [
    ("12 March 2023", "2023-03-12"),
    ("15 September 2024", "2024-09-15"),
    ("04 February 2024", "2024-02-04"),
    ("18 January 2024", "2024-01-18"),
    ("01 April 2022", "2022-04-01"),
    ("22 June 2021", "2021-06-22"),
    ("30 November 2020", "2020-11-30"),
    ("05 May 2019", "2019-05-05"),
    ("14 August 2023", "2023-08-14"),
    ("09 December 2022", "2022-12-09"),
    ("03 July 2024", "2024-07-03"),
    ("21 October 2021", "2021-10-21"),
    ("28 February 2023", "2023-02-28"),
    ("11 May 2024", "2024-05-11"),
    ("19 August 2020", "2020-08-19"),
    ("07 January 2022", "2022-01-07"),
    ("16 March 2021", "2021-03-16"),
    ("25 September 2023", "2023-09-25"),
    ("02 April 2024", "2024-04-02"),
    ("13 June 2022", "2022-06-13"),
]


def page_text(i: int) -> str:
    """Page i (1-based). Embed one known date on even pages for first 20 even pages."""
    lines = [
        f"IN THE COURT OF [SYNTHETIC CALIBRATION] — page {i} of 40",
        "Plaintiff: Acme Calibration Pvt. Ltd.",
        "Defendant: Northern Sample Traders",
        "",
    ]
    if i <= 20:
        raw, _iso = KNOWN_DATES[i - 1]
        lines.append(f"On {raw}, the parties exchanged correspondence regarding supply of goods.")
        lines.append(f"Exhibit A-{i} was filed with the registry.")
        if i == 1:
            lines.append("Section 138 of the Negotiable Instruments Act is invoked.")
            lines.append("The plaintiff prays for interim protection and recovery of INR 1,25,000.")
    else:
        lines.append("Procedural listing notes — no new operative dates on this page.")
        lines.append(f"Status quo continues as previously ordered (page {i}).")
    lines.append("")
    lines.append("[CASEFORGE_SYNTHETIC_CASE]")
    lines.append("NOT LAW — product-owner GOLDEN for AC-6.2 measurement.")
    return "\n".join(lines) + "\n"


def main() -> int:
    FIX.mkdir(parents=True, exist_ok=True)
    pages_dir = FIX / "pages"
    pages_dir.mkdir(exist_ok=True)
    for i in range(1, 41):
        (pages_dir / f"page_{i:02d}.txt").write_text(page_text(i), encoding="utf-8")

    # Combined packet for single-doc extract path
    packet = "\n\n--- PAGE BREAK ---\n\n".join(page_text(i) for i in range(1, 41))
    (FIX / "case_packet.txt").write_text(packet, encoding="utf-8")

    now = datetime.now(timezone.utc).isoformat()
    known = {
        "schema_version": "1.0.0",
        "authoritative": True,
        "status": "GOLDEN",
        "fixture_id": "G-CASE-IN/synth_chrono_40",
        "case_graph_contract": "contracts/case_graph.schema.json",
        "case_graph_schema_status": "FROZEN",
        "jurisdiction": "India",
        "chronology_known_dates": [iso for _raw, iso in KNOWN_DATES],
        "chronology_date_count": len(KNOWN_DATES),
        "page_count": 40,
        "eval_policy": "SHIP",
        "notes": "Product-owner synthetic pedagogy for AC-6.2 date recall. Not client matter.",
    }
    meta = {
        "fixture_id": "G-CASE-IN/synth_chrono_40",
        "set_id": "G-CASE-IN",
        "fixture_version": "1.0.0-GOLDEN",
        "status": "GOLDEN",
        "authoritative": True,
        "batch": "B-S1-07",
        "primary_acs": ["AC-6.2"],
        "jurisdiction": "India",
        "description": "40-page synthetic chronology GOLDEN for AC-6.2 (product-owner)",
        "inputs": {
            "case_packet": "case_packet.txt",
            "pages_dir": "pages",
        },
        "known_answer": "known_answer.json",
        "authored_by": "CaseForge Product Owner",
        "reviewed_by": "CaseForge Product Owner",
        "locked_at": now,
        "content_hash": None,
    }
    (FIX / "known_answer.json").write_text(json.dumps(known, indent=2) + "\n", encoding="utf-8")
    (FIX / "meta.json").write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
    (FIX / "case_stub.txt").write_text(
        "Synthetic India chronology pack for AC-6.2. See pages/ and case_packet.txt.\n",
        encoding="utf-8",
    )
    print(f"PASS  wrote {FIX} with 40 pages and {len(KNOWN_DATES)} known dates")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
