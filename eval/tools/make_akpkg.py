#!/usr/bin/env python3
"""Build CaseForge .akpkg directory packs + Ed25519-sign official packs.

Uses cryptography (Ed25519). Updates contracts/pack_trust.json public key.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import re
import sys
from pathlib import Path

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

ROOT = Path(__file__).resolve().parents[2]
KEY_DIR = ROOT / "eval" / "tools" / ".pack_keys"
TRUST = ROOT / "contracts" / "pack_trust.json"

HEADNOTE_RE = re.compile(
    r"(?i)\b(head\s*notes?|editorial\s+headnote|held\s*:|ratio\s+decidendi\s*summary)\b"
)
KEY_ID = "caseforge-official-dev-2026"


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def canonical_content_hash(files: list[dict]) -> str:
    lines = sorted(f"{f['path']}:{f['sha256']}" for f in files)
    blob = ("\n".join(lines) + "\n").encode("utf-8")
    return hashlib.sha256(blob).hexdigest()


def headnote_scan(content_root: Path) -> list[str]:
    hits = []
    for p in sorted(content_root.rglob("*")):
        if not p.is_file():
            continue
        text = p.read_text(encoding="utf-8", errors="replace")
        if HEADNOTE_RE.search(text):
            hits.append(str(p.relative_to(content_root)).replace("\\", "/"))
    return hits


def ensure_keypair() -> Ed25519PrivateKey:
    KEY_DIR.mkdir(parents=True, exist_ok=True)
    sk_path = KEY_DIR / "official_dev.sk.pem"
    if sk_path.exists():
        data = sk_path.read_bytes()
        return serialization.load_pem_private_key(data, password=None)  # type: ignore
    sk = Ed25519PrivateKey.generate()
    sk_path.write_bytes(
        sk.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.PKCS8,
            encryption_algorithm=serialization.NoEncryption(),
        )
    )
    pk = sk.public_key()
    (KEY_DIR / "official_dev.pk.pem").write_bytes(
        pk.public_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PublicFormat.SubjectPublicKeyInfo,
        )
    )
    raw_pk = pk.public_bytes(
        encoding=serialization.Encoding.Raw,
        format=serialization.PublicFormat.Raw,
    )
    update_trust(raw_pk.hex())
    return sk


def update_trust(pk_hex: str) -> None:
    data = json.loads(TRUST.read_text(encoding="utf-8"))
    for k in data.get("keys") or []:
        if k.get("key_id") == KEY_ID:
            k["public_key_hex"] = pk_hex
            k["status"] = "active"
    TRUST.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def build_pack(
    out_dir: Path,
    pack_id: str,
    pack_kind: str,
    jurisdiction: str | None,
    display_name: str,
    version: str,
    content_files: list[Path],
    description: str | None = None,
) -> Path:
    if out_dir.exists():
        # clean content only
        content_old = out_dir / "content"
        if content_old.is_dir():
            for p in content_old.rglob("*"):
                if p.is_file():
                    p.unlink()
    out_dir.mkdir(parents=True, exist_ok=True)
    content_dir = out_dir / "content"
    content_dir.mkdir(exist_ok=True)
    files_meta: list[dict] = []
    for src in content_files:
        dest = content_dir / src.name
        dest.write_bytes(src.read_bytes())
        rel = f"content/{src.name}".replace("\\", "/")
        files_meta.append(
            {
                "path": rel,
                "sha256": sha256_file(dest),
                "bytes": dest.stat().st_size,
            }
        )

    if pack_kind == "official":
        hits = headnote_scan(content_dir)
        if hits:
            raise SystemExit(f"AC-7.1 headnote scan failed: {hits}")

    content_hash = canonical_content_hash(files_meta)
    manifest: dict = {
        "schema_version": "1.0.0",
        "pack_id": pack_id,
        "pack_kind": pack_kind,
        "jurisdiction": jurisdiction,
        "version": version,
        "display_name": display_name,
        "description": description,
        "content_sha256": content_hash,
        "files": files_meta,
        "headnotes_stripped": pack_kind == "official",
    }
    if pack_kind == "official":
        manifest["signature_alg"] = "Ed25519"
        manifest["signer_key_id"] = KEY_ID

    (out_dir / "manifest.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )

    if pack_kind == "official":
        sk = ensure_keypair()
        # Refresh trust public key every build
        pk_raw = sk.public_key().public_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PublicFormat.Raw,
        )
        update_trust(pk_raw.hex())
        sig = sk.sign(content_hash.encode("ascii"))
        (out_dir / "signature.ed25519").write_text(
            base64.b64encode(sig).decode("ascii") + "\n", encoding="utf-8"
        )

    print(f"PASS  built pack {pack_id} kind={pack_kind} at {out_dir}")
    print(f"  content_sha256={content_hash}")
    return out_dir


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--pack-id", required=True)
    ap.add_argument("--kind", choices=["official", "custom"], required=True)
    ap.add_argument("--jurisdiction", default=None)
    ap.add_argument("--name", required=True)
    ap.add_argument("--version", default="0.1.0")
    ap.add_argument("--content", type=Path, action="append", default=[])
    ap.add_argument("--description", default=None)
    ap.add_argument("--placeholder-core", choices=["india", "us"], default=None)
    args = ap.parse_args()

    content_files: list[Path] = list(args.content)
    if args.placeholder_core:
        staging = args.out.parent / f"_staging_{args.placeholder_core}"
        staging.mkdir(parents=True, exist_ok=True)
        p = staging / "neutral_citation_sample.txt"
        if args.placeholder_core == "india":
            p.write_text(
                "[CASEFORGE_PACK_PLACEHOLDER_NOT_LAW]\n"
                "Neutral citation: [2020] INSC DEMO 1\n"
                "Court: Supreme Court of India (synthetic)\n"
                "Raw judgment text excerpt for pack plumbing only.\n"
                "The Court observed that consideration under the contract was admitted.\n",
                encoding="utf-8",
            )
            args.jurisdiction = args.jurisdiction or "India"
        else:
            p.write_text(
                "[CASEFORGE_PACK_PLACEHOLDER_NOT_LAW]\n"
                "Neutral citation: 560 U.S. DEMO (synthetic)\n"
                "Court: United States Supreme Court (synthetic)\n"
                "Raw judgment text excerpt for pack plumbing only.\n"
                "The Court noted the statutory text as enacted by Congress.\n",
                encoding="utf-8",
            )
            args.jurisdiction = args.jurisdiction or "United States"
        content_files.append(p)

    if not content_files:
        print("FAIL  provide --content or --placeholder-core", file=sys.stderr)
        return 1

    build_pack(
        args.out,
        pack_id=args.pack_id,
        pack_kind=args.kind,
        jurisdiction=args.jurisdiction,
        display_name=args.name,
        version=args.version,
        content_files=content_files,
        description=args.description,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
