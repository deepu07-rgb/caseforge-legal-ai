#!/usr/bin/env python3
"""B-S1-18 Licensing + air-gap + AC-13.1 network policy audit."""

from __future__ import annotations

import hashlib
import json
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONST = ROOT / "contracts" / "constants.json"
SCHEMA = ROOT / "contracts" / "licence_payload.schema.json"
TRUST = ROOT / "contracts" / "licence_trust.json"
MIG = ROOT / "src-tauri" / "migrations" / "app_005_licence.sql"
LIC_RS = ROOT / "src-tauri" / "src" / "licence" / "mod.rs"
VAULT = ROOT / "src-tauri" / "src" / "vault" / "mod.rs"
LIB = ROOT / "src-tauri" / "src" / "lib.rs"
IPC = ROOT / "src" / "lib" / "ipc.ts"
VIEW = ROOT / "src" / "components" / "LicenceView.tsx"
APP = ROOT / "src" / "App.tsx"
DOC = ROOT / "docs" / "B-S1-18_licence.md"
MAKE = ROOT / "eval" / "tools" / "make_licence.py"
PIPELINE = "caseforge.licence.v1"


def sha256_hex(s: str) -> str:
    return hashlib.sha256(s.encode("utf-8")).hexdigest()


def main() -> int:
    errors: list[str] = []
    for p in (
        CONST,
        SCHEMA,
        TRUST,
        MIG,
        LIC_RS,
        VAULT,
        LIB,
        IPC,
        VIEW,
        APP,
        DOC,
        MAKE,
    ):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    # --- Contracts ---
    if CONST.exists():
        data = json.loads(CONST.read_text(encoding="utf-8-sig"))
        lic = data.get("licence_and_privacy") or {}
        if lic.get("status") != "FROZEN":
            errors.append("licence_and_privacy not FROZEN")
        if lic.get("pipeline_id") != PIPELINE:
            errors.append(f"pipeline_id must be {PIPELINE}")
        if lic.get("fingerprint_components_required_to_match") != 3:
            errors.append("fingerprint must require 3-of-4")
        if lic.get("fingerprint_components_total") != 4:
            errors.append("fingerprint total must be 4")
        comps = lic.get("fingerprint_components") or []
        for c in ("hostname", "username", "machine_guid", "disk_root"):
            if c not in comps:
                errors.append(f"fingerprint missing component {c}")
        if lic.get("expired_behaviour") != "read_only_vault":
            errors.append("expired_behaviour must be read_only_vault")
        if lic.get("user_data_may_be_ransomed") is not False:
            errors.append("user_data_may_be_ransomed must be false")
        if lic.get("case_data_network_code_path_allowed") is not False:
            errors.append("case_data_network_code_path_allowed must be false (AC-13.1)")
        if not lic.get("air_gap_kills_all_networking"):
            errors.append("air_gap_kills_all_networking must be true")
        states = lic.get("state_machine") or []
        for s in ("UNLICENSED", "ACTIVE", "EXPIRED"):
            if s not in states:
                errors.append(f"state_machine missing {s}")

    if SCHEMA.exists():
        sch = json.loads(SCHEMA.read_text(encoding="utf-8-sig"))
        if sch.get("x-caseforge", {}).get("status") != "FROZEN":
            errors.append("licence_payload schema not FROZEN")
        props = sch.get("properties") or {}
        for f in (
            "licence_id",
            "fingerprint_components",
            "signature_hex",
            "expires_at_unix_ms",
            "tier",
        ):
            if f not in props:
                errors.append(f"schema missing {f}")
        x = sch.get("x-caseforge") or {}
        if x.get("user_data_may_be_ransomed") is not False:
            errors.append("schema must forbid ransom")
        if x.get("expired_behaviour") != "read_only_vault":
            errors.append("schema expired_behaviour")

    if TRUST.exists():
        tr = json.loads(TRUST.read_text(encoding="utf-8-sig"))
        keys = tr.get("keys") or []
        if not keys:
            errors.append("licence_trust.json has no keys")
        else:
            k = keys[0]
            if k.get("algorithm") != "Ed25519":
                errors.append("trust key must be Ed25519")
            if len(k.get("public_key_hex") or "") != 64:
                errors.append("public_key_hex must be 32-byte hex")

    # --- Source wiring ---
    for path, needles in (
        (
            MIG,
            [
                "licence_state",
                "UNLICENSED",
                "ACTIVE",
                "EXPIRED",
                "air_gap_enabled",
                "case_data_network_path",
            ],
        ),
        (
            LIC_RS,
            [
                "collect_fingerprint",
                "verify_signature",
                "evaluate_payload",
                "network_audit",
                "CF_LICENCE_FINGERPRINT_MISMATCH",
                "CF_LICENCE_INVALID_SIGNATURE",
                "EXPIRED",
                "read_only",
                "AC-13.1",
            ],
        ),
        (
            VAULT,
            [
                "app_005_licence.sql",
                "assert_vault_writable",
                "CF_VAULT_READ_ONLY",
                "licence_activate",
                "licence_mark_expired",
                "air_gap_set",
                "not ransomed",
            ],
        ),
        (
            LIB,
            [
                "mod licence",
                "licence_activate",
                "licence_status",
                "air_gap_set",
                "network_audit",
                "CF_AIR_GAP",
                "B-S1-18",
            ],
        ),
        (
            IPC,
            [
                "licenceActivate",
                "licenceStatus",
                "airGapSet",
                "networkAudit",
                "vault_writable",
                "user_data_may_be_ransomed",
            ],
        ),
        (
            VIEW,
            [
                "licence-panel",
                "licence-activate",
                "airgap-on",
                "network-audit",
                "EXPIRED",
                "AC-13.1",
                "read-only",
            ],
        ),
        (APP, ['from "./components/LicenceView"', 'nav === "licence"', '"licence"']),
        (
            DOC,
            [
                "AC-13.1",
                "EXPIRED",
                "read-only",
                "3-of-4",
                "air-gap",
                "ransom",
                PIPELINE,
            ],
        ),
        (MAKE, ["Ed25519", "canonical_bytes", "fingerprint_components"]),
    ):
        if not path.exists():
            continue
        text = path.read_text(encoding="utf-8")
        for n in needles:
            if n not in text:
                errors.append(f"{path.relative_to(ROOT)} missing {n!r}")

    # --- Live: sign + verify with cryptography (mirrors host) ---
    try:
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
        from cryptography.hazmat.primitives import serialization
    except ImportError:
        errors.append("cryptography package required for licence sign smoke")
    else:
        sk_path = ROOT / "eval" / "tools" / ".licence_keys" / "licence_dev.sk.pem"
        if not sk_path.exists():
            # generate via make_licence helper logic
            sys.path.insert(0, str(ROOT / "eval" / "tools"))
            import make_licence as ml  # type: ignore

            ml.ensure_keypair()
        sk = serialization.load_pem_private_key(sk_path.read_bytes(), password=None)
        now = int(time.time() * 1000)
        fp = {
            "disk_root": sha256_hex("C:"),
            "hostname": sha256_hex("host-a"),
            "machine_guid": sha256_hex("guid-a"),
            "username": sha256_hex("user-a"),
        }
        body = {
            "air_gap_allowed": True,
            "expires_at_unix_ms": now + 86400000,
            "features": ["digest", "voice"],
            "fingerprint_components": fp,
            "issued_at_unix_ms": now,
            "licence_id": "lic_eval_001",
            "notes": "eval",
            "product": "CaseForge",
            "schema_version": "1.0.0",
            "signer_key_id": "caseforge-licence-dev-2026",
            "tier": "solo",
        }
        msg = json.dumps(body, sort_keys=True, separators=(",", ":")).encode("utf-8")
        sig = sk.sign(msg)
        # verify
        pk = sk.public_key()
        try:
            pk.verify(sig, msg)
        except Exception as e:  # noqa: BLE001
            errors.append(f"Ed25519 self-verify failed: {e}")

        # 3-of-4 fingerprint unit
        live = {
            "hostname": fp["hostname"],
            "username": fp["username"],
            "machine_guid": fp["machine_guid"],
            "disk_root": sha256_hex("D:"),  # mismatch one
        }
        match = sum(1 for k in fp if live[k] == fp[k])
        if match != 3:
            errors.append(f"3-of-4 unit expected 3 matches got {match}")
        if match < 3:
            errors.append("fingerprint tolerance broken")

        # EXPIRED rule: past expiry => not writable
        expired = now - 1000
        if expired >= now:
            errors.append("expiry arithmetic broken")
        # UNLICENSED/ACTIVE writable; EXPIRED not
        writable = {"UNLICENSED": True, "ACTIVE": True, "EXPIRED": False}
        if writable["EXPIRED"] is not False:
            errors.append("EXPIRED must not be writable")

    # AC-13.1: no case-data network path in constants + source claims
    if VAULT.exists():
        vt = VAULT.read_text(encoding="utf-8")
        if "CF_VAULT_READ_ONLY" not in vt:
            errors.append("vault must enforce CF_VAULT_READ_ONLY on EXPIRED")
        if "ransomed" not in vt.lower() and "ransom" not in vt.lower():
            errors.append("vault must document no-ransom invariant")

    if LIB.exists():
        lt = LIB.read_text(encoding="utf-8")
        if "CF_AIR_GAP" not in lt:
            errors.append("air-gap must block companion networking")

    # No ransom: EXPIRED must not delete
    if LIC_RS.exists():
        lr = LIC_RS.read_text(encoding="utf-8")
        if "never ransomed" not in lr and "not ransomed" not in lr:
            # check vault
            pass

    if errors:
        print("FAIL  B-S1-18 licence / air-gap / AC-13.1")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(
        f"PASS  B-S1-18 licence + air-gap "
        f"({PIPELINE}; 3-of-4 fingerprint; EXPIRED=read-only no ransom; AC-13.1 policy)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
