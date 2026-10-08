#!/usr/bin/env python3
"""B-S4-27: add passage embeddings to a built pack (before signing).

Writes metadata/embeddings.e5s.i8 (int8, 384 dims, one row per chunk in legal_chunks.json order) and
metadata/embeddings.json (model id, dims, row count, SHA-256 of the ordered chunk-id list), adds both to the manifest
as DERIVED_ARTEFACT, and recomputes content_sha256 (authoritative content and chunk checksums are unchanged). Any
existing signature is removed: the pack must be signed again.

Usage: python eval/tools/embed_pack.py [--index packs_index.json] <pack_dir> [<pack_dir> ...]   (model: $CASEFORGE_EMBED_MODEL)
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "sidecars" / "docpipe"))
from embed import MODEL_ID, embed, quantise  # noqa: E402

MAX_FILE_BYTES = 64 * 1024 * 1024  # src-tauri/src/packs/mod.rs MAX_PACK_FILE_BYTES
LABEL = re.compile(r"^\[[^\]\n]{0,300}\]\n")


def digest(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def canonical(files: list[dict]) -> str:
    return digest(("\n".join(sorted(f"{f['path']}:{f['sha256']}" for f in files)) + "\n").encode())


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("packs", nargs="+", type=Path)
    ap.add_argument("--index", type=Path, help="pack index (packs_index.json) whose content_sha256/bytes entries are updated")
    a = ap.parse_args()
    updated: dict[str, dict] = {}
    for root in a.packs:
        chunks = json.loads((root / "metadata" / "legal_chunks.json").read_text(encoding="utf-8"))
        texts = [LABEL.sub("", c["exact_text"], count=1)[:2000] for c in chunks]
        t0 = time.time()
        vecs = embed(texts, kind="passage")
        blob = quantise(vecs)
        if len(blob) > MAX_FILE_BYTES:
            raise SystemExit(f"{root.name}: {len(blob)} bytes of vectors exceed the app's {MAX_FILE_BYTES}-byte pack file limit; split the pack")
        (root / "metadata" / "embeddings.e5s.i8").write_bytes(blob)
        ids = [c["chunk_id"] for c in chunks]
        meta = {"schema_version": "1.0.0", "model": MODEL_ID, "dims": len(vecs[0]) if vecs else 384, "rows": len(vecs), "dtype": "int8", "scale": 127,
                "row_order": "metadata/legal_chunks.json", "chunk_ids_sha256": digest("\n".join(ids).encode())}
        (root / "metadata" / "embeddings.json").write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8", newline="\n")
        mpath = root / "manifest.json"
        man = json.loads(mpath.read_text(encoding="utf-8"))
        files = [f for f in man["files"] if f["path"] not in ("metadata/embeddings.e5s.i8", "metadata/embeddings.json")]
        for rel in ("metadata/embeddings.e5s.i8", "metadata/embeddings.json"):
            p = root / rel
            files.append({"path": rel, "sha256": digest(p.read_bytes()), "bytes": p.stat().st_size, "role": "DERIVED_ARTEFACT"})
        files.sort(key=lambda f: f["path"])
        man["files"] = files
        man["content_sha256"] = canonical(files)
        mpath.write_text(json.dumps(man, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
        (root / "signature.ed25519").unlink(missing_ok=True)
        updated[man["pack_id"]] = {"content_sha256": man["content_sha256"], "bytes": sum(f["bytes"] for f in files)}
        print(f"{man['pack_id']}: {len(vecs)} vectors in {time.time() - t0:.0f}s; content_sha256 {man['content_sha256'][:16]} (re-sign required)", flush=True)
    if a.index:
        index = json.loads(a.index.read_text(encoding="utf-8"))
        for entry in index.get("packs", []):
            entry.update(updated.get(entry.get("pack_id"), {}))
        a.index.write_text(json.dumps(index, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
