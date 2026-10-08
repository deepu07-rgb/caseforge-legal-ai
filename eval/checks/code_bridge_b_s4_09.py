#!/usr/bin/env python3
"""B-S4-09 Code Bridge gate.

Behaviour is proven by Rust unit tests in src-tauri/src/code_bridge.rs against golden/G-CODEBRIDGE (synthetic codes);
pass the `cargo test --lib` log with --cargo-log. With --annotations (written by the contract test when
CASEFORGE_EVIDENCE_DIR is set) every annotation is validated against the full frozen statute_mapping schema.
AC-S4-04.1 requires legal-editor-signed real mappings (Q-S4-02): it stays BLOCKED until signed STATUTE_SUCCESSION rows
for real codes ship in an official pack; the engine's agreement with the synthetic signed fixture is reported.

Usage: python eval/checks/code_bridge_b_s4_09.py [--cargo-log PATH] [--annotations PATH]
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
FIXTURE = ROOT / "golden" / "G-CODEBRIDGE" / "fixtures" / "code_bridge_core.json"
FIXTURE_SHA256 = "cf805c6d4f435bd7a6dfb2153aa08820ce5fe0193164af4d7167df092ce4cc36"  # committed before the engine (c3f8005)
TESTS = {
    "known_dates": "code_bridge::tests::golden_mappings_with_known_dates",
    "missing_dates": "code_bridge::tests::missing_key_date_always_needs_counsel_input",
    "boundary": "code_bridge::tests::switch_date_boundary",
    "contract": "code_bridge::tests::annotations_match_frozen_contract_shape",
    "reference_forms": "code_bridge::tests::real_world_reference_forms_are_found",
}
LLM_TOKENS = re.compile(r"infer_llm|ollama|openai|llm_chat|chat_with_timeout|reqwest", re.I)
PROHIBITED_UI = ["fake", "fabricated", "court-approved", "guaranteed", "good law", "valid law", "predicts", "decides"]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    ap.add_argument("--annotations", type=Path, default=None)
    args = ap.parse_args()
    errors: list[str] = []
    acs: dict[str, dict] = {}

    if hashlib.sha256(FIXTURE.read_bytes()).hexdigest() != FIXTURE_SHA256:
        errors.append("G-CODEBRIDGE fixture changed since it was pinned")
    fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))

    schema_valid = None
    if args.annotations:
        try:
            import jsonschema

            v = jsonschema.Draft202012Validator(json.loads((ROOT / "contracts" / "statute_mapping.schema.json").read_text(encoding="utf-8")))
            annotations = json.loads(args.annotations.read_text(encoding="utf-8"))
            for a in annotations:
                v.validate(a)
            statuses = {a["governing"]["status"] for a in annotations}
            if not {"DETERMINED", "NEEDS_COUNSEL_INPUT", "UNKNOWN"} <= statuses or not any(a["equivalence"] == "MODIFIED" for a in annotations):
                raise ValueError("evidence must exercise DETERMINED, NEEDS_COUNSEL_INPUT, UNKNOWN and MODIFIED")
            schema_valid = True
        except Exception as e:  # noqa: BLE001 - reported as a gate failure
            errors.append(f"annotation does not satisfy contracts/statute_mapping.schema.json: {e}")
            schema_valid = False

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = lambda *k: all(res[x] == "ok" for x in k)  # noqa: E731
        planted = fixture["scenarios"][1]["planted_missing_date_refs"]
        acs["AC-S4-04.2"] = {"status": "PASS" if ok("missing_dates") and schema_valid is not False else "FAIL", "evidence": {"missing_dates": res["missing_dates"]},
                             "measured": f"{planted}/{planted} planted missing-date references -> NEEDS_COUNSEL_INPUT" if ok("missing_dates") else None}
        acs["AC-S4-04.3"] = {"status": "PASS" if ok("known_dates", "contract") and schema_valid is not False else "FAIL",
                             "evidence": {"known_dates": res["known_dates"], "contract": res["contract"], "full_schema_validation": schema_valid},
                             "detail": "MODIFIED mapping carries the exact old and new authoritative texts; the validator rejects a MODIFIED row without both"}
        engine_agrees = ok("known_dates", "boundary", "reference_forms")
    else:
        for ac in ("AC-S4-04.2", "AC-S4-04.3"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
        engine_agrees = None
    acs["AC-S4-04.1"] = {
        "status": "BLOCKED",
        "reason": "needs legal-editor-signed STATUTE_SUCCESSION rows for real codes in an official pack (Q-S4-02); none are installed",
        "engine_matches_synthetic_signed_fixture": engine_agrees,
    }

    engine = "\n".join(l for l in (SRC / "code_bridge.rs").read_text(encoding="utf-8").split("#[cfg(test)]")[0].splitlines() if not l.strip().startswith("//"))
    lib = (SRC / "lib.rs").read_text(encoding="utf-8")
    panel = (ROOT / "src" / "components" / "CodeBridgePanel.tsx").read_text(encoding="utf-8")
    drafting = (ROOT / "src" / "components" / "DraftingView.tsx").read_text(encoding="utf-8")
    structure = {
        "engine_model_free": not LLM_TOKENS.search(engine),
        "no_legal_values_in_code": not re.search(r'"(IPC|BNS|CrPC|BNSS|BSA|Indian Penal Code)"', engine),
        "commands_registered": all(re.search(rf"generate_handler!\[[^\]]*\b{c}\b", lib, re.S) for c in ("code_bridge_annotate", "code_bridge_set_key_date", "code_bridge_key_dates", "rule_tables_status")),
        "audit_integration": "code_bridge::add_audit_notes(" in lib,
        "ui_wired": "<CodeBridgePanel" in drafting and "codeBridgeAnnotate(" in panel,
        "ui_vocabulary_honest": not any(re.search(rf"(?i)['\"“>][^'\"”<]*\b{re.escape(w)}\b", panel) for w in PROHIBITED_UI),
    }
    acs["STRUCTURE"] = {"status": "PASS" if all(structure.values()) else "FAIL", "checks": structure}

    out = {"batch": "B-S4-09", "acceptance": acs, "errors": errors}
    (ROOT / "eval" / "reports" / "b_s4_09_code_bridge.json").write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8", newline="\n")
    for k, r in acs.items():
        print(f"{r['status']:8} {k}")
    for e in errors:
        print(f"FAIL     {e}")
    return 1 if errors or any(r["status"] == "FAIL" for r in acs.values()) else 0


if __name__ == "__main__":
    sys.exit(main())
