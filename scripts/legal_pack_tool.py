#!/usr/bin/env python3
"""Build/check CaseForge .akpkg v2 metadata. No network access or model dependency."""
from __future__ import annotations
import argparse, hashlib, json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def digest(data: bytes) -> str: return hashlib.sha256(data).hexdigest()
def read(path: Path): return json.loads(path.read_text(encoding="utf-8"))
def write(path: Path, value) -> None: path.write_text(json.dumps(value, indent=2, ensure_ascii=False)+"\n", encoding="utf-8")
def stable(prefix: str, pack_id: str, checksum: str) -> str: return f"{prefix}:{pack_id}:{checksum.removeprefix('sha256:')[:16]}"
def stable_chunk(pack_id: str, source_id: str, checksum: str) -> str:
    return f"cfchunk:{pack_id}:{digest((source_id+'|'+checksum).encode())[:16]}"

def schema_validate(name: str, value) -> None:
    try:
        import jsonschema
    except ImportError as exc:
        raise SystemExit("jsonschema is required: python -m pip install jsonschema") from exc
    jsonschema.Draft202012Validator(read(ROOT/"contracts"/name), format_checker=jsonschema.FormatChecker()).validate(value)

def canonical(files: list[dict]) -> str:
    return digest(("\n".join(sorted(f"{f['path']}:{f['sha256']}" for f in files))+"\n").encode())
def authoritative(files: list[dict]) -> str:
    return canonical([f for f in files if f.get("role")=="AUTHORITATIVE_CONTENT"])

def build(pack: Path) -> None:
    manifest=read(pack/"manifest.json"); pack_id=manifest["pack_id"]
    sources=read(pack/"metadata/legal_sources.json")
    for source in sources:
        source["schema_version"]="2.0.0"; source["pack_id"]=pack_id
        source["checksum"]="sha256:"+digest(source["exact_text"].encode())
        source["source_id"]=stable("cfsrc",pack_id,source["checksum"])
    by_title={s["title"]:s for s in sources}; by_id={s["source_id"]:s for s in sources}
    chunks=read(pack/"metadata/legal_chunks.json")
    for chunk in chunks:
        chunk["schema_version"]="2.0.0"; chunk["pack_id"]=pack_id
        if not chunk.get("source_id") and chunk.get("source_title") in by_title: chunk["source_id"]=by_title[chunk.pop("source_title")]["source_id"]
        source=by_id[chunk["source_id"]]
        chunk["checksum"]="sha256:"+digest(chunk["exact_text"].encode())
        chunk["chunk_id"]=stable_chunk(pack_id,chunk["source_id"],chunk["checksum"])
        chunk["source_checksum"]=source["checksum"]
    write(pack/"metadata/legal_sources.json",sources); write(pack/"metadata/legal_chunks.json",chunks)
    files=[]
    for path in sorted(p for p in pack.rglob("*") if p.is_file() and p.name not in {"manifest.json","signature.ed25519"}):
        rel=path.relative_to(pack).as_posix(); role="AUTHORITATIVE_CONTENT"
        if rel=="metadata/legal_sources.json": role="LEGAL_SOURCES"
        elif rel=="metadata/legal_chunks.json": role="LEGAL_CHUNKS"
        files.append({"path":rel,"sha256":digest(path.read_bytes()),"bytes":path.stat().st_size,"role":role})
    manifest.update(schema_version="2.0.0",metadata_version="2.0.0",files=files,sources_file="metadata/legal_sources.json",chunks_file="metadata/legal_chunks.json")
    manifest["authoritative_content_sha256"]=authoritative(files)
    for chunk in chunks: chunk["pack_checksum"]=manifest["authoritative_content_sha256"]
    write(pack/"metadata/legal_chunks.json",chunks)
    for f in files:
        path=pack/f["path"]; f["sha256"]=digest(path.read_bytes()); f["bytes"]=path.stat().st_size
    manifest["content_sha256"]=canonical(files); write(pack/"manifest.json",manifest)
    print(f"BUILT {pack} sources={len(sources)} chunks={len(chunks)}; sign content_sha256 before official install")

def validate(pack: Path) -> None:
    manifest=read(pack/"manifest.json")
    if manifest.get("schema_version")=="1.0.0":
        print("DEVELOPMENT_ONLY legacy v1; controlled_beta_eligible=false"); return
    schema_validate("akpkg_v2_1.schema.json" if manifest.get("schema_version")=="2.1.0" else "akpkg.schema.json",manifest)
    for f in manifest["files"]:
        path=pack/f["path"]
        assert path.is_file() and digest(path.read_bytes())==f["sha256"], f"file checksum mismatch: {f['path']}"
    assert canonical(manifest["files"])==manifest["content_sha256"], "pack checksum mismatch"
    assert authoritative(manifest["files"])==manifest["authoritative_content_sha256"], "authoritative checksum mismatch"
    sources=read(pack/manifest["sources_file"]); chunks=read(pack/manifest["chunks_file"])
    by_id={}
    for source in sources:
        schema_validate("legal_source.schema.json",source)
        assert source["checksum"]=="sha256:"+digest(source["exact_text"].encode())
        assert source["source_id"]==stable("cfsrc",manifest["pack_id"],source["checksum"])
        assert source["exact_text"] in (pack/source["source_file"]).read_text(encoding="utf-8")
        by_id[source["source_id"]]=source
    for chunk in chunks:
        schema_validate("legal_chunk.schema.json",chunk); source=by_id[chunk["source_id"]]
        assert chunk["checksum"]=="sha256:"+digest(chunk["exact_text"].encode())
        assert chunk["chunk_id"]==stable_chunk(manifest["pack_id"],chunk["source_id"],chunk["checksum"])
        assert chunk["source_checksum"]==source["checksum"] and chunk["exact_text"] in source["exact_text"]
        assert chunk["pack_checksum"]==manifest["authoritative_content_sha256"]
    print(f"PASS .akpkg v2 metadata sources={len(sources)} chunks={len(chunks)}")

def main() -> None:
    p=argparse.ArgumentParser(); p.add_argument("command",choices=["build","validate"]); p.add_argument("pack_dir",type=Path); a=p.parse_args()
    (build if a.command=="build" else validate)(a.pack_dir.resolve())
if __name__=="__main__": main()
