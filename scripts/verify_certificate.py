#!/usr/bin/env python3
"""Verify a CaseForge Audit Certificate offline, with the Python standard library only.

    python verify_certificate.py certificate.json [--draft draft.txt]

Independent of the CaseForge app: re-implements canonical JSON, SHA-256 checks and Ed25519 verification
(RFC 8032 §5.1.7) from scratch, so anyone can check a certificate without installing anything.

What a valid result means: the certificate (and, with --draft, the document) is byte-for-byte what the signing
install certified. It does NOT certify that the document is legally correct — see the certificate's scope statement.
To trust WHO signed it, compare the printed key_id with the one the signing advocate publishes.
Exit code: 0 valid, 1 invalid, 2 unreadable input.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

SCOPE_STATEMENT = (
    "This certificate records that CaseForge compared the citations and quotations in the identified document against "
    "the listed installed legal packs on the stated date. It does not certify that the document is legally correct, "
    "complete, or compliant with the rules of any court. Items marked NOT_IN_CORPUS were not found in the installed "
    "packs; that is not a finding that they do not exist."
)
PROHIBITED = ["guaranteed", "court-approved", "sanction-proof", "AI judge", "predicts", "decides", "good law", "valid law"]

# --- Ed25519 verification, RFC 8032 reference arithmetic (extended twisted Edwards coordinates) ---------------------
_P = 2**255 - 19
_L = 2**252 + 27742317777372353535851937790883648493
_D = -121665 * pow(121666, _P - 2, _P) % _P
_SQRT_M1 = pow(2, (_P - 1) // 4, _P)


def _recover_x(y: int, sign: int) -> int | None:
    if y >= _P:
        return None
    x2 = (y * y - 1) * pow(_D * y * y + 1, _P - 2, _P) % _P
    if x2 == 0:
        return None if sign else 0
    x = pow(x2, (_P + 3) // 8, _P)
    if (x * x - x2) % _P != 0:
        x = x * _SQRT_M1 % _P
    if (x * x - x2) % _P != 0:
        return None
    if (x & 1) != sign:
        x = _P - x
    return x


def _add(p: tuple, q: tuple) -> tuple:
    a = (p[1] - p[0]) * (q[1] - q[0]) % _P
    b = (p[1] + p[0]) * (q[1] + q[0]) % _P
    c = 2 * p[3] * q[3] * _D % _P
    d = 2 * p[2] * q[2] % _P
    e, f, g, h = b - a, d - c, d + c, b + a
    return (e * f % _P, g * h % _P, f * g % _P, e * h % _P)


def _mul(s: int, p: tuple) -> tuple:
    q = (0, 1, 1, 0)
    while s > 0:
        if s & 1:
            q = _add(q, p)
        p = _add(p, p)
        s >>= 1
    return q


def _equal(p: tuple, q: tuple) -> bool:
    return (p[0] * q[2] - q[0] * p[2]) % _P == 0 and (p[1] * q[2] - q[1] * p[2]) % _P == 0


def _decode_point(b: bytes) -> tuple | None:
    y = int.from_bytes(b, "little")
    sign = y >> 255
    y &= (1 << 255) - 1
    x = _recover_x(y, sign)
    return None if x is None else (x, y, 1, x * y % _P)


_GY = 4 * pow(5, _P - 2, _P) % _P
_GX = _recover_x(_GY, 0)
_G = (_GX, _GY, 1, _GX * _GY % _P)


def ed25519_verify(public_key: bytes, message: bytes, signature: bytes) -> bool:
    if len(public_key) != 32 or len(signature) != 64:
        return False
    a, r = _decode_point(public_key), _decode_point(signature[:32])
    if a is None or r is None:
        return False
    s = int.from_bytes(signature[32:], "little")
    if s >= _L:
        return False
    h = int.from_bytes(hashlib.sha512(signature[:32] + public_key + message).digest(), "little") % _L
    return _equal(_mul(s, _G), _add(r, _mul(h, a)))


# --- Certificate checks (mirror src-tauri/src/certificate.rs) --------------------------------------------------------
def canonical_json(value) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def key_id(public_key: bytes) -> str:
    return "cf-install-" + sha256_hex(public_key)[:16]


def verify(cert: dict, draft_bytes: bytes | None) -> tuple[bool, list[tuple[str, bool, str]]]:
    checks: list[tuple[str, bool, str]] = []
    checks.append(("scope_statement_exact", cert.get("scope_statement") == SCOPE_STATEMENT, "scope statement is the fixed text"))
    disclosure = (cert.get("disclosure") or {}).get("text") or ""
    hits = [w for w in PROHIBITED if re.search(rf"(?i)\b{re.escape(w)}\b", disclosure)]
    checks.append(("no_prohibited_wording", not hits, ", ".join(hits) or "none"))
    report = cert.get("audit_report")
    checks.append(("audit_report_hash", cert.get("audit_report_sha256") == sha256_hex(canonical_json(report).encode("utf-8")), "embedded audit report matches its recorded SHA-256"))
    checks.append(("draft_hash_consistent", cert.get("draft_sha256") == ((report or {}).get("subject") or {}).get("sha256"), "certificate and report name the same draft"))
    if draft_bytes is not None:
        ok = cert.get("draft_sha256") == sha256_hex(draft_bytes)
        checks.append(("draft_matches_document", ok, "the supplied document is the certified draft" if ok else "the supplied document differs from the certified draft"))
    else:
        checks.append(("draft_matches_document", True, "not checked: no document supplied"))
    signing = cert.get("signing") or {}
    try:
        pk = bytes.fromhex(signing.get("public_key_hex", ""))
        sig = bytes.fromhex(signing.get("signature_hex", ""))
    except ValueError:
        pk, sig = b"", b""
    checks.append(("key_id_matches_key", len(pk) == 32 and signing.get("key_id") == key_id(pk), "key_id is the fingerprint of the embedded public key"))
    unsigned = json.loads(json.dumps(cert))
    (unsigned.get("signing") or {}).pop("signature_hex", None)
    checks.append(("signature", ed25519_verify(pk, canonical_json(unsigned).encode("utf-8"), sig), "Ed25519 signature over the canonical certificate"))
    return all(c[1] for c in checks), checks


def main() -> int:
    ap = argparse.ArgumentParser(description="Verify a CaseForge Audit Certificate offline")
    ap.add_argument("certificate", type=Path)
    ap.add_argument("--draft", type=Path, help="the certified document (plain text exactly as audited)")
    args = ap.parse_args()
    try:
        cert = json.loads(args.certificate.read_text(encoding="utf-8"))
        cert = cert.get("certificate", cert)  # accept a wrapped reference file too
        draft = args.draft.read_bytes() if args.draft else None
    except (OSError, json.JSONDecodeError) as exc:
        print(f"UNREADABLE  {exc}")
        return 2
    valid, checks = verify(cert, draft)
    for name, ok, detail in checks:
        print(f"{'PASS' if ok else 'FAIL'}  {name}: {detail}")
    print(f"\n{'VALID' if valid else 'INVALID'}  certificate {cert.get('certificate_id')}  signed by key {(cert.get('signing') or {}).get('key_id')}")
    print("Valid = unaltered since signing. It does not certify legal correctness. Compare the key_id with the one the advocate publishes.")
    return 0 if valid else 1


if __name__ == "__main__":
    sys.exit(main())
