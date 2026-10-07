#!/usr/bin/env python3
"""AC-6.2 chronology dated-event recall on GOLDEN G-CASE fixtures.

PASS: recall >= chronology_dated_event_min_recall_percent on a GOLDEN set
with page_count >= chronology_golden_page_count (or explicit known dates).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
GCASE_IN = ROOT / "golden" / "G-CASE-IN" / "fixtures"
GCASE_US = ROOT / "golden" / "G-CASE-US" / "fixtures"
CONSTANTS = ROOT / "contracts" / "constants.json"
SIDECAR = ROOT / "sidecars" / "docpipe"


def load_json(p: Path) -> Any:
    return json.loads(p.read_text(encoding="utf-8-sig"))


def thresholds() -> tuple[float, int]:
    data = load_json(CONSTANTS)
    pipe = data.get("document_pipeline") or {}
    return (
        float(pipe.get("chronology_dated_event_min_recall_percent", 95)),
        int(pipe.get("chronology_golden_page_count", 40)),
    )


def iter_golden() -> list[Path]:
    out = []
    for root in (GCASE_IN, GCASE_US):
        if not root.is_dir():
            continue
        for d in sorted(root.iterdir()):
            if not d.is_dir():
                continue
            meta_p = d / "meta.json"
            if not meta_p.exists():
                continue
            meta = load_json(meta_p)
            if meta.get("status") == "GOLDEN":
                out.append(d)
    return out


def extract_dates_from_fixture(fx: Path) -> set[str]:
    sys.path.insert(0, str(SIDECAR))
    from skeleton import extract_from_pages  # type: ignore

    pages: list[dict[str, Any]] = []
    pages_dir = fx / "pages"
    if pages_dir.is_dir():
        for p in sorted(pages_dir.glob("*.txt")):
            idx = 0
            try:
                # page_01.txt → 0
                num = int("".join(c for c in p.stem if c.isdigit()) or "1")
                idx = max(0, num - 1)
            except ValueError:
                idx = len(pages)
            pages.append(
                {
                    "doc_id": "case_packet",
                    "page_index": idx,
                    "text": p.read_text(encoding="utf-8"),
                    "mean_confidence": 0.9,
                }
            )
    else:
        packet = fx / "case_packet.txt"
        stub = fx / "case_stub.txt"
        text = ""
        if packet.exists():
            text = packet.read_text(encoding="utf-8")
        elif stub.exists():
            text = stub.read_text(encoding="utf-8")
        pages.append(
            {
                "doc_id": "case_packet",
                "page_index": 0,
                "text": text,
                "mean_confidence": 0.9,
            }
        )

    meta = load_json(fx / "meta.json")
    jur = meta.get("jurisdiction") or "India"
    graph = extract_from_pages(
        case_id=meta.get("fixture_id") or fx.name,
        jurisdiction=jur,
        pages=pages,
    )
    dates = set()
    for e in graph.get("chronology") or []:
        d = e.get("date")
        if d:
            dates.add(str(d))
    return dates


def score() -> dict[str, Any]:
    min_recall, min_pages = thresholds()
    fixtures = iter_golden()
    if not fixtures:
        return {
            "status": "SKIP",
            "reason": "No GOLDEN G-CASE fixtures for AC-6.2",
            "golden_count": 0,
        }

    # Prefer fixture with known dates + page_count
    target = None
    for fx in fixtures:
        ans = load_json(fx / "known_answer.json")
        if ans.get("chronology_known_dates"):
            target = fx
            break
    if target is None:
        return {
            "status": "SKIP",
            "reason": "GOLDEN fixtures exist but none declare chronology_known_dates",
            "golden_count": len(fixtures),
        }

    ans = load_json(target / "known_answer.json")
    known = set(ans.get("chronology_known_dates") or [])
    page_count = int(ans.get("page_count") or 0)
    if page_count < min_pages:
        return {
            "status": "FAIL",
            "reason": f"GOLDEN page_count {page_count} < required {min_pages}",
            "fixture": target.name,
        }
    if not known:
        return {
            "status": "SKIP",
            "reason": "empty chronology_known_dates",
            "fixture": target.name,
        }

    predicted = extract_dates_from_fixture(target)
    hit = known & predicted
    recall = 100.0 * len(hit) / len(known)
    status = "PASS" if recall >= min_recall else "FAIL"
    return {
        "status": status,
        "reason": (
            f"dated-event recall {recall:.1f}% ({len(hit)}/{len(known)}) "
            f"on {target.name} pages={page_count} (need >= {min_recall}% on >= {min_pages} pages)"
        ),
        "recall_percent": recall,
        "known": sorted(known),
        "predicted": sorted(predicted),
        "missed": sorted(known - predicted),
        "fixture": str(target.relative_to(ROOT)),
    }


def main() -> int:
    import argparse

    ap = argparse.ArgumentParser()
    ap.add_argument("--allow-skip", action="store_true")
    args = ap.parse_args()
    report = score()
    st = report["status"]
    print(f"{st}  AC-6.2  {report['reason']}")
    if report.get("missed"):
        print(f"  missed: {report['missed']}")
    if st == "PASS":
        return 0
    if st == "SKIP" and args.allow_skip:
        return 0
    if st == "SKIP":
        return 2
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
