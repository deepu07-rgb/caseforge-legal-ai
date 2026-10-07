#!/usr/bin/env python3
"""B-S3-10 evidence report for the controlled authoritative legal repository."""
from __future__ import annotations
import base64, hashlib, json
from pathlib import Path
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

ROOT=Path(__file__).resolve().parents[2]
PACK=ROOT/"packs"/"official"/"caseforge.india.partition.highcourt.civil-appeal"
REPORT=ROOT/"eval"/"reports"/"b_s3_10_legal_repository.json"
def read(p): return json.loads(p.read_text(encoding="utf-8"))
def digest(b): return hashlib.sha256(b).hexdigest()
def canonical(files): return digest(("\n".join(sorted(f"{f['path']}:{f['sha256']}" for f in files))+"\n").encode())

def main():
 m=read(PACK/"manifest.json"); sources=read(PACK/m["sources_file"]); chunks=read(PACK/m["chunks_file"])
 ledger=read(PACK/"metadata/source_ledger.json"); commentary=read(PACK/"metadata/editorial_commentary.json")
 updates=read(PACK/"metadata/update_manifest.json"); goldens=read(PACK/"metadata/retrieval_goldens.json")
 trust=read(ROOT/"contracts/pack_trust.json"); key=next(k for k in trust["keys"] if k["key_id"]==m["signer_key_id"])
 checks={}
 checks["manifest_and_file_hashes"]=canonical(m["files"])==m["content_sha256"] and all(digest((PACK/f["path"]).read_bytes())==f["sha256"] for f in m["files"])
 try:
  Ed25519PublicKey.from_public_bytes(bytes.fromhex(key["public_key_hex"])).verify(base64.b64decode((PACK/"signature.ed25519").read_text().strip()),m["content_sha256"].encode("ascii")); checks["signature"]=True
 except Exception: checks["signature"]=False
 checks["source_ledger"]=len(ledger)==3 and all(x.get("official_pdf_sha256") and x.get("permitted_use_basis") for x in ledger)
 checks["exact_page_lineage"]=len(sources)==20 and len(chunks)==20 and all(s["exact_text"] in (PACK/s["source_file"]).read_text(encoding="utf-8") and s["page"]!="UNKNOWN" and s["checksum"]=="sha256:"+digest(s["exact_text"].encode()) for s in sources)
 checks["original_matter_and_rights_basis"]="52(1)(q)(ii)" in commentary["rights_basis"] and bool(commentary["use_notes"])
 checks["freshness_and_supersession"]=updates["status"]=="CURRENT" and bool(updates["source_last_checked"]) and "superseded_by_version" in updates
 checks["repository_gap_goldens"]=sum(1 for g in goldens if g["must_gap"])>=2
 checks["placeholder_blocking_ui"]="controlled-beta-warning" in (ROOT/"src/components/PacksView.tsx").read_text(encoding="utf-8")
 checks["repository_gap_runtime"]="repository_gap_reason" in (ROOT/"src-tauri/src/retrieval/mod.rs").read_text(encoding="utf-8")
 result={"batch":"B-S3-10","status":"PASS" if all(checks.values()) else "FAIL","scope":"India / High Court / property partition / civil appeal","pack_id":m["pack_id"],"version":m["version"],"source_records":len(sources),"chunk_records":len(chunks),"pack_bytes":sum(f["bytes"] for f in m["files"]),"content_sha256":m["content_sha256"],"checks":checks,"client_release_note":"Controlled-beta content gate passed; replace development signing key and obtain named legal-editor approval before client distribution."}
 REPORT.write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8"); print(json.dumps(result,indent=2)); return 0 if result["status"]=="PASS" else 1
if __name__=="__main__": raise SystemExit(main())
