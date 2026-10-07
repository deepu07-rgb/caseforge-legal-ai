#!/usr/bin/env python3
"""Deterministic Argument Integrity scorer for demo cases (PRD §8 eval stub).

VI = 100 - min(Fact, Evidence, Law, Exposure)

Does NOT use an LLM. Not a ship-gate scorer for AC-8.x GOLDEN sets.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

DEMO_ROOT = Path(__file__).resolve().parents[1]
CASES_DIR = DEMO_ROOT / "cases"
LINK_KEYS = ("Fact", "Evidence", "Law", "Exposure")


def load_tree(case_id: str) -> dict:
    path = CASES_DIR / case_id / "argument_tree.json"
    if not path.is_file():
        raise SystemExit(f"FAIL  argument_tree.json not found: {path}")
    with path.open(encoding="utf-8") as f:
        return json.load(f)


def score_limb(limb: dict, threshold: int) -> dict:
    links = limb.get("links") or {}
    missing = [k for k in LINK_KEYS if k not in links]
    if missing:
        raise SystemExit(
            f"FAIL  limb {limb.get('node_id')}: missing link keys {missing}"
        )
    scores = {k: int(links[k]) for k in LINK_KEYS}
    for k, v in scores.items():
        if v < 0 or v > 100:
            raise SystemExit(
                f"FAIL  limb {limb.get('node_id')}: {k}={v} out of 0..100"
            )
    min_link = min(scores, key=scores.get)
    min_val = scores[min_link]
    vi = 100 - min_val
    weak_links = [k for k, v in scores.items() if v < threshold]
    strength = limb.get("strength_assessment") or {}
    return {
        "node_id": limb.get("node_id"),
        "label": limb.get("label"),
        "links": scores,
        "min_link": min_link,
        "min_score": min_val,
        "VI": vi,
        "missing_links": weak_links,
        "overall": strength.get("overall"),
        "why": strength.get("why"),
        "where_argument_needs_more_strength": strength.get(
            "where_argument_needs_more_strength", []
        ),
        "expected_VI": limb.get("expected_VI"),
        "expected_VI_match": (
            limb.get("expected_VI") is None
            or int(limb.get("expected_VI")) == vi
        ),
    }


def score_case(case_id: str, threshold: int) -> dict:
    tree = load_tree(case_id)
    limbs = tree.get("limbs") or []
    if not limbs:
        raise SystemExit(f"FAIL  {case_id}: no limbs")
    scored = [score_limb(L, threshold) for L in limbs]
    weakest = max(scored, key=lambda r: r["VI"])
    all_missing = []
    for r in scored:
        for link in r["missing_links"]:
            all_missing.append(
                {
                    "limb": r["node_id"],
                    "link": link,
                    "score": r["links"][link],
                    "threshold": threshold,
                }
            )
    return {
        "case_id": case_id,
        "fixture_id": tree.get("fixture_id"),
        "difficulty": tree.get("difficulty"),
        "status": tree.get("status"),
        "authoritative": tree.get("authoritative", False),
        "threshold": threshold,
        "relief": (tree.get("relief_root") or {}).get("label"),
        "limbs": scored,
        "weakest_limb": weakest["node_id"],
        "weakest_VI": weakest["VI"],
        "missing_links_panel": all_missing,
        "pedagogy": tree.get("pedagogy"),
        "all_expected_VI_match": all(r["expected_VI_match"] for r in scored),
    }


def list_cases() -> list[str]:
    if not CASES_DIR.is_dir():
        return []
    out = []
    for p in sorted(CASES_DIR.iterdir()):
        if p.is_dir() and (p / "argument_tree.json").is_file():
            out.append(p.name)
    return out


def print_human(report: dict, verbose: bool) -> None:
    print(f"\n=== {report['case_id']} ({report.get('difficulty')}) ===")
    print(f"Relief: {report.get('relief')}")
    print(f"Status: {report.get('status')}  authoritative={report.get('authoritative')}")
    for limb in report["limbs"]:
        print(
            f"\n  Limb {limb['node_id']}  VI={limb['VI']}  "
            f"min={limb['min_link']}:{limb['min_score']}"
        )
        print(f"    {limb['label']}")
        print(
            "    links: "
            + ", ".join(f"{k}={limb['links'][k]}" for k in LINK_KEYS)
        )
        if limb["missing_links"]:
            print(f"    Missing Links (<{report['threshold']}): {', '.join(limb['missing_links'])}")
        else:
            print(f"    Missing Links (<{report['threshold']}): (none)")
        if limb.get("expected_VI") is not None:
            flag = "OK" if limb["expected_VI_match"] else "MISMATCH"
            print(f"    expected_VI={limb['expected_VI']} [{flag}]")
        if verbose:
            if limb.get("overall"):
                print(f"    overall: {limb['overall']}")
            if limb.get("why"):
                print(f"    why: {limb['why']}")
            for item in limb.get("where_argument_needs_more_strength") or []:
                print(
                    f"    + strengthen {item.get('link')} "
                    f"(score {item.get('current_score')}, {item.get('gap')}): "
                    f"{item.get('action')}"
                )
    print(
        f"\n  WEAKEST: {report['weakest_limb']}  VI={report['weakest_VI']}  "
        f"missing_panel_items={len(report['missing_links_panel'])}"
    )
    ped = report.get("pedagogy") or {}
    if ped.get("learning_objective"):
        print(f"  Teach: {ped['learning_objective']}")


def main() -> int:
    p = argparse.ArgumentParser(description="Score demo argument trees (PRD §8 VI)")
    p.add_argument("--case", help="Case folder name under demo/cases/")
    p.add_argument("--all", action="store_true", help="Score all demo cases")
    p.add_argument(
        "--threshold",
        type=int,
        default=60,
        help="Missing Link threshold (default 60)",
    )
    p.add_argument("--verbose", "-v", action="store_true")
    p.add_argument("--json", action="store_true", help="Emit JSON report")
    p.add_argument(
        "--out",
        type=Path,
        default=None,
        help="Write JSON report to path",
    )
    args = p.parse_args()

    if not args.case and not args.all:
        args.all = True

    cases = list_cases() if args.all else [args.case]
    if not cases:
        print("FAIL  no demo cases found", file=sys.stderr)
        return 1

    reports = []
    for cid in cases:
        if cid not in list_cases() and not (CASES_DIR / cid / "argument_tree.json").is_file():
            print(f"FAIL  unknown case: {cid}", file=sys.stderr)
            return 1
        reports.append(score_case(cid, args.threshold))

    if args.json or args.out:
        payload = {
            "product": "CaseForge",
            "layer": "demo",
            "formula": "VI = 100 - min(Fact, Evidence, Law, Exposure)",
            "reports": reports,
        }
        text = json.dumps(payload, indent=2)
        if args.out:
            args.out.parent.mkdir(parents=True, exist_ok=True)
            args.out.write_text(text + "\n", encoding="utf-8")
            print(f"Wrote {args.out}")
        if args.json:
            print(text)
    else:
        for r in reports:
            print_human(r, args.verbose)
        print("\n--- summary ---")
        for r in reports:
            match = "OK" if r["all_expected_VI_match"] else "VI_MISMATCH"
            print(
                f"  {r['case_id']}: weakest={r['weakest_limb']} "
                f"VI={r['weakest_VI']} [{match}]"
            )

    # Exit 1 only on structural failure; expected_VI mismatch is reported but
    # still exit 0 so demo pack remains usable while tuning scores.
    bad = [r["case_id"] for r in reports if not r["all_expected_VI_match"]]
    if bad:
        print(
            f"\nNOTE  expected_VI mismatch on: {', '.join(bad)} "
            "(demo authoring check; not a product ship gate)",
            file=sys.stderr,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
