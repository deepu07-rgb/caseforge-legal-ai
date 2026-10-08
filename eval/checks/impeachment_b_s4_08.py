#!/usr/bin/env python3
"""B-S4-08 Contradiction engine and Impeachment Kit gate.

Behaviour is proven by the Rust unit tests in src-tauri/src/impeach.rs (golden/G-IMPEACH) and
src-tauri/src/guardrails/mod.rs (golden/G-REDTEAM-S4 witness-coaching slice); pass the `cargo test --lib` log with
--cargo-log. With --kits (written by the kit test when CASEFORGE_EVIDENCE_DIR is set) every kit is validated against
the full frozen contracts/impeachment_kit.schema.json, including its OMISSION / MODEL_VERIFIED rules.
AC-S4-03.1 stays BLOCKED until the owner approves the recall/precision thresholds (Q-S4-01); measured values reported.

Usage: python eval/checks/impeachment_b_s4_08.py [--cargo-log PATH] [--kits PATH]
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
FIXTURE = ROOT / "golden" / "G-IMPEACH" / "fixtures" / "impeach_core.json"
FIXTURE_SHA256 = "626a134d6da0f9bb3cd7a780fc46c658dd0d63ea76317fd730c0eeed3815218f"  # committed before the engines (57ec899)
COACHING = ROOT / "golden" / "G-REDTEAM-S4" / "fixtures" / "witness_coaching.json"
COACHING_SHA256 = "58f23f604814f19973b91cf83aa0d2b16a219765ac35555d0bd17127e67243b9"
TESTS = {
    "recall_precision": "impeach::tests::golden_deterministic_recall_and_precision",
    "quotes_exact": "impeach::tests::every_finding_quotes_both_sides_exactly",
    "model_verified_only": "impeach::tests::unverified_model_candidates_never_render",
    "kit_contract": "impeach::tests::kit_matches_frozen_contract_shape",
    "g3_coaching": "guardrails::tests::g3_witness_coaching_suite",
}
LLM_TOKENS = re.compile(r"infer_llm|ollama|openai|llm_chat|chat_with_timeout|reqwest", re.I)
PROHIBITED_UI = ["fake", "fabricated", "court-approved", "guaranteed", "good law", "predicts", "decides", "AI judge"]


def fn_body(src: str, name: str) -> str:
    start = src.find(f"fn {name}(")
    if start < 0:
        return ""
    nxt = re.search(r"\n(?:pub(?:\(crate\))? )?fn |\n#\[tauri::command\]", src[start + 3:])
    return src[start: start + 3 + nxt.start()] if nxt else src[start:]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    ap.add_argument("--kits", type=Path, default=None)
    args = ap.parse_args()
    errors: list[str] = []
    acs: dict[str, dict] = {}

    if hashlib.sha256(FIXTURE.read_bytes()).hexdigest() != FIXTURE_SHA256:
        errors.append("G-IMPEACH fixture changed since it was pinned")
    if hashlib.sha256(COACHING.read_bytes()).hexdigest() != COACHING_SHA256:
        errors.append("G-REDTEAM-S4 witness-coaching fixture changed since it was pinned")
    fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
    planted = fixture["expected"]["planted_contradictions"]

    schema_valid = None
    if args.kits:
        try:
            import jsonschema

            v = jsonschema.Draft202012Validator(json.loads((ROOT / "contracts" / "impeachment_kit.schema.json").read_text(encoding="utf-8")))
            kits = json.loads(args.kits.read_text(encoding="utf-8"))
            for k in kits:
                v.validate(k)
            if not any(f["detection"] == "MODEL_VERIFIED" for k in kits for f in k["findings"]) or not any(f["finding_kind"] == "OMISSION" for k in kits for f in k["findings"]):
                raise ValueError("evidence must exercise OMISSION and MODEL_VERIFIED findings")
            schema_valid = True
        except Exception as e:  # noqa: BLE001 - reported as a gate failure
            errors.append(f"kit does not satisfy contracts/impeachment_kit.schema.json: {e}")
            schema_valid = False

    engine = (SRC / "impeach.rs").read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    lib = (SRC / "lib.rs").read_text(encoding="utf-8")
    deterministic_part = engine.split("pub fn model_request")[0]

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = lambda *k: all(res[x] == "ok" for x in k)  # noqa: E731
        measured = {"recall": 1.0, "precision": 1.0} if ok("recall_precision") else None  # the test asserts all planted found and zero extras
        model_path = all("verify_model_candidates(" in fn_body(lib, f) for f in ("impeach_model_scan", "witness_matrix_build")) and "proposition::verify(" in engine
        acs["AC-S4-03.2"] = {"status": "PASS" if ok("quotes_exact", "kit_contract") and schema_valid is not False else "FAIL",
                             "evidence": {"quotes_exact": res["quotes_exact"], "kit_contract": res["kit_contract"], "full_schema_validation": schema_valid}}
        acs["AC-S4-03.3"] = {"status": "PASS" if ok("model_verified_only") and model_path else "FAIL",
                             "evidence": {"test": res["model_verified_only"], "every_model_path_verifies": model_path},
                             "detail": "model candidates render only if both quotes are exact substrings of two different statements (B-S3-14 verifier); 4 of 6 fixture candidates discarded"}
        acs["AC-S4-03.4"] = {"status": "PASS" if ok("g3_coaching") else "FAIL", "evidence": {"g3_coaching": res["g3_coaching"]},
                             "scope": "witness-coaching slice of G-REDTEAM-S4 (10 refuse, 8 allow); the full suite is authored in B-S4-02 and run in B-S4-23"}
    else:
        for ac in ("AC-S4-03.2", "AC-S4-03.3", "AC-S4-03.4"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
        measured = None

    thresholds = {t["id"]: t for t in json.loads((ROOT / "contracts" / "stage4_thresholds.json").read_text(encoding="utf-8"))["thresholds"]}
    rec, pre = thresholds["S4.impeach.deterministic_recall_min"], thresholds["S4.impeach.deterministic_precision_min"]
    approved = rec["approval"] is not None and pre["approval"] is not None
    acs["AC-S4-03.1"] = {
        "status": "BLOCKED" if not approved else ("PASS" if measured and measured["recall"] >= rec["approval"]["approved_value"] and measured["precision"] >= pre["approval"]["approved_value"] else "FAIL"),
        "measured": measured, "planted_contradictions": planted, "proposed": {"recall": rec["proposed"], "precision": pre["proposed"]},
        "reason": None if approved else "thresholds awaiting owner approval (Q-S4-01); measured on the small synthetic G-IMPEACH fixture only",
    }

    ui = (ROOT / "src" / "components" / "WitnessView.tsx").read_text(encoding="utf-8")
    kit_ts = (ROOT / "src" / "lib" / "impeachKit.ts").read_text(encoding="utf-8")
    app = (ROOT / "src" / "App.tsx").read_text(encoding="utf-8")
    structure = {
        "deterministic_layer_model_free": not LLM_TOKENS.search(engine),
        "commands_registered": all(re.search(rf"generate_handler!\[[^\]]*\b{c}\b", lib, re.S) for c in ("impeach_finding_status", "impeach_model_scan", "witness_matrix_build")),
        "export_carries_g4_footer": "EXPORT_FOOTER_LINES" in kit_ts and "renderDocx(" in ui and "renderPdf(" in ui,
        "mock_court_consumes_kit": "seed={courtSeed}" in app and "Witness_Cross_Examination" in app and "crossOutline(" in ui,
        "ui_vocabulary_honest": not any(re.search(rf"(?i)['\"“>][^'\"”<]*\b{re.escape(w)}\b", ui + kit_ts) for w in PROHIBITED_UI),
        "deterministic_part_has_no_proposition_bypass": "MODEL_VERIFIED" not in deterministic_part.split("pub fn verify_model_candidates")[0],
    }
    acs["STRUCTURE"] = {"status": "PASS" if all(structure.values()) else "FAIL", "checks": structure}

    out = {"batch": "B-S4-08", "acceptance": acs, "errors": errors}
    (ROOT / "eval" / "reports" / "b_s4_08_impeachment.json").write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8", newline="\n")
    for k, r in acs.items():
        print(f"{r['status']:8} {k}")
    for e in errors:
        print(f"FAIL     {e}")
    return 1 if errors or any(r["status"] == "FAIL" for r in acs.values()) else 0


if __name__ == "__main__":
    sys.exit(main())
