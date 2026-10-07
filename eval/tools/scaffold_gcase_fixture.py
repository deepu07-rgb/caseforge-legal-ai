#!/usr/bin/env python3
"""Scaffold a G-CASE-IN or G-CASE-US DRAFT fixture.

Same pattern as scaffold_gdocs_fixture.py:
  - status = DRAFT (never GOLDEN)
  - authoritative = false
  - known_answer concept shells are EMPTY (do not invent legal facts)
  - humans dual-review and lock per golden/POLICY.md

Usage:
  python eval/tools/scaffold_gcase_fixture.py --jurisdiction India --id matter_0001 --author "alice"
  python eval/tools/scaffold_gcase_fixture.py --jurisdiction US --id matter_0002 --file pleading.pdf
  python eval/tools/scaffold_gcase_fixture.py --jurisdiction "United States" --id m3 --dir C:\\cases\\packet
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

# Map CLI jurisdiction → set_id + frozen PRD enum value
JURISDICTION_MAP = {
    "india": ("G-CASE-IN", "India"),
    "in": ("G-CASE-IN", "India"),
    "g-case-in": ("G-CASE-IN", "India"),
    "united states": ("G-CASE-US", "United States"),
    "united_states": ("G-CASE-US", "United States"),
    "us": ("G-CASE-US", "United States"),
    "usa": ("G-CASE-US", "United States"),
    "g-case-us": ("G-CASE-US", "United States"),
}

PRIMARY_ACS = ["AC-6.2", "AC-6.3", "AC-6.5", "AC-6.6", "AC-6.8"]
ALLOWED_SOURCE_EXT = {".pdf", ".docx", ".jpg", ".jpeg", ".png", ".txt", ".md"}


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return "sha256:" + h.hexdigest()


def resolve_jurisdiction(raw: str) -> tuple[str, str]:
    key = raw.strip().lower()
    if key not in JURISDICTION_MAP:
        allowed = "India | United States | IN | US"
        raise SystemExit(f"FAIL  --jurisdiction must be one of: {allowed} (got {raw!r})")
    return JURISDICTION_MAP[key]


def collect_sources(files: list[Path], directory: Path | None) -> list[Path]:
    out: list[Path] = []
    for f in files:
        if not f.is_file():
            raise SystemExit(f"FAIL  source not found: {f}")
        out.append(f)
    if directory is not None:
        if not directory.is_dir():
            raise SystemExit(f"FAIL  --dir not a directory: {directory}")
        for p in sorted(directory.rglob("*")):
            if p.is_file() and p.suffix.lower() in ALLOWED_SOURCE_EXT:
                out.append(p)
    # de-dupe by resolve
    seen: set[Path] = set()
    unique: list[Path] = []
    for p in out:
        r = p.resolve()
        if r not in seen:
            seen.add(r)
            unique.append(p)
    return unique


def empty_concept_shell() -> dict:
    """PRD concept areas only — empty. Never invent parties, dates, or statutes."""
    return {
        "chronology_dated_events": [],
        "parties_and_roles": [],
        "statutes_rules_invoked": [],
        "evidence_inventory": [],
        "procedural_posture": None,
        "prayer_relief": None,
        "contradiction_report": [],
        "notes": (
            "Empty arrays intentionally. Real GOLDEN answers must be human-authored "
            "against frozen case_graph schema when Q-CG-01 is resolved. "
            "Do not invent events, parties, or statutes."
        ),
    }


def main() -> int:
    p = argparse.ArgumentParser(
        description="Scaffold DRAFT G-CASE-IN / G-CASE-US fixture (no invented legal answers)"
    )
    p.add_argument(
        "--jurisdiction",
        required=True,
        help='India | United States (aliases: IN, US, USA)',
    )
    p.add_argument("--id", required=True, help="Fixture id slug, e.g. matter_0001")
    p.add_argument(
        "--file",
        type=Path,
        action="append",
        default=[],
        dest="files",
        help="Source document to copy into fixture (repeatable)",
    )
    p.add_argument(
        "--dir",
        type=Path,
        default=None,
        help="Copy all pdf/docx/jpg/png/txt from this directory tree",
    )
    p.add_argument(
        "--author",
        default=None,
        help="authored_by (labeler / content owner name)",
    )
    p.add_argument(
        "--label",
        default=None,
        help="Optional human label for meta.description only (not a case_graph fact)",
    )
    p.add_argument(
        "--force",
        action="store_true",
        help="Overwrite existing fixture directory",
    )
    args = p.parse_args()

    set_id, jurisdiction = resolve_jurisdiction(args.jurisdiction)
    slug = args.id.strip().replace(" ", "_")
    if not slug:
        print("FAIL  --id required", file=sys.stderr)
        return 1

    fixtures_root = ROOT / "golden" / set_id / "fixtures"
    dest = fixtures_root / slug
    if dest.exists() and not args.force:
        print(f"FAIL  exists (use --force): {dest}", file=sys.stderr)
        return 1

    try:
        sources = collect_sources(list(args.files or []), args.dir)
    except SystemExit as e:
        print(str(e), file=sys.stderr)
        return 1

    if dest.exists() and args.force:
        shutil.rmtree(dest)
    dest.mkdir(parents=True, exist_ok=True)

    source_names: list[str] = []
    hashes: list[str] = []
    if sources:
        sources_dir = dest / "sources"
        sources_dir.mkdir(parents=True, exist_ok=True)
        for src in sources:
            # Avoid path collisions
            target_name = src.name
            target = sources_dir / target_name
            n = 1
            while target.exists():
                target = sources_dir / f"{src.stem}_{n}{src.suffix}"
                n += 1
            shutil.copy2(src, target)
            source_names.append(f"sources/{target.name}")
            hashes.append(sha256_file(target))

    # case_stub: text overview for humans; never legal gold
    stub_lines = [
        "[DRAFT_FIXTURE_NOT_LAW]",
        f"Jurisdiction: {jurisdiction}",
        f"Set: {set_id}",
        f"Fixture: {set_id}/{slug}",
        "",
        "This is a DRAFT scaffold for human authoring of case_graph known-answers.",
        "Do not invent parties, dates, statutes, or relief as GOLDEN truth.",
        "Dual-review and lock per golden/POLICY.md before status=GOLDEN.",
        "",
    ]
    if args.label:
        stub_lines.append(f"Working label (meta only): {args.label}")
        stub_lines.append("")
    if source_names:
        stub_lines.append("Source files:")
        for name in source_names:
            stub_lines.append(f"  - {name}")
    else:
        stub_lines.append("No source files attached. Add under sources/ or re-run with --file/--dir.")
    stub_lines.append("")
    (dest / "case_stub.txt").write_text("\n".join(stub_lines), encoding="utf-8")
    hashes.append(sha256_file(dest / "case_stub.txt"))

    # Aggregate content hash (sorted component hashes)
    content_hash = "sha256:" + hashlib.sha256(
        "\n".join(sorted(hashes)).encode("utf-8")
    ).hexdigest()

    fixture_id = f"{set_id}/{slug}"
    desc = (
        f"DRAFT {jurisdiction} case scaffold"
        + (f" — {args.label}" if args.label else "")
        + " — not GOLDEN; empty known-answer shells"
    )

    meta = {
        "fixture_id": fixture_id,
        "set_id": set_id,
        "fixture_version": "0.1.0-DRAFT",
        "status": "DRAFT",
        "authoritative": False,
        "batch": "B-S1-05",
        "primary_acs": PRIMARY_ACS,
        "jurisdiction": jurisdiction,
        "description": desc,
        "inputs": {
            "case_stub": "case_stub.txt",
            "sources": source_names,
        },
        "known_answer": "known_answer.json",
        "authored_by": args.author,
        "reviewed_by": None,
        "locked_at": None,
        "content_hash": content_hash,
        "scaffolded_at": datetime.now(timezone.utc).isoformat(),
        "notes": (
            "case_graph field inventory still BLOCKED (Q-CG-01). "
            "concept_shell lists PRD areas only; fill only after Layer 2 freeze + human review."
        ),
    }

    ans = {
        "schema_version": "0.1.0-DRAFT",
        "authoritative": False,
        "status": "DRAFT",
        "fixture_id": fixture_id,
        "case_graph_contract": "contracts/case_graph.schema.json",
        "case_graph_schema_status": "BLOCKED",
        "jurisdiction": jurisdiction,
        "concept_shell": empty_concept_shell(),
        "eval_policy": "SKIP_IF_STATUS_NOT_GOLDEN",
        "open_questions": ["Q-CG-01", "Q-CG-02", "Q-CG-03"],
    }

    (dest / "meta.json").write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
    (dest / "known_answer.json").write_text(
        json.dumps(ans, indent=2) + "\n", encoding="utf-8"
    )

    print(f"PASS  scaffolded DRAFT fixture {fixture_id}")
    print(f"  path: {dest}")
    print(f"  jurisdiction: {jurisdiction}")
    print(f"  sources: {len(source_names)}")
    print(f"  hash: {content_hash}")
    print("  known_answer: empty concept_shell (no invented legal facts)")
    print("  next: human author graph fields when schema unblocked → dual-review → GOLDEN")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
