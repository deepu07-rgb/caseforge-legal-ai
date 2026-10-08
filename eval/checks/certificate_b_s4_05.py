#!/usr/bin/env python3
"""B-S4-05 Audit Certificate gate.

Independent evidence (no Rust involved):
  * the committed reference certificate (signed by the Rust signer with a fixed TEST seed) validates against the
    frozen certificate contract AND its embedded audit report against the audit-report contract;
  * scripts/verify_certificate.py (stdlib-only Ed25519) accepts it and rejects five kinds of tampering.
Rust evidence (pass --cargo-log): the certificate unit tests, incl. byte-for-byte reproduction of the reference.

Usage: python eval/checks/certificate_b_s4_05.py [--cargo-log PATH]
"""
from __future__ import annotations

import argparse
import ast
import copy
import importlib.util
import json
import re
import sys
from pathlib import Path

import jsonschema

ROOT = Path(__file__).resolve().parents[2]
REFERENCE = ROOT / "golden" / "G-AUDIT" / "fixtures" / "certificate_reference.json"
VERIFIER = ROOT / "scripts" / "verify_certificate.py"
RUST_TESTS = [
    "certificate::tests::issued_certificate_verifies_and_matches_contract",
    "certificate::tests::any_change_breaks_verification",
    "certificate::tests::disclosure_rules",
    "certificate::tests::stale_report_is_refused",
    "certificate::tests::reference_certificate_is_reproducible_and_cross_verifiable",
]
STDLIB_OK = {"__future__", "argparse", "hashlib", "json", "re", "sys", "pathlib"}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    args = ap.parse_args()
    acs: dict[str, dict] = {}
    errors: list[str] = []

    spec = importlib.util.spec_from_file_location("verify_certificate", VERIFIER)
    vc = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(vc)  # type: ignore[union-attr]
    imports = {n.names[0].name.split(".")[0] if isinstance(n, ast.Import) else (n.module or "").split(".")[0]
               for n in ast.walk(ast.parse(VERIFIER.read_text(encoding="utf-8"))) if isinstance(n, (ast.Import, ast.ImportFrom))}
    stdlib_only = imports <= STDLIB_OK

    ref = json.loads(REFERENCE.read_text(encoding="utf-8"))
    cert, draft = ref.get("certificate") or {}, (ref.get("draft_text") or "").encode("utf-8")
    if not cert:
        errors.append("reference certificate not generated (run the Rust test with CF_WRITE_CERT_REFERENCE=1)")
        independent = {"reference_valid": False}
    else:
        contract_errors = []
        for schema_file, instance in (("audit_certificate.schema.json", cert), ("audit_report.schema.json", cert.get("audit_report"))):
            schema = json.loads((ROOT / "contracts" / schema_file).read_text(encoding="utf-8"))
            for e in jsonschema.Draft202012Validator(schema, format_checker=jsonschema.FormatChecker()).iter_errors(instance):
                contract_errors.append(f"{schema_file}: {'/'.join(map(str, e.path))}: {e.message[:120]}")
        valid, _ = vc.verify(cert, draft)
        tampers = {}
        tampers["draft_one_extra_byte"] = vc.verify(cert, draft + b" ")[0]
        c = copy.deepcopy(cert); c["audit_report"]["items"][0]["result"] = "VERIFIED_WITH_NOTE"; tampers["report_item_changed"] = vc.verify(c, draft)[0]
        c = copy.deepcopy(cert); c["summary"]["total_items"] += 1; tampers["summary_changed"] = vc.verify(c, draft)[0]
        c = copy.deepcopy(cert); s = c["signing"]["signature_hex"]; c["signing"]["signature_hex"] = ("1" if s[0] == "0" else "0") + s[1:]; tampers["signature_byte_flipped"] = vc.verify(c, draft)[0]
        c = copy.deepcopy(cert); c["scope_statement"] = "This filing is court-approved."; tampers["scope_statement_changed"] = vc.verify(c, draft)[0]
        independent = {"contract_errors": contract_errors, "reference_valid": valid, "tampering_accepted": {k: v for k, v in tampers.items() if v}}
        if contract_errors:
            errors.extend(contract_errors)

    rust = {}
    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        rust = {t.split("::")[-1]: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for t in RUST_TESTS}
    rust_ok = bool(rust) and all(v == "ok" for v in rust.values())
    indep_ok = independent.get("reference_valid") is True and not independent.get("tampering_accepted") and not independent.get("contract_errors")

    def status(ok_independent: bool) -> str:
        if not rust:
            return "BLOCKED" if ok_independent else "FAIL"
        return "PASS" if ok_independent and rust_ok else "FAIL"

    acs["AC-S4-01.4"] = {"status": status(indep_ok), "independent": independent, "rust": rust or "pass --cargo-log"}
    disclosure = (cert.get("disclosure") or {}).get("text") or ""
    clean = not any(re.search(rf"(?i)\b{re.escape(w)}\b", (cert.get("scope_statement") or "") + " " + disclosure) for w in vc.PROHIBITED)
    acs["AC-S4-01.5"] = {"status": status(clean and bool(cert)), "reference_copy_clean": clean, "rust_disclosure_rules": rust.get("disclosure_rules")}

    lib = (ROOT / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    vault = (ROOT / "src-tauri" / "src" / "vault" / "mod.rs").read_text(encoding="utf-8")
    cert_rs = (ROOT / "src-tauri" / "src" / "certificate.rs").read_text(encoding="utf-8")
    structure = {
        "verifier_stdlib_only": stdlib_only,
        "seed_only_in_encrypted_vault": "audit_signing_seed_v1" in vault and "fs::write" not in vault.split("pub fn audit_signing_seed")[1].split("\n    pub fn ")[0],
        "certificate_never_writes_seed": "seed" not in "".join(l for l in cert_rs.splitlines() if "fs::write" in l),
        "commands_registered": all(re.search(rf"generate_handler!\[[^\]]*\b{c}\b", lib, re.S) for c in ("audit_certificate_issue", "audit_certificate_verify")),
        "ui_wired": "<CertificatePanel" in (ROOT / "src" / "components" / "AuditPanel.tsx").read_text(encoding="utf-8"),
    }
    acs["STRUCTURE"] = {"status": "PASS" if all(structure.values()) else "FAIL", "checks": structure}

    (ROOT / "eval" / "reports" / "b_s4_05_certificate.json").write_text(json.dumps({"batch": "B-S4-05", "acceptance": acs, "errors": errors}, indent=2) + "\n", encoding="utf-8", newline="\n")
    for k, r in acs.items():
        print(f"{r['status']:8} {k}")
    for e in errors:
        print(f"FAIL     {e}")
    return 1 if errors or any(r["status"] == "FAIL" for r in acs.values()) else 0


if __name__ == "__main__":
    sys.exit(main())
