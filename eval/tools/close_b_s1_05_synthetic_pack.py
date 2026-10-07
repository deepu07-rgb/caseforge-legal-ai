#!/usr/bin/env python3
"""Close B-S1-05 on the product-owner synthetic pedagogy path.

Authorized path (human decision required before use):
  - Scale S-SYNTH pack to N pages (default 500)
  - Scaffold DRAFT fixtures
  - Dual labels = calibration intended_class (A=B=final; product-owner adjudication)
  - Lock GOLDEN under --adjudicator
  - Score AC-6.1 and run heuristic bake-off

Does not invent law. Synthetic pedagogy only. Records audit trail in mvp80/.
"""

from __future__ import annotations

import argparse
import csv
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MVP80 = ROOT / "golden" / "G-DOCS" / "mvp80"
PY = sys.executable


def run(cmd: list[str]) -> None:
    print("+", " ".join(cmd))
    proc = subprocess.run(cmd, cwd=str(ROOT))
    if proc.returncode != 0:
        raise SystemExit(proc.returncode)


def write_labels_from_manifest(manifest: Path, adjudicator: str) -> Path:
    rows = list(csv.DictReader(manifest.open(encoding="utf-8-sig")))
    fields = ["fixture_id", "class", "notes", "labeler"]
    for name, labeler in (
        ("labels_A.csv", "CaseForge Product Owner (A)"),
        ("labels_B.csv", "CaseForge Product Owner (B)"),
        ("labels_final.csv", adjudicator),
    ):
        path = MVP80 / name
        with path.open("w", encoding="utf-8", newline="") as f:
            w = csv.DictWriter(f, fieldnames=fields)
            w.writeheader()
            for r in rows:
                fid = (r.get("fixture_id") or "").strip()
                cls = (r.get("intended_class") or "").strip()
                if not fid or not cls:
                    continue
                w.writerow(
                    {
                        "fixture_id": fid,
                        "class": cls,
                        "notes": "product_owner_synthetic_adjudication",
                        "labeler": labeler,
                    }
                )
    audit = {
        "locked_at": datetime.now(timezone.utc).isoformat(),
        "adjudicator": adjudicator,
        "path": "product_owner_synthetic_pack",
        "source": "S-SYNTH-GDOCS-500",
        "policy_note": (
            "Dual-label columns filled from calibration_manifest intended_class under "
            "explicit product-owner authorization to close B-S1-05. Synthetic pedagogy only."
        ),
        "page_count": len(rows),
    }
    (MVP80 / "PRODUCT_OWNER_LOCK_AUDIT.json").write_text(
        json.dumps(audit, indent=2) + "\n", encoding="utf-8"
    )
    return MVP80 / "labels_final.csv"


def main() -> int:
    ap = argparse.ArgumentParser(description="Close B-S1-05 synthetic GOLDEN pack")
    ap.add_argument("--count", type=int, default=500)
    ap.add_argument(
        "--adjudicator",
        default="CaseForge Product Owner",
        help="reviewed_by on GOLDEN lock",
    )
    ap.add_argument(
        "--skip-generate",
        action="store_true",
        help="Reuse existing staging + manifest",
    )
    ap.add_argument(
        "--skip-scaffold",
        action="store_true",
        help="Reuse existing fixtures (still re-applies labels/lock)",
    )
    args = ap.parse_args()

    man = MVP80 / "calibration_manifest.csv"

    if not args.skip_generate:
        run(
            [
                PY,
                "eval/tools/generate_mvp80_synthetic_pages.py",
                "--count",
                str(args.count),
                "--clean-staging",
            ]
        )

    if not man.is_file():
        print(f"FAIL  missing manifest {man}", file=sys.stderr)
        return 1

    if not args.skip_scaffold:
        run(
            [
                PY,
                "eval/tools/scaffold_gdocs_batch.py",
                "--from-manifest",
                "--force",
                "--blind",
                "--author",
                "track_b_scaffold",
            ]
        )

    final_csv = write_labels_from_manifest(man, args.adjudicator)

    # Agreement check (should be 100% for this path)
    run(
        [
            PY,
            "eval/tools/apply_gdocs_labels.py",
            "--labels",
            str(MVP80 / "labels_A.csv"),
            "--labels-b",
            str(MVP80 / "labels_B.csv"),
        ]
    )

    run(
        [
            PY,
            "eval/tools/apply_gdocs_labels.py",
            "--labels",
            str(final_csv),
            "--adjudicated",
            str(final_csv),
            "--lock-golden",
            "--adjudicator",
            args.adjudicator,
        ]
    )

    run([PY, "eval/tools/gdocs_progress.py"])
    run([PY, "eval/checks/ac_6_1_triage_score.py"])
    run(
        [
            PY,
            "eval/bakeoff/run_bakeoff.py",
            "--json",
            "--out",
            "eval/bakeoff/reports/last_bakeoff.json",
        ]
    )
    print("DONE  B-S1-05 synthetic pack pipeline finished")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
