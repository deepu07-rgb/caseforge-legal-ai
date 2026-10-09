#!/usr/bin/env python3
"""B-S4-19 Both-sides citation check and record lookup gate.

Behaviour is proven by Rust tests in src-tauri/src/chambers_lookup.rs against golden/G-CHAMBERS/fixtures/record_lookup_core.json
(a synthetic record and 24 questions with hand-marked answers, pinned before the engine); pass the `cargo test --lib` log
with --cargo-log, and, if the tests ran with CASEFORGE_EVIDENCE_DIR set, --evidence-dir to read the measured hit rate.

AC-S4-13.1 top-5 hit rate >= the PROPOSED 90 % threshold (S4.chambers.lookup_top5_hit_rate_min, Q-S4-01) is measured on the
synthetic golden. AC-S4-13.2 lookup returns only record excerpts: every hit is a verbatim slice of a document at its
stated place, the hit has exactly stored-text/position fields, and the engine contains no model, network or text generation.

Usage: python eval/checks/chambers_lookup_b_s4_19.py [--cargo-log PATH] [--evidence-dir DIR]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "golden" / "G-CHAMBERS" / "fixtures" / "record_lookup_core.json"
FIXTURE_SHA256 = "b8655d2333ef8343e23df0b0176e7fd5f17ec086c0ef78fd8d921547c7d90cdf"  # committed before the engine (2c300d5)
VOCAB = ROOT / "packs" / "builtin" / "record_synonyms.v1.json"
T = "chambers_lookup::tests::"
TESTS = {
    "hit_rate": T + "golden_top_five_contains_the_answering_passage",
    "no_answer": T + "a_question_the_record_cannot_answer_returns_nothing",
    "verbatim": T + "every_hit_is_a_verbatim_slice_of_the_record_with_a_locator_and_nothing_else",
    "passages": T + "passages_are_pleading_paragraphs_or_blank_line_blocks",
    "long": T + "a_very_long_passage_is_cut_at_sentence_ends_and_nothing_is_lost",
    "determinism": T + "lookup_is_deterministic_and_synonyms_count_for_less_than_the_word_itself",
    "no_prose": T + "the_engine_writes_no_prose",
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
        errors.append("record-lookup golden changed since it was pinned")
    fx = json.loads(FIXTURE.read_text(encoding="utf-8"))
    if len(fx["queries"]) < 20 or len(fx["no_answer"]) < 2:
        errors.append("the golden needs at least 20 questions and 2 unanswerable ones")
    vocab = json.loads(VOCAB.read_text(encoding="utf-8"))
    if not vocab["groups"] or not all(isinstance(g, list) and len(g) >= 2 for g in vocab["groups"]):
        errors.append("record_synonyms.v1.json is malformed")

    engine = (ROOT / "src-tauri" / "src" / "chambers_lookup.rs").read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    lib = (ROOT / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    view = (ROOT / "src" / "components" / "ChambersView.tsx").read_text(encoding="utf-8")

    def body(fn: str) -> str:
        start = lib.index(f"fn {fn}(")
        end = min([i for i in (lib.find("#[tauri::command]", start), lib.find(chr(10) + "fn ", start + 5)) if i > 0] or [len(lib)])
        return lib[start:end]

    audit_all, lookup = body("chambers_audit_all"), body("chambers_lookup")
    static = {
        "engine_has_no_model_or_network": not LLM_TOKENS.search(engine),
        "lookup_command_never_calls_a_model": not LLM_TOKENS.search(lookup),
        "index_is_the_matters_own_documents_only": "doc_list(&case_id)" in lookup and not any(t in lookup for t in ("retrieval::", "build_small_corpus", "load_installed", "store::open")),
        "lookup_needs_the_chambers_edition": "edition::require_chambers" in lookup and '"record_lookup"' in lookup,
        "audit_all_needs_the_chambers_edition": "edition::require_chambers" in audit_all and '"citation_audit"' in audit_all,
        "audit_all_runs_the_same_check_on_every_filing": "audit_with_store" in audit_all and "treatment::decorate_report" in audit_all,
        "audit_all_groups_by_party_without_ranking": "sort" not in audit_all and "ranks one party" not in view,
        "commands_registered": "chambers_audit_all," in lib and "chambers_lookup," in lib,
        "screen_shows_excerpt_and_place": all(t in view for t in ("h.excerpt", "h.filename", "h.page", "highlight(")),
        "screen_voice_needs_a_check": "voiceChecked" in view and "speechStt" in view and "production_eligible" in view,
        "screen_audit_is_neutral": "nothing here compares or ranks the parties" in view,
        "screen_vocabulary_honest": not any(re.search(rf"(?i)['\"`>][^'\"`<]*\b{re.escape(w)}\b", view) for w in PROHIBITED_UI),
    }
    for k, v in static.items():
        if not v:
            errors.append(f"static check failed: {k}")

    measured = None
    if args.evidence_dir and (args.evidence_dir / "record_lookup.json").is_file():
        measured = json.loads((args.evidence_dir / "record_lookup.json").read_text(encoding="utf-8"))
    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = lambda *ks: all(res[k] == "ok" for k in ks)  # noqa: E731
        threshold = fx["threshold"]["proposed"]
        acs["AC-S4-13.1"] = {"status": "PASS" if ok("hit_rate", "no_answer", "passages") and not errors else "FAIL",
                             "evidence": {"queries": len(fx["queries"]), "measured": measured, "threshold": f"PROPOSED {threshold:.0%} (Q-S4-01)", "test": res["hit_rate"], "unanswerable_return_nothing": res["no_answer"]},
                             "detail": "Top-5 contains a marked answering passage for the proposed share of questions on the synthetic G-CHAMBERS record. Re-measure on real matter records; the threshold is PROPOSED and the golden synthetic."}
        acs["AC-S4-13.2"] = {"status": "PASS" if ok("verbatim", "no_prose", "determinism") and static["engine_has_no_model_or_network"] and static["lookup_command_never_calls_a_model"] else "FAIL",
                             "evidence": {"verbatim_slices": res["verbatim"], "no_prose": res["no_prose"], "static": {k: static[k] for k in ("engine_has_no_model_or_network", "lookup_command_never_calls_a_model", "index_is_the_matters_own_documents_only")}},
                             "detail": "Every hit is the record's own text at the stated document, page and character range, character for character; the hit carries only stored text, positions and numbers; the engine has no model, network or generation."}
        acs["both-sides-audit"] = {"status": "PASS" if all(static[k] for k in ("audit_all_needs_the_chambers_edition", "audit_all_runs_the_same_check_on_every_filing", "audit_all_groups_by_party_without_ranking", "screen_audit_is_neutral")) else "FAIL",
                                   "evidence": {k: static[k] for k in ("audit_all_needs_the_chambers_edition", "audit_all_runs_the_same_check_on_every_filing", "audit_all_groups_by_party_without_ranking", "screen_audit_is_neutral")},
                                   "detail": "One action runs the Citation Audit on every filing of every party, results grouped by party in the order the parties are named; the check is the same engine as the Citation Audit and is covered by its own gate."}
        for k, v in res.items():
            if v != "ok":
                errors.append(f"test not passing: {TESTS[k]} ({v})")
    else:
        for ac in ("AC-S4-13.1", "AC-S4-13.2", "both-sides-audit"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
    report = {"batch": "B-S4-19", "fixture_sha256": FIXTURE_SHA256, "acs": acs, "static": static, "errors": errors}
    print(json.dumps(report, indent=2))
    out = ROOT / "eval" / "reports" / "b_s4_19_chambers_lookup.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
