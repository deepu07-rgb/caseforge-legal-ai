#!/usr/bin/env python3
"""One command for the Stage 4 ship check (`npm run ship:stage4`).

Steps, in order, each recorded in eval/reports/stage4_run.json:
  1. production_build   npm run build (tsc -b && vite build)
  2. rust_tests         cargo test --lib, with CASEFORGE_EVIDENCE_DIR set so the engines write the evidence the gates read
                        (skipped if --cargo-log names a log you already have; build the Rust crate from a shell that has the MSVC and
                        Perl environment, on Windows `. src-tauri/msvc-env.ps1` first)
  3. ci_gates           every `python eval/checks/*.py` line of .github/workflows/checks.yml (the Stage 1-4 gates, contract/migration
                        validation, the licence and network checks), except the checkpoint and the ship gate
  4. ship_gate          eval/checks/ship_gate_b_s4_23.py --strict: the full red-team roll-up, acceptance, sign-off, waivers, network,
                        size, controlled matters and the verdict, written to eval/reports/stage4_ship_gate.json

The exit code is the ship gate's: 0 only for READY FOR CONTROLLED TESTING or better. Nothing here edits a result or waives a gate.

Usage: python eval/run_stage4.py [--cargo-log PATH] [--evidence-dir DIR] [--skip-build]
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPORTS = ROOT / "eval" / "reports"


def load(path: Path):
    spec = importlib.util.spec_from_file_location(path.stem, path)
    mod = importlib.util.module_from_spec(spec)  # type: ignore[arg-type]
    sys.modules[path.stem] = mod
    spec.loader.exec_module(mod)  # type: ignore[union-attr]
    return mod


def run(cmd: list[str], **kw) -> tuple[int, float]:
    t = time.time()
    r = subprocess.run(cmd, cwd=ROOT, **kw)
    return r.returncode, round(time.time() - t, 1)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None, help="use an existing `cargo test --lib` log instead of running the tests")
    ap.add_argument("--evidence-dir", type=Path, default=ROOT / "evidence")
    ap.add_argument("--skip-build", action="store_true", help="do not run the production build (the report will say it was not run)")
    args = ap.parse_args()
    args.evidence_dir.mkdir(parents=True, exist_ok=True)
    steps: list[dict] = []

    if not args.skip_build:
        npm = shutil.which("npm.cmd") or shutil.which("npm")
        if npm:
            code, secs = run([npm, "run", "build"])
            steps.append({"name": "production_build", "exit": code, "seconds": secs})
        else:
            steps.append({"name": "production_build", "exit": 127, "seconds": 0, "note": "npm not found"})

    log = args.cargo_log
    if log is None:
        log = ROOT / "cargo-test.log"
        env = {**os.environ, "CASEFORGE_EVIDENCE_DIR": str(args.evidence_dir.resolve())}
        with log.open("w", encoding="utf-8") as fh:
            code, secs = run(["cargo", "test", "--manifest-path", "src-tauri/Cargo.toml", "--lib"], stdout=fh, stderr=subprocess.STDOUT, env=env)
        steps.append({"name": "rust_tests", "exit": code, "seconds": secs, "log": log.name})
    else:
        steps.append({"name": "rust_tests", "exit": 0, "seconds": 0, "log": str(log), "note": "existing log used"})

    wc = load(ROOT / "eval" / "checks" / "stage4_wave_c.py")
    t = time.time()
    gates = wc.run_gates(str(log), str(args.evidence_dir))
    steps.append({"name": "ci_gates", "exit": 0 if all(g["exit"] == 0 for g in gates) else 1, "seconds": round(time.time() - t, 1), "gates": gates})

    REPORTS.mkdir(parents=True, exist_ok=True)
    (REPORTS / "stage4_run.json").write_text(json.dumps({"generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"), "steps": steps}, indent=2) + "\n", encoding="utf-8")

    code, secs = run([sys.executable, "-I", "eval/checks/ship_gate_b_s4_23.py", "--cargo-log", str(log), "--evidence-dir", str(args.evidence_dir), "--strict"])
    print(f"ship gate exit {code} in {secs} s - see eval/reports/stage4_ship_gate.json")
    return code


if __name__ == "__main__":
    sys.exit(main())
