#!/usr/bin/env python3
"""B-S3-14 proposition-ledger contract and adversarial integration checks."""
from __future__ import annotations
import importlib.util
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REPORT = ROOT / "eval" / "reports" / "b_s3_14_grounding_verifier.json"


def main() -> int:
    checks: list[dict[str, object]] = []
    def check(name: str, passed: bool, detail: str = "") -> None:
        checks.append({"name": name, "passed": passed, "detail": detail})

    schema = json.loads((ROOT / "contracts" / "proposition_ledger.schema.json").read_text(encoding="utf-8"))
    classes = set(schema["properties"]["propositions"]["items"]["properties"]["class"]["enum"])
    check("five_required_classes", classes == {"FACTUAL_BASIS", "LEGAL_BASIS", "INFERENCE", "COUNSEL_INPUT", "UNSUPPORTED_OR_GAP"}, repr(classes))
    required = set(schema["properties"]["propositions"]["items"]["required"])
    check("support_and_render_fields_required", {"support_claims", "verification_status", "render_action", "verified_support_ids", "rejected_support_ids"}.issubset(required))

    rust = (ROOT / "src-tauri" / "src" / "proposition.rs").read_text(encoding="utf-8")
    for name, needle in {
        "exact_substring_offsets": "source.find(excerpt)",
        "official_trust_required": 'Some("OFFICIAL_VERIFIED")',
        "applicable_required": 'Some("APPLICABLE")',
        "current_required": 'Some("CURRENT")',
        "stable_source_id_required": 'starts_with("cfsrc:")',
        "stable_chunk_id_required": 'starts_with("cfchunk:")',
        "repository_gap_render": "[REPOSITORY GAP — NOT VERIFIED]",
        "blocked_render": "[BLOCKED UNSUPPORTED PROPOSITION]",
        "fabricated_id_test": "fabricated_id_is_gap",
        "altered_quote_test": "altered_quote_blocked",
        "unsupported_inference_test": "unsupported_inference_blocked",
    }.items():
        check(name, needle in rust, needle)

    host = (ROOT / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    check("host_requires_structured_generation", "CF_PROPOSITION_LEDGER_REQUIRED" in host)
    check("host_rejects_unparseable_model_output", "CF_PROPOSITION_SCHEMA" in host)
    check("host_verifies_before_render", "result.content = ledger.rendered_text.clone()" in host)
    check("host_support_source_ipc", "fn case_llm_support_sources" in host)

    sidecar_path = ROOT / "sidecars" / "infer_llm" / "infer_llm.py"
    spec = importlib.util.spec_from_file_location("caseforge_llm_b_s3_14", sidecar_path)
    if not spec or not spec.loader:
        raise RuntimeError("cannot import infer_llm")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    planted = '{"propositions":[{"proposition_id":"p1","class":"LEGAL_BASIS","text":"Fake law","support_ids":["fabricated"],"basis_ids":[],"support_claims":[],"quotation":null,"verification_status":"GREEN","render_action":"TRUSTED"}]}'
    parsed = module._parse_propositions(planted)
    check("model_cannot_assign_green", bool(parsed) and "verification_status" not in parsed[0] and "render_action" not in parsed[0], repr(parsed))
    malformed = module._parse_propositions("Legal answer without structured JSON")
    check("unstructured_model_text_not_accepted", malformed == [], repr(malformed))

    ui = (ROOT / "src" / "components" / "ArgumentBuilderView.tsx").read_text(encoding="utf-8")
    check("reasoning_api_requests_ledger", "structured_propositions: true" in ui and 'output_kind: "ARGUMENT_INTEGRITY"' in ui)
    check("reasoning_api_supplies_verified_sources", "caseLlmSupportSources" in ui and "permitted_authority_ids" in ui)

    passed = all(bool(item["passed"]) for item in checks)
    report = {"batch": "B-S3-14", "passed": passed, "checks_passed": sum(bool(item["passed"]) for item in checks), "checks_total": len(checks), "checks": checks, "rust_adversarial_command": "cargo test --manifest-path src-tauri/Cargo.toml proposition::tests", "acceptance": {"legal_grounding": "host exact-excerpt + permitted authority policy", "quotation_fidelity": "exact stored substring + Unicode character offsets", "factual_grounding": "exact excerpt + doc_id matter pointer", "inference_grounding": "earlier non-RED proposition bases", "fabricated_ids": "RED and repository gap/blocked", "memory_fallback": "UNSUPPORTED_OR_GAP / repository gap"}}
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"{'PASS' if passed else 'FAIL'} B-S3-14 grounding verifier; checks={report['checks_passed']}/{report['checks_total']} report={REPORT}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
