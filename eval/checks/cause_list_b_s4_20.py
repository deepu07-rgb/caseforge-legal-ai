#!/usr/bin/env python3
"""B-S4-20 Daily Cause-List Brief gate.

Behaviour is proven by Rust tests in src-tauri/src/cause_list.rs against golden/G-CAUSELIST/fixtures/cause_list_core.json
(a CSV and plain-line cause list, five synthetic matters, hand-authored briefs; pinned before the engine). Pass the
`cargo test --lib` log with --cargo-log. With --evidence-dir (written when CASEFORGE_EVIDENCE_DIR is set) the rendered
brief documents are validated against the frozen contracts/cause_list_brief.schema.json and the measured 100-matter render
time is read.

AC-S4-14.1 every brief field cites an order/document locator (an exact slice of the record) or says "not found in record".
AC-S4-14.2 a 100-matter list renders within the PROPOSED 60 s (S4.chambers.cause_list_100_render_max_s, Q-S4-01). It is
measured on the machine the tests ran on; the Full reference machine is the owner's.
The pilot court's real cause-list format is unavailable (Q-S4-10): the importer reads CSV and numbered lines, and the real
format is a QUESTION.

Usage: python eval/checks/cause_list_b_s4_20.py [--cargo-log PATH] [--evidence-dir DIR]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "golden" / "G-CAUSELIST" / "fixtures" / "cause_list_core.json"
FIXTURE_SHA256 = "75b2630d218e78e9dae035bd7a3718825f181c5d67bd4ba959330140df22205d"  # committed before the engine (fd0c765)
T = "cause_list::tests::"
TESTS = {
    "import": T + "golden_import_reads_csv_and_lines",
    "briefs": T + "golden_matching_and_briefs",
    "cites": T + "every_brief_field_cites_the_record_or_says_not_found",
    "budget": T + "a_hundred_listed_matters_render_within_the_budget",
    "rules": T + "dates_sentences_and_numbers_follow_the_stated_rules",
    "completion": T + "a_direction_is_not_shown_done_by_a_negated_or_undated_or_earlier_sentence",
    "no_model": T + "the_engine_uses_no_model_or_network",
}
LLM_TOKENS = re.compile(r"infer_llm|ollama|openai|llm_chat|chat_with_timeout|reqwest|TcpStream|http://|https://", re.I)
PROHIBITED_UI = ["fake", "fabricated", "court-approved", "guaranteed", "good law", "valid law", "predicts", "decides", "inadmissible", "will be admitted", "stronger case", "weaker case", "the court should"]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    ap.add_argument("--evidence-dir", type=Path, default=None)
    args = ap.parse_args()
    errors: list[str] = []
    acs: dict[str, dict] = {}

    if hashlib.sha256(FIXTURE.read_bytes()).hexdigest() != FIXTURE_SHA256:
        errors.append("G-CAUSELIST fixture changed since it was pinned")
    fx = json.loads(FIXTURE.read_text(encoding="utf-8"))
    statuses = {e["status"] for e in fx["expect"].values()} | {fx["expect_confirmed_3"]["status"]}
    if statuses != {"MATCHED", "AMBIGUOUS_UNCONFIRMED", "AMBIGUOUS_CONFIRMED", "NO_LOCAL_RECORD"}:
        errors.append(f"the golden must exercise every match status: {sorted(statuses)}")
    done = {c["record_status"] for e in fx["expect"].values() if e["brief"] for c in e["brief"]["compliance"]}
    if done != {"DONE_ON_RECORD", "NOT_SHOWN_ON_RECORD"}:
        errors.append(f"the golden must exercise both compliance states: {sorted(done)}")
    if not any(a["sought_by_party_id"] is None for e in fx["expect"].values() if e["brief"] for a in e["brief"]["adjournments"]["entries"]):
        errors.append("the golden must include an adjournment whose order does not say who sought it")

    engine = (ROOT / "src-tauri" / "src" / "cause_list.rs").read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    lib = (ROOT / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    view = (ROOT / "src" / "components" / "CauseListView.tsx").read_text(encoding="utf-8")

    def body(fn: str) -> str:
        start = lib.index(f"fn {fn}(")
        end = min([i for i in (lib.find("#[tauri::command]", start), lib.find(chr(10) + "fn ", start + 5)) if i > 0] or [len(lib)])
        return lib[start:end]

    imp, brf = body("cause_list_import"), body("cause_list_briefs")
    schema_valid = None
    measured = None
    if args.evidence_dir:
        try:
            import jsonschema

            validator = jsonschema.Draft202012Validator(json.loads((ROOT / "contracts" / "cause_list_brief.schema.json").read_text(encoding="utf-8")), format_checker=jsonschema.FormatChecker())
            for name in ("cause_list_unconfirmed.json", "cause_list_confirmed.json"):
                validator.validate(json.loads((args.evidence_dir / name).read_text(encoding="utf-8")))
            schema_valid = True
            measured = json.loads((args.evidence_dir / "cause_list_budget.json").read_text(encoding="utf-8"))
        except Exception as e:  # noqa: BLE001
            schema_valid = False
            errors.append(f"brief document does not satisfy contracts/cause_list_brief.schema.json (or evidence missing): {e}")
    static = {
        "engine_has_no_model_or_network": not LLM_TOKENS.search(engine),
        "commands_registered": "cause_list_import," in lib and "cause_list_briefs," in lib,
        "commands_need_the_chambers_edition": all("edition::require_chambers" in b and '"cause_list"' in b for b in (imp, brf)),
        "import_sends_nothing_anywhere": not LLM_TOKENS.search(imp) and "reqwest" not in imp,
        "orders_only_from_the_record": "orders_of(" in engine and "doc_list" not in engine,
        "screen_cites_the_source_of_every_line": all(t in view for t in ("not found in the record", "where(", "Shown done on the record", "does not say who sought it")),
        "screen_prints_one_page_per_matter": "page-break-after" in view and "window.print()" in view,
        "screen_carries_the_export_footer": "EXPORT_FOOTER_LINES" in view,
        "screen_asks_the_judge_to_choose_when_unsure": "AMBIGUOUS_UNCONFIRMED" in view and "choose(" in view,
        "screen_vocabulary_honest": not any(re.search(rf"(?i)['\"`>][^'\"`<]*\b{re.escape(w)}\b", view) for w in PROHIBITED_UI),
    }
    for k, v in static.items():
        if not v:
            errors.append(f"static check failed: {k}")

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = lambda *ks: all(res[k] == "ok" for k in ks)  # noqa: E731
        acs["AC-S4-14.1"] = {"status": "PASS" if ok("import", "briefs", "cites", "rules", "completion") and schema_valid is not False and not errors else "FAIL",
                             "evidence": {"matches": sorted(statuses), "compliance_states": sorted(done), "schema_validated": schema_valid, "briefs_test": res["briefs"], "every_field_cites_test": res["cites"]},
                             "detail": "Every field of every brief is either FOUND with a locator that is the exact record text at the stated document, page and character range, or NOT_FOUND_IN_RECORD with nothing; a direction is shown done only by a later dated, non-negated sentence that shares at least three content words with it; who sought an adjournment appears only where the order says."}
        threshold = fx["budget"]["max_seconds"]
        acs["AC-S4-14.2"] = {"status": "PASS" if ok("budget") and (measured is None or measured["seconds"] <= threshold) else "FAIL",
                             "evidence": {"entries": fx["budget"]["entries"], "measured_seconds": (measured or {}).get("seconds"), "threshold_seconds": f"PROPOSED {threshold} (Q-S4-01)", "machine": "the machine the tests ran on, not the Full reference machine"},
                             "detail": "100 listed matters, each with an order sheet, matched and briefed within the proposed 60 s. Re-measure on the Full reference machine."}
        for k, v in res.items():
            if v != "ok":
                errors.append(f"test not passing: {TESTS[k]} ({v})")
    else:
        for ac in ("AC-S4-14.1", "AC-S4-14.2"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
    report = {"batch": "B-S4-20", "fixture_sha256": FIXTURE_SHA256, "acs": acs, "static": static, "errors": errors}
    print(json.dumps(report, indent=2))
    out = ROOT / "eval" / "reports" / "b_s4_20_cause_list.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
