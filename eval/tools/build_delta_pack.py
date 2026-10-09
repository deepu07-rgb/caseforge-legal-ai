#!/usr/bin/env python3
"""B-S4-13: build and check a DELTA pack (akpkg v2.1 `delta`): an update that carries only what changed since a base.

Pipeline (the cadence is policy, Q-S4-05; the channel is a file the user installs):
  1. build the NEW full pack and sign it as usual (eval/tools/sign_pack_manifests.py signs its content_sha256);
  2. build   python eval/tools/build_delta_pack.py build --base <full base pack dir> --new <signed full new pack dir> --out <delta dir>
     The delta has the new manifest plus `delta: {base_version, base_content_sha256}`, the new signature (it signs the
     new pack's content_sha256, which is unchanged by sending fewer files), and only the files that differ from the base;
  3. check   python eval/tools/build_delta_pack.py check --base <base dir> --delta <delta dir>
     re-applies the delta to the base exactly as the app does and confirms the result hashes to the signed content hash;
  4. the user installs the delta folder like any pack. The app refuses it unless the installed pack is exactly the base
     (version and content hash), takes unchanged files from the installed base (each must match the hash the manifest
     lists), runs the ordinary install on the result (signature, lineage, protection, transactional activate), and a
     failed install leaves the base pack installed. Not newer than the base, or a base that is missing, fails closed.
No network access, no key needed (the signature is the new pack's own).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
from pathlib import Path


def digest(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def canonical(files: list[dict]) -> str:
    return digest(("\n".join(sorted(f"{f['path']}:{f['sha256']}" for f in files)) + "\n").encode())


def read_manifest(d: Path) -> dict:
    return json.loads((d / "manifest.json").read_text(encoding="utf-8"))


def triple(v: str) -> tuple:
    parts = [int(x) for x in "".join(c if c.isdigit() else " " for c in v).split()[:3]]
    return tuple(parts + [0] * (3 - len(parts)))


def verify_tree(d: Path, m: dict, label: str, allow_missing: bool = False) -> None:
    for f in m["files"]:
        p = d / f["path"]
        if not p.is_file():
            if allow_missing:
                continue
            raise SystemExit(f"{label}: {f['path']} is listed but missing")
        if digest(p.read_bytes()) != f["sha256"]:
            raise SystemExit(f"{label}: {f['path']} does not match its listed hash")
    if not allow_missing and canonical(m["files"]) != m["content_sha256"]:
        raise SystemExit(f"{label}: content_sha256 is not the hash of the listed files")


def build(base: Path, new: Path, out: Path) -> dict:
    bm, nm = read_manifest(base), read_manifest(new)
    if bm["pack_id"] != nm["pack_id"] or bm["pack_kind"] != nm["pack_kind"]:
        raise SystemExit("the base and the new pack must be the same pack")
    if triple(nm["version"]) <= triple(bm["version"]):
        raise SystemExit(f"the new version {nm['version']} must be newer than the base {bm['version']}")
    if nm.get("delta"):
        raise SystemExit("the new pack must be a full pack, not a delta")
    verify_tree(base, bm, "base")
    verify_tree(new, nm, "new")
    if nm["pack_kind"] == "official" and not (new / "signature.ed25519").is_file():
        raise SystemExit("an official pack must be signed first: the delta carries the new pack's signature")
    before = {f["path"]: f["sha256"] for f in bm["files"]}
    changed = [f for f in nm["files"] if before.get(f["path"]) != f["sha256"]]
    if out.exists():
        shutil.rmtree(out)
    for f in changed:
        (out / f["path"]).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(new / f["path"], out / f["path"])
    nm = dict(nm)
    nm["delta"] = {"base_version": bm["version"], "base_content_sha256": bm["content_sha256"]}
    (out / "manifest.json").write_text(json.dumps(nm, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    if (new / "signature.ed25519").is_file():
        shutil.copyfile(new / "signature.ed25519", out / "signature.ed25519")
    full = sum(f["bytes"] for f in nm["files"])
    sent = sum(f["bytes"] for f in changed)
    return {"pack_id": nm["pack_id"], "base_version": bm["version"], "new_version": nm["version"], "files_sent": len(changed), "files_total": len(nm["files"]), "bytes_sent": sent, "bytes_full": full}


def apply(base: Path, delta: Path, target: Path) -> str:
    """The app's materialisation, in Python: returns the content hash of the result."""
    dm, bm = read_manifest(delta), read_manifest(base)
    d = dm.get("delta")
    if not d:
        raise SystemExit("not a delta")
    if bm["version"] != d["base_version"] or bm["content_sha256"] != d["base_content_sha256"]:
        raise SystemExit("the base is not the one this delta was built for")
    verify_tree(base, bm, "base")
    before = {f["path"]: f["sha256"] for f in bm["files"]}
    for f in dm["files"]:
        src = delta / f["path"]
        if not src.is_file():
            if before.get(f["path"]) != f["sha256"]:
                raise SystemExit(f"{f['path']} is neither in the delta nor unchanged from the base")
            src = base / f["path"]
        (target / f["path"]).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(src, target / f["path"])
    result = {k: v for k, v in dm.items() if k != "delta"}
    verify_tree(target, result, "result")
    return result["content_sha256"]


def main() -> int:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build")
    b.add_argument("--base", type=Path, required=True)
    b.add_argument("--new", type=Path, required=True)
    b.add_argument("--out", type=Path, required=True)
    c = sub.add_parser("check")
    c.add_argument("--base", type=Path, required=True)
    c.add_argument("--delta", type=Path, required=True)
    a = ap.parse_args()
    if a.cmd == "build":
        print(json.dumps(build(a.base, a.new, a.out), indent=2))
        return 0
    import tempfile

    with tempfile.TemporaryDirectory() as t:
        h = apply(a.base, a.delta, Path(t))
    print(json.dumps({"ok": True, "content_sha256": h}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
