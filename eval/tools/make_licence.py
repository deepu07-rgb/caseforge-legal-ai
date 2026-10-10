#!/usr/bin/env python3
"""Build and sign CaseForge offline licences (B-S1-18, extended for editions by B-S4-17 and B-S4-22).

Uses cryptography Ed25519. Keys under eval/tools/.licence_keys/ (never committed; *.pem is git-ignored).

  v1 licence          python make_licence.py [--tier solo]
  v2 licence          python make_licence.py --edition PRO [--days 365] [--pack-days 180] [--seat-transfers-per-year 2]
                      Features default to what the edition includes in contracts/edition_entitlements.json (a DEVELOPMENT_ONLY
                      placeholder until the owner decides Q-S4-06); --features may only narrow that list.
  seat transfer       python make_licence.py --transfer-receipt receipt.json --prior-licence old.json --new-fingerprint new_fp.json
                      Re-issues the prior licence bound to the new machine, with the transfer recorded in its signed
                      seat_transfers. Refused if the receipt is not valid, does not belong to the licence, is not the next
                      transfer, or the yearly limit would be exceeded.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
from pathlib import Path

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey, Ed25519PublicKey

ROOT = Path(__file__).resolve().parents[2]
KEY_DIR = ROOT / "eval" / "tools" / ".licence_keys"
TRUST = ROOT / "contracts" / "licence_trust.json"
ENTITLEMENTS = ROOT / "contracts" / "edition_entitlements.json"
KEY_ID = "caseforge-licence-dev-2026"
DAY_MS = 86_400_000
SEAT_WINDOW_DAYS = 365
EDITIONS = ["ADVOCATE", "PRO", "CHAMBERS_FIRM", "JUDICIARY"]
RECEIPT_KIND = "CASEFORGE_SEAT_RELEASE"
FP_KEYS = ("hostname", "username", "machine_guid", "disk_root")


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
    # Sorted keys via json.dumps sort_keys; ensure_ascii escapes non-ASCII as \uXXXX (the app's rule).
    return json.dumps(body, sort_keys=True, separators=(",", ":")).encode("utf-8")


def sign(sk: Ed25519PrivateKey, payload: dict) -> dict:
    payload = {k: v for k, v in payload.items() if k not in ("signature_hex", "signature_alg")}
    payload["signature_alg"] = "Ed25519"
    payload["signature_hex"] = sk.sign(canonical_bytes(payload)).hex()
    return payload


def verify_signature(payload: dict, public_key_hex: str) -> bool:
    try:
        Ed25519PublicKey.from_public_bytes(bytes.fromhex(public_key_hex)).verify(bytes.fromhex(payload["signature_hex"]), canonical_bytes(payload))
        return True
    except (InvalidSignature, ValueError, KeyError):
        return False


# --- editions (B-S4-22) ---


def entitlements() -> dict:
    return json.loads(ENTITLEMENTS.read_text(encoding="utf-8"))


def edition_features(edition: str) -> list[str]:
    return list(entitlements()["editions"][edition]["features"])


def build_v2(
    sk: Ed25519PrivateKey,
    *,
    edition: str,
    licence_id: str,
    fp: dict,
    now: int,
    features: list[str] | None = None,
    days: int = 365,
    pack_days: int | None = None,
    seat_transfers_per_year: int | None = None,
    seat_transfers: list[dict] | None = None,
    notes: str = "Dev licence for B-S4-22 tests",
) -> dict:
    """A signed version 2 licence. `days` is the app licence (0 = no end); `pack_days` the pack subscription (default: same
    as the app licence, 0 = no end). Features default to the edition's list and can only be narrowed."""
    if edition not in EDITIONS:
        raise ValueError(f"unknown edition {edition}")
    allowed = edition_features(edition)
    feats = list(features) if features else allowed
    outside = [f for f in feats if f not in allowed]
    if outside:
        raise ValueError(f"features not included in the {edition} edition: {', '.join(outside)}")
    if len(set(feats)) != len(feats):
        raise ValueError("features must be unique")
    judiciary = edition == "JUDICIARY"
    exp = None if days <= 0 else now + days * DAY_MS
    pack = exp if pack_days is None else (None if pack_days <= 0 else now + pack_days * DAY_MS)
    payload = {
        "schema_version": "2.0.0",
        "licence_id": licence_id,
        "product": "CaseForge",
        "edition": edition,
        "issued_to_kind": "INSTITUTION" if judiciary else "INDIVIDUAL",
        "issued_at_unix_ms": now,
        "expires_at_unix_ms": exp,
        "pack_subscription_until_unix_ms": pack,
        "fingerprint_components": fp,
        "features": feats,
        "air_gap_forced": judiciary,
        "telemetry_allowed": not judiciary,
        "seat_transfers_per_year": seat_transfers_per_year,
        "institution_key_id": None,
        "signer_key_id": KEY_ID,
        "notes": notes,
    }
    if seat_transfers:
        payload["seat_transfers"] = seat_transfers
    return sign(sk, payload)


def fp_matches(a: dict, b: dict) -> int:
    return sum(1 for k in FP_KEYS if a.get(k) == b.get(k))


def verify_receipt(receipt: dict) -> None:
    """Checks the receipt's own signature (made by the old install's key, carried in the receipt)."""
    if receipt.get("kind") != RECEIPT_KIND or receipt.get("signature_alg") != "Ed25519":
        raise ValueError("CF_SEAT_RECEIPT_INVALID: not a seat release receipt")
    if not verify_signature(receipt, receipt.get("public_key_hex", "")):
        raise ValueError("CF_SEAT_RECEIPT_INVALID: signature does not verify")


def transfers_in_window(transfers: list[dict], now: int) -> int:
    return sum(1 for t in transfers if t["transferred_at_unix_ms"] > now - SEAT_WINDOW_DAYS * DAY_MS)


def rebind(sk: Ed25519PrivateKey, prior: dict, receipt: dict, new_fp: dict, *, now: int, trusted_public_key_hex: str | None = None) -> dict:
    """Re-issues `prior` for a new machine after the old machine released its seat. The yearly limit is counted here and again by
    the app when it activates the result."""
    if prior.get("schema_version") != "2.0.0":
        raise ValueError("only a version 2 licence can be transferred")
    if trusted_public_key_hex and not verify_signature(prior, trusted_public_key_hex):
        raise ValueError("the prior licence does not carry a valid signature")
    verify_receipt(receipt)
    if receipt.get("licence_id") != prior["licence_id"]:
        raise ValueError("CF_SEAT_RECEIPT_INVALID: the receipt belongs to another licence")
    if fp_matches(receipt.get("fingerprint_components", {}), prior["fingerprint_components"]) < 3:
        raise ValueError("CF_SEAT_RECEIPT_INVALID: the receipt was not made on the machine this licence is bound to")
    if any(not str(new_fp.get(k, "")) for k in FP_KEYS):
        raise ValueError("the new machine's fingerprint is incomplete")
    transfers = list(prior.get("seat_transfers") or [])
    if receipt.get("transfer_ordinal") != len(transfers) + 1:
        raise ValueError("CF_SEAT_RECEIPT_INVALID: this receipt is not the next transfer of this licence")
    if any(t["receipt_id"] == receipt["receipt_id"] for t in transfers):
        raise ValueError("CF_SEAT_RECEIPT_INVALID: this receipt has already been used")
    at = int(receipt["deactivated_at_unix_ms"])
    if at > now + DAY_MS:
        raise ValueError("CF_SEAT_RECEIPT_INVALID: the receipt is dated in the future")
    limit = prior.get("seat_transfers_per_year")
    entry = {"transferred_at_unix_ms": at, "receipt_id": receipt["receipt_id"]}
    used = transfers_in_window(transfers + [entry], now)
    if limit is None:
        raise ValueError("CF_SEAT_TRANSFER_LIMIT: seat transfer is not enabled for this licence")
    if used > limit:
        raise ValueError(f"CF_SEAT_TRANSFER_LIMIT: this would be transfer {used} in {SEAT_WINDOW_DAYS} days; the limit is {limit}")
    body = {k: v for k, v in prior.items() if k not in ("signature_hex", "signature_alg")}
    body.update({"fingerprint_components": {k: new_fp[k] for k in FP_KEYS}, "seat_transfers": transfers + [entry], "issued_at_unix_ms": now})
    return sign(sk, body)


def live_fingerprint(args: argparse.Namespace) -> dict:
    import os

    host = os.environ.get("COMPUTERNAME") or os.environ.get("HOSTNAME") or args.hostname
    user = os.environ.get("USERNAME") or os.environ.get("USER") or args.username
    guid = os.environ.get("CASEFORGE_MACHINE_GUID") or (f"win:{os.environ.get('PROCESSOR_IDENTIFIER','')}:{host}")
    disk = os.environ.get("SystemDrive") or args.disk_root
    return {"hostname": sha256_hex(host), "username": sha256_hex(user), "machine_guid": sha256_hex(guid), "disk_root": sha256_hex(disk)}


def main() -> int:
    ap = argparse.ArgumentParser(description="Sign a CaseForge offline licence")
    ap.add_argument("--licence-id", default="lic_dev_001")
    ap.add_argument("--tier", default="solo")
    ap.add_argument("--days", type=int, default=365, help="App licence expiry in days (0 = perpetual)")
    ap.add_argument("--pack-days", type=int, default=None, help="v2: pack subscription end in days, separate from the app licence (default: same as --days; 0 = no end)")
    ap.add_argument("--hostname", default="dev-host")
    ap.add_argument("--username", default="dev-user")
    ap.add_argument("--machine-guid", default="dev-machine-guid")
    ap.add_argument("--disk-root", default="C:")
    ap.add_argument("--bind-live", action="store_true", help="Hash live env COMPUTERNAME/USERNAME for this machine")
    ap.add_argument("--edition", choices=EDITIONS, default=None,
                    help="Issue a version 2 licence (contracts/licence_payload_v2.schema.json). A JUDICIARY licence is institution-issued, forces air-gap on, allows no telemetry and carries the chambers feature.")
    ap.add_argument("--features", default=None, help="v2: comma-separated feature ids; may only narrow the edition's list in contracts/edition_entitlements.json")
    ap.add_argument("--seat-transfers-per-year", type=int, default=None, help="v2: seat transfers allowed per rolling 365 days (Q-S4-07, PROPOSED 2); omit = transfer disabled")
    ap.add_argument("--list-editions", action="store_true", help="Print what each edition includes (the DEVELOPMENT_ONLY placeholder map) and exit")
    ap.add_argument("--transfer-receipt", type=Path, default=None, help="Seat release receipt written by the old machine")
    ap.add_argument("--prior-licence", type=Path, default=None, help="The licence being transferred")
    ap.add_argument("--new-fingerprint", type=Path, default=None, help="JSON of the new machine's fingerprint (the licence_fingerprint output)")
    ap.add_argument("-o", "--out", type=Path, default=None)
    args = ap.parse_args()

    if args.list_editions:
        m = entitlements()
        print(f"entitlement map status: {m['status']} ({m['question']})")
        for e in EDITIONS:
            print(f"{e}: {', '.join(m['editions'][e]['features'])}")
        return 0

    now = int(time.time() * 1000)

    if args.transfer_receipt or args.prior_licence or args.new_fingerprint:
        if not (args.transfer_receipt and args.prior_licence and args.new_fingerprint):
            print("a seat transfer needs --transfer-receipt, --prior-licence and --new-fingerprint", file=sys.stderr)
            return 2
        sk = ensure_keypair()
        try:
            pub = json.loads(TRUST.read_text(encoding="utf-8"))["keys"][0]["public_key_hex"]
            new = rebind(sk, json.loads(args.prior_licence.read_text(encoding="utf-8")), json.loads(args.transfer_receipt.read_text(encoding="utf-8")), json.loads(args.new_fingerprint.read_text(encoding="utf-8")), now=now, trusted_public_key_hex=pub)
        except (ValueError, KeyError) as e:
            print(f"refused: {e}", file=sys.stderr)
            return 1
        out = args.out or args.prior_licence.with_name(args.prior_licence.stem + "_transferred.json")
        out.write_text(json.dumps(new, indent=2) + "\n", encoding="utf-8")
        print(f"Wrote {out} (transfer {len(new['seat_transfers'])} of this licence)")
        return 0

    sk = ensure_keypair()
    exp = None if args.days <= 0 else now + args.days * DAY_MS

    if args.bind_live:
        fp = live_fingerprint(args)
    else:
        fp = {
            "hostname": sha256_hex(args.hostname),
            "username": sha256_hex(args.username),
            "machine_guid": sha256_hex(args.machine_guid),
            "disk_root": sha256_hex(args.disk_root),
        }

    if args.edition:
        try:
            payload = build_v2(
                sk,
                edition=args.edition,
                licence_id=args.licence_id,
                fp=fp,
                now=now,
                features=args.features.split(",") if args.features else None,
                days=args.days,
                pack_days=args.pack_days,
                seat_transfers_per_year=args.seat_transfers_per_year,
            )
        except ValueError as e:
            print(f"refused: {e}", file=sys.stderr)
            return 1
        out = args.out or (ROOT / "packs" / "fixtures" / f"licence_dev_{args.edition.lower()}.json")
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(payload, indent=2) + chr(10), encoding="utf-8")
        print(f"Wrote {out}")
        return 0

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
