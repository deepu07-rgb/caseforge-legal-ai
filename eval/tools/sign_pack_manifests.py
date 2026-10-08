#!/usr/bin/env python3
"""Sign pack manifests locally, and insert verified signatures in CI. The private key never leaves the owner's machine.

Sign (owner's machine):
  python eval/tools/sign_pack_manifests.py --manifests manifests.zip --key <official_dev.sk.pem> [--out packs/central/signatures.json]
  Signs each manifest's content_sha256 (the same message the app verifies) and writes {pack_id: {content_sha256, signature}}.

Insert (CI, no key):
  python eval/tools/sign_pack_manifests.py --insert <packs dir> --signatures packs/central/signatures.json --trust contracts/pack_trust.json
  For every pack: recomputes every file hash and the canonical content hash from the files on disk, requires them to
  equal the manifest AND the signed content_sha256, verifies the signature with the trusted public key, then writes
  signature.ed25519. Any mismatch fails the run.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import sys
import zipfile
from pathlib import Path


def digest(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def canonical(files: list[dict]) -> str:
    return digest(("\n".join(sorted(f"{f['path']}:{f['sha256']}" for f in files)) + "\n").encode())


def sign(manifests: Path, key_path: Path, out: Path) -> int:
    from cryptography.hazmat.primitives import serialization

    key = serialization.load_pem_private_key(key_path.read_bytes(), password=None)
    sigs = json.loads(out.read_text(encoding="utf-8")) if out.exists() else {}
    with zipfile.ZipFile(manifests) as z:
        for name in z.namelist():
            if not name.endswith("/manifest.json"):
                continue
            m = json.loads(z.read(name))
            if canonical(m["files"]) != m["content_sha256"]:
                raise SystemExit(f"{name}: content_sha256 does not match its file list")
            sigs[m["pack_id"]] = {"content_sha256": m["content_sha256"], "signer_key_id": m["signer_key_id"],
                                  "signature": base64.b64encode(key.sign(m["content_sha256"].encode("ascii"))).decode("ascii")}
            print(f"signed {m['pack_id']} {m['content_sha256'][:16]}")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(dict(sorted(sigs.items())), indent=2) + "\n", encoding="utf-8", newline="\n")
    return 0


def insert(packs: Path, signatures: Path, trust: Path) -> int:
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

    sigs = json.loads(signatures.read_text(encoding="utf-8"))
    keys = {k["key_id"]: k["public_key_hex"] for k in json.loads(trust.read_text(encoding="utf-8"))["keys"]}
    failures = 0
    for mpath in sorted(packs.glob("*/manifest.json")):
        root = mpath.parent
        m = json.loads(mpath.read_text(encoding="utf-8"))
        s = sigs.get(m["pack_id"])
        try:
            if not s:
                raise ValueError("no signature")
            for f in m["files"]:
                if digest((root / f["path"]).read_bytes()) != f["sha256"]:
                    raise ValueError(f"file hash mismatch {f['path']}")
            if canonical(m["files"]) != m["content_sha256"] or s["content_sha256"] != m["content_sha256"]:
                raise ValueError("content hash differs from the signed hash")
            Ed25519PublicKey.from_public_bytes(bytes.fromhex(keys[m["signer_key_id"]])).verify(base64.b64decode(s["signature"]), m["content_sha256"].encode("ascii"))
            (root / "signature.ed25519").write_text(s["signature"] + "\n", encoding="ascii", newline="\n")
            print(f"OK {m['pack_id']}")
        except Exception as e:  # noqa: BLE001 - every failure is reported, then the run fails
            failures += 1
            print(f"FAIL {m['pack_id']}: {e}")
    return 1 if failures else 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--manifests", type=Path)
    ap.add_argument("--key", type=Path)
    ap.add_argument("--out", type=Path, default=Path("packs/central/signatures.json"))
    ap.add_argument("--insert", type=Path)
    ap.add_argument("--signatures", type=Path)
    ap.add_argument("--trust", type=Path)
    a = ap.parse_args()
    if a.insert:
        return insert(a.insert, a.signatures, a.trust)
    return sign(a.manifests, a.key, a.out)


if __name__ == "__main__":
    sys.exit(main())
