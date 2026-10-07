#!/usr/bin/env python3
"""B-S4-00 default-model bake-off harness.

For each candidate manifest in eval/bakeoff/candidates/*.json: temporarily place it in the
DEFAULT slot (contracts/supported_models.json), run the live B-S3-13 fitness gate and the
live B-S3-16 Mock Court gate, archive both reports under eval/bakeoff/, then ALWAYS restore
the qualified manifest. A candidate only becomes the default when a human edits the contract
after reviewing the evidence; this tool never promotes anything.

Usage: python eval/tools/bakeoff_b_s4_00.py [candidate-stem ...]
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT = ROOT / "contracts" / "supported_models.json"
CANDIDATES = ROOT / "eval" / "bakeoff" / "candidates"
OUT = ROOT / "eval" / "bakeoff"
REPORTS = ROOT / "eval" / "reports"


def run(script: str, *args: str) -> int:
    env = {k: v for k, v in os.environ.items() if k != "CASEFORGE_LLM_TIER"}
    return subprocess.run([sys.executable, f"eval/checks/{script}", *args], cwd=ROOT, env=env, timeout=1800).returncode


def summarise(stem: str) -> dict:
    fit = json.loads((OUT / f"b_s4_00_{stem}_fitness.json").read_text(encoding="utf-8"))
    mc = json.loads((OUT / f"b_s4_00_{stem}_b_s3_16.json").read_text(encoding="utf-8"))
    held_path = OUT / f"b_s4_00_{stem}_heldout.json"
    held = json.loads(held_path.read_text(encoding="utf-8")) if held_path.is_file() else {}
    return {
        "prompt": fit.get("status", {}).get("prompt_version"),
        "heldout_passed": held.get("passed"), "heldout_score": held.get("score"),
        "heldout_failed": [r["id"] for r in held.get("results", []) if not r["passed"]],
        "candidate": stem, "fitness_passed": fit.get("passed"), "score": fit.get("score"), "critical_pass": fit.get("critical_pass"),
        "failed_cases": [c["name"] for c in fit.get("checks", []) if not c["passed"]],
        "mock_court_passed": mc.get("passed"), "mock_court_turns": f"{sum(bool(t.get('passed')) for t in mc.get('live_turns', []))}/{len(mc.get('live_turns', []))}",
        "mock_court_failed": [t.get("name") for t in mc.get("live_turns", []) if not t.get("passed")],
        "mean_warm_ms": (fit.get("performance") or {}).get("mean_warm_latency_ms"), "resident_gb": (fit.get("performance") or {}).get("resident_model_size_gb"),
    }


def main(argv: list[str]) -> int:
    stems = argv or sorted(p.stem for p in CANDIDATES.glob("*.json"))
    backup = DEFAULT.read_bytes()
    # The live gates write the default-tier reports; keep the qualified default's evidence intact.
    shared_reports = [REPORTS / "b_s3_13_model_eval.json", REPORTS / "b_s3_13_model_eval.md", REPORTS / "b_s3_16_grounded_mock_court.json"]
    report_backup = {p: p.read_bytes() for p in shared_reports if p.is_file()}
    results = []
    try:
        for stem in stems:
            shutil.copyfile(CANDIDATES / f"{stem}.json", DEFAULT)
            run("model_fitness_b_s3_13.py", "--tier", "default")
            shutil.copyfile(REPORTS / "b_s3_13_model_eval.json", OUT / f"b_s4_00_{stem}_fitness.json")
            run("grounded_mock_court_b_s3_16.py")
            shutil.copyfile(REPORTS / "b_s3_16_grounded_mock_court.json", OUT / f"b_s4_00_{stem}_b_s3_16.json")
            run("model_heldout_b_s4_00.py", "--tier", "default", "--label", stem)
            shutil.copyfile(REPORTS / f"b_s4_00_heldout_{stem}.json", OUT / f"b_s4_00_{stem}_heldout.json")
            results.append(summarise(stem))
            print(json.dumps(results[-1]), flush=True)
    finally:
        DEFAULT.write_bytes(backup)
        for path, data in report_backup.items():
            path.write_bytes(data)
        print("restored qualified default manifest and its gate reports", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
