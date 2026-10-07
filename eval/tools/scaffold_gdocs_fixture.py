#!/usr/bin/env python3
"""Scaffold a G-DOCS DRAFT fixture from a page image/PDF/text file.

Does NOT set GOLDEN. Humans must dual-review and lock per POLICY.md.

Usage:
  python eval/tools/scaffold_gdocs_fixture.py --file scan.png --id page_0042 --class typed_english
  python eval/tools/scaffold_gdocs_fixture.py --file notes.jpg --id page_0043
      # class omitted → DRAFT with null class for labelers
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
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


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return "sha256:" + h.hexdigest()


def main() -> int:
    p = argparse.ArgumentParser(description="Scaffold DRAFT G-DOCS fixture")
    p.add_argument("--file", type=Path, required=True, help="Source page file")
    p.add_argument("--id", required=True, help="Fixture id slug, e.g. page_0042")
    p.add_argument(
        "--class",
        dest="triage_class",
        default=None,
        help="Optional proposed class (DRAFT only; not GOLDEN)",
    )
    p.add_argument(
        "--author",
        default=None,
        help="authored_by field (labeler name)",
    )
    p.add_argument(
        "--force",
        action="store_true",
        help="Overwrite existing fixture directory",
    )
    args = p.parse_args()

    src: Path = args.file
    if not src.is_file():
        print(f"FAIL  source not found: {src}", file=sys.stderr)
        return 1

    if args.triage_class and args.triage_class not in CLASSES:
        print(f"FAIL  class must be one of {sorted(CLASSES)}", file=sys.stderr)
        return 1

    slug = args.id.strip().replace(" ", "_")
    dest = FIXTURES / slug
    if dest.exists() and not args.force:
        print(f"FAIL  exists (use --force): {dest}", file=sys.stderr)
        return 1
    dest.mkdir(parents=True, exist_ok=True)

    dest_file = dest / src.name
    shutil.copy2(src, dest_file)
    content_hash = sha256_file(dest_file)

    # Optional text companion for heuristic / search
    stub = dest / "page_stub.txt"
    if src.suffix.lower() in {".txt", ".md"}:
        stub.write_text(src.read_text(encoding="utf-8", errors="replace"), encoding="utf-8")
    elif not stub.exists():
        stub.write_text(
            f"[DRAFT_FIXTURE]\nSource: {src.name}\n"
            "Human labelers: assign triage class; dual-review before GOLDEN.\n",
            encoding="utf-8",
        )

    fixture_id = f"G-DOCS/{slug}"
    meta = {
        "fixture_id": fixture_id,
        "set_id": "G-DOCS",
        "fixture_version": "0.1.0-DRAFT",
        "status": "DRAFT",
        "authoritative": False,
        "batch": "B-S1-05",
        "primary_acs": ["AC-6.1"],
        "jurisdiction": None,
        "description": f"DRAFT scaffold from {src.name} — not GOLDEN",
        "inputs": {
            "page_text_stub": "page_stub.txt",
            "content_file": src.name,
        },
        "known_answer": "known_answer.json",
        "authored_by": args.author,
        "reviewed_by": None,
        "locked_at": None,
        "content_hash": content_hash,
        "scaffolded_at": datetime.now(timezone.utc).isoformat(),
    }
    ans: dict = {
        "schema_version": "0.1.0",
        "authoritative": False,
        "status": "DRAFT",
        "fixture_id": fixture_id,
        "triage": {
            "page_index": 0,
            "class": args.triage_class,
            "notes": "DRAFT — dual-review required before GOLDEN lock",
        },
        "eval_policy": "SKIP_IF_STATUS_NOT_GOLDEN",
    }

    (dest / "meta.json").write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
    (dest / "known_answer.json").write_text(json.dumps(ans, indent=2) + "\n", encoding="utf-8")

    print(f"PASS  scaffolded DRAFT fixture {fixture_id}")
    print(f"  path: {dest}")
    print(f"  hash: {content_hash}")
    print("  next: dual-label -> adjudicate -> set status GOLDEN (human only)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
