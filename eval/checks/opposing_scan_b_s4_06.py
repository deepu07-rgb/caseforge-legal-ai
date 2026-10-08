#!/usr/bin/env python3
"""B-S4-06 Opposing Filing Scan gate.

Behaviour (AC-S4-02.1, 02.3, 02.4, contract shape) is proven by the Rust unit tests in src-tauri/src/opposition.rs
and src-tauri/src/docpipe/mod.rs against golden/G-OPPOSING; pass the `cargo test --lib` log with --cargo-log.
Without it those ACs are BLOCKED. With --record (written by the contract test when CASEFORGE_EVIDENCE_DIR is set)
the scan output is validated against the full frozen JSON schema. AC-S4-02.2 (record-conflict recall >= threshold)
stays BLOCKED until the owner approves the threshold (Q-S4-01); the measured value is reported.

Usage: python eval/checks/opposing_scan_b_s4_06.py [--cargo-log PATH] [--record PATH]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src-tauri" / "src"
FIXTURE = ROOT / "golden" / "G-OPPOSING" / "fixtures" / "respondent_reply.json"
FIXTURE_SHA256 = "c5f31ddf00cbbae7424926b033d68e171aa493b7dc0f8f7858fac0184573ec12"  # committed before the engine (4563f9e)
TESTS = {
    "golden": "opposition::tests::golden_respondent_reply",
    "injection": "opposition::tests::instruction_inside_the_filing_changes_nothing",
    "contract": "opposition::tests::record_matches_frozen_contract_shape",
    "stale_graph": "opposition::tests::stale_graph_is_detected",
    "barrier": "docpipe::external_filing_barrier_tests::external_filing_pages_never_reach_graph_extraction",
}
LLM_TOKENS = re.compile(r"infer_llm|ollama|openai|llm_chat|chat_with_timeout|reqwest", re.I)
GRAPH_WRITES = re.compile(r"case_graph_put|case_graph_correct_item|argument_tree_put|counsel_fact")
PROHIBITED_UI = ["fake", "fabricated", "court-approved", "guaranteed", "good law", "predicts", "decides"]


def fn_body(src: str, name: str) -> str:
    start = src.find(f"fn {name}(")
    if start < 0:
        return ""
    nxt = re.search(r"\n(?:pub(?:\(crate\))? )?fn |\n#\[tauri::command\]", src[start + 3:])
    return src[start: start + 3 + nxt.start()] if nxt else src[start:]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    ap.add_argument("--record", type=Path, default=None)
    args = ap.parse_args()
    errors: list[str] = []
    acs: dict[str, dict] = {}

    if hashlib.sha256(FIXTURE.read_bytes()).hexdigest() != FIXTURE_SHA256:
        errors.append("G-OPPOSING fixture changed since it was pinned")
    fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
    planted = fixture["expected"]["planted_conflicts"]

    engine = (SRC / "opposition.rs").read_text(encoding="utf-8")
    docpipe = (SRC / "docpipe" / "mod.rs").read_text(encoding="utf-8")
    lib = (SRC / "lib.rs").read_text(encoding="utf-8")
    vault = (SRC / "vault" / "mod.rs").read_text(encoding="utf-8")
    engine_code = engine.split("#[cfg(test)]")[0]

    # Leak barrier structure: every case-graph extraction from documents filters EXTERNAL_FILING pages first,
    # and the other matter-fact feeds (model context, proposition support) skip EXTERNAL_FILING documents.
    sites = [m.start() for m in re.finditer(r"extract_case_graph\(", docpipe.split("#[cfg(test)]\nmod external_filing_barrier_tests")[0])]
    sites = [s for s in sites if not docpipe[max(0, s - 7):s].endswith("pub fn ") and not docpipe[max(0, s - 3):s].endswith("fn ")]
    barrier_sites = [("exclude_external_pages(" in docpipe[max(0, s - 1200):s]) for s in sites]
    structure_leak = {
        "graph_extraction_sites_found": len(sites) >= 2,
        "every_extraction_site_filtered": bool(barrier_sites) and all(barrier_sites),
        "barrier_fails_closed": "CF_AUDIT_EXTERNAL_ASSERTION_LEAK" in fn_body(docpipe, "external_doc_ids"),
        "llm_context_excludes_external": "EXTERNAL_FILING_TAG" in fn_body(vault, "case_llm_context"),
        "proposition_support_excludes_external": "EXTERNAL_FILING_TAG" in fn_body(lib, "case_llm_support_sources"),
        "scan_writes_nothing": not GRAPH_WRITES.search(engine_code) and not GRAPH_WRITES.search(fn_body(lib, "opposition_scan")),
    }

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = lambda *keys: all(res[k] == "ok" for k in keys)  # noqa: E731
        acs["AC-S4-02.1"] = {"status": "PASS" if ok("golden") else "FAIL", "evidence": {"golden": res["golden"]},
                             "detail": "planted fabricated citation -> NOT_IN_CORPUS, one-word misquotation -> MISQUOTED, real citation VERIFIED"}
        measured = planted / planted if ok("golden") else None  # the test asserts every planted conflict, two locators each, no extras
        leak_ok = ok("barrier", "stale_graph", "contract") and all(structure_leak.values())
        acs["AC-S4-02.3"] = {"status": "PASS" if leak_ok else "FAIL", "evidence": {k: res[k] for k in ("barrier", "stale_graph", "contract")}, "checks": structure_leak}
        acs["AC-S4-02.4"] = {"status": "PASS" if ok("injection") and not LLM_TOKENS.search(engine_code) else "FAIL",
                             "evidence": {"injection": res["injection"], "engine_model_free": not LLM_TOKENS.search(engine_code)},
                             "detail": "findings identical with and without the embedded instruction; the scan never calls a model"}
    else:
        for ac in ("AC-S4-02.1", "AC-S4-02.3", "AC-S4-02.4"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
        measured = None

    thresholds = json.loads((ROOT / "contracts" / "stage4_thresholds.json").read_text(encoding="utf-8"))
    t = next(x for x in thresholds["thresholds"] if x["id"] == "S4.opposing.record_conflict_recall_min")
    acs["AC-S4-02.2"] = {
        "status": "BLOCKED" if t["approval"] is None else ("PASS" if measured is not None and measured >= t["approval"]["approved_value"] else "FAIL"),
        "measured_recall": measured, "planted_conflicts": planted, "proposed_threshold": t["proposed"],
        "reason": "threshold awaiting owner approval (Q-S4-01); measured on the small synthetic G-OPPOSING fixture only" if t["approval"] is None else None,
    }

    if args.record:
        try:
            import jsonschema

            schema = json.loads((ROOT / "contracts" / "external_filing.schema.json").read_text(encoding="utf-8"))
            record = json.loads(args.record.read_text(encoding="utf-8"))
            jsonschema.Draft202012Validator(schema).validate(record)
            schema_valid = True
        except Exception as e:  # noqa: BLE001 - reported as a gate failure
            errors.append(f"scan record does not satisfy contracts/external_filing.schema.json: {e}")
            schema_valid = False
    else:
        schema_valid = None

    ui = (ROOT / "src" / "components" / "OppositionView.tsx").read_text(encoding="utf-8")
    app = (ROOT / "src" / "App.tsx").read_text(encoding="utf-8")
    structure = {
        "command_registered": "fn opposition_scan(" in lib and re.search(r"generate_handler!\[[^\]]*\bopposition_scan\b", lib, re.S) is not None,
        "ui_wired": "oppositionScan(" in ui and "<OppositionView" in app and re.search(r'PRIMARY_NAV_IDS[^;]*"opposition"', app) is not None,
        "ui_vocabulary_honest": not any(re.search(rf"(?i)['\"“][^'\"”]*\b{re.escape(w)}\b", ui) for w in PROHIBITED_UI),
        "full_schema_validation": schema_valid if schema_valid is not None else "not run (pass --record)",
    }
    acs["STRUCTURE"] = {"status": "PASS" if all(v is True for k, v in structure.items() if k != "full_schema_validation") and schema_valid is not False else "FAIL", "checks": structure}

    out = {"batch": "B-S4-06", "acceptance": acs, "errors": errors}
    (ROOT / "eval" / "reports" / "b_s4_06_opposing_scan.json").write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8", newline="\n")
    for k, r in acs.items():
        print(f"{r['status']:8} {k}")
    for e in errors:
        print(f"FAIL     {e}")
    return 1 if errors or any(r["status"] == "FAIL" for r in acs.values()) else 0


if __name__ == "__main__":
    sys.exit(main())
