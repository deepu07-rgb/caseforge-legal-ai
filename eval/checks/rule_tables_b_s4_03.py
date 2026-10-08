#!/usr/bin/env python3
"""B-S4-03 (core) Signed rule tables gate.

AC-S4-RT.1 / RT.2 behaviour is proven by Rust unit tests (src-tauri/src/rule_tables.rs, code_bridge.rs) against the
G-CODEBRIDGE exploits; pass the `cargo test --lib` log with --cargo-log (otherwise BLOCKED). The B-S3-09 malicious-pack
suite must stay green. AC-S4-RT.3 is a static audit: no jurisdiction-name branching in Stage 4 feature code (Rust
modules and Stage 4 UI); jurisdiction behaviour comes from data (rule tables, vocabularies).

Usage: python eval/checks/rule_tables_b_s4_03.py [--cargo-log PATH]
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src-tauri" / "src"
TESTS = {
    "golden_table": "rule_tables::tests::golden_table_validates_and_queries_by_date",
    "exploits": "rule_tables::tests::every_rule_table_exploit_is_rejected",
    "manifest_precheck": "rule_tables::tests::manifest_precheck_rejects_custom_and_v1_rule_tables",
    "dev_only_rows": "rule_tables::tests::unsigned_rows_install_only_as_development_only",
    "dev_only_never_verifies": "code_bridge::tests::unsigned_rows_never_verify_and_signed_rows_annotate_audit",
}
B_S3_09 = re.compile(r"test (packs::tests::b_s3_09_\w+) \.\.\. (ok|FAILED)")
STAGE4_RUST = ["audit.rs", "answer_feedback.rs", "certificate.rs", "opposition.rs", "witness.rs", "impeach.rs", "rule_tables.rs", "code_bridge.rs"]
STAGE4_UI = ["components/AuditPanel.tsx", "components/CertificatePanel.tsx", "components/AnswerFeedbackPanel.tsx", "components/OppositionView.tsx",
             "components/WitnessView.tsx", "components/CodeBridgePanel.tsx", "lib/impeachKit.ts"]
# A jurisdiction name used as a literal to branch on (string literals only; comments ignored).
BRANCH = re.compile(r"""["'](?:India|Indian|United States|US|USA|U\.S\.|IN|united states|india)["']|is_us\(|jurisdiction\s*===?\s*["'](?!\*["'])""")


def code_only(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    if path.suffix == ".rs":
        text = text.split("#[cfg(test)]")[0]
    return "\n".join(line for line in text.splitlines() if not line.strip().startswith(("//", "*", "/*")))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    args = ap.parse_args()
    acs: dict[str, dict] = {}

    packs = (SRC / "packs" / "mod.rs").read_text(encoding="utf-8")
    install = packs[packs.find("pub fn install_pack_from_dir"):]
    rt = (SRC / "rule_tables.rs").read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    structure_rt1 = {
        "install_prechecks_manifest": "crate::rule_tables::precheck_manifest(&man)?" in install,
        "install_validates_tables": "crate::rule_tables::validate_pack(paths, source_dir, &man)?" in install,
        "load_revalidates": "fn load_installed" in rt and "validate_file(dir, &man, file, &index)" in rt,
        "authority_must_be_official": "not OFFICIAL_VERIFIED" in rt,
        "table_bound_to_signed_manifest": 'role.as_deref() == Some("RULE_TABLE")' in rt,
    }
    structure_rt2 = {
        "signed_requires_signoff": "fn signed(&self)" in rt and 'self.trust_state == "SIGNED_OFF" && self.signed_by.is_some()' in rt,
        "audit_note_only_from_signed": 'a.trust == "SIGNED_OFF"' in (SRC / "code_bridge.rs").read_text(encoding="utf-8"),
    }

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        b309 = dict(B_S3_09.findall(log))
        ok = lambda *k: all(res[x] == "ok" for x in k)  # noqa: E731
        acs["AC-S4-RT.1"] = {"status": "PASS" if ok("golden_table", "exploits", "manifest_precheck") and all(structure_rt1.values()) else "FAIL",
                             "evidence": {k: res[k] for k in ("golden_table", "exploits", "manifest_precheck")}, "checks": structure_rt1,
                             "detail": "8 G-CODEBRIDGE exploits rejected: dangling chunk, non-official authority, overlapping windows, forged sign-off, custom pack, MODIFIED without both texts, kind mismatch, inverted window"}
        acs["AC-S4-RT.2"] = {"status": "PASS" if ok("dev_only_rows", "dev_only_never_verifies") and all(structure_rt2.values()) else "FAIL",
                             "evidence": {k: res[k] for k in ("dev_only_rows", "dev_only_never_verifies")}, "checks": structure_rt2}
        acs["B-S3-09_MALICIOUS_PACK_SUITE"] = {"status": "PASS" if b309 and all(v == "ok" for v in b309.values()) else "FAIL", "evidence": b309}
    else:
        for ac in ("AC-S4-RT.1", "AC-S4-RT.2", "B-S3-09_MALICIOUS_PACK_SUITE"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}

    hits = []
    for rel in STAGE4_RUST:
        for i, line in enumerate(code_only(SRC / rel).splitlines(), 1):
            if BRANCH.search(line):
                hits.append(f"src-tauri/src/{rel}: {line.strip()[:120]}")
    for rel in STAGE4_UI:
        p = ROOT / "src" / rel
        if p.is_file():
            for line in code_only(p).splitlines():
                if BRANCH.search(line):
                    hits.append(f"src/{rel}: {line.strip()[:120]}")
    acs["AC-S4-RT.3"] = {"status": "PASS" if not hits else "FAIL", "scanned": STAGE4_RUST + STAGE4_UI, "jurisdiction_branches": hits}

    out = {"batch": "B-S4-03", "scope": "core: rule-table install validation, query API, exploit suite; pack-tool authoring support deferred", "acceptance": acs}
    (ROOT / "eval" / "reports" / "b_s4_03_rule_tables.json").write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8", newline="\n")
    for k, r in acs.items():
        print(f"{r['status']:8} {k}")
    for h in hits:
        print(f"         branch: {h}")
    return 1 if any(r["status"] == "FAIL" for r in acs.values()) else 0


if __name__ == "__main__":
    sys.exit(main())
