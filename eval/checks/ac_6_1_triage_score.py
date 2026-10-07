#!/usr/bin/env python3
"""AC-6.1 triage accuracy scorer (B-S1-05).

Ship rules (PRD §6.1 + golden/POLICY.md):
  - Only fixtures with meta/known_answer status == GOLDEN count toward ship.
  - PASS requires >= triage_min_accuracy_percent on >= triage_golden_page_count pages.
  - SYNTHETIC / DRAFT fixtures are scored only as dev diagnostics (never ship PASS).
  - Silent PASS is forbidden.

Exit codes:
  0 = ship PASS (GOLDEN metric met) OR intentional SKIP with no GOLDEN (use --allow-skip)
  1 = ship FAIL (GOLDEN metric missed) OR structural error
  2 = SKIP / not ready (no GOLDEN pages) when --allow-skip not set for harness use

When imported, call score_ac_6_1() -> dict with status PASS|FAIL|SKIP.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
GDOCS = ROOT / "golden" / "G-DOCS" / "fixtures"
CONSTANTS = ROOT / "contracts" / "constants.json"
SIDECAR_DIR = ROOT / "sidecars" / "docpipe"

CLASSES = {
    "typed_english",
    "other",
    "mixed",
    "clean_handwriting",
    "messy_handwriting",
    "non_text",
}


def load_json(path: Path) -> Any:
    with path.open(encoding="utf-8-sig") as f:
        return json.load(f)


def thresholds() -> tuple[float, int]:
    data = load_json(CONSTANTS)
    pipe = data.get("document_pipeline") or {}
    min_acc = float(pipe.get("triage_min_accuracy_percent", 92))
    min_pages = int(pipe.get("triage_golden_page_count", 500))
    return min_acc, min_pages


def iter_fixtures() -> list[Path]:
    if not GDOCS.is_dir():
        return []
    return sorted(p for p in GDOCS.iterdir() if p.is_dir())


def load_fixture(dir_path: Path) -> dict[str, Any] | None:
    meta_p = dir_path / "meta.json"
    ans_p = dir_path / "known_answer.json"
    if not meta_p.exists() or not ans_p.exists():
        return None
    meta = load_json(meta_p)
    ans = load_json(ans_p)
    status = meta.get("status") or ans.get("status") or "UNKNOWN"
    triage = ans.get("triage") or {}
    # Support single-page or multi-page known answers
    pages: list[dict[str, Any]] = []
    if isinstance(triage, list):
        for t in triage:
            pages.append(
                {
                    "page_index": int(t.get("page_index", 0)),
                    "class": t.get("class") or t.get("triage_class"),
                }
            )
    elif isinstance(triage, dict) and triage.get("class"):
        pages.append(
            {
                "page_index": int(triage.get("page_index", 0)),
                "class": triage.get("class"),
            }
        )
    elif isinstance(ans.get("pages"), list):
        for t in ans["pages"]:
            pages.append(
                {
                    "page_index": int(t.get("page_index", 0)),
                    "class": t.get("class") or t.get("triage_class"),
                }
            )
    inputs = meta.get("inputs") or {}
    content_name = inputs.get("content_file") or ""
    content_path = (dir_path / content_name) if content_name else None
    return {
        "id": meta.get("fixture_id") or dir_path.name,
        "dir": dir_path,
        "status": status,
        "authoritative": bool(meta.get("authoritative") or ans.get("authoritative")),
        "pages": pages,
        "page_stub": dir_path / "page_stub.txt",
        "content_file": content_path,
    }


def classify_fixture(fx: dict[str, Any]) -> list[dict[str, Any]]:
    sys.path.insert(0, str(SIDECAR_DIR))
    from triage import pages_from_document, triage_pages  # type: ignore

    content_path = fx.get("content_file")
    if isinstance(content_path, Path) and content_path.exists():
        import base64

        raw = content_path.read_bytes()
        content_b64 = base64.b64encode(raw).decode("ascii")
        pages = pages_from_document(
            content_base64=content_b64,
            text=None,
            media_type=content_path.suffix.lstrip(".") or "bin",
            filename=content_path.name,
        )
        return triage_pages(pages)

    text = ""
    if fx["page_stub"].exists():
        text = fx["page_stub"].read_text(encoding="utf-8-sig")
    pages = pages_from_document(
        content_base64=None,
        text=text,
        media_type="text/plain",
        filename="page_stub.txt",
    )
    return triage_pages(pages)


def score_bucket(fixtures: list[dict[str, Any]]) -> dict[str, Any]:
    total = 0
    correct = 0
    details: list[dict[str, Any]] = []
    for fx in fixtures:
        preds = classify_fixture(fx)
        pred_by_idx = {int(p["page_index"]): p for p in preds}
        for exp in fx["pages"]:
            cls = exp.get("class")
            if cls not in CLASSES:
                details.append(
                    {
                        "fixture": fx["id"],
                        "page_index": exp.get("page_index"),
                        "error": f"invalid expected class {cls}",
                    }
                )
                continue
            total += 1
            pred = pred_by_idx.get(int(exp["page_index"]))
            got = pred.get("triage_class") if pred else None
            ok = got == cls
            if ok:
                correct += 1
            details.append(
                {
                    "fixture": fx["id"],
                    "page_index": exp["page_index"],
                    "expected": cls,
                    "predicted": got,
                    "match": ok,
                    "confidence": pred.get("confidence") if pred else None,
                }
            )
    acc = (100.0 * correct / total) if total else 0.0
    return {
        "pages": total,
        "correct": correct,
        "accuracy_percent": round(acc, 4),
        "details": details,
    }


def score_ac_6_1() -> dict[str, Any]:
    min_acc, min_pages = thresholds()
    all_fx = []
    for d in iter_fixtures():
        fx = load_fixture(d)
        if fx and fx["pages"]:
            all_fx.append(fx)

    golden = [f for f in all_fx if f["status"] == "GOLDEN"]
    synthetic = [
        f
        for f in all_fx
        if f["status"] in {"SYNTHETIC_PLACEHOLDER", "SYNTHETIC_DEVELOPMENT", "DRAFT"}
    ]

    golden_score = score_bucket(golden) if golden else {
        "pages": 0,
        "correct": 0,
        "accuracy_percent": 0.0,
        "details": [],
    }
    synth_score = score_bucket(synthetic) if synthetic else {
        "pages": 0,
        "correct": 0,
        "accuracy_percent": 0.0,
        "details": [],
    }

    result: dict[str, Any] = {
        "ac": "AC-6.1",
        "thresholds": {
            "min_accuracy_percent": min_acc,
            "min_golden_pages": min_pages,
        },
        "classifier_id": "caseforge.triage.heuristic.v1",
        "golden": {
            "fixtures": len(golden),
            **{k: golden_score[k] for k in ("pages", "correct", "accuracy_percent")},
        },
        "synthetic_dev_only": {
            "fixtures": len(synthetic),
            **{k: synth_score[k] for k in ("pages", "correct", "accuracy_percent")},
            "note": "Never used for ship PASS (POLICY.md).",
        },
        "mismatches": [d for d in golden_score["details"] if not d.get("match", True)],
    }

    g_pages = golden_score["pages"]
    g_acc = golden_score["accuracy_percent"]

    if g_pages == 0:
        result["status"] = "SKIP"
        result["reason"] = (
            f"No GOLDEN G-DOCS pages authored. Need >= {min_pages} GOLDEN pages "
            f"at >= {min_acc}% (PRD §6.1). Synthetic pages={synth_score['pages']} "
            f"dev_acc={synth_score['accuracy_percent']}% (not ship)."
        )
        return result

    if g_pages < min_pages:
        result["status"] = "FAIL"
        result["reason"] = (
            f"GOLDEN page count {g_pages} < required {min_pages}. "
            f"Accuracy so far {g_acc}% (need >= {min_acc}%)."
        )
        return result

    if g_acc < min_acc:
        result["status"] = "FAIL"
        result["reason"] = (
            f"GOLDEN accuracy {g_acc}% < {min_acc}% on {g_pages} pages "
            f"({golden_score['correct']}/{g_pages}). Fix classifier before B-S1-06."
        )
        return result

    result["status"] = "PASS"
    result["reason"] = (
        f"GOLDEN accuracy {g_acc}% on {g_pages} pages "
        f"(>= {min_acc}% and >= {min_pages} pages)."
    )
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description="AC-6.1 triage scorer")
    parser.add_argument("--json", action="store_true", help="Print full JSON report")
    parser.add_argument(
        "--allow-skip",
        action="store_true",
        help="Exit 0 on SKIP (for structural CI). Default exit 2 on SKIP.",
    )
    args = parser.parse_args()
    report = score_ac_6_1()
    if args.json:
        print(json.dumps(report, indent=2))
    else:
        print(f"{report['status']}  AC-6.1  {report['reason']}")
        g = report["golden"]
        s = report["synthetic_dev_only"]
        print(
            f"  golden: pages={g['pages']} correct={g['correct']} "
            f"acc={g['accuracy_percent']}%"
        )
        print(
            f"  synthetic_dev: pages={s['pages']} correct={s['correct']} "
            f"acc={s['accuracy_percent']}% (not ship)"
        )
        if report.get("mismatches"):
            print(f"  mismatches: {len(report['mismatches'])}")
            for m in report["mismatches"][:10]:
                print(
                    f"    - {m.get('fixture')} p{m.get('page_index')}: "
                    f"exp={m.get('expected')} got={m.get('predicted')}"
                )

    status = report["status"]
    if status == "PASS":
        return 0
    if status == "FAIL":
        return 1
    # SKIP
    return 0 if args.allow_skip else 2


if __name__ == "__main__":
    sys.exit(main())
