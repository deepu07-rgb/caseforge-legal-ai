#!/usr/bin/env python3
"""B-S3-08 legal source/chunk metadata acceptance gate."""
from __future__ import annotations
import json, subprocess, tempfile
from datetime import datetime, timezone
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def need(ok,msg):
    if not ok: raise AssertionError(msg)
def run(args,cwd=ROOT):
    p=subprocess.run(args,cwd=cwd,capture_output=True,text=True,encoding="utf-8",errors="replace",timeout=240)
    need(p.returncode==0,(p.stdout+p.stderr)[-5000:]); return p.stdout+p.stderr
for name in ("legal_source.schema.json","legal_chunk.schema.json","akpkg.schema.json","citation_record.schema.json"):
    data=json.loads((ROOT/"contracts"/name).read_text(encoding="utf-8")); need(data.get("schema_version")=="2.0.0",f"{name} not v2")
source_schema=json.loads((ROOT/"contracts/legal_source.schema.json").read_text(encoding="utf-8")); chunk_schema=json.loads((ROOT/"contracts/legal_chunk.schema.json").read_text(encoding="utf-8"))
for token in ("authority_type","trust_level","superseded_status","effective_from","exact_text","checksum","source_reference"): need(token in source_schema["properties"],f"source missing {token}")
for token in ("source_checksum","pack_checksum","temporal_status","content_kind"): need(token in chunk_schema["properties"],f"chunk missing {token}")
rust=(ROOT/"src-tauri/src/retrieval/mod.rs").read_text(encoding="utf-8")+(ROOT/"src-tauri/src/packs/mod.rs").read_text(encoding="utf-8")
for token in ("validate_legal_metadata","load_legal_chunks","MetadataFilters","OFFICIAL_VERIFIED","DEVELOPMENT_ONLY","controlling_metadata","DERIVED_SUMMARY"): need(token in rust,f"runtime missing {token}")
legacy=run(["python","scripts/legal_pack_tool.py","validate","packs/fixtures/caseforge.core.india.placeholder"]); need("DEVELOPMENT_ONLY" in legacy,"legacy pack not isolated")
with tempfile.TemporaryDirectory(prefix="caseforge-s3-08-") as td:
    pack=Path(td); (pack/"content").mkdir(); (pack/"metadata").mkdir(); exact="Section 1. The verified metadata rule applies."
    (pack/"content/rule.txt").write_text(exact,encoding="utf-8")
    manifest={"schema_version":"2.0.0","metadata_version":"2.0.0","pack_id":"caseforge.eval.metadata","pack_kind":"official","jurisdiction":"India","jurisdiction_path":["India"],"version":"1.0.0","display_name":"Metadata Eval","description":None,"content_sha256":"0"*64,"authoritative_content_sha256":"0"*64,"files":[],"headnotes_stripped":True,"signature_alg":"Ed25519","signer_key_id":"eval","trust_level":"OFFICIAL_VERIFIED","controlled_beta_eligible":True,"sources_file":"metadata/legal_sources.json","chunks_file":"metadata/legal_chunks.json"}
    source={"schema_version":"2.0.0","source_id":"placeholder","title":"Verified Metadata Rule","jurisdiction":"India","jurisdiction_path":["India"],"authority_type":"RULE","issuing_authority":"Supreme Court","citation":"Metadata Rule 2026","section":"1","effective_from":"2026-01-01","effective_to":"UNKNOWN","version":"1","amendment":"NONE","source_file":"content/rule.txt","page":1,"paragraph":"1","heading_path":["Rule","Section 1"],"exact_text":exact,"checksum":"sha256:"+"0"*64,"pack_id":"caseforge.eval.metadata","trust_level":"OFFICIAL_VERIFIED","superseded_status":"CURRENT","publisher":"Official Gazette","source_reference":"urn:caseforge:eval:rule","content_kind":"AUTHORITATIVE_TEXT","derived_from_source_id":None}
    chunk={"schema_version":"2.0.0","chunk_id":"placeholder","source_title":"Verified Metadata Rule","source_id":"","pack_id":"caseforge.eval.metadata","title":source["title"],"jurisdiction":"India","jurisdiction_path":["India"],"authority_type":"RULE","issuing_authority":"Supreme Court","citation":"Metadata Rule 2026","section":"1","effective_from":"2026-01-01","effective_to":"UNKNOWN","version":"1","amendment":"NONE","source_file":"content/rule.txt","page":1,"paragraph":"1","heading_path":["Rule","Section 1"],"exact_text":exact,"checksum":"sha256:"+"0"*64,"source_checksum":"sha256:"+"0"*64,"pack_checksum":"0"*64,"trust_level":"OFFICIAL_VERIFIED","superseded_status":"CURRENT","publisher":"Official Gazette","source_reference":"urn:caseforge:eval:rule","content_kind":"AUTHORITATIVE_TEXT","derived_from_source_id":None,"temporal_status":"APPLICABLE"}
    for path,value in ((pack/"manifest.json",manifest),(pack/"metadata/legal_sources.json",[source]),(pack/"metadata/legal_chunks.json",[chunk])): path.write_text(json.dumps(value,indent=2)+"\n",encoding="utf-8")
    run(["python","scripts/legal_pack_tool.py","build",str(pack)]); run(["python","scripts/legal_pack_tool.py","validate",str(pack)])
run(["cargo","test","b_s3_08","--lib","--quiet"],ROOT/"src-tauri")
report={"batch":"B-S3-08","generated_at":datetime.now(timezone.utc).isoformat(),"implementation_status":"PASS","formal_acceptance_status":"PASS","acceptance_gates":{f"AC-S3-8.{i}":"PASS" for i in range(1,6)},"legacy_placeholder_policy":"DEVELOPMENT_ONLY","evidence_note":"Synthetic contract/lineage fixture; no claim that placeholder packs are legal authorities."}
out=ROOT/"eval/reports/b_s3_08_legal_metadata.json"; out.parent.mkdir(parents=True,exist_ok=True); out.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
print(f"PASS B-S3-08 legal source/chunk metadata; report={out}")
