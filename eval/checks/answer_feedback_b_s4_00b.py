#!/usr/bin/env python3
"""B-S4-00b per-answer Mock Court feedback gate.

Structural checks run anywhere. The behavioural ACs (00b.1-00b.3, and the language/record-ref part of 00b.4) are proven
by the Rust unit tests in src-tauri/src/answer_feedback.rs against the golden fixture; pass the cargo test log with
--cargo-log to count them. Without it those ACs are BLOCKED, never PASS.

Usage: python eval/checks/answer_feedback_b_s4_00b.py [--cargo-log PATH]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

import jsonschema

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "golden" / "G-ANSWER-FEEDBACK" / "fixtures" / "partition_appeal.json"
FIXTURE_SHA256 = "68d39eaac3975eb8e08a04c353437ea0e6a8e3ba5d9f52e2484e381e8fd45219"  # authored before the engine; editing it after seeing results fails this gate
ENGINE = ROOT / "src-tauri" / "src" / "answer_feedback.rs"
RUST_TESTS = ["answer_feedback::tests::golden_partition_appeal_turns_match_exactly", "answer_feedback::tests::date_and_amount_parsing"]
LLM_TOKENS = re.compile(r"infer_llm|ollama|openai|llm_chat|chat_with_timeout|reqwest", re.I)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    args = ap.parse_args()
    acs: dict[str, dict] = {}
    errors: list[str] = []

    schema = json.loads((ROOT / "contracts" / "answer_feedback.schema.json").read_text(encoding="utf-8"))
    jsonschema.Draft202012Validator.check_schema(schema)
    fixture_bytes = FIXTURE.read_bytes()
    if hashlib.sha256(fixture_bytes).hexdigest() != FIXTURE_SHA256:
        errors.append("golden fixture changed since it was pinned")
    fixture = json.loads(fixture_bytes)
    if fixture.get("status") != "GOLDEN":
        errors.append("fixture is not GOLDEN")

    # Behavioural ACs come from the Rust tests (golden fixture) when a cargo log is supplied.
    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        results = {t: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for t in RUST_TESTS}
        status = "PASS" if all(v == "ok" for v in results.values()) else "FAIL"
        for ac in ("AC-S4-00b.1", "AC-S4-00b.2", "AC-S4-00b.3"):
            acs[ac] = {"status": status, "evidence": results}
    else:
        for ac in ("AC-S4-00b.1", "AC-S4-00b.2", "AC-S4-00b.3"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}

    # AC-00b.4: deterministic engine, honest flags, record refs required by contract.
    engine = ENGINE.read_text(encoding="utf-8")
    det = not LLM_TOKENS.search(engine) and 'model_used: false' in engine and 'outcome_prediction: false' in engine
    contract_ok = schema["properties"]["model_used"].get("const") is False and schema["properties"]["outcome_prediction"].get("const") is False
    tests_ok = acs["AC-S4-00b.1"]["status"]
    acs["AC-S4-00b.4"] = {"status": ("PASS" if det and contract_ok and tests_ok == "PASS" else "FAIL" if not (det and contract_ok) else tests_ok),
                          "engine_model_free": det, "contract_flags": contract_ok, "language_and_refs_by_rust_test": tests_ok}

    # AC-00b.5: returned with every grounded turn and rendered under counsel's message.
    lib = (ROOT / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    resp = (ROOT / "src-tauri" / "src" / "mock_court_retrieval.rs").read_text(encoding="utf-8")
    view = (ROOT / "src" / "components" / "MockCourtView.tsx").read_text(encoding="utf-8")
    ipc = (ROOT / "src" / "lib" / "ipc.ts").read_text(encoding="utf-8")
    wiring = {
        "graded_before_next_challenge": "answer_feedback::evaluate(" in lib and "answer_feedback::previous_challenge(" in lib,
        "challenge_remembered": "answer_feedback::remember_challenge(" in lib,
        "returned_in_response": "answer_feedback: Some(feedback)" in lib and "pub answer_feedback: Option<crate::answer_feedback::AnswerFeedback>" in resp,
        "typed_in_ui": "answer_feedback?: AnswerFeedback | null" in ipc,
        "rendered_under_counsel_message": 'm.role === "user" && answerFeedback[m.id] && <AnswerFeedbackPanel' in view,
    }
    acs["AC-S4-00b.5"] = {"status": "PASS" if all(wiring.values()) else "FAIL", "wiring": wiring}

    report = {"batch": "B-S4-00b", "acceptance": acs, "errors": errors}
    (ROOT / "eval" / "reports" / "b_s4_00b_answer_feedback.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
    for ac, r in acs.items():
        print(f"{r['status']:8} {ac}")
    for e in errors:
        print(f"FAIL     {e}")
    return 1 if errors or any(r["status"] == "FAIL" for r in acs.values()) else 0


if __name__ == "__main__":
    sys.exit(main())
