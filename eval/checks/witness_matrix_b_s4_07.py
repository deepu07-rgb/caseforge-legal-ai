#!/usr/bin/env python3
"""B-S4-07 Witness resolution and Statement Matrix gate.

Behaviour is proven by the Rust unit tests in src-tauri/src/witness.rs against golden/G-IMPEACH; pass the
`cargo test --lib` log with --cargo-log (without it the ACs are BLOCKED). With --matrix (written by the matrix test
when CASEFORGE_EVIDENCE_DIR is set) the matrix is validated against the full frozen contracts/witness_matrix.schema.json.

Usage: python eval/checks/witness_matrix_b_s4_07.py [--cargo-log PATH] [--matrix PATH]
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
TESTS = {
    "proposals": "witness::tests::proposals_match_headers",
    "resolution": "witness::tests::auto_resolution_then_counsel_merge_and_split",
    "survival": "witness::tests::counsel_decisions_survive_rerun",
    "no_false_merge": "witness::tests::similar_names_are_never_merged_automatically",
    "cells_exact": "witness::tests::every_matrix_cell_is_an_exact_located_excerpt",
}
LLM_TOKENS = re.compile(r"infer_llm|ollama|openai|llm_chat|chat_with_timeout|reqwest", re.I)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    ap.add_argument("--matrix", type=Path, default=None)
    args = ap.parse_args()
    errors: list[str] = []
    acs: dict[str, dict] = {}

    if hashlib.sha256(FIXTURE.read_bytes()).hexdigest() != FIXTURE_SHA256:
        errors.append("G-IMPEACH fixture changed since it was pinned")

    schema_valid = None
    if args.matrix:
        try:
            import jsonschema

            schema = json.loads((ROOT / "contracts" / "witness_matrix.schema.json").read_text(encoding="utf-8"))
            jsonschema.Draft202012Validator(schema).validate(json.loads(args.matrix.read_text(encoding="utf-8")))
            schema_valid = True
        except Exception as e:  # noqa: BLE001 - reported as a gate failure
            errors.append(f"matrix does not satisfy contracts/witness_matrix.schema.json: {e}")
            schema_valid = False

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = lambda *k: all(res[x] == "ok" for x in k)  # noqa: E731
        acs["AC-S4-03.2(matrix)"] = {"status": "PASS" if ok("cells_exact") and schema_valid is not False else "FAIL", "evidence": {"cells_exact": res["cells_exact"], "full_schema_validation": schema_valid},
                                     "detail": "every matrix cell is the exact stored substring at its page/span or page:line locator"}
        acs["MERGE_SPLIT_SURVIVES_RERUN"] = {"status": "PASS" if ok("resolution", "survival", "no_false_merge") else "FAIL", "evidence": {k: res[k] for k in ("resolution", "survival", "no_false_merge")}}
        acs["STATEMENT_PROPOSALS"] = {"status": "PASS" if ok("proposals") else "FAIL", "evidence": {"proposals": res["proposals"]}}
    else:
        for ac in ("AC-S4-03.2(matrix)", "MERGE_SPLIT_SURVIVES_RERUN", "STATEMENT_PROPOSALS"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}

    engine = (SRC / "witness.rs").read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    lib = (SRC / "lib.rs").read_text(encoding="utf-8")
    app = (ROOT / "src" / "App.tsx").read_text(encoding="utf-8")
    vocab = json.loads((ROOT / "packs" / "builtin" / "statement_kinds.v1.json").read_text(encoding="utf-8"))
    commands = ["witness_propose", "witness_register_list", "witness_register_save", "witness_register_remove", "witness_decision_add", "witness_decision_revoke", "witness_matrix_build"]
    structure = {
        "engine_model_free": not LLM_TOKENS.search(engine),
        "statement_kinds_are_data": "include_str!(\"../../packs/builtin/statement_kinds.v1.json\")" in engine and not re.search(r"examination\[- \]in|section\|sec", engine) and len(vocab["kinds"]) >= 10,
        "commands_registered": all(re.search(rf"generate_handler!\[[^\]]*\b{c}\b", lib, re.S) for c in commands),
        "register_survives_rerun": "merge_proposal(existing.get(&id), proposal)" in lib,
        "migration_present": (ROOT / "src-tauri" / "migrations" / "case_017_witness_store.sql").is_file() and "case_017_witness_store.sql" in (SRC / "vault" / "mod.rs").read_text(encoding="utf-8"),
        "ui_wired": "<WitnessView" in app and re.search(r'PRIMARY_NAV_IDS[^;]*"witnesses"', app) is not None,
    }
    acs["STRUCTURE"] = {"status": "PASS" if all(structure.values()) else "FAIL", "checks": structure}

    out = {"batch": "B-S4-07", "acceptance": acs, "errors": errors}
    (ROOT / "eval" / "reports" / "b_s4_07_witness_matrix.json").write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8", newline="\n")
    for k, r in acs.items():
        print(f"{r['status']:8} {k}")
    for e in errors:
        print(f"FAIL     {e}")
    return 1 if errors or any(r["status"] == "FAIL" for r in acs.values()) else 0


if __name__ == "__main__":
    sys.exit(main())
