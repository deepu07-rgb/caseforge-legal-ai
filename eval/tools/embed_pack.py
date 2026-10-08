#!/usr/bin/env python3
"""B-S4-27: add passage embeddings to a built pack (before signing).

Writes metadata/embeddings.e5s.i8 (int8, 384 dims, one row per chunk in legal_chunks.json order) and
metadata/embeddings.json (model id, dims, row count, SHA-256 of the ordered chunk-id list), adds both to the manifest
as DERIVED_ARTEFACT, and recomputes content_sha256 (authoritative content and chunk checksums are unchanged). Any
existing signature is removed: the pack must be signed again.

Vector cache (--cache / --cache-out / --harvest): vectors are keyed by the SHA-256 of the exact passage text that was embedded,
so a pack that is rebuilt (new parts, text clean-ups, new Acts) only embeds passages it has not seen. --harvest copies the
vectors already present in the given packs into the cache without running the model.

Usage: python eval/tools/embed_pack.py [--index packs_index.json] [--cache in.bin] [--cache-out out.bin] [--harvest]
                                       <pack_dir> [<pack_dir> ...]   (model: $CASEFORGE_EMBED_MODEL)
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

DIMS = 384
MAX_FILE_BYTES = 64 * 1024 * 1024  # src-tauri/src/packs/mod.rs MAX_PACK_FILE_BYTES
LABEL = re.compile(r"^\[[^\]\n]{0,300}\]\n")


def digest(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def canonical(files: list[dict]) -> str:
    return digest(("\n".join(sorted(f"{f['path']}:{f['sha256']}" for f in files)) + "\n").encode())


def load_cache(path: Path | None) -> dict[bytes, bytes]:
    if not path or not path.is_file():
        return {}
    raw = path.read_bytes()
    rec = 32 + DIMS
    assert len(raw) % rec == 0, "vector cache is damaged"
    return {raw[i:i + 32]: raw[i + 32:i + rec] for i in range(0, len(raw), rec)}


def save_cache(path: Path, cache: dict[bytes, bytes]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"".join(k + v for k, v in sorted(cache.items())))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("packs", nargs="+", type=Path)
    ap.add_argument("--cache", type=Path, help="vector cache to reuse")
    ap.add_argument("--cache-out", type=Path, help="write the cache (input + new vectors) here")
    ap.add_argument("--harvest", action="store_true", help="only copy the vectors already in the packs into the cache")
    ap.add_argument("--index", type=Path, help="pack index (packs_index.json) whose content_sha256/bytes entries are updated")
    a = ap.parse_args()
    updated: dict[str, dict] = {}
    cache = load_cache(a.cache)
    for root in a.packs:
        chunks = json.loads((root / "metadata" / "legal_chunks.json").read_text(encoding="utf-8"))
        texts = [LABEL.sub("", c["exact_text"], count=1)[:2000] for c in chunks]
        keys = [bytes.fromhex(digest(t.encode())) for t in texts]
        if a.harvest:
            old = (root / "metadata" / "embeddings.e5s.i8").read_bytes()
            assert len(old) == DIMS * len(chunks), f"{root.name}: vectors do not match the chunk list"
            cache.update({k: old[i * DIMS:(i + 1) * DIMS] for i, k in enumerate(keys)})
            print(f"{root.name}: harvested {len(chunks)} vectors")
            continue
        t0 = time.time()
        missing = sorted({i for i, k in enumerate(keys) if k not in cache}, key=lambda i: len(texts[i]))
        if missing:
            fresh = quantise(embed([texts[i] for i in missing], kind="passage"))
            for n, i in enumerate(missing):
                cache[keys[i]] = fresh[n * DIMS:(n + 1) * DIMS]
        print(f"{root.name}: {len(chunks) - len(missing)} vectors reused, {len(missing)} embedded", flush=True)
        blob = b"".join(cache[k] for k in keys)
        if len(blob) > MAX_FILE_BYTES:
            raise SystemExit(f"{root.name}: {len(blob)} bytes of vectors exceed the app's {MAX_FILE_BYTES}-byte pack file limit; split the pack")
        (root / "metadata" / "embeddings.e5s.i8").write_bytes(blob)
        ids = [c["chunk_id"] for c in chunks]
        meta = {"schema_version": "1.0.0", "model": MODEL_ID, "dims": DIMS, "rows": len(chunks), "dtype": "int8", "scale": 127,
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
        print(f"{man['pack_id']}: {len(chunks)} vectors in {time.time() - t0:.0f}s; content_sha256 {man['content_sha256'][:16]} (re-sign required)", flush=True)
    if a.cache_out:
        save_cache(a.cache_out, cache)
    if a.index:
        index = json.loads(a.index.read_text(encoding="utf-8"))
        for entry in index.get("packs", []):
            entry.update(updated.get(entry.get("pack_id"), {}))
        a.index.write_text(json.dumps(index, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
