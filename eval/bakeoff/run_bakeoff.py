#!/usr/bin/env python3
"""Run triage bake-off on G-DOCS labeled fixtures.

Default: GOLDEN only, exclude SYNTHETIC. Baseline heuristic always available.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
GDOCS = ROOT / "golden" / "G-DOCS" / "fixtures"
BAKEOFF_DIR = Path(__file__).resolve().parent
REPORTS = BAKEOFF_DIR / "reports"
CLASSES = [
    "typed_english",
    "other",
    "mixed",
    "clean_handwriting",
    "messy_handwriting",
    "non_text",
]


def load_json(path: Path) -> Any:
    with path.open(encoding="utf-8-sig") as f:
        return json.load(f)


def load_fixtures(*, include_draft: bool) -> list[dict[str, Any]]:
    allowed = {"GOLDEN"}
    if include_draft:
        allowed.add("DRAFT")
    out: list[dict[str, Any]] = []
    if not GDOCS.is_dir():
        return out
    for d in sorted(GDOCS.iterdir()):
        if not d.is_dir():
            continue
        meta_p, ans_p = d / "meta.json", d / "known_answer.json"
        if not meta_p.exists() or not ans_p.exists():
            continue
        meta, ans = load_json(meta_p), load_json(ans_p)
        status = meta.get("status") or ans.get("status")
        if status not in allowed:
            continue
        triage = ans.get("triage") or {}
        pages_exp: list[dict[str, Any]] = []
        if isinstance(triage, list):
            for t in triage:
                pages_exp.append(
                    {
                        "page_index": int(t.get("page_index", 0)),
                        "class": t.get("class") or t.get("triage_class"),
                    }
                )
        elif isinstance(triage, dict) and (triage.get("class") or triage.get("triage_class")):
            pages_exp.append(
                {
                    "page_index": int(triage.get("page_index", 0)),
                    "class": triage.get("class") or triage.get("triage_class"),
                }
            )
        if not pages_exp:
            continue
        inputs = meta.get("inputs") or {}
        content_name = inputs.get("content_file")
        content_path = str(d / content_name) if content_name else None
        text = ""
        stub = d / "page_stub.txt"
        if stub.exists():
            text = stub.read_text(encoding="utf-8-sig")
        out.append(
            {
                "id": meta.get("fixture_id") or d.name,
                "status": status,
                "dir": str(d),
                "text": text,
                "content_path": content_path,
                "expected_pages": pages_exp,
            }
        )
    return out


def score_candidate(
    name: str,
    fixtures: list[dict[str, Any]],
    predict_fn,
) -> dict[str, Any]:
    total = 0
    correct = 0
    confusion: dict[str, dict[str, int]] = defaultdict(lambda: defaultdict(int))
    latencies: list[float] = []
    errors: list[str] = []
    for fx in fixtures:
        try:
            preds = predict_fn(fx)
        except Exception as e:  # noqa: BLE001 — bake-off must continue other fixtures
            errors.append(f"{fx['id']}: {e}")
            continue
        by_idx = {int(p["page_index"]): p for p in preds}
        for exp in fx["expected_pages"]:
            cls = exp["class"]
            if cls not in CLASSES:
                errors.append(f"{fx['id']}: bad expected class {cls}")
                continue
            total += 1
            pred = by_idx.get(int(exp["page_index"]))
            got = (pred or {}).get("triage_class") or "MISSING"
            if got == cls:
                correct += 1
            confusion[cls][got] += 1
            if pred and pred.get("latency_ms") is not None:
                latencies.append(float(pred["latency_ms"]))
    acc = (100.0 * correct / total) if total else 0.0
    latencies.sort()
    p50 = latencies[len(latencies) // 2] if latencies else None
    p95 = latencies[int(len(latencies) * 0.95)] if latencies else None
    per_class = {}
    for c in CLASSES:
        row = confusion.get(c) or {}
        support = sum(row.values())
        hit = row.get(c, 0)
        per_class[c] = {
            "support": support,
            "correct": hit,
            "recall_percent": round(100.0 * hit / support, 2) if support else None,
        }
    return {
        "candidate_id": name,
        "pages": total,
        "correct": correct,
        "accuracy_percent": round(acc, 4),
        "per_class": per_class,
        "confusion": {k: dict(v) for k, v in confusion.items()},
        "latency_ms_p50": p50,
        "latency_ms_p95": p95,
        "errors": errors,
    }


def make_predict(candidate: dict[str, Any]):
    adapter = candidate.get("adapter") or "heuristic"
    if adapter == "heuristic":
        from adapters.heuristic import classify_fixture

        return lambda fx: classify_fixture(fx)
    if adapter == "external_cli":
        from adapters.external_cli import classify_fixture

        cmd = candidate.get("command")
        if not cmd:
            raise ValueError(f"{candidate.get('id')}: external_cli needs command[]")
        return lambda fx: classify_fixture(fx, list(cmd))
    raise ValueError(f"Unknown adapter: {adapter}")


def default_candidates() -> list[dict[str, Any]]:
    return [
        {
            "id": "caseforge.triage.heuristic.v1",
            "adapter": "heuristic",
            "provisional": True,
            "enabled": True,
        }
    ]


def main() -> int:
    parser = argparse.ArgumentParser(description="G-DOCS triage bake-off")
    parser.add_argument(
        "--candidates",
        type=Path,
        default=BAKEOFF_DIR / "candidates.json",
        help="JSON file of candidates (default: eval/bakeoff/candidates.json if present)",
    )
    parser.add_argument(
        "--include-draft",
        action="store_true",
        help="Include DRAFT fixtures (pre-lock experiments)",
    )
    parser.add_argument("--json", action="store_true", help="Print JSON report")
    parser.add_argument(
        "--out",
        type=Path,
        default=None,
        help="Write JSON report to path",
    )
    args = parser.parse_args()

    fixtures = load_fixtures(include_draft=args.include_draft)
    cand_path: Path | None = args.candidates
    if cand_path and cand_path.exists():
        data = load_json(cand_path)
        candidates = [
            c
            for c in (data.get("candidates") or [])
            if c.get("enabled", True) is not False
        ]
        if not candidates:
            candidates = default_candidates()
    else:
        candidates = default_candidates()

    results = []
    for c in candidates:
        cid = c.get("id") or c.get("adapter")
        try:
            predict = make_predict(c)
            results.append(score_candidate(cid, fixtures, predict))
        except Exception as e:  # noqa: BLE001
            results.append(
                {
                    "candidate_id": cid,
                    "pages": 0,
                    "correct": 0,
                    "accuracy_percent": 0.0,
                    "errors": [str(e)],
                }
            )

    report = {
        "product": "CaseForge",
        "tool": "eval/bakeoff/run_bakeoff.py",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "fixture_filter": "GOLDEN" + ("+DRAFT" if args.include_draft else ""),
        "fixture_count": len(fixtures),
        "pages_labeled": sum(len(f["expected_pages"]) for f in fixtures),
        "candidates": results,
        "policy": {
            "synthetic_excluded": True,
            "ac_6_1_still_requires_500_golden": True,
            "note": "Bake-off is comparative; AC-6.1 ship gate is separate.",
        },
    }

    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    if args.json:
        print(json.dumps(report, indent=2))
    else:
        print("CaseForge triage bake-off")
        print(
            f"Fixtures: {report['fixture_count']}  "
            f"labeled pages: {report['pages_labeled']}  "
            f"filter: {report['fixture_filter']}"
        )
        if report["pages_labeled"] == 0:
            print(
                "NOTE: No GOLDEN (or DRAFT) pages yet. "
                "Author MVP-80 per golden/G-DOCS/MVP_80_PLAN.md"
            )
        print("-" * 72)
        for r in results:
            print(
                f"{r['candidate_id']:40}  "
                f"acc={r.get('accuracy_percent', 0):6.2f}%  "
                f"pages={r.get('pages', 0)}  "
                f"errors={len(r.get('errors') or [])}"
            )
            if r.get("errors"):
                for e in r["errors"][:3]:
                    print(f"    ! {e}")
        print("-" * 72)
        print("Ship gate: python eval/checks/ac_6_1_triage_score.py")

    # Exit 0 even with 0 pages (informational tool). Exit 1 if a candidate hard-failed setup.
    if any(r.get("pages", 0) == 0 and r.get("errors") and fixtures for r in results):
        return 1
    return 0


if __name__ == "__main__":
    # Allow `python eval/bakeoff/run_bakeoff.py` imports
    sys.path.insert(0, str(BAKEOFF_DIR))
    raise SystemExit(main())
