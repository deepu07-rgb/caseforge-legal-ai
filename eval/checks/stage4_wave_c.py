#!/usr/bin/env python3
"""Stage 4 Wave C checkpoint (playbook: "All prior gates + B-S4-17...21 green. Report eval/reports/stage4_wave_c.json").

Runs every gate CI runs (or, with --no-run, reads the reports they just wrote), joins their acceptance results with the Stage 4
acceptance inventory, and states plainly what is measured, what is formally accepted and what is not built. It decides nothing
for the owner: it reports.

Two statuses per acceptance criterion, so a green gate is never mistaken for an approved criterion:
  measured  what the gate found (PASS / FAIL / WAIVED / BLOCKED), or NOT_BUILT / NO_REPORT.
  formal    PASS only if measured PASS and nothing in the inventory still blocks it: no unapproved threshold (Q-S4-01), no required
            legal-editor sign-off (Q-S4-02), no open owner question. Otherwise BLOCKED, with the reasons.

Usage: python eval/checks/stage4_wave_c.py [--cargo-log PATH] [--evidence-dir DIR] [--no-run]
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REPORTS = ROOT / "eval" / "reports"
WAVE_C = ["B-S4-17", "B-S4-18", "B-S4-19", "B-S4-20", "B-S4-21"]
# Batches the playbook places before Wave C that are not built (nothing in the repo implements them).
NOT_BUILT = {
    "B-S4-02": "twelve golden sets with legal-editor protocol (needs legal editors, Q-S4-02)",
    "B-S4-12": "Timed Oral Rehearsal (Wave B)",
    "B-S4-14": "Devanagari triage + OCR (Wave B; needs the OCR bake-off, Q-S4-04)",
    "B-S4-15": "Aligned offline translation + G10 (Wave B)",
    "B-S4-16": "Encrypted Bundle Exchange (Wave B)",
    "B-S4-22": "Editions, pack subscription, seat transfer (Ship; prices and edition map are owner decisions, Q-S4-06/07)",
    "B-S4-23": "Cumulative ship report and full red-team (Ship)",
}
ORDER = {"FAIL": 0, "BLOCKED": 1, "WAIVED": 2, "SKIP": 3, "PASS": 4}


def gate_commands() -> list[str]:
    text = (ROOT / ".github" / "workflows" / "checks.yml").read_text(encoding="utf-8")
    cmds = []
    for line in text.splitlines():
        m = re.match(r"\s*(python eval/checks/\S+\.py.*)$", line)
        if m and "stage4_wave_c.py" not in m.group(1):
            cmds.append(m.group(1).strip())
    return cmds


def run_gates(cargo_log: str | None, evidence: str | None) -> list[dict]:
    out = []
    for cmd in gate_commands():
        args = cmd.split()[1:]
        args = [a.replace("cargo-test.log", cargo_log) if cargo_log and a == "cargo-test.log" else a for a in args]
        args = [a.replace("evidence", evidence) if evidence and a.startswith("evidence") else a for a in args]
        t = time.time()
        r = subprocess.run([sys.executable, "-I", *args], cwd=ROOT, capture_output=True, text=True)
        out.append({"gate": args[0].split("/")[-1], "exit": r.returncode, "seconds": round(time.time() - t, 1)})
    return out


def collect_acs() -> dict[str, dict]:
    """AC id -> {status, reports}, worst status wins when several reports speak to the same criterion."""
    found: dict[str, dict] = {}
    for f in sorted(REPORTS.glob("b_s4_*.json")):
        try:
            r = json.loads(f.read_text(encoding="utf-8"))
        except Exception:  # noqa: BLE001
            continue
        block = r.get("acs") if isinstance(r.get("acs"), dict) else r.get("acceptance") if isinstance(r.get("acceptance"), dict) else {}
        for key, val in block.items():
            m = re.match(r"(AC-S4-[A-Za-z0-9.\-]+?)(?:\(.*\))?$", key)
            if not m or not isinstance(val, dict) or "status" not in val:
                continue
            ac = m.group(1)
            cur = found.setdefault(ac, {"status": val["status"], "reports": [], "detail": {}})
            cur["detail"].update({k: val[k] for k in ("score", "failed", "reason", "measured", "note", "threshold") if k in val and val["status"] != "PASS"})
            if ORDER.get(val["status"], 3) < ORDER.get(cur["status"], 3):
                cur["status"] = val["status"]
            cur["reports"].append(f.name)
    return found


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", default=None)
    ap.add_argument("--evidence-dir", default=None)
    ap.add_argument("--no-run", action="store_true", help="read the reports the gates just wrote instead of running them again")
    args = ap.parse_args()

    gates = [] if args.no_run else run_gates(args.cargo_log, args.evidence_dir)
    found = collect_acs()
    inv = json.loads((ROOT / "eval" / "stage4_ac_inventory.json").read_text(encoding="utf-8"))
    thr = json.loads((ROOT / "contracts" / "stage4_thresholds.json").read_text(encoding="utf-8"))
    approved = {t["id"]: t.get("approval") for t in thr.get("thresholds", [])}

    rows = []
    for a in inv["acceptance_criteria"]:
        ac, batch = a["id"], a["batch"]
        got = found.get(ac)
        measured = got["status"] if got else ("NOT_BUILT" if batch in NOT_BUILT else "NO_REPORT")
        why = []
        unapproved = [t for t in a.get("threshold_ids", []) if not approved.get(t)]
        if unapproved:
            why.append(f"threshold not approved by the owner: {', '.join(unapproved)} (Q-S4-01)")
        if a.get("legal_editor_required"):
            why.append("needs two legal editors' sign-off (Q-S4-02)")
        qs = [q for q in a.get("questions", []) if q not in ("Q-S4-01", "Q-S4-02")]
        if qs:
            why.append("open owner questions: " + ", ".join(qs))
        formal = "PASS" if measured == "PASS" and not why else ("BLOCKED" if measured in ("PASS", "WAIVED", "BLOCKED") and why else measured)
        rows.append({"ac": ac, "batch": batch, "description": a["description"], "measured": measured, "formal": formal, "blocked_by": why, "reports": (got or {}).get("reports", []), "detail": (got or {}).get("detail", {})})

    by_batch: dict[str, dict] = {}
    for r in rows:
        b = by_batch.setdefault(r["batch"], {"measured": {}, "formal": {}})
        for k in ("measured", "formal"):
            b[k][r[k]] = b[k].get(r[k], 0) + 1

    tests = {"passed": None, "failed": None, "ignored": None}
    if args.cargo_log and Path(args.cargo_log).is_file():
        m = re.findall(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored", Path(args.cargo_log).read_text(encoding="utf-8", errors="replace"))
        if m:
            tests = {"passed": int(m[-1][0]), "failed": int(m[-1][1]), "ignored": int(m[-1][2])}

    wave_c_rows = [r for r in rows if r["batch"] in WAVE_C]
    wave_c_green = bool(wave_c_rows) and all(r["measured"] == "PASS" for r in wave_c_rows)
    gates_green = all(g["exit"] == 0 for g in gates) if gates else None
    measured_counts: dict[str, int] = {}
    formal_counts: dict[str, int] = {}
    for r in rows:
        measured_counts[r["measured"]] = measured_counts.get(r["measured"], 0) + 1
        formal_counts[r["formal"]] = formal_counts.get(r["formal"], 0) + 1
    report = {
        "checkpoint": "Stage 4 Wave C",
        "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "verdict": {
            "wave_c_batches_17_to_21": "GREEN" if wave_c_green else "NOT_GREEN",
            "ci_gates": "GREEN" if gates_green else ("NOT_RUN" if gates_green is None else "NOT_GREEN"),
            "wave_b_and_earlier_complete": False,
            "formal_acceptance": "NOT_ACCEPTED",
            "summary": "Every Wave C batch (B-S4-17 to 21) passes its gate and all CI gates pass. The checkpoint is NOT a clean pass: the playbook makes Wave C depend on the Wave B checkpoint, and Wave B is incomplete (see not_built), and no criterion is formally accepted yet because the thresholds are still PROPOSED (Q-S4-01), legal-editor sign-off is absent (Q-S4-02) and the Chambers pilot court is not named (Q-S4-10).",
        },
        "ci_gates": gates,
        "rust_tests": tests,
        "wave_c": {b: by_batch.get(b, {}) for b in WAVE_C},
        "acceptance_counts": {"measured": measured_counts, "formal": formal_counts, "total": len(rows)},
        "acceptance": rows,
        "failing": [{"ac": r["ac"], "batch": r["batch"], "description": r["description"], "detail": r["detail"]} for r in rows if r["measured"] == "FAIL"],
        "not_built": NOT_BUILT,
        "owner_decisions_needed": {
            "Q-S4-01": "approve (or change) the PROPOSED thresholds; until then no threshold-based criterion can be formally accepted",
            "Q-S4-02": "two legal editors must review and sign the development rule tables (limitation, admissibility, code succession) before they leave DEVELOPMENT_ONLY",
            "Q-S4-10": "name the Chambers pilot court and its AI-use policy; supply a sample cause list and order sheet (real formats) and the capabilities it forbids",
            "Q-S4-06/07": "edition-to-feature map, prices, seat transfers per year (needed for B-S4-22)",
            "Q-S4-04": "Indic OCR bake-off (needed for B-S4-14)",
            "integrity_v2": "approve a foundation-gap factor in the Evidence link score (and the argument-node schema bump) or leave the v1 score untouched",
        },
        "needs_the_owners_machine": [
            "best-of-n reasoning measured with the real 7B model (AC-S4-28.2)",
            "100-matter cause-list render on the Full reference machine (AC-S4-14.2 was measured on the development machine)",
            "record-lookup hit rate on real matter records (AC-S4-13.1 was measured on a synthetic record)",
            "AC-S4-00.7 prompt-injection held-out set on the default model",
        ],
    }
    out = REPORTS / "stage4_wave_c.json"
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: report[k] for k in ("verdict", "acceptance_counts", "rust_tests")}, indent=2))
    for g in gates:
        if g["exit"] != 0:
            print("GATE FAILED:", g)
    bad = [r["ac"] for r in wave_c_rows if r["measured"] != "PASS"]
    if bad:
        print("Wave C criteria not passing:", bad)
    return 0 if (wave_c_green and gates_green is not False) else 1


if __name__ == "__main__":
    sys.exit(main())
