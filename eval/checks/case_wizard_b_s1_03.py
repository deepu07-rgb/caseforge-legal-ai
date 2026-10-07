#!/usr/bin/env python3
"""
B-S1-03 structural checks: New Case wizard (PRD §5).

- Jurisdiction enum frozen India | United States | Other
- Wizard fields present; no invented extra product fields in contract
- Rust + TS validation require jurisdiction
- case_create takes wizard payload
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "contracts" / "case_wizard.schema.json"
WIZARD_RS = ROOT / "src-tauri" / "src" / "vault" / "wizard.rs"
MOD_RS = ROOT / "src-tauri" / "src" / "vault" / "mod.rs"
LIB_RS = ROOT / "src-tauri" / "src" / "lib.rs"
TS = ROOT / "src" / "lib" / "caseWizard.ts"
APP = ROOT / "src" / "App.tsx"
APP_MIG = ROOT / "src-tauri" / "migrations" / "app_002_case_wizard.sql"
CASE_MIG = ROOT / "src-tauri" / "migrations" / "case_002_profile.sql"

EXPECTED_J = {"India", "United States", "Other"}
EXPECTED_FIELDS = {
    "cause_title",
    "court",
    "jurisdiction",
    "matter_type",
    "user_side",
    "code_era",
    "language",
}


def main() -> int:
    errors: list[str] = []

    if not SCHEMA.exists():
        errors.append("missing contracts/case_wizard.schema.json")
    else:
        data = json.loads(SCHEMA.read_text(encoding="utf-8"))
        props = set((data.get("properties") or {}).keys())
        if props != EXPECTED_FIELDS:
            errors.append(f"schema properties {props} != {EXPECTED_FIELDS}")
        enum = set((data.get("properties") or {}).get("jurisdiction", {}).get("enum") or [])
        if enum != EXPECTED_J:
            errors.append(f"jurisdiction enum {enum} != {EXPECTED_J}")
        req = set(data.get("required") or [])
        if "jurisdiction" not in req:
            errors.append("jurisdiction not required in schema")
        if data.get("x-caseforge", {}).get("immutability") != ["jurisdiction"]:
            errors.append("jurisdiction immutability not declared")

    for p in (WIZARD_RS, MOD_RS, LIB_RS, TS, APP, APP_MIG, CASE_MIG):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    if WIZARD_RS.exists():
        text = WIZARD_RS.read_text(encoding="utf-8")
        for j in EXPECTED_J:
            if f'"{j}"' not in text:
                errors.append(f"wizard.rs missing jurisdiction {j}")
        if "validate_and_normalize" not in text:
            errors.append("wizard.rs missing validate_and_normalize")

    if MOD_RS.exists():
        text = MOD_RS.read_text(encoding="utf-8")
        if "create_case_with_wizard" not in text:
            errors.append("mod.rs missing create_case_with_wizard")
        if "case_profile" not in text:
            errors.append("mod.rs missing case_profile insert")

    if LIB_RS.exists():
        text = LIB_RS.read_text(encoding="utf-8")
        if "CaseWizardInput" not in text:
            errors.append("lib.rs missing CaseWizardInput")
        if "case_wizard_meta" not in text:
            errors.append("lib.rs missing case_wizard_meta")
        # Ensure create takes wizard not bare label
        if re.search(r"fn case_create\([\s\S]*?label:\s*Option", text):
            errors.append("case_create still uses label-only API")

    if TS.exists():
        text = TS.read_text(encoding="utf-8")
        if "Jurisdiction is required" not in text and "jurisdiction is required" not in text.lower():
            errors.append("caseWizard.ts missing jurisdiction required validation message")
        for j in EXPECTED_J:
            if j not in text:
                errors.append(f"caseWizard.ts missing {j}")

    if APP.exists():
        text = APP.read_text(encoding="utf-8")
        if "New Case wizard" not in text:
            errors.append("App.tsx missing New Case wizard UI")
        if "Select jurisdiction" not in text:
            errors.append("App.tsx missing jurisdiction selector")

    if errors:
        print("FAIL  B-S1-03 case wizard structural")
        for e in errors:
            print(f"  - {e}")
        return 1

    print("PASS  B-S1-03 case wizard structural (PRD §5 fields, jurisdiction required+immutable)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
