#!/usr/bin/env python3
"""B-S4-17 Chambers Edition shell and Record-only mode (G7) gate.

Behaviour is proven by Rust tests (pass the `cargo test --lib` log with --cargo-log):
  - record_only.rs against golden/G-CHAMBERS/record_only_core.json (pinned before the engine): statements go through the
    real proposition verifier, then the G7 classifier. Planted extra-record content - model memory, an invented quotation, a
    custom-pack law, a fact supplied in the prompt, inferences resting on those, a statement without support - never renders;
    every rendered statement has a class and a locator; with record-only off the unsupported ones show labelled and never as
    RECORD or LAW; record-only cannot be switched off once a judgment scaffold exists.
  - edition.rs: a signed JUDICIARY licence enables Chambers and hides advocacy surfaces; every other edition (and a v1 licence,
    a tampered, foreign-machine or mis-built licence) gets none; the host refuses free generation in Chambers (only the
    record-only path holds a permit); the air-gap switch cannot be turned off; pilot-court constraints are configuration and
    fail closed.
Static checks tie the pieces together (the model gate, command guards, the screens, the Python/Rust licence canonical form).

Usage: python eval/checks/chambers_record_only_b_s4_17.py [--cargo-log PATH]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "golden" / "G-CHAMBERS" / "fixtures" / "record_only_core.json"
FIXTURE_SHA256 = "0f4ad28924e7617cba6fa14caac2199525837cd0bd88e6ac486bb692adbc2021"  # committed before the engine (d725ba0)
R = "record_only::tests::"
E = "edition::tests::"
TESTS = {
    "golden": R + "golden_planted_extra_record_content_never_renders",
    "off": R + "with_record_only_off_unsupported_statements_show_labelled_and_never_as_record_or_law",
    "lock": R + "record_only_cannot_be_switched_off_once_a_scaffold_exists",
    "inference": R + "an_inference_rests_only_on_what_itself_renders",
    "canonical": E + "canonical_bytes_follow_the_python_rule",
    "judiciary": E + "a_signed_judiciary_licence_enables_chambers_and_hides_advocacy",
    "others": E + "every_other_edition_sees_no_chambers_surface",
    "unverified": E + "a_licence_that_does_not_verify_grants_nothing",
    "invariants": E + "the_contract_invariants_of_the_judiciary_edition_are_enforced",
    "expired": E + "an_expired_judiciary_licence_keeps_its_edition_and_is_read_only",
    "generation": E + "free_generation_is_refused_in_chambers_and_only_the_record_only_path_may_reach_the_model",
    "air_gap": E + "the_air_gap_cannot_be_turned_off_under_a_judiciary_licence",
    "pilot": E + "pilot_constraints_are_configuration_and_fail_closed",
}
LLM_TOKENS = re.compile(r"infer_llm|ollama|openai|llm_chat|chat_with_timeout|reqwest|TcpStream|http://|https://", re.I)
PROHIBITED_UI = ["fake", "fabricated", "court-approved", "guaranteed", "good law", "valid law", "predicts", "decides", "inadmissible", "will be admitted", "stronger case", "weaker case", "should be decreed", "the court should"]
# Planted kinds the golden must contain, by proposition id.
PLANTED = {"x1": "model memory (no support)", "x2": "invented quotation", "x3": "custom-pack law", "x4": "fact supplied in the prompt", "x5": "inference resting on the prompt", "x6": "inference without bases", "x7": "statute passage offered as a fact", "x8": "inference resting on model memory", "x9": "explicit gap"}


CHAMBERS_ONLY_COMMANDS = ["issue_matrix_build", "issue_matrix_set_matter_type", "issue_matrix_add", "issue_matrix_decide", "issue_matrix_set_filing", "issue_matrix_memo", "chambers_status", "chambers_set_record_only", "chambers_g7_log", "chambers_reason"]
ADVOCACY_COMMANDS = ["reasoning_run", "opposition_scan", "impeach_model_scan"]


def guarded(lib: str, name: str, guard: str) -> bool:
    """The command's own body calls the edition guard."""
    start = lib.index(f"fn {name}(")
    end = lib.find("#[tauri::command]", start)
    return f"edition::{guard}(" in lib[start: end if end > 0 else len(lib)]


def python_canonical(obj) -> str:
    body = {k: v for k, v in obj.items() if k not in ("signature_hex", "signature_alg")}
    return json.dumps(body, sort_keys=True, separators=(",", ":"))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    args = ap.parse_args()
    errors: list[str] = []
    acs: dict[str, dict] = {}

    if hashlib.sha256(FIXTURE.read_bytes()).hexdigest() != FIXTURE_SHA256:
        errors.append("record-only golden changed since it was pinned")
    fx = json.loads(FIXTURE.read_text(encoding="utf-8"))
    ids = {p["proposition_id"] for p in fx["propositions"]}
    if not set(PLANTED) <= ids:
        errors.append(f"the golden is missing planted cases: {sorted(set(PLANTED) - ids)}")
    kinds = {s["kind"] for s in fx["support_sources"]}
    if not {"MATTER_SOURCE", "LEGAL_AUTHORITY", "COUNSEL_INPUT"} <= kinds or not any(s["trust_level"] == "CUSTOM" for s in fx["support_sources"]):
        errors.append("the golden must include record, official law, a custom-pack law and a prompt-supplied fact")
    on = fx["expect"]["record_only_on"]
    if {r["class"] for r in on["rendered"]} != {"RECORD", "LAW", "INFERENCE_FROM_RECORD"}:
        errors.append("the golden must render all three allowed classes")

    # --- sources of truth agree ---
    ed = (ROOT / "src-tauri" / "src" / "edition.rs").read_text(encoding="utf-8")
    ro = (ROOT / "src-tauri" / "src" / "record_only.rs").read_text(encoding="utf-8")
    engine_ed, engine_ro = ed.split("#[cfg(test)]")[0], ro.split("#[cfg(test)]")[0]
    lib = (ROOT / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    llm = (ROOT / "src-tauri" / "src" / "infer_llm" / "mod.rs").read_text(encoding="utf-8")
    app = (ROOT / "src" / "App.tsx").read_text(encoding="utf-8")
    view = (ROOT / "src" / "components" / "ChambersView.tsx").read_text(encoding="utf-8")

    rust_surfaces = re.findall(r'"([a-z-]+)"', re.search(r"pub const ALL_SURFACES: \[&str; \d+\] = \[(.*?)\];", ed, re.S).group(1))
    nav_union = set(re.findall(r'\|\s*"([a-z-]+)"', re.search(r"type NavId =(.*?);", app, re.S).group(1)))
    nav_union |= set(re.findall(r'type NavId =\s*\|?\s*"([a-z-]+)"', app))
    if set(rust_surfaces) != nav_union:
        errors.append(f"edition.rs surfaces and App.tsx NavId differ: {sorted(set(rust_surfaces) ^ nav_union)}")
    if len(rust_surfaces) != len(set(rust_surfaces)):
        errors.append("duplicate surface ids")

    # the licence canonical form is the same in Rust and in the issuing tool
    sample = {"b": 1, "a": "— x\n", "c": [True, None, {"z": 1, "y": "\\"}], "signature_hex": "x", "signature_alg": "Ed25519"}
    for obj, literal in ((sample, python_canonical(sample)), ({"n": "\U0001F600"}, python_canonical({"n": "\U0001F600"}))):
        if literal not in ro + ed:
            errors.append(f"the Rust canonical-form test does not pin the Python form {literal!r}")

    static = {
        "engine_has_no_model_or_network": not LLM_TOKENS.search(engine_ro) and not LLM_TOKENS.search(engine_ed),
        "model_gate_in_both_entry_points": llm.count("crate::edition::check_generation()?") == 2,
        "dev_override_is_debug_only": "if !cfg!(debug_assertions)" in engine_ed and "CASEFORGE_DEV_EDITION" in engine_ed,
        "permit_only_in_record_only_path": lib.count("edition::record_only_permit") == 1 and "chambers.then(edition::record_only_permit)" in lib,
        "chambers_commands_registered": all(f"fn {n}" in lib for n in ("edition_status", "chambers_status", "chambers_set_record_only", "chambers_g7_log", "chambers_reason")),
        "chambers_only_commands_guarded": all(guarded(lib, n, "require_chambers") for n in CHAMBERS_ONLY_COMMANDS),
        "advocacy_commands_guarded": all(guarded(lib, n, "require_advocacy") for n in ADVOCACY_COMMANDS),
        "air_gap_cannot_be_turned_off": "edition::check_air_gap_change" in lib,
        "reasoning_ends_at_the_classifier": "record_only::classify(ledger, &run.sources, record_only_on)" in lib,
        "question_is_never_a_source_in_chambers": "support_sources(case_id, &graph" in lib and "chambers_record_sources" in lib and 'kind: "COUNSEL_INPUT"' not in lib.split("fn chambers_record_sources")[1].split("fn chambers_reason")[0],
        "withheld_logged_by_hash_only": "text_sha256" in engine_ro and "chambers:g7:" in lib,
        "vault_refreshes_edition_on_unlock": "self.refresh_edition()" in (ROOT / "src-tauri" / "src" / "vault" / "mod.rs").read_text(encoding="utf-8"),
        "v2_licence_activation": "licence_activate_v2" in (ROOT / "src-tauri" / "src" / "vault" / "mod.rs").read_text(encoding="utf-8"),
        "screen_shows_class_and_source": all(t in view for t in ("s.class", "l.excerpt", "withheld_count", "record_coverage")),
        "screen_never_calls_raw_generation": not re.search(r"llmChat|reasoningRun|mockCourt", view),
        "screen_vocabulary_honest": not any(re.search(rf"(?i)['\"`>][^'\"`<]*\b{re.escape(w)}\b", view) for w in PROHIBITED_UI),
        "menu_filtered_by_edition": "PRIMARY_NAV.filter(showNav)" in app and "visible_surfaces" in app,
    }
    for k, v in static.items():
        if not v:
            errors.append(f"static check failed: {k}")

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = lambda *ks: all(res[k] == "ok" for k in ks)  # noqa: E731
        acs["AC-S4-11.1"] = {"status": "PASS" if ok("golden", "off", "inference", "lock") and not errors else "FAIL",
                             "evidence": {"planted": PLANTED, "golden": res["golden"], "model_gate": static["model_gate_in_both_entry_points"], "question_never_a_source": static["question_is_never_a_source_in_chambers"]},
                             "detail": "Planted extra-record content (model memory, an invented quotation, a custom-pack law, a fact in the prompt, inferences resting on them) never renders; it is counted and logged by hash. The model is reachable in Chambers only through the record-only path."}
        acs["AC-S4-11.2"] = {"status": "PASS" if ok("golden") and static["screen_shows_class_and_source"] else "FAIL",
                             "evidence": {"golden": res["golden"], "screen_shows_class_and_source": static["screen_shows_class_and_source"]},
                             "detail": "Every rendered statement carries RECORD, LAW or INFERENCE_FROM_RECORD and at least one locator (document, page and paragraph with the exact excerpt; or authority, chunk and excerpt); inferences carry their bases' locators."}
        acs["edition-gating"] = {"status": "PASS" if ok("judiciary", "others", "unverified", "invariants", "expired", "generation", "air_gap", "pilot", "canonical") else "FAIL",
                                 "evidence": {k: res[k] for k in ("judiciary", "others", "unverified", "invariants", "expired", "generation", "air_gap", "pilot", "canonical")},
                                 "detail": "A signed JUDICIARY licence enables the Chambers surface and hides advocacy-only screens; every other edition sees no Chambers screen; the host (not the menu) refuses what is hidden; the air-gap switch stays on; pilot-court constraints are configuration. The edition-to-feature map and prices stay with B-S4-22 (Q-S4-06)."}
        for k, v in res.items():
            if v != "ok":
                errors.append(f"test not passing: {TESTS[k]} ({v})")
    else:
        for ac in ("AC-S4-11.1", "AC-S4-11.2", "edition-gating"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
    report = {"batch": "B-S4-17", "fixture_sha256": FIXTURE_SHA256, "acs": acs, "static": static, "errors": errors}
    print(json.dumps(report, indent=2))
    out = ROOT / "eval" / "reports" / "b_s4_17_chambers.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
