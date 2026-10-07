#!/usr/bin/env python3
"""Apply dual-label CSVs onto DRAFT fixtures and optionally lock GOLDEN after adjudication.

Safety:
- Never invents classes.
- --lock-golden requires --adjudicator and both label columns resolved.
- By default only writes DRAFT known_answer class from a single CSV (labeler pass).
"""

from __future__ import annotations

import argparse
import csv
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "golden" / "G-DOCS" / "fixtures"
CLASSES = {
    "typed_english",
    "other",
    "mixed",
    "clean_handwriting",
    "messy_handwriting",
    "non_text",
}


def load_csv(path: Path) -> dict[str, dict[str, str]]:
    out: dict[str, dict[str, str]] = {}
    with path.open(encoding="utf-8-sig", newline="") as f:
        for row in csv.DictReader(f):
            fid = (row.get("fixture_id") or "").strip()
            if not fid:
                continue
            out[fid] = row
    return out


def slug_from_fid(fid: str) -> str:
    return fid.split("/", 1)[-1] if "/" in fid else fid


def main() -> int:
    ap = argparse.ArgumentParser(description="Apply G-DOCS labels from CSV")
    ap.add_argument("--labels", type=Path, required=True, help="CSV with fixture_id,class,labeler")
    ap.add_argument(
        "--labels-b",
        type=Path,
        default=None,
        help="Optional second labeler CSV for agreement check",
    )
    ap.add_argument(
        "--adjudicated",
        type=Path,
        default=None,
        help="CSV of final classes after adjudicator (fixture_id,class)",
    )
    ap.add_argument(
        "--lock-golden",
        action="store_true",
        help="Lock fixtures to GOLDEN (requires --adjudicated and --adjudicator)",
    )
    ap.add_argument("--adjudicator", default=None, help="reviewed_by name when locking")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    if args.lock_golden:
        if not args.adjudicated or not args.adjudicator:
            print("FAIL  --lock-golden requires --adjudicated and --adjudicator", file=sys.stderr)
            return 1

    primary = load_csv(args.labels)
    secondary = load_csv(args.labels_b) if args.labels_b else {}
    final = load_csv(args.adjudicated) if args.adjudicated else primary

    if args.labels_b and not args.adjudicated:
        agree = disagree = missing = 0
        for fid, a in primary.items():
            ca = (a.get("class") or "").strip()
            cb = (secondary.get(fid) or {}).get("class", "").strip()
            if not ca or not cb:
                missing += 1
                continue
            if ca == cb:
                agree += 1
            else:
                disagree += 1
                print(f"DISAGREE  {fid}: A={ca} B={cb}")
        total = agree + disagree
        rate = (100.0 * agree / total) if total else 0.0
        print(f"AGREEMENT  agree={agree} disagree={disagree} missing={missing} rate={rate:.1f}%")
        print("  (no files written — provide --adjudicated to apply finals)")
        return 0

    ok = fail = 0
    for fid, row in final.items():
        cls = (row.get("class") or "").strip()
        if not cls:
            continue
        if cls not in CLASSES:
            print(f"FAIL  {fid}: bad class {cls}", file=sys.stderr)
            fail += 1
            continue
        slug = slug_from_fid(fid)
        dest = FIXTURES / slug
        meta_p, ans_p = dest / "meta.json", dest / "known_answer.json"
        if not meta_p.exists() or not ans_p.exists():
            print(f"FAIL  missing fixture {slug}", file=sys.stderr)
            fail += 1
            continue
        meta = json.loads(meta_p.read_text(encoding="utf-8-sig"))
        ans = json.loads(ans_p.read_text(encoding="utf-8-sig"))
        labeler = (row.get("labeler") or primary.get(fid, {}).get("labeler") or "").strip()

        tri = ans.get("triage")
        if isinstance(tri, list):
            if tri:
                tri[0]["class"] = cls
            else:
                ans["triage"] = {"page_index": 0, "class": cls}
        else:
            if not isinstance(tri, dict):
                tri = {"page_index": 0}
            tri["class"] = cls
            ans["triage"] = tri

        if args.lock_golden:
            now = datetime.now(timezone.utc).isoformat()
            for obj in (meta, ans):
                obj["status"] = "GOLDEN"
                obj["authoritative"] = True
            meta["authored_by"] = labeler or meta.get("authored_by")
            meta["reviewed_by"] = args.adjudicator
            meta["locked_at"] = now
            ans["eval_policy"] = "SHIP"
            ans["status"] = "GOLDEN"
            ans["authoritative"] = True
        else:
            meta["status"] = "DRAFT"
            ans["status"] = "DRAFT"
            ans["authoritative"] = False
            if labeler:
                meta["authored_by"] = labeler

        if args.dry_run:
            print(f"DRY  {fid} -> {cls} golden={args.lock_golden}")
            ok += 1
            continue

        meta_p.write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
        ans_p.write_text(json.dumps(ans, indent=2) + "\n", encoding="utf-8")
        print(f"OK  {fid} class={cls} status={meta['status']}")
        ok += 1

    print(f"DONE  ok={ok} fail={fail}")
    return 0 if fail == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
