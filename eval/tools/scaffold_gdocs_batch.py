#!/usr/bin/env python3
"""Batch-scaffold G-DOCS DRAFT fixtures from MVP-80 manifest or a staging folder.

Never sets GOLDEN. Uses scaffold_gdocs_fixture logic via subprocess for one code path.
"""

from __future__ import annotations

import argparse
import csv
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCAFFOLD = ROOT / "eval" / "tools" / "scaffold_gdocs_fixture.py"
DEFAULT_MANIFEST = ROOT / "golden" / "G-DOCS" / "mvp80" / "calibration_manifest.csv"
STAGING = ROOT / "golden" / "G-DOCS" / "mvp80" / "staging"
CLASSES = {
    "typed_english",
    "other",
    "mixed",
    "clean_handwriting",
    "messy_handwriting",
    "non_text",
}


def run_one(
    file: Path,
    slug: str,
    author: str | None,
    triage_class: str | None,
    force: bool,
) -> int:
    cmd = [
        sys.executable,
        str(SCAFFOLD),
        "--file",
        str(file),
        "--id",
        slug,
    ]
    if author:
        cmd.extend(["--author", author])
    if triage_class:
        cmd.extend(["--class", triage_class])
    if force:
        cmd.append("--force")
    proc = subprocess.run(cmd, cwd=str(ROOT), capture_output=True, text=True)
    if proc.stdout:
        print(proc.stdout, end="")
    if proc.returncode != 0:
        print(proc.stderr or f"FAIL  scaffold {slug}", file=sys.stderr)
    return proc.returncode


def from_manifest(path: Path, author: str | None, force: bool, with_class: bool) -> int:
    if not path.is_file():
        print(f"FAIL  manifest not found: {path}", file=sys.stderr)
        return 1
    rows = list(csv.DictReader(path.open(encoding="utf-8-sig")))
    ok = 0
    fail = 0
    for row in rows:
        slug = (row.get("slug") or "").strip()
        rel = (row.get("staging_path") or "").strip()
        cls = (row.get("intended_class") or "").strip() or None
        if not slug or not rel:
            print(f"FAIL  bad row: {row}", file=sys.stderr)
            fail += 1
            continue
        src = ROOT / rel
        if not src.is_file():
            print(f"FAIL  missing file: {src}", file=sys.stderr)
            fail += 1
            continue
        use_class = cls if with_class and cls in CLASSES else None
        rc = run_one(src, slug, author, use_class, force)
        if rc == 0:
            ok += 1
        else:
            fail += 1
    print(f"DONE  batch scaffold ok={ok} fail={fail}")
    return 0 if fail == 0 else 1


def from_staging(author: str | None, force: bool, with_class: bool) -> int:
    if not STAGING.is_dir():
        print(f"FAIL  staging missing: {STAGING}", file=sys.stderr)
        return 1
    files: list[tuple[Path, str | None]] = []
    for cls_dir in sorted(STAGING.iterdir()):
        if not cls_dir.is_dir():
            continue
        hint = cls_dir.name if cls_dir.name in CLASSES else None
        for f in sorted(cls_dir.iterdir()):
            if f.is_file() and f.suffix.lower() in {
                ".txt",
                ".png",
                ".jpg",
                ".jpeg",
                ".pdf",
                ".md",
            }:
                if f.name.startswith("_"):
                    continue
                files.append((f, hint))
    if not files:
        print("FAIL  no staging files found", file=sys.stderr)
        return 1
    ok = fail = 0
    for i, (f, hint) in enumerate(files, start=1):
        slug = f.stem if f.stem.startswith("page_") else f"page_{i:04d}"
        use_class = hint if with_class else None
        rc = run_one(f, slug, author, use_class, force)
        if rc == 0:
            ok += 1
        else:
            fail += 1
    print(f"DONE  staging scaffold ok={ok} fail={fail}")
    return 0 if fail == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="Batch scaffold G-DOCS DRAFT fixtures")
    ap.add_argument(
        "--from-manifest",
        type=Path,
        nargs="?",
        const=DEFAULT_MANIFEST,
        default=None,
        help="CSV manifest (default mvp80/calibration_manifest.csv)",
    )
    ap.add_argument(
        "--from-staging",
        action="store_true",
        help="Scaffold all files under mvp80/staging/*/",
    )
    ap.add_argument("--author", default="track_b_scaffold", help="authored_by")
    ap.add_argument("--force", action="store_true")
    ap.add_argument(
        "--with-class",
        action="store_true",
        help="Copy intended/folder class into DRAFT known_answer (still not GOLDEN)",
    )
    ap.add_argument(
        "--blind",
        action="store_true",
        help="Leave class null for blind dual-label (default if neither flag)",
    )
    args = ap.parse_args()

    with_class = bool(args.with_class) and not bool(args.blind)
    if args.from_manifest is not None:
        return from_manifest(args.from_manifest, args.author, args.force, with_class)
    if args.from_staging:
        return from_staging(args.author, args.force, with_class)
    print("FAIL  specify --from-manifest and/or --from-staging", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
