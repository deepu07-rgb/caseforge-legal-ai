#!/usr/bin/env python3
"""B-S4-10 Admissibility & Foundation Gaps gate.

Behaviour is proven by Rust unit tests in src-tauri/src/foundation.rs against golden/G-ADMISS (synthetic rows, hand-derived
statuses) and by an install-and-check test of the shipped development pack caseforge.india.admissibility-rules; pass the
`cargo test --lib` log with --cargo-log. With --checks (written by the golden test when CASEFORGE_EVIDENCE_DIR is set)
every foundation check is validated against the frozen contracts/foundation_check.schema.json.

The integrity v2 *score* (a foundation factor in the Evidence link) is frozen in the contract as needing owner approval and a
schema bump before any scorer code. This batch therefore changes no score: the v1 scorer file is pinned below and the gate
fails if it changes (AC-S4-05.2). AC-S4-05.1 with legal-editor rows stays BLOCKED (Q-S4-02).

Usage: python eval/checks/foundation_b_s4_10.py [--cargo-log PATH] [--checks PATH]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "golden" / "G-ADMISS" / "fixtures" / "foundation_core.json"
FIXTURE_SHA256 = "140b5ea0e5c04a5ab1a46425810b26b87ec70c65317e9b2f0f51c0ea6a0a3c79"  # committed before the engine (fb502dc)
VOCAB = ROOT / "packs" / "builtin" / "evidence_types.v1.json"
VOCAB_SHA256 = "0c03e4c28c466c360ffc109e14416dfb8fe46f17ee81621443d9d0fd6a6df67b"
INTEGRITY_V1_SHA256 = "bf3345bef0ef91a7cf66e3e7acfc5745b9dfc163a552254b4ba190ca84ff3746"  # src-tauri/src/integrity/mod.rs, line endings normalised
TESTS = {
    "golden": "foundation::tests::golden_foundation_cases",
    "panel": "foundation::tests::foundation_gaps_lead_the_panel_and_planted_gaps_are_in_the_top_three",
    "types": "foundation::tests::evidence_types_are_matched_on_whole_words_and_ties_go_to_the_earlier_type",
    "shipped_rules": "foundation::tests::shipped_admissibility_rules_install_cite_the_right_text_and_check_a_matter",
}
LLM_TOKENS = re.compile(r"infer_llm|ollama|openai|llm_chat|chat_with_timeout|reqwest|TcpStream|http://|https://", re.I)
PROHIBITED_UI = ["fake", "fabricated", "court-approved", "guaranteed", "good law", "valid law", "predicts", "decides", "inadmissible", "will be admitted"]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    ap.add_argument("--checks", type=Path, default=None)
    args = ap.parse_args()
    errors: list[str] = []
    acs: dict[str, dict] = {}

    if hashlib.sha256(FIXTURE.read_bytes()).hexdigest() != FIXTURE_SHA256:
        errors.append("G-ADMISS fixture changed since it was pinned")
    if hashlib.sha256(VOCAB.read_bytes()).hexdigest() != VOCAB_SHA256:
        errors.append("the evidence-type vocabulary changed since the golden was pinned")
    fx = json.loads(FIXTURE.read_text(encoding="utf-8"))
    expected = {s for c in fx["cases"] for e in c["expect"] for s in e["elements"].values()}
    if expected != {"PRESENT", "ABSENT", "UNKNOWN"}:
        errors.append(f"golden does not exercise every element status: {sorted(expected)}")

    v1 = (ROOT / "src-tauri" / "src" / "integrity" / "mod.rs").read_bytes().replace(b"\r\n", b"\n")
    v1_untouched = hashlib.sha256(v1).hexdigest() == INTEGRITY_V1_SHA256 and b"integrity.v2" not in v1
    if not v1_untouched:
        errors.append("the integrity v1 scorer changed (its score formula needs owner approval and a schema bump first)")

    schema_valid = None
    if args.checks:
        try:
            import jsonschema

            validator = jsonschema.Draft202012Validator(json.loads((ROOT / "contracts" / "foundation_check.schema.json").read_text(encoding="utf-8")))
            checks = json.loads(args.checks.read_text(encoding="utf-8"))
            for c in checks:
                validator.validate(c)
            statuses = {e["status"] for c in checks for e in c["elements"]}
            kinds = {c["evidence_type_status"] for c in checks}
            if statuses != {"PRESENT", "ABSENT", "UNKNOWN"} or kinds != {"PROPOSED", "COUNSEL_CONFIRMED"}:
                raise ValueError(f"evidence must exercise every status and both type states (statuses {statuses}, types {kinds})")
            schema_valid = True
        except Exception as e:  # noqa: BLE001 - reported as a gate failure
            errors.append(f"foundation check does not satisfy contracts/foundation_check.schema.json: {e}")
            schema_valid = False

    engine = (ROOT / "src-tauri" / "src" / "foundation.rs").read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    lib = (ROOT / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    ui = (ROOT / "src" / "components" / "FoundationPanel.tsx").read_text(encoding="utf-8")
    static = {
        "engine_has_no_model_or_network": not LLM_TOKENS.search(engine),
        "engine_changes_no_score": not any(t in engine for t in ("score_tree", "ScoreReport", "crate::integrity", "MissingLinkRecord")),
        "every_gap_cites_rule_and_authority": '"rule_id": rule.rule_id' in engine and '"authority": authority' in engine,
        "commands_registered": all(f"fn {n}" in lib for n in ("foundation_check", "foundation_set_type", "foundation_link")),
        "ui_says_gaps_are_not_rulings": "not rulings on admissibility" in ui and "change no score" in ui,
        "ui_shows_the_cited_text": "authority_excerpt" in ui,
        "ui_vocabulary_honest": not any(re.search(rf"(?i)['\"`>][^'\"`<]*\b{re.escape(w)}\b", ui) for w in PROHIBITED_UI),
    }
    for k, v in static.items():
        if not v:
            errors.append(f"static check failed: {k}")

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        integrity_tests = re.findall(r"test (integrity::tests::\S+) \.\.\. (\w+)", log)
        v1_ok = v1_untouched and bool(integrity_tests) and all(r == "ok" for _, r in integrity_tests)
        all_ok = all(v == "ok" for v in res.values())
        acs["AC-S4-05.2"] = {"status": "PASS" if v1_ok else "FAIL",
                             "evidence": {"integrity_v1_file_pinned": v1_untouched, "integrity_v1_tests": len(integrity_tests)},
                             "detail": "The v1 scorer file is byte-identical to the pinned version and its tests pass; this batch adds no scorer code, so v1 outputs cannot change."}
        acs["AC-S4-05.3"] = {"status": "PASS" if all_ok and schema_valid is not False and static["every_gap_cites_rule_and_authority"] else "FAIL",
                             "evidence": {"golden": res["golden"], "shipped_rules": res["shipped_rules"], "full_schema_validation": schema_valid},
                             "detail": "Every element of every check cites a rule row and an authority; each gap carries the rule, its trust state and the cited sentence; the shipped rules' citations are the sentences of the section they rest on."}
        acs["AC-S4-05.1"] = {"status": "BLOCKED",
                             "engine_measured": "a planted missing foundation element is in the top 3 of the merged Missing Links panel" if res["panel"] == "ok" else "panel test did not pass",
                             "reason": "The criterion is measured against legal-editor-approved rows (Q-S4-02). The shipped admissibility rows (caseforge.india.admissibility-rules) are DEVELOPMENT_ONLY, so this stays BLOCKED until two legal editors sign them."}
    else:
        for ac in ("AC-S4-05.2", "AC-S4-05.3", "AC-S4-05.1"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
    report = {"batch": "B-S4-10", "fixture_sha256": FIXTURE_SHA256, "cases": len(fx["cases"]), "acs": acs, "errors": errors}
    print(json.dumps(report, indent=2))
    out = ROOT / "eval" / "reports" / "b_s4_10_foundation.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
