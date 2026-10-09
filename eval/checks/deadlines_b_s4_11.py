#!/usr/bin/env python3
"""B-S4-11 Deadline & Limitation Watch gate.

Behaviour is proven by Rust unit tests in src-tauri/src/deadlines.rs against golden/G-DEADLINE (synthetic rules with
hand-derived dates) plus property-style tests (leap years, month ends, consecutive holidays, calendar export); pass the
`cargo test --lib` log with --cargo-log. With --records (written by the golden test when CASEFORGE_EVIDENCE_DIR is set)
every computed record is validated against the frozen contracts/deadline_record.schema.json.
AC-S4-06.1 needs legal-editor answers on real signed rows (Q-S4-02/Q-S4-11): it stays BLOCKED; the shipped rules
(caseforge.india.limitation-rules) are DEVELOPMENT_ONLY and the UI and records say so.

Usage: python eval/checks/deadlines_b_s4_11.py [--cargo-log PATH] [--records PATH]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "golden" / "G-DEADLINE" / "fixtures" / "deadline_core.json"
FIXTURE_SHA256 = "ae4ac55da440f46a928ee5b4a5065561a1ff89ee1bf8dbd9beb017e81b675fbb"  # committed before the engine (810a929)
TESTS = {
    "golden": "deadlines::tests::golden_deadline_cases",
    "arithmetic": "deadlines::tests::date_arithmetic_matches_day_by_day_counting",
    "months": "deadlines::tests::month_arithmetic_clamps_and_is_monotonic",
    "roll": "deadlines::tests::roll_forward_never_lands_on_a_closed_day_or_before_the_start",
    "ics": "deadlines::tests::calendar_export_is_a_local_valid_looking_ics",
    "triggers": "deadlines::tests::chronology_events_become_triggers_without_guessing",
    "shipped_rules": "deadlines::tests::shipped_limitation_rules_install_cite_the_right_lines_and_compute",
}
LLM_TOKENS = re.compile(r"infer_llm|ollama|openai|llm_chat|chat_with_timeout|reqwest|TcpStream|http://|https://", re.I)
PROHIBITED_UI = ["fake", "fabricated", "court-approved", "guaranteed", "good law", "valid law", "predicts", "decides"]
ALL_REASONS = {"TRIGGER_DATE_IMPRECISE", "TRIGGER_DATE_CONFLICT", "CALENDAR_MISSING", "EXCLUSION_FACT_UNCONFIRMED", "RULE_WINDOW_AMBIGUOUS"}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    ap.add_argument("--records", type=Path, default=None)
    args = ap.parse_args()
    errors: list[str] = []
    acs: dict[str, dict] = {}

    if hashlib.sha256(FIXTURE.read_bytes()).hexdigest() != FIXTURE_SHA256:
        errors.append("G-DEADLINE fixture changed since it was pinned")
    fx = json.loads(FIXTURE.read_text(encoding="utf-8"))
    cases = fx["cases"]
    expected_reasons = {r for c in cases if c["expect"] for r in c["expect"]["reasons"]}
    expected_errors = {c["error"] for c in cases if c["error"]}
    if expected_reasons != ALL_REASONS:
        errors.append(f"golden does not exercise every uncertainty reason: missing {sorted(ALL_REASONS - expected_reasons)}")
    for need in ("CF_DEADLINE_TRIGGER_UNKNOWN", "CF_DEADLINE_NO_AUTHORITY", "CF_DEADLINE_NO_LOCATOR", "CF_DEADLINE_NO_RULE"):
        if need not in expected_errors:
            errors.append(f"golden does not exercise refusal {need}")

    schema_valid = None
    if args.records:
        try:
            import jsonschema

            schema = json.loads((ROOT / "contracts" / "deadline_record.schema.json").read_text(encoding="utf-8"))
            validator = jsonschema.Draft202012Validator(schema)
            records = json.loads(args.records.read_text(encoding="utf-8"))
            for r in records:
                validator.validate(r)
            kinds = {r["result"]["kind"] for r in records}
            reasons = {u for r in records for u in r["result"]["uncertainty_reasons"]}
            if kinds != {"DATE", "RANGE"} or reasons != ALL_REASONS:
                raise ValueError(f"evidence must exercise DATE and RANGE and every reason (kinds {kinds}, reasons {sorted(reasons)})")
            schema_valid = True
        except Exception as e:  # noqa: BLE001 - reported as a gate failure
            errors.append(f"deadline record does not satisfy contracts/deadline_record.schema.json: {e}")
            schema_valid = False

    src = (ROOT / "src-tauri" / "src" / "deadlines.rs").read_text(encoding="utf-8")
    engine = src.split("#[cfg(test)]")[0]
    ui = (ROOT / "src" / "components" / "DeadlinesView.tsx").read_text(encoding="utf-8")
    lib = (ROOT / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    static = {
        "engine_has_no_model_or_network": not LLM_TOKENS.search(engine),
        "every_record_is_labelled_verify_and_g9": '"verify_label": "VERIFY"' in engine and '"guardrail": "G9"' in engine,
        "refuses_without_locator_or_authority": "CF_DEADLINE_NO_LOCATOR" in engine and "CF_DEADLINE_NO_AUTHORITY" in engine,
        "ui_shows_verify_and_the_working": "VERIFY" in ui and "Show the working" in ui,
        "ui_explains_every_range_reason": all(r in ui for r in ALL_REASONS),
        "ui_vocabulary_honest": not any(re.search(rf"(?i)['\"`>][^'\"`<]*\b{re.escape(w)}\b", ui) for w in PROHIBITED_UI),
        "calendar_export_is_local": "fn deadlines_export_ics" in lib and "http" not in engine.split("pub fn to_ics")[1],
        "development_rules_are_flagged": "not yet signed off" in ui and "unsigned_rules" in lib,
    }
    for k, v in static.items():
        if not v:
            errors.append(f"static check failed: {k}")

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = all(v == "ok" for v in res.values())
        acs["AC-S4-06.2"] = {
            "status": "PASS" if ok and schema_valid is not False and expected_reasons == ALL_REASONS else "FAIL",
            "evidence": {"golden": res["golden"], "reasons_exercised": sorted(expected_reasons), "triggers": res["triggers"], "full_schema_validation": schema_valid},
            "detail": "An imprecise or conflicting trigger date, an unconfirmed exclusion, a missing calendar year and a rule that changes inside the trigger range each give a RANGE with its reason; never a single date.",
        }
        acs["AC-S4-06.3"] = {
            "status": "PASS" if ok and schema_valid is not False and not errors else "FAIL",
            "evidence": {"refusals_in_golden": sorted(expected_errors), "static": static, "shipped_rules": res["shipped_rules"]},
            "detail": "No deadline without a rule, an authority and a trigger locator; every record carries VERIFY and G9; every rule row cites the exact line it comes from.",
        }
        acs["AC-S4-06.arithmetic"] = {"status": "PASS" if all(res[k] == "ok" for k in ("arithmetic", "months", "roll", "ics")) else "FAIL",
                                      "evidence": {k: res[k] for k in ("arithmetic", "months", "roll", "ics")},
                                      "detail": "Property tests: day counting against an independent walk (1996-2032), month-end and leap-day clamping, monotonic month arithmetic, roll-forward never on a closed day, calendar-file format."}
    else:
        for ac in ("AC-S4-06.2", "AC-S4-06.3"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
    acs["AC-S4-06.1"] = {
        "status": "BLOCKED",
        "reason": "100% match with legal-editor answers needs real signed rule rows and court calendars (Q-S4-02, Q-S4-11). The shipped limitation rules (caseforge.india.limitation-rules) are DEVELOPMENT_ONLY; no court calendar is installed, so shipped rules give a range, not a single date.",
    }
    report = {"batch": "B-S4-11", "fixture_sha256": FIXTURE_SHA256, "cases": len(cases), "acs": acs, "errors": errors}
    print(json.dumps(report, indent=2))
    out = ROOT / "eval" / "reports" / "b_s4_11_deadlines.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
