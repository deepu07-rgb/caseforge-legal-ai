#!/usr/bin/env python3
"""B-S4-04 Citation & Quote Audit gate.

Behaviour (AC-S4-01.1, 01.2, contract shape, real-pack check) is proven by the Rust unit tests in
src-tauri/src/audit.rs against golden/G-AUDIT; pass the `cargo test --lib` log with --cargo-log. Without it those
ACs are BLOCKED. AC-S4-01.3 (extraction recall >= threshold) stays BLOCKED until the owner approves the threshold
(Q-S4-01); the measured value is reported.

Usage: python eval/checks/citation_audit_b_s4_04.py [--cargo-log PATH]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "golden" / "G-AUDIT" / "fixtures" / "audit_core.json"
FIXTURE_SHA256 = "677e5b335fc72db3ede1364d3e4ef810bbaebbb43cd72617d0e418534224c791"  # committed before the engine (5902203)
ENGINE = ROOT / "src-tauri" / "src" / "audit.rs"
TESTS = {
    "synthetic": "audit::tests::golden_synthetic_drafts",
    "real_pack": "audit::tests::golden_real_india_partition_pack",
    "contract": "audit::tests::report_matches_frozen_contract_shape",
}
LLM_TOKENS = re.compile(r"infer_llm|ollama|openai|llm_chat|chat_with_timeout|reqwest", re.I)
PROHIBITED_UI = ["fake", "fabricated", "court-approved", "guaranteed", "good law"]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    args = ap.parse_args()
    errors: list[str] = []
    acs: dict[str, dict] = {}

    if hashlib.sha256(FIXTURE.read_bytes()).hexdigest() != FIXTURE_SHA256:
        errors.append("G-AUDIT fixture changed since it was pinned")
    fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
    planted = sum(d["planted_citations"] for d in fixture["drafts"])

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = all(v == "ok" for v in res.values())
        acs["AC-S4-01.1"] = {"status": "PASS" if ok else "FAIL", "evidence": res, "detail": "fabricated citations/quotations never VERIFIED; NOT_IN_CORPUS carries no resolved source"}
        acs["AC-S4-01.2"] = {"status": "PASS" if ok else "FAIL", "evidence": res, "detail": "single-word misquotation -> MISQUOTED with DELETE/INSERT word diff"}
        recall_measured = 1.0 if res["synthetic"] == "ok" else None  # the test asserts detected == planted
    else:
        for ac in ("AC-S4-01.1", "AC-S4-01.2"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
        recall_measured = None

    thresholds = json.loads((ROOT / "contracts" / "stage4_thresholds.json").read_text(encoding="utf-8"))
    t = next(x for x in thresholds["thresholds"] if x["id"] == "S4.audit.citation_extraction_recall_min")
    acs["AC-S4-01.3"] = {
        "status": "BLOCKED" if t["approval"] is None else ("PASS" if recall_measured is not None and recall_measured >= t["approval"]["approved_value"] else "FAIL"),
        "measured_recall": recall_measured, "planted_citations": planted, "proposed_threshold": t["proposed"],
        "reason": "threshold awaiting owner approval (Q-S4-01); measured on the small synthetic G-AUDIT fixture only" if t["approval"] is None else None,
    }

    engine = ENGINE.read_text(encoding="utf-8")
    ui = (ROOT / "src" / "components" / "AuditPanel.tsx").read_text(encoding="utf-8")
    lib = (ROOT / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    structure = {
        "engine_model_free": not LLM_TOKENS.search(engine),
        "command_registered": "fn audit_draft(" in lib and re.search(r"generate_handler!\[[^\]]*\baudit_draft\b", lib, re.S) is not None,
        "ui_wired": "auditDraft(" in ui and "<AuditPanel" in (ROOT / "src" / "components" / "DraftingView.tsx").read_text(encoding="utf-8"),
        "ui_vocabulary_honest": not any(re.search(rf"(?i)['\"][^'\"]*\b{re.escape(w)}\b", ui) for w in PROHIBITED_UI),
    }
    acs["STRUCTURE"] = {"status": "PASS" if all(structure.values()) else "FAIL", "checks": structure}

    out = {"batch": "B-S4-04", "acceptance": acs, "errors": errors}
    (ROOT / "eval" / "reports" / "b_s4_04_citation_audit.json").write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8", newline="\n")
    for k, r in acs.items():
        print(f"{r['status']:8} {k}")
    for e in errors:
        print(f"FAIL     {e}")
    return 1 if errors or any(r["status"] == "FAIL" for r in acs.values()) else 0


if __name__ == "__main__":
    sys.exit(main())
