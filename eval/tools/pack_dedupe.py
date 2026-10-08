#!/usr/bin/env python3
"""Remove duplicate sources and passages (same source_id / chunk_id, first occurrence kept) from built packs, together
with their rows in the pack's meaning vectors (metadata/embeddings.e5s.i8), and refresh the manifest hashes.

Why: the processed judgment corpus lists a few old reports under two year archives, so a pack built from it can hold
the same judgment twice; the app rejects a pack with duplicate ids. Any existing signature is removed: sign again.

Usage: python eval/tools/pack_dedupe.py [--index packs_index.json] <pack_dir> [<pack_dir> ...]
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def digest(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def canonical(files: list[dict]) -> str:
    return digest(("\n".join(sorted(f"{f['path']}:{f['sha256']}" for f in files)) + "\n").encode())


def write_json(path: Path, value) -> None:
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")


def first_of(items: list[dict], key: str) -> list[int]:
    seen, keep = set(), []
    for i, it in enumerate(items):
        if it[key] not in seen:
            seen.add(it[key])
            keep.append(i)
    return keep


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("packs", nargs="+", type=Path)
    ap.add_argument("--index", type=Path)
    a = ap.parse_args()
    updated: dict[str, dict] = {}
    for root in a.packs:
        meta = root / "metadata"
        sources = json.loads((meta / "legal_sources.json").read_text(encoding="utf-8"))
        chunks = json.loads((meta / "legal_chunks.json").read_text(encoding="utf-8"))
        keep_s, keep_c = first_of(sources, "source_id"), first_of(chunks, "chunk_id")
        if len(keep_s) == len(sources) and len(keep_c) == len(chunks):
            print(f"{root.name}: no duplicates")
            continue
        changed = ["metadata/legal_sources.json", "metadata/legal_chunks.json"]
        write_json(meta / "legal_sources.json", [sources[i] for i in keep_s])
        write_json(meta / "legal_chunks.json", [chunks[i] for i in keep_c])
        vec, emb = meta / "embeddings.e5s.i8", meta / "embeddings.json"
        if vec.exists():
            info = json.loads(emb.read_text(encoding="utf-8"))
            dims, blob = info["dims"], vec.read_bytes()
            assert len(blob) == dims * len(chunks) == dims * info["rows"], "vector rows do not match the chunk list"
            vec.write_bytes(b"".join(blob[i * dims:(i + 1) * dims] for i in keep_c))
            info["rows"] = len(keep_c)
            info["chunk_ids_sha256"] = digest("\n".join(chunks[i]["chunk_id"] for i in keep_c).encode())
            emb.write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8", newline="\n")
            changed += ["metadata/embeddings.e5s.i8", "metadata/embeddings.json"]
        ji = meta / "judgment_index.json"
        if ji.exists():
            rows = json.loads(ji.read_text(encoding="utf-8"))
            write_json(ji, [rows[i] for i in first_of(rows, "path")])
            changed.append("metadata/judgment_index.json")
        mpath = root / "manifest.json"
        man = json.loads(mpath.read_text(encoding="utf-8"))
        for f in man["files"]:
            if f["path"] in changed:
                data = (root / f["path"]).read_bytes()
                f["sha256"], f["bytes"] = digest(data), len(data)
        man["content_sha256"] = canonical(man["files"])
        write_json(mpath, man)
        (root / "signature.ed25519").unlink(missing_ok=True)
        updated[man["pack_id"]] = {"content_sha256": man["content_sha256"], "bytes": sum(f["bytes"] for f in man["files"])}
        print(f"{man['pack_id']}: sources {len(sources)}->{len(keep_s)}, passages {len(chunks)}->{len(keep_c)}; content_sha256 {man['content_sha256'][:16]} (re-sign required)")
    if a.index and updated:
        index = json.loads(a.index.read_text(encoding="utf-8"))
        for entry in index.get("packs", []):
            entry.update(updated.get(entry.get("pack_id"), {}))
        write_json(a.index, index)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
