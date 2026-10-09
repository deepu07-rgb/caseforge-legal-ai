#!/usr/bin/env python3
"""B-S4-18 Issue Matrix gate.

Behaviour is proven by Rust unit tests in src-tauri/src/issue_matrix.rs against golden/G-CHAMBERS (synthetic two-party
pleadings with planted admitted, disputed and unanswered points, derived by an independent implementation of the written
rules) plus property-style tests (party swap, merge, a party that filed nothing, refusals, bench memo). Pass the
`cargo test --lib` log with --cargo-log. With --evidence-dir (written by the golden tests when CASEFORGE_EVIDENCE_DIR is
set) every matrix is validated against the frozen contracts/issue_matrix.schema.json.

AC-S4-12.1 / 12.2 are measured on the synthetic golden; 12.2's 95 % threshold is PROPOSED (Q-S4-01) and the golden is
synthetic, so re-measure on judge-annotated pleadings when they exist. AC-S4-12.3 is a red-team of what the engine and the
screen can say, plus the party-swap test.

Usage: python eval/checks/issue_matrix_b_s4_18.py [--cargo-log PATH] [--evidence-dir DIR]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = {
    "core": ("golden/G-CHAMBERS/fixtures/issue_matrix_core.json", "301c5742795323ea726c33207b7bee4ebfaa2ff2d522cc48ae3247dc74ce01da"),  # committed before the engine (6c78a28)
    "fallback": ("golden/G-CHAMBERS/fixtures/issue_matrix_fallback.json", "96a2321af5d5429203c8bb5b6ca81be1e35c8b7b0a80eb08d4ba1631139f9120"),
}
T = "issue_matrix::tests::"
TESTS = {
    "golden_core": T + "golden_chambers_cross_referenced_pleadings",
    "golden_fallback": T + "golden_chambers_pairing_by_wording_when_nothing_is_cross_referenced",
    "evidence": T + "evidence_authorities_and_classes_are_taken_from_the_paragraphs",
    "merge": T + "merged_issue_contributes_to_its_target_and_shows_nothing_itself",
    "no_filing": T + "a_party_with_no_filing_is_never_shown_as_having_failed_to_answer",
    "swap": T + "the_same_rules_apply_to_both_parties_when_their_roles_are_swapped",
    "refusals": T + "invalid_inputs_are_refused_with_a_reason",
    "templates": T + "templates_are_read_from_installed_tables_for_the_matter_type",
    "proposals": T + "proposals_skip_what_the_judge_has_already_decided",
    "memo": T + "bench_memo_is_neutral_located_and_carries_the_footer",
    "footer": T + "footer_lines_match_the_frozen_constants",
    "audit_map": T + "audit_items_map_to_page_relative_positions",
}
LLM_TOKENS = re.compile(r"infer_llm|ollama|openai|llm_chat|chat_with_timeout|reqwest|TcpStream|http://|https://", re.I)
# What an issue matrix must never say about a party or about how an issue should be answered.
RANKING_WORDS = re.compile(r"(?i)\b(stronger|weaker|strongest|weakest|better case|worse case|wins?|losing|likely to (?:succeed|win|fail)|probab\w*|chances?|favou?rs?|favou?rable|prevails?|should (?:succeed|win|be decreed|be dismissed)|recommend\w*|ranked?|ranking|scor(?:e|es|ed|ing))\b")
ALLOWED_NEGATIONS = ["party_scoring", "ranks nothing, scores nothing", "never win prediction"]
PROHIBITED_UI = ["fake", "fabricated", "court-approved", "guaranteed", "good law", "valid law", "predicts", "decides", "inadmissible", "will be admitted"]


def strip_rust_comments(src: str) -> str:
    return "\n".join(l for l in src.splitlines() if not l.strip().startswith("//"))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    ap.add_argument("--evidence-dir", type=Path, default=None)
    args = ap.parse_args()
    errors: list[str] = []
    acs: dict[str, dict] = {}

    fixtures = {}
    for name, (rel, sha) in FIXTURES.items():
        path = ROOT / rel
        if hashlib.sha256(path.read_bytes()).hexdigest() != sha:
            errors.append(f"G-CHAMBERS {name} fixture changed since it was pinned")
        fixtures[name] = json.loads(path.read_text(encoding="utf-8"))
    planted = {n: sum(len(e.get("unanswered", [])) for e in fx["expect"].values()) for n, fx in fixtures.items()}
    if not all(planted.values()):
        errors.append(f"every G-CHAMBERS fixture must plant an unanswered argument: {planted}")
    facts = {n: sum(len(e.get("admitted", [])) + len(e.get("disputed", [])) for e in fx["expect"].values()) for n, fx in fixtures.items()}
    if not all(facts.values()):
        errors.append(f"every G-CHAMBERS fixture must plant admitted or disputed facts: {facts}")
    if not any(e.get("admitted") for fx in fixtures.values() for e in fx["expect"].values()) or not any(e.get("disputed") for fx in fixtures.values() for e in fx["expect"].values()):
        errors.append("the goldens must exercise both admitted and disputed facts")
    decisions = {e["decision"] for e in fixtures["core"]["expect"].values()}
    if decisions != {"ACCEPTED", "EDITED", "PROPOSED", "REJECTED", "MERGED"}:
        errors.append(f"the core golden must exercise every judge decision: {sorted(decisions)}")

    schema_valid = None
    if args.evidence_dir:
        try:
            import jsonschema

            validator = jsonschema.Draft202012Validator(json.loads((ROOT / "contracts" / "issue_matrix.schema.json").read_text(encoding="utf-8")), format_checker=jsonschema.FormatChecker())
            seen = 0
            for name in FIXTURES:
                matrix = json.loads((args.evidence_dir / f"issue_matrix_{name}.json").read_text(encoding="utf-8"))
                validator.validate(matrix)
                seen += 1
                if matrix["neutrality"] != {"party_scoring": False, "outcome_suggestion": False, "record_only": True}:
                    raise ValueError("neutrality block changed")
                for issue in matrix["issues"]:
                    if issue["judge_decision"] in ("ACCEPTED", "EDITED") and len(issue["positions"]) != len(matrix["parties"]):
                        raise ValueError(f"issue {issue['issue_id']} does not show every party")
            schema_valid = seen == len(FIXTURES)
        except Exception as e:  # noqa: BLE001 - reported as a gate failure
            errors.append(f"issue matrix does not satisfy contracts/issue_matrix.schema.json: {e}")
            schema_valid = False

    engine = (ROOT / "src-tauri" / "src" / "issue_matrix.rs").read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    lib = (ROOT / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    ui = (ROOT / "src" / "components" / "IssueMatrixView.tsx").read_text(encoding="utf-8")
    scan = strip_rust_comments(engine)
    ui_scan = ui
    for ok in ALLOWED_NEGATIONS:
        scan = scan.replace(ok, "")
        ui_scan = ui_scan.replace(ok, "")
    ranking_hits = sorted({m.group(0) for m in RANKING_WORDS.finditer(scan)} | {m.group(0) for m in RANKING_WORDS.finditer(re.sub(r"/\*.*?\*/", "", ui_scan, flags=re.S))})
    static = {
        "engine_has_no_model_or_network": not LLM_TOKENS.search(engine),
        "engine_writes_no_graph_or_score": not any(t in engine for t in ("case_graph_put", "score_tree", "ScoreReport", "crate::integrity", "witness_kv_put")),
        "neutrality_block_is_constant": '"neutrality": {"party_scoring": false, "outcome_suggestion": false, "record_only": true}' in engine,
        "commands_registered": all(f"fn {n}" in lib for n in ("issue_matrix_build", "issue_matrix_decide", "issue_matrix_add", "issue_matrix_set_filing", "issue_matrix_memo", "issue_matrix_set_matter_type")),
        "only_judge_accepted_issues_get_content": 'issue.decision == "ACCEPTED" || issue.decision == "EDITED"' in engine,
        "ui_says_nothing_is_ranked": "ranks nothing, scores nothing and suggests no answer" in ui,
        "ui_proposals_need_acceptance": "nothing below is used until you accept it" in ui,
        "ui_vocabulary_honest": not any(re.search(rf"(?i)['\"`>][^'\"`<]*\b{re.escape(w)}\b", ui) for w in PROHIBITED_UI),
        "no_ranking_or_favour_words_in_engine_or_screen": not ranking_hits,
    }
    for k, v in static.items():
        if not v:
            errors.append(f"static check failed: {k}" + (f" {ranking_hits}" if k.startswith("no_ranking") else ""))

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = lambda *ks: all(res[k] == "ok" for k in ks)  # noqa: E731
        acs["AC-S4-12.1"] = {"status": "PASS" if ok("golden_core", "golden_fallback") and not errors else "FAIL",
                             "evidence": {"fixtures": len(fixtures), "planted_unanswered_per_fixture": planted, "golden_core": res["golden_core"], "golden_fallback": res["golden_fallback"], "no_filing_never_blamed": res["no_filing"]},
                             "detail": "Every planted unanswered argument is listed, in each fixture, and nothing else is."}
        acs["AC-S4-12.2"] = {"status": "PASS" if ok("golden_core", "golden_fallback") and not errors else "FAIL",
                             "evidence": {"planted_admitted_or_disputed_per_fixture": facts, "agreement": "100 % of expected admitted/disputed facts, exact match, no extras"},
                             "threshold": "PROPOSED 95 % (Q-S4-01)",
                             "detail": "Measured on the synthetic G-CHAMBERS golden (cross-referenced pleadings, and pleadings paired by wording). Re-measure on judge-annotated pleadings when available."}
        acs["AC-S4-12.3"] = {"status": "PASS" if ok("swap", "memo", "evidence") and schema_valid is not False and static["no_ranking_or_favour_words_in_engine_or_screen"] and static["neutrality_block_is_constant"] else "FAIL",
                             "evidence": {"party_swap": res["swap"], "memo_vocabulary": res["memo"], "ranking_words_found": ranking_hits, "schema_validated": schema_valid},
                             "detail": "Swapping which party filed which pleading mirrors the result and changes nothing else; the engine, memo and screen contain no ranking, scoring or favouring words; the neutrality block is constant."}
        all_ok = all(v == "ok" for v in res.values())
        if not all_ok:
            errors.append(f"tests not passing: { {k: v for k, v in res.items() if v != 'ok'} }")
    else:
        for ac in ("AC-S4-12.1", "AC-S4-12.2", "AC-S4-12.3"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
    report = {"batch": "B-S4-18", "fixtures": {n: FIXTURES[n][1] for n in FIXTURES}, "acs": acs, "static": static, "errors": errors}
    print(json.dumps(report, indent=2))
    out = ROOT / "eval" / "reports" / "b_s4_18_issue_matrix.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
