#!/usr/bin/env python3
"""B-S4-28 gate: step-by-step reasoning with best-of-n verification.

What this proves without a model (CI): the selection engine, driven by scripted candidate answers (golden/G-REASON),
never calls an answer verified unless the host verifier verified every step; picks the best-verified candidate; stops
early; repairs; honours a time budget; skips unusable candidates. The sidecar's sample variants are deterministic, leave
the contract's settings untouched for variant 0, and differ from each other.

What it cannot prove without the local model: that sampling several candidates actually raises the share of verified
answers for the 7B model. That is reported as BLOCKED (AC-S4-28.2) until measured on a machine with the model.

Usage: python eval/checks/reasoning_b_s4_28.py [--cargo-log PATH]
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "golden" / "G-REASON" / "fixtures" / "best_of_n.json"
FIXTURE_SHA256 = "24cb84dbf711cc54a3a85012598db1cb31fa3cd8618f4a6f1f1ed34f8492c194"  # committed before the engine (be3c43b)
TESTS = {
    "scenarios": "reasoning::tests::golden_best_of_n_scenarios",
    "fabrication": "reasoning::tests::a_fabricated_excerpt_never_reaches_a_verified_answer",
    "prompts": "reasoning::tests::prompts_carry_the_question_the_order_and_the_failures",
    "depth": "reasoning::tests::depth_parses_and_sets_the_number_of_candidates",
}
LLM_TOKENS = re.compile(r"infer_llm|ollama|openai|llm_chat|chat_with_timeout|reqwest", re.I)
PROHIBITED_UI = ["fake", "fabricated", "court-approved", "guaranteed", "good law", "valid law", "predicts", "decides"]


def sidecar():
    os.environ["CASEFORGE_LLM_TIER"] = "default"
    spec = importlib.util.spec_from_file_location("cf_infer_llm_b_s4_28", ROOT / "sidecars" / "infer_llm" / "infer_llm.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)  # type: ignore[union-attr]
    return module


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    args = ap.parse_args()
    errors: list[str] = []
    acs: dict[str, dict] = {}

    if hashlib.sha256(FIXTURE.read_bytes()).hexdigest() != FIXTURE_SHA256:
        errors.append("G-REASON fixture changed since it was pinned")
    scenarios = json.loads(FIXTURE.read_text(encoding="utf-8"))["scenarios"]

    # Sample variants in the sidecar.
    side = sidecar()
    contract = side.GENERATION
    v0 = side._options(512, 0)
    default_ok = v0 == {"temperature": float(contract["temperature"]), "top_p": float(contract["top_p"]), "seed": int(contract["seed"]), "num_ctx": int(contract["num_ctx"]), "num_predict": 512}
    variants = [side._options(512, k) for k in range(8)]
    distinct = len({(v["seed"], v["temperature"]) for v in variants}) == 8
    reproducible = all(side._options(512, k) == variants[k] for k in range(8))
    bounded = side._options(512, 99) == variants[7] and side._options(512, -3) == variants[0] and side._options(512, None) == variants[0]
    default_path_unchanged = side._options(512) == v0
    variant_ok = default_ok and distinct and reproducible and bounded and default_path_unchanged
    if not variant_ok:
        errors.append(f"sidecar variants: default={default_ok} distinct={distinct} reproducible={reproducible} bounded={bounded}")

    # Static properties of the host code.
    rs = (ROOT / "src-tauri" / "src" / "reasoning.rs").read_text(encoding="utf-8")
    engine = rs.split("#[cfg(test)]")[0]
    lib = (ROOT / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    cmd = lib[lib.index("fn reasoning_run"):]
    cmd = cmd[: cmd.index("\n}\n")]
    ui = (ROOT / "src" / "components" / "ReasoningPanel.tsx").read_text(encoding="utf-8")
    static = {
        "engine_does_not_call_a_model": not LLM_TOKENS.search(engine),
        "scores_come_from_the_host_verifier": "proposition::verify(" in engine and "verification_status" in engine,
        "verified_answer_requires_every_step_verified": "self.red == 0 && self.conclusion_ok" in engine and 'verified_answer: status == "VERIFIED"' in engine,
        "command_checks_input_and_output_guards": "check_user_input" in cmd and "check_model_output" in cmd,
        "command_requires_controlled_beta_model": "controlled_beta_ready" in cmd,
        "command_uses_working_set_authorities_only": "mock_court_retrieval::retrieve" in cmd and "readiness" in cmd,
        "ui_vocabulary_honest": not any(re.search(rf"(?i)['\"`>][^'\"`<]*\b{re.escape(w)}\b", ui) for w in PROHIBITED_UI),
        "ui_never_shows_unverified_as_verified": "NO_VERIFIED_ANSWER" in ui and "Nothing verified" in ui,
    }
    for k, v in static.items():
        if not v:
            errors.append(f"static check failed: {k}")

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = all(v == "ok" for v in res.values())
        acs["AC-S4-28.1"] = {
            "status": "PASS" if ok and variant_ok and all(static.values()) and not errors else "FAIL",
            "evidence": {**res, "scenarios_in_golden": len(scenarios), "sidecar_variants": variant_ok, "static": static},
            "detail": "An answer is called verified only if the host verified every step; the best-verified of several candidates is kept; a fabricated excerpt never reaches a verified answer in any depth.",
        }
    else:
        acs["AC-S4-28.1"] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
    acs["AC-S4-28.2"] = {
        "status": "BLOCKED",
        "reason": "Whether best-of-n raises the share of fully verified answers for the 7B model must be measured with the model on the owner's machine; the threshold (e.g. +X points of verified answers over a single attempt on held-out questions) awaits owner approval (Q-S4-01). The engine and its guarantees are proven above with scripted candidates, not with the model.",
    }
    report = {"batch": "B-S4-28", "fixture_sha256": FIXTURE_SHA256, "acs": acs, "errors": errors}
    print(json.dumps(report, indent=2))
    out = ROOT / "eval" / "reports" / "b_s4_28_reasoning.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
