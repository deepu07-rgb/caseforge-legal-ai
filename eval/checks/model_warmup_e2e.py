#!/usr/bin/env python3
"""The local model is warm when a rehearsal starts (found by driving the real app, 2026-10-11).

Mock Court's model turn has a 15-second limit. On a 4 GB laptop GPU the 7B model takes about 11 s just to load, so the first turn after the
model had gone idle always missed the limit and the screen fell back to the deterministic rehearsal, which looked like "the model is too slow".
Measured on that machine: a cold turn is over 15 s, a warm one 3-4 s. Two rules fix the cold start without keeping the model resident:

  * the backend is told to keep the model for the choreography's own idle window (IDLE_UNLOAD_S = 120 s), not its 5-minute default;
  * opening the Mock Court screen asks the host to load the model (`llm_warm`), with the SAME context size the turns use (the backend
    reloads a model whose context size differs, which made a first attempt at this no faster than nothing).

This gate checks those rules (and the stdin rule below) in the source; the timing was measured on the real model and is in docs/E2E_RUN_2026-10-11.md.

Usage: python eval/checks/model_warmup_e2e.py
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def main() -> int:
    errors: list[str] = []
    py = (ROOT / "sidecars" / "infer_llm" / "infer_llm.py").read_text(encoding="utf-8")
    choreo = (ROOT / "src-tauri" / "src" / "choreography" / "mod.rs").read_text(encoding="utf-8")
    lib = (ROOT / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    llm_rs = (ROOT / "src-tauri" / "src" / "infer_llm" / "mod.rs").read_text(encoding="utf-8")
    ipc = (ROOT / "src" / "lib" / "ipc.ts").read_text(encoding="utf-8")
    court = (ROOT / "src" / "components" / "MockCourtView.tsx").read_text(encoding="utf-8")

    idle = int(re.search(r"pub const IDLE_UNLOAD_S: u64 = (\d+);", choreo).group(1))
    m = re.search(r'KEEP_ALIVE = os\.environ\.get\("CASEFORGE_LLM_KEEP_ALIVE", "(\d+)s"\)', py)
    if not m or int(m.group(1)) != idle:
        errors.append(f"the model keep-alive is not the choreography idle window ({idle} s)")
    if py.count('"stream": False, "keep_alive": KEEP_ALIVE') != 3:
        errors.append("chat, translate and warm must all pass the keep-alive")
    warm = py[py.index('if method == "llm.warm"'): py.index('if method == "llm.translate"')]
    if '"options": {"num_ctx": int(GENERATION["num_ctx"])}' not in warm or '"prompt": ""' not in warm:
        errors.append("the warm-up must load with the turns' own context size and generate nothing")
    if "never an error" not in llm_rs.split("pub fn warm()")[0][-200:] or "unwrap_or_else(|e| json!({\"warmed\": false" not in llm_rs:
        errors.append("a failed warm-up must not be an error")
    if "#[tauri::command(async)]\nfn llm_warm()" not in lib.replace("\r\n", "\n") or "            llm_warm,\n" not in lib.replace("\r\n", "\n"):
        errors.append("llm_warm must be an async (off-main-thread) registered command")
    # A rehearsal prompt carries the case record and is longer than a Windows command line (~32,000 characters): os error 206 "filename or
    # extension too long", seen as a degraded rehearsal with a spawn error. The request must go in on stdin.
    run_rpc = llm_rs.split("fn run_rpc", 1)[1].split("fn ", 1)[0]
    if '.arg(&payload)' in run_rpc or ".stdin(Stdio::piped())" not in run_rpc or "write_all(payload.as_bytes())" not in run_rpc:
        errors.append("the model request must be sent on stdin, not as a command-line argument")
    if "sys.stdin.buffer.read().decode(\"utf-8\")" not in py:
        errors.append("the sidecar must read its request from stdin as UTF-8")
    if "export async function llmWarm" not in ipc or "void llmWarm()" not in court:
        errors.append("opening the Mock Court screen must ask for the model to be loaded")

    report = {"batch": "B-S4-e2e", "keep_alive_seconds": int(m.group(1)) if m else None, "choreography_idle_seconds": idle, "errors": errors}
    out = ROOT / "eval" / "reports" / "model_warmup_e2e.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(("PASS" if not errors else "FAIL") + f" model warm-up rules; keep-alive {report['keep_alive_seconds']} s = idle window {idle} s")
    for e in errors:
        print("  ERROR:", e)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
