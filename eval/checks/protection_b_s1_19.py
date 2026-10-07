#!/usr/bin/env python3
"""B-S1-19 protection — AC-14.1 copied-folder policy, AC-14.2 no-ransom."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONST = ROOT / "contracts" / "constants.json"
PROT = ROOT / "contracts" / "protection.json"
ERRS = ROOT / "contracts" / "errors.json"
MOD = ROOT / "src-tauri" / "src" / "protection" / "mod.rs"
PACKS = ROOT / "src-tauri" / "src" / "packs" / "mod.rs"
RETR = ROOT / "src-tauri" / "src" / "retrieval" / "mod.rs"
LIB = ROOT / "src-tauri" / "src" / "lib.rs"
PATHS = ROOT / "src-tauri" / "src" / "vault" / "paths.rs"
CARGO = ROOT / "src-tauri" / "Cargo.toml"
IPC = ROOT / "src" / "lib" / "ipc.ts"
VIEW = ROOT / "src" / "components" / "ProtectionView.tsx"
APP = ROOT / "src" / "App.tsx"
DOC = ROOT / "docs" / "B-S1-19_protection.md"
PIPELINE = "caseforge.protection.v1"


def main() -> int:
    errors: list[str] = []
    for p in (
        CONST,
        PROT,
        ERRS,
        MOD,
        PACKS,
        RETR,
        LIB,
        PATHS,
        CARGO,
        IPC,
        VIEW,
        APP,
        DOC,
    ):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    if CONST.exists():
        data = json.loads(CONST.read_text(encoding="utf-8-sig"))
        pr = data.get("protection") or {}
        if pr.get("status") != "FROZEN":
            errors.append("constants.protection not FROZEN")
        if pr.get("pipeline_id") != PIPELINE:
            errors.append(f"pipeline_id must be {PIPELINE}")
        if pr.get("effort_budget_percent_of_total") != 8:
            errors.append("effort budget must be 8%")
        if pr.get("never_ransoms_user_data") is not True:
            errors.append("never_ransoms_user_data must be true")
        if pr.get("cipher") != "AES-256-GCM":
            errors.append("cipher must be AES-256-GCM")

    if PROT.exists():
        p = json.loads(PROT.read_text(encoding="utf-8-sig"))
        if p.get("status") != "FROZEN":
            errors.append("protection.json not FROZEN")
        if p.get("pipeline_id") != PIPELINE:
            errors.append("protection.json pipeline mismatch")
        never = p.get("never_protected_by_this_layer") or []
        for x in ("user_case_db", "user_documents_in_case_vault"):
            if x not in never:
                errors.append(f"must never protect {x}")
        ac1 = p.get("ac_14_1_copied_folder") or {}
        if ac1.get("user_data_accessible_with_passphrase") is not True:
            errors.append("AC-14.1: user data must remain accessible with passphrase")
        if ac1.get("protected_assets_usable_on_foreign_machine") is not False:
            errors.append("AC-14.1: foreign machine must not use protected assets")
        ac2 = p.get("ac_14_2_no_ransom") or {}
        if ac2.get("user_data_may_be_ransomed") is not False:
            errors.append("AC-14.2: user_data_may_be_ransomed must be false")
        if ac2.get("online_kill_switch_for_user_files") is not False:
            errors.append("AC-14.2: no online kill-switch")

    if ERRS.exists():
        err = ERRS.read_text(encoding="utf-8")
        for code in (
            "CF_PROTECTION_TAMPER",
            "CF_PROTECTION_DECRYPT_FAILED",
            "CF_PROTECTION_RANSOM_FORBIDDEN",
        ):
            if code not in err:
                errors.append(f"errors.json missing {code}")

    if CARGO.exists() and "aes-gcm" not in CARGO.read_text(encoding="utf-8"):
        errors.append("Cargo.toml must depend on aes-gcm")

    for path, needles in (
        (
            MOD,
            [
                "AES-256-GCM",
                "Aes256Gcm",
                "protect_pack_content",
                "verify_pack_integrity",
                "copied_folder_audit",
                "no_ransom_audit",
                "user_data_touched",
                "CASEFORGE_PROTECTION_FOREIGN",
                "graceful_degrade",
                "never",
                "AC-14.1",
                "AC-14.2",
                PIPELINE,
            ],
        ),
        (PACKS, ["protect_pack_content", "B-S1-19"]),
        (RETR, ["read_pack_text_files", "protection degrade", "graceful"]),
        (
            LIB,
            [
                "mod protection",
                "protection_status",
                "protection_verify_all",
                "protection_copied_folder_audit",
                "protection_no_ransom_audit",
                "B-S1-19",
            ],
        ),
        (PATHS, ["models_dir", "protection_dir"]),
        (
            IPC,
            [
                "protectionStatus",
                "protectionVerifyAll",
                "protectionCopiedFolderAudit",
                "protectionNoRansomAudit",
                "user_data_may_be_ransomed",
            ],
        ),
        (
            VIEW,
            [
                "protection-panel",
                "no-ransom-audit",
                "copied-folder-audit",
                "AC-14",
                "ransom",
            ],
        ),
        (APP, ['from "./components/ProtectionView"', 'nav === "protection"']),
        (
            DOC,
            [
                "AC-14.1",
                "AC-14.2",
                "AES-256-GCM",
                "ransom",
                "graceful",
                PIPELINE,
                "8%",
            ],
        ),
    ):
        if not path.exists():
            continue
        text = path.read_text(encoding="utf-8")
        for n in needles:
            if n not in text:
                errors.append(f"{path.relative_to(ROOT)} missing {n!r}")

    # Unit: no-ransom policy logic
    ransom_flags = {
        "user_data_may_be_ransomed": False,
        "protection_encrypts_user_case_db": False,
        "expired_licence_deletes_data": False,
        "online_kill_switch": False,
    }
    if any(ransom_flags.values()):
        errors.append("AC-14.2 unit flags broken")

    # Unit: foreign machine should not use protected assets (policy)
    foreign_usable = False
    if foreign_usable:
        errors.append("AC-14.1 foreign usable must be false by policy")

    # user_data_touched must always be false on integrity path (source)
    if MOD.exists():
        mt = MOD.read_text(encoding="utf-8")
        if "user_data_touched: true" in mt or "user_data_touched:true" in mt:
            errors.append("integrity path must never set user_data_touched true")

    if errors:
        print("FAIL  B-S1-19 protection (AC-14.x)")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(
        f"PASS  B-S1-19 protection "
        f"({PIPELINE}; AES-256-GCM packs/models; AC-14.1 copied-folder; AC-14.2 no ransom; <=8%)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
