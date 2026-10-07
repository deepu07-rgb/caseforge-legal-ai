#!/usr/bin/env python3
"""B-S4-00 held-out model fitness: cases frozen before any small-model prompt work.

Runs golden/G-MODEL-HELDOUT/cases.json through the infer-llm sidecar for the selected tier.
The case file hash is pinned; editing cases after seeing results fails this gate.
A model may be promoted to the DEFAULT tier only if it passes BOTH this gate and B-S3-13.

Usage: python eval/checks/model_heldout_b_s4_00.py [--tier default|addon] [--label NAME]
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CASES = ROOT / "golden" / "G-MODEL-HELDOUT" / "cases.json"
PINNED_SHA256 = "be48594aa07c105b4f59f40711ea7a4e696989ba2d2a068fb2366a1708a055b7"  # v1.1.0 (see change_log in cases.json)
MIN_SCORE = 0.85  # same bar as the B-S3-13 fitness gate


def score_text(case: dict, text: str, error: object = None) -> bool:
    low = text.lower()
    return (error is None
            and all(t.lower() in low for t in case["must_include_all"])
            and (not case["must_include_any"] or any(t.lower() in low for t in case["must_include_any"]))
            and not any(t.lower() in low for t in case["must_exclude"]))


def rescore(report_path: Path, cases: list[dict]) -> int:
    """Re-grade stored model answers against the current pinned case file (no model calls)."""
    report = json.loads(report_path.read_text(encoding="utf-8"))
    by_id = {c["id"]: c for c in cases}
    for r in report["results"]:
        r["passed"] = score_text(by_id[r["id"]], r.get("content") or "", r.get("error"))
    score = sum(r["passed"] for r in report["results"]) / len(report["results"])
    critical_ok = all(r["passed"] for r in report["results"] if r["critical"])
    report.update({"case_file_sha256": hashlib.sha256(CASES.read_bytes()).hexdigest(), "rescored": True, "score": round(score, 4),
                   "critical_pass": critical_ok, "passed": report.get("controlled_beta_ready") is True and critical_ok and score >= MIN_SCORE})
    report_path.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    print(f"{'PASS' if report['passed'] else 'HALT'}  rescored {report_path.name} model={report.get('model')} prompt={report.get('prompt_version')} "
          f"score={score:.2f} critical={critical_ok} failed={[r['id'] for r in report['results'] if not r['passed']]}")
    return 0 if report["passed"] else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rescore", type=Path, default=None, help="re-grade a stored held-out report instead of calling the model")
    parser.add_argument("--tier", choices=["default", "addon"], default="default")
    parser.add_argument("--label", default=None, help="report suffix, e.g. a candidate name")
    args = parser.parse_args()
    raw = CASES.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != PINNED_SHA256:
        print(f"FAIL  held-out case file changed (sha256 {digest} != pinned {PINNED_SHA256})")
        return 1
    if args.rescore:
        return rescore(args.rescore, json.loads(raw)["cases"])
    os.environ["CASEFORGE_LLM_TIER"] = args.tier
    spec = importlib.util.spec_from_file_location("cf_llm_heldout", ROOT / "sidecars" / "infer_llm" / "infer_llm.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)  # type: ignore[union-attr]
    status = module.handle({"jsonrpc": "2.0", "id": 1, "method": "llm.status", "params": {}})["result"]
    results = []
    for case in json.loads(raw)["cases"]:
        response = module.handle({"jsonrpc": "2.0", "id": 1, "method": "llm.chat", "params": case["params"]})
        text = ((response.get("result") or {}).get("content") or "")
        passed = score_text(case, text, response.get("error"))
        results.append({"id": case["id"], "category": case["category"], "critical": case["critical"], "passed": passed,
                        "content": text, "error": response.get("error")})
    score = sum(r["passed"] for r in results) / len(results)
    critical_ok = all(r["passed"] for r in results if r["critical"])
    passed = status.get("controlled_beta_ready") is True and critical_ok and score >= MIN_SCORE
    label = args.label or status.get("model", "unknown").replace(":", "_").replace(".", "_")
    out = ROOT / "eval" / "reports" / f"b_s4_00_heldout_{label}.json"
    out.write_text(json.dumps({"batch": "B-S4-00", "set": "G-MODEL-HELDOUT", "case_file_sha256": digest, "tier": args.tier,
                               "model": status.get("model"), "model_digest": status.get("model_digest"),
                               "prompt_version": status.get("prompt_version"), "controlled_beta_ready": status.get("controlled_beta_ready"),
                               "passed": passed, "score": round(score, 4), "critical_pass": critical_ok, "results": results}, indent=2, ensure_ascii=False) + "\n",
                   encoding="utf-8", newline="\n")
    print(f"{'PASS' if passed else 'HALT'}  held-out {status.get('model')} prompt={status.get('prompt_version')} score={score:.2f} critical={critical_ok} "
          f"failed={[r['id'] for r in results if not r['passed']]} report={out.name}")
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
