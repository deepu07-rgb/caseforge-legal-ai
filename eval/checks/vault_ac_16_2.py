#!/usr/bin/env python3
"""
AC-16.2 structural + contract checks (B-S1-02).

Live SQLCipher round-trip is exercised by `cargo test -p caseforge --lib`
when MSVC + OpenSSL build deps are available. This script verifies:

1. Q-DB-01 Argon2id freeze in contracts/constants.json
2. Rust KDF constants match the frozen contract
3. Vault module + migrations + commands exist
4. Error code CF_VAULT_PASSPHRASE_INVALID is defined
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONSTANTS = ROOT / "contracts" / "constants.json"
KDF_RS = ROOT / "src-tauri" / "src" / "vault" / "kdf.rs"
MOD_RS = ROOT / "src-tauri" / "src" / "vault" / "mod.rs"
LIB_RS = ROOT / "src-tauri" / "src" / "lib.rs"
ERRORS = ROOT / "contracts" / "errors.json"
APP_SQL = ROOT / "src-tauri" / "migrations" / "app_001_init.sql"
CASE_SQL = ROOT / "src-tauri" / "migrations" / "case_001_init.sql"


def main() -> int:
    errors: list[str] = []

    if not CONSTANTS.exists():
        errors.append("missing constants.json")
        print_fail(errors)
        return 1

    data = json.loads(CONSTANTS.read_text(encoding="utf-8"))
    crypto = data.get("crypto") or {}
    argon = crypto.get("argon2id") or {}
    if crypto.get("status") != "FROZEN" or argon.get("status") != "FROZEN":
        errors.append("crypto.argon2id not FROZEN in constants.json (Q-DB-01)")
    expected = {
        "memory_kib": 65536,
        "iterations": 3,
        "parallelism": 4,
        "salt_len_bytes": 16,
        "key_len_bytes": 32,
        "params_version": 1,
    }
    for k, v in expected.items():
        if argon.get(k) != v:
            errors.append(f"constants argon2id.{k}={argon.get(k)!r} expected {v!r}")

    if not KDF_RS.exists():
        errors.append("missing vault/kdf.rs")
    else:
        text = KDF_RS.read_text(encoding="utf-8")
        pairs = {
            "MEMORY_KIB": 65536,
            "ITERATIONS": 3,
            "PARALLELISM": 4,
            "SALT_LEN": 16,
            "KEY_LEN": 32,
            "PARAMS_VERSION": 1,
        }
        for name, val in pairs.items():
            if not re.search(rf"const {name}:\s*\w+\s*=\s*{val}\b", text):
                errors.append(f"kdf.rs missing matching const {name} = {val}")

    for p in (MOD_RS, APP_SQL, CASE_SQL):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    if MOD_RS.exists():
        text = MOD_RS.read_text(encoding="utf-8")
        for needle in (
            "PassphraseInvalid",
            "probe_db_unreadable_without_key",
            # B-S1-03: cases are created via jurisdiction-first wizard profile
            # (replaces the earlier create_case_with_passphrase provisional name).
            "create_case_with_wizard",
            "init_and_unlock",
        ):
            if needle not in text:
                errors.append(f"vault/mod.rs missing {needle}")

    if LIB_RS.exists():
        text = LIB_RS.read_text(encoding="utf-8")
        for cmd in (
            "vault_status",
            "vault_init_unlock",
            "vault_unlock",
            "vault_lock",
            "case_create",
            "case_open",
            "case_close",
            "case_list",
        ):
            if f"fn {cmd}" not in text:
                errors.append(f"lib.rs missing command {cmd}")
    else:
        errors.append("missing lib.rs")

    if ERRORS.exists():
        err = json.loads(ERRORS.read_text(encoding="utf-8"))
        codes = {c["code"] for c in err.get("codes", [])}
        if "CF_VAULT_PASSPHRASE_INVALID" not in codes:
            errors.append("errors.json missing CF_VAULT_PASSPHRASE_INVALID")
    else:
        errors.append("missing errors.json")

    if errors:
        print_fail(errors)
        return 1

    print("PASS  AC-16.2 structural (Argon2id frozen + vault module + commands wired)")
    print("  note: run `cargo test -p caseforge --lib vault::tests` for live SQLCipher AC-16.2 when MSVC available")
    return 0


def print_fail(errors: list[str]) -> None:
    print("FAIL  AC-16.2 structural")
    for e in errors:
        print(f"  - {e}")


if __name__ == "__main__":
    sys.exit(main())
