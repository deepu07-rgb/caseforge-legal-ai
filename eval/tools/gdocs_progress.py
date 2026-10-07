#!/usr/bin/env python3
"""Report G-DOCS fixture progress vs MVP-80 / AC-6.1 quotas.

Never flips GOLDEN. Read-only inventory of fixtures under golden/G-DOCS/fixtures.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "golden" / "G-DOCS" / "fixtures"

# MVP-80 quotas (golden/G-DOCS/MVP_80_PLAN.md)
MVP80_QUOTAS = {
    "typed_english": 32,
    "clean_handwriting": 12,
    "messy_handwriting": 10,
    "mixed": 10,
    "other": 10,
    "non_text": 6,
}

SHIP_MIN_PAGES = 500
CLASSES = set(MVP80_QUOTAS)


def triage_class(ans: dict) -> str | None:
    tri = ans.get("triage")
    if isinstance(tri, dict):
        c = tri.get("class")
        return c if isinstance(c, str) else None
    if isinstance(tri, list) and tri:
        c = tri[0].get("class") if isinstance(tri[0], dict) else None
        return c if isinstance(c, str) else None
    return None


def load_fixture(dir_path: Path) -> dict | None:
    meta_p = dir_path / "meta.json"
    ans_p = dir_path / "known_answer.json"
    if not meta_p.exists() or not ans_p.exists():
        return None
    try:
        meta = json.loads(meta_p.read_text(encoding="utf-8-sig"))
        ans = json.loads(ans_p.read_text(encoding="utf-8-sig"))
    except json.JSONDecodeError as e:
        return {"error": f"{dir_path.name}: {e}"}
    status = meta.get("status") or ans.get("status") or "UNKNOWN"
    return {
        "slug": dir_path.name,
        "fixture_id": meta.get("fixture_id") or ans.get("fixture_id"),
        "status": status,
        "authoritative": bool(meta.get("authoritative") or ans.get("authoritative")),
        "class": triage_class(ans),
        "authored_by": meta.get("authored_by"),
        "reviewed_by": meta.get("reviewed_by"),
        "locked_at": meta.get("locked_at"),
        "has_content_file": bool((meta.get("inputs") or {}).get("content_file")),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="G-DOCS progress vs MVP-80 / ship gate")
    ap.add_argument("--json", action="store_true", help="Machine-readable JSON report")
    ap.add_argument(
        "--out",
        type=Path,
        default=None,
        help="Optional write path for JSON report",
    )
    args = ap.parse_args()

    if not FIXTURES.is_dir():
        print(f"FAIL  missing fixtures dir: {FIXTURES}", file=sys.stderr)
        return 1

    rows: list[dict] = []
    errors: list[str] = []
    for d in sorted(FIXTURES.iterdir()):
        if not d.is_dir():
            continue
        row = load_fixture(d)
        if row is None:
            continue
        if "error" in row:
            errors.append(row["error"])
            continue
        rows.append(row)

    by_status: Counter[str] = Counter(r["status"] for r in rows)
    golden = [r for r in rows if r["status"] == "GOLDEN"]
    draft = [r for r in rows if r["status"] == "DRAFT"]
    synthetic = [
        r
        for r in rows
        if r["status"] in {"SYNTHETIC_PLACEHOLDER", "SYNTHETIC_DEVELOPMENT"}
    ]

    golden_by_class: Counter[str] = Counter(
        r["class"] for r in golden if r["class"] in CLASSES
    )
    draft_by_class: Counter[str] = Counter(
        r["class"] for r in draft if r["class"] in CLASSES
    )
    draft_unlabeled = sum(1 for r in draft if not r["class"])

    mvp_filled = sum(min(golden_by_class.get(c, 0), q) for c, q in MVP80_QUOTAS.items())
    mvp_target = sum(MVP80_QUOTAS.values())

    report = {
        "fixtures_dir": str(FIXTURES),
        "total_fixtures": len(rows),
        "by_status": dict(by_status),
        "golden_pages": len(golden),
        "draft_pages": len(draft),
        "synthetic_pages": len(synthetic),
        "draft_unlabeled": draft_unlabeled,
        "golden_by_class": dict(golden_by_class),
        "draft_by_class": dict(draft_by_class),
        "mvp80": {
            "quotas": MVP80_QUOTAS,
            "golden_toward_quotas": mvp_filled,
            "target": mvp_target,
            "complete": mvp_filled >= mvp_target and len(golden) >= mvp_target,
            "class_progress": {
                c: {
                    "golden": golden_by_class.get(c, 0),
                    "draft": draft_by_class.get(c, 0),
                    "quota": q,
                    "remaining_golden": max(0, q - golden_by_class.get(c, 0)),
                }
                for c, q in MVP80_QUOTAS.items()
            },
        },
        "ship_ac61": {
            "min_pages": SHIP_MIN_PAGES,
            "golden_pages": len(golden),
            "remaining": max(0, SHIP_MIN_PAGES - len(golden)),
            "note": "Accuracy >= 92% still required; this tool only counts pages",
        },
        "errors": errors,
        "golden_missing_review": [
            r["slug"]
            for r in golden
            if not r.get("reviewed_by") or not r.get("locked_at")
        ],
    }

    if args.json or args.out:
        text = json.dumps(report, indent=2) + "\n"
        if args.out:
            args.out.parent.mkdir(parents=True, exist_ok=True)
            args.out.write_text(text, encoding="utf-8")
            print(f"WROTE  {args.out}")
        if args.json:
            print(text, end="")
            return 0 if not errors else 1

    print("G-DOCS progress")
    print(f"  fixtures total:     {len(rows)}")
    print(f"  GOLDEN:             {len(golden)}")
    print(f"  DRAFT:              {len(draft)} (unlabeled class: {draft_unlabeled})")
    print(f"  SYNTHETIC:          {len(synthetic)} (plumbing only · not ship)")
    print()
    print("MVP-80 quotas (GOLDEN toward class targets)")
    for c, q in MVP80_QUOTAS.items():
        g = golden_by_class.get(c, 0)
        d = draft_by_class.get(c, 0)
        rem = max(0, q - g)
        bar = "OK" if g >= q else f"need {rem} more GOLDEN"
        print(f"  {c:20} golden={g:3}/{q}  draft={d:3}  {bar}")
    print(f"  MVP-80 fill score:  {mvp_filled}/{mvp_target} (capped per class)")
    print()
    print("AC-6.1 ship page count")
    print(f"  GOLDEN pages:       {len(golden)} / {SHIP_MIN_PAGES}")
    print(f"  remaining pages:    {max(0, SHIP_MIN_PAGES - len(golden))}")
    print("  (accuracy scored separately: python eval/checks/ac_6_1_triage_score.py)")
    if report["golden_missing_review"]:
        print()
        print("WARN  GOLDEN fixtures missing reviewed_by or locked_at:")
        for s in report["golden_missing_review"][:20]:
            print(f"  - {s}")
    if errors:
        print()
        print("ERRORS")
        for e in errors:
            print(f"  - {e}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
