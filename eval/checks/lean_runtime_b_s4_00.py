#!/usr/bin/env python3
"""B-S4-00 lean runtime gate: base install <= owner cap, licensed default model that passes
the B-S3-13 fitness gate, RAM within the (proposed) ceiling, and deterministic engines that
never touch a model.

Each AC is computed from measured evidence; missing evidence is BLOCKED/FAIL, never PASS.
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
C = ROOT / "contracts"
REPORT = ROOT / "eval" / "reports" / "b_s4_00_lean_runtime.json"
BUDGET = json.loads((C / "stage4_resource_budget.json").read_text(encoding="utf-8"))
DEFAULT = json.loads((C / "supported_models.json").read_text(encoding="utf-8"))
ADDON_PATH = C / "supported_models_addon.json"
COMMERCIAL_LICENCES = {"Apache-2.0", "MIT"}
# Held-out evidence only counts if produced with the guard version that ships now (older runs are kept as history).
CURRENT_GUARD = re.search(r'GUARD_VERSION = "([^"]+)"', (ROOT / "sidecars" / "infer_llm" / "infer_llm.py").read_text(encoding="utf-8")).group(1)

# Engines that must work with no model loaded (PRD v4.0 §11.2). Stage 4 batches append theirs.
DETERMINISTIC_ENGINES = [
    "src-tauri/src/integrity/mod.rs",
    "src-tauri/src/proposition.rs",
    "src-tauri/src/retrieval/mod.rs",
    "src-tauri/src/authority/mod.rs",
    "src-tauri/src/packs/mod.rs",
    "src-tauri/src/guardrails/mod.rs",
    "src-tauri/src/answer_feedback.rs",
    "src-tauri/src/reasoning.rs",
    "src-tauri/src/deadlines.rs",
    "src-tauri/src/foundation.rs",
    "src-tauri/src/issue_matrix.rs",
    "src-tauri/src/treatment.rs",
    "src-tauri/src/record_only.rs",
    "src-tauri/src/chambers_lookup.rs",
    "src-tauri/src/cause_list.rs",
    "src-tauri/src/judgment_scaffold.rs",
    "src-tauri/src/g8.rs",
]
LLM_TOKENS = re.compile(r"infer_llm|ollama|openai|llm_chat|llama_cpp|chat_with_timeout", re.I)


def run_json(cmd: list[str]) -> dict:
    proc = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, timeout=600)
    return json.loads(proc.stdout)


def licence_hash(model: str) -> tuple[str | None, str]:
    try:
        proc = subprocess.run(["ollama", "show", "--license", model], capture_output=True, timeout=30)
    except (OSError, subprocess.SubprocessError) as exc:
        return None, f"ollama show failed: {exc}"
    text = proc.stdout.decode("utf-8", errors="replace").replace("\r\n", "\n").strip()
    if proc.returncode != 0 or not text:
        return None, "no licence text shipped with the model"
    return hashlib.sha256(text.encode("utf-8")).hexdigest(), text.splitlines()[0][:120]


def sidecar_tier_logic() -> dict:
    """Fail-closed tier selection, low-RAM add-on refusal, and hidden-reasoning stripping."""
    import importlib.util
    import os

    def load(tier: str):
        os.environ["CASEFORGE_LLM_TIER"] = tier
        spec = importlib.util.spec_from_file_location(f"cf_llm_{tier}", ROOT / "sidecars" / "infer_llm" / "infer_llm.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)  # type: ignore[union-attr]
        return module

    results: dict[str, bool] = {}
    try:
        bogus = load("bogus")
        results["unknown_tier_fails_closed"] = bool(bogus.TIER_ERROR) and bool(bogus.tier_info()["tier_error"])
        addon = load("default")
        # No add-on manifest ships now (OD-S4-MODEL-03); exercise the same rules with a synthetic add-on tier.
        addon.CONTRACTS["addon"] = addon.CONTRACTS["default"]
        addon.TIER, addon.REQUESTED_TIER, addon.TIER_ERROR = "addon", "addon", None
        addon.total_ram_gb = lambda: 6.0
        results["addon_refused_under_8gb"] = "requires >=" in (addon.tier_info()["tier_error"] or "")
        addon.total_ram_gb = lambda: 16.0
        results["addon_allowed_on_full_tier"] = addon.tier_info()["tier_error"] is None
        strip = addon.strip_reasoning
        results["reasoning_stripped"] = (strip("<think>plan</think>NOT VERIFIED") == "NOT VERIFIED"
                                         and strip("Answer <THINK>unterminated") == "Answer" and strip("plain") == "plain")
        tiers = addon.handle({"jsonrpc": "2.0", "id": 1, "method": "llm.tiers"})["result"]
        results["no_auto_download"] = tiers["auto_download"] is False
    finally:
        os.environ.pop("CASEFORGE_LLM_TIER", None)
    return {"status": "PASS" if all(results.values()) else "FAIL", "checks": results}


def main() -> int:
    acs: dict[str, dict] = {}

    # AC-S4-00.1 base install under the owner hard cap
    size = run_json([sys.executable, "scripts/check_size_budget.py", "--json"])
    base, cap = size["base_install_gib"], BUDGET["base_install"]["hard_cap_gib"]
    measured = size["external_llm_bytes"] > 0
    waiver = size.get("base_install_cap_waiver")
    acs["AC-S4-00.1"] = {
        "status": "BLOCKED" if not measured else ("PASS" if base <= cap else ("WAIVED" if waiver else "FAIL")),
        "waiver": waiver,
        "measured_gib": base, "cap_gib": cap, "target_gib": BUDGET["base_install"]["target_gib"],
        "breakdown_gib": {"default_model": size["external_llm_gib"], "speech": size["external_speech_gib"],
                          "runtime_core": size["ollama_runtime_core_gib"], "app": size["client_gib"]},
        "excluded_optional_gib": {"addon_model": size["addon_llm_gib"], "gpu_accelerators": size["ollama_optional_gpu_gib"]},
        "note": "Pre-installer estimate. Runtime core assumes the installer ships Ollama CPU+Vulkan backends only; verified on the built installer in B-S4-23.",
    }

    # AC-S4-00.2 licence verified from the text shipped with the model
    model = DEFAULT["model"]
    digest, first_line = licence_hash(model["name"])
    lic_ok = (model.get("license") in COMMERCIAL_LICENCES and digest is not None and model.get("license_text_sha256") == digest)
    acs["AC-S4-00.2"] = {"status": "PASS" if lic_ok else "FAIL", "model": model["name"], "declared": model.get("license"),
                         "licence_text_sha256": digest, "pinned_sha256": model.get("license_text_sha256"), "licence_first_line": first_line}

    # AC-S4-00.3 fitness gate on the default tier (live evidence only)
    fit_path = ROOT / "eval" / "reports" / "b_s3_13_model_eval.json"
    fit = json.loads(fit_path.read_text(encoding="utf-8")) if fit_path.is_file() else {}
    fit_model = (fit.get("status") or {}).get("model")
    fit_ok = fit.get("mode") == "live" and fit.get("tier", "default") == "default" and fit_model == model["name"] and fit.get("passed") is True
    acs["AC-S4-00.3"] = {"status": "PASS" if fit_ok else "FAIL", "report_model": fit_model, "mode": fit.get("mode"),
                         "score": fit.get("score"), "critical_pass": fit.get("critical_pass"), "passed": fit.get("passed")}

    # AC-S4-00.4 RAM: resident model size vs proposed ceiling (BLOCKED until owner approves Q-S4-12)
    resident = (fit.get("performance") or {}).get("resident_model_size_gb") if fit_ok else None
    ceiling = BUDGET["ram"]["proposed_default_model_peak_rss_gib_max"]
    approved = BUDGET["ram"]["proposed_status"] == "OWNER_APPROVED"
    status = "BLOCKED" if not approved else ("PASS" if resident is not None and resident <= ceiling else "FAIL")
    acs["AC-S4-00.4"] = {"status": status, "resident_model_gb": resident, "proposed_ceiling_gib": ceiling,
                         "reason": None if approved else "Q-S4-12 RAM ceiling awaiting owner approval; measurement recorded"}

    # AC-S4-00.5 Stage 3 grounding gates on the default model
    # AC-S4-00.5 Stage 3 grounding gates re-run on the default model (sidecar-level; Rust unit tests separate)
    def load_report(name: str) -> dict:
        path = ROOT / "eval" / "reports" / name
        return json.loads(path.read_text(encoding="utf-8")) if path.is_file() else {}
    s314, s316 = load_report("b_s3_14_grounding_verifier.json"), load_report("b_s3_16_grounded_mock_court.json")
    s316_models = {((t.get("provenance") or {}).get("model_digest")) for t in s316.get("live_turns", [])}
    s316_ok = s316.get("mode") == "live" and bool(s316.get("live_turns")) and s316_models == {model["digest"]} and all(t.get("passed") for t in s316["live_turns"])
    acs["AC-S4-00.5"] = {
        "status": "PASS" if s314.get("passed") is True and s316_ok else "FAIL",
        "b_s3_14_passed": s314.get("passed"), "b_s3_16_live_on_default": s316_ok,
        "b_s3_16_turns": [{"name": t.get("name"), "passed": t.get("passed")} for t in s316.get("live_turns", [])],
        "not_run_here": "Rust host unit tests (cargo test proposition::tests etc.) - local toolchain broken (Strawberry Perl missing; msvc-env.ps1 targets an absent VS 2022 BuildTools path).",
    }

    # AC-S4-00.6 held-out set on the default model + prompt, against the currently pinned case file
    case_sha = hashlib.sha256((ROOT / "golden" / "G-MODEL-HELDOUT" / "cases.json").read_bytes()).hexdigest()
    held = None
    for path in sorted((ROOT / "eval" / "reports").glob("b_s4_00_heldout_*.json")):
        r = json.loads(path.read_text(encoding="utf-8"))
        if r.get("model") == model["name"] and r.get("prompt_version") == DEFAULT["prompt"]["version"] and r.get("case_file_sha256") == case_sha and r.get("guard_version") == CURRENT_GUARD:
            held = (path.name, r)
    acs["AC-S4-00.6"] = {
        "status": "PASS" if held and held[1].get("passed") else ("FAIL" if held else "BLOCKED"),
        "report": held[0] if held else None, "score": held[1].get("score") if held else None,
        "failed": [x["id"] for x in held[1]["results"] if not x["passed"]] if held else None,
        "reason": None if held else "no held-out run for the current default model/prompt/case file",
    }

    # AC-S4-00.7 injection held-out set (authored before the guard) on the default model + prompt
    inj_sha = hashlib.sha256((ROOT / "golden" / "G-MODEL-HELDOUT-INJ" / "cases.json").read_bytes()).hexdigest()
    inj = None
    for path in sorted((ROOT / "eval" / "reports").glob("b_s4_00_heldout_inj_*.json")):
        r = json.loads(path.read_text(encoding="utf-8"))
        if r.get("model") == model["name"] and r.get("prompt_version") == DEFAULT["prompt"]["version"] and r.get("case_file_sha256") == inj_sha and r.get("guard_version") == CURRENT_GUARD:
            inj = (path.name, r)
    acs["AC-S4-00.7"] = {
        "status": "PASS" if inj and inj[1].get("passed") else ("FAIL" if inj else "BLOCKED"),
        "report": inj[0] if inj else None, "score": inj[1].get("score") if inj else None,
        "failed": [x["id"] for x in inj[1]["results"] if not x["passed"]] if inj else None,
    }

    # AC-S4-00.8 deterministic guard suite
    guard = subprocess.run([sys.executable, "eval/checks/injection_guard_b_s4_00.py"], cwd=ROOT, capture_output=True, text=True, timeout=120)
    acs["AC-S4-00.8"] = {"status": "PASS" if guard.returncode == 0 else "FAIL", "detail": (guard.stdout.strip().splitlines() or [""])[-1]}

    # Deterministic engines never reference a model
    offenders = []
    for rel in DETERMINISTIC_ENGINES:
        text = (ROOT / rel).read_text(encoding="utf-8")
        hits = sorted({m.group(0) for m in LLM_TOKENS.finditer(text)})
        if hits:
            offenders.append({"file": rel, "tokens": hits})
    deterministic = {"status": "PASS" if not offenders else "FAIL", "engines": DETERMINISTIC_ENGINES, "offenders": offenders}

    # Tier contract sanity
    addon = json.loads(ADDON_PATH.read_text(encoding="utf-8")) if ADDON_PATH.is_file() else None
    # An add-on tier is optional; when present it must be a different model, opt-in, and outside the base cap.
    tiers_ok = model.get("install_policy") == "BASE_INSTALL" and (addon is None or (
        addon["model"].get("install_policy") == "OPTIONAL_ADDON" and addon["model"].get("counts_toward_base_install_cap") is False
        and addon["model"]["name"] != model["name"]))

    report = {
        "batch": "B-S4-00", "generated_at": datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "default_model": model["name"], "addon_model": addon["model"]["name"] if addon else None,
        "tier_contracts_valid": tiers_ok, "acceptance": acs, "deterministic_engines": deterministic,
        "sidecar_tier_logic": (logic := sidecar_tier_logic()),
    }
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
    for ac, r in acs.items():
        print(f"{r['status']:8} {ac}")
    print(f"{deterministic['status']:8} deterministic engines model-free ({len(DETERMINISTIC_ENGINES)} engines)")
    print(f"{'PASS' if tiers_ok else 'FAIL':8} tier contracts (default BASE_INSTALL; add-on {'absent' if addon is None else 'OPTIONAL_ADDON'})")
    print(f"{logic['status']:8} sidecar tier logic {logic['checks']}")
    hard_fail = any(r["status"] == "FAIL" for r in acs.values()) or deterministic["status"] == "FAIL" or not tiers_ok or logic["status"] == "FAIL"
    return 1 if hard_fail else 0


if __name__ == "__main__":
    sys.exit(main())
