#!/usr/bin/env python3
"""Build a signed CaseForge offline licence JSON (B-S1-18).

Uses cryptography Ed25519. Keys under eval/tools/.licence_keys/.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
from pathlib import Path

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

ROOT = Path(__file__).resolve().parents[2]
KEY_DIR = ROOT / "eval" / "tools" / ".licence_keys"
TRUST = ROOT / "contracts" / "licence_trust.json"
KEY_ID = "caseforge-licence-dev-2026"


def sha256_hex(s: str) -> str:
    return hashlib.sha256(s.encode("utf-8")).hexdigest()


def ensure_keypair() -> Ed25519PrivateKey:
    KEY_DIR.mkdir(parents=True, exist_ok=True)
    sk_path = KEY_DIR / "licence_dev.sk.pem"
    if sk_path.exists():
        return serialization.load_pem_private_key(sk_path.read_bytes(), password=None)  # type: ignore
    sk = Ed25519PrivateKey.generate()
    sk_path.write_bytes(
        sk.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.PKCS8,
            encryption_algorithm=serialization.NoEncryption(),
        )
    )
    pk = sk.public_key()
    (KEY_DIR / "licence_dev.pk.pem").write_bytes(
        pk.public_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PublicFormat.SubjectPublicKeyInfo,
        )
    )
    raw = pk.public_bytes(
        encoding=serialization.Encoding.Raw,
        format=serialization.PublicFormat.Raw,
    )
    update_trust(raw.hex())
    return sk


def update_trust(public_key_hex: str) -> None:
    data = {
        "schema_version": "1.0.0",
        "batch": "B-S1-18",
        "description": "Trusted Ed25519 public keys for CaseForge offline licences. Private keys never ship in the app.",
        "keys": [
            {
                "key_id": KEY_ID,
                "algorithm": "Ed25519",
                "public_key_hex": public_key_hex,
                "status": "active",
                "notes": "Dev/product-owner licence signer for Stage-1 activation tests.",
            }
        ],
    }
    TRUST.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def canonical_bytes(payload: dict) -> bytes:
    body = {k: v for k, v in payload.items() if k not in ("signature_hex", "signature_alg")}
    # Sorted keys via json.dumps sort_keys
    return json.dumps(body, sort_keys=True, separators=(",", ":")).encode("utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description="Sign a CaseForge offline licence")
    ap.add_argument("--licence-id", default="lic_dev_001")
    ap.add_argument("--tier", default="solo")
    ap.add_argument("--days", type=int, default=365, help="Expiry in days (0 = perpetual)")
    ap.add_argument("--hostname", default="dev-host")
    ap.add_argument("--username", default="dev-user")
    ap.add_argument("--machine-guid", default="dev-machine-guid")
    ap.add_argument("--disk-root", default="C:")
    ap.add_argument(
        "--bind-live",
        action="store_true",
        help="Hash live env COMPUTERNAME/USERNAME for this machine",
    )
    ap.add_argument("-o", "--out", type=Path, default=None)
    args = ap.parse_args()

    sk = ensure_keypair()
    now = int(time.time() * 1000)
    exp = None if args.days <= 0 else now + args.days * 24 * 3600 * 1000

    if args.bind_live:
        import os

        host = os.environ.get("COMPUTERNAME") or os.environ.get("HOSTNAME") or args.hostname
        user = os.environ.get("USERNAME") or os.environ.get("USER") or args.username
        guid = os.environ.get("CASEFORGE_MACHINE_GUID") or (
            f"win:{os.environ.get('PROCESSOR_IDENTIFIER','')}:{host}"
        )
        disk = os.environ.get("SystemDrive") or args.disk_root
        fp = {
            "hostname": sha256_hex(host),
            "username": sha256_hex(user),
            "machine_guid": sha256_hex(guid),
            "disk_root": sha256_hex(disk),
        }
    else:
        fp = {
            "hostname": sha256_hex(args.hostname),
            "username": sha256_hex(args.username),
            "machine_guid": sha256_hex(args.machine_guid),
            "disk_root": sha256_hex(args.disk_root),
        }

    payload = {
        "schema_version": "1.0.0",
        "licence_id": args.licence_id,
        "product": "CaseForge",
        "tier": args.tier,
        "issued_at_unix_ms": now,
        "expires_at_unix_ms": exp,
        "fingerprint_components": fp,
        "features": ["digest", "voice", "retrieval", "export"],
        "air_gap_allowed": True,
        "signer_key_id": KEY_ID,
        "notes": "Dev licence for B-S1-18 tests",
    }
    msg = canonical_bytes(payload)
    sig = sk.sign(msg)
    payload["signature_alg"] = "Ed25519"
    payload["signature_hex"] = sig.hex()

    text = json.dumps(payload, indent=2)
    out = args.out or (ROOT / "packs" / "fixtures" / "licence_dev.json")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text + "\n", encoding="utf-8")
    print(f"Wrote {out}")
    print(f"fingerprint hostname={fp['hostname'][:16]}…")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
