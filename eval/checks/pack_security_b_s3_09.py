#!/usr/bin/env python3
"""B-S3-09 cumulative malicious-pack and transactional installer gate."""
from __future__ import annotations
import json, subprocess
from datetime import datetime, timezone
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def need(ok,msg):
    if not ok: raise AssertionError(msg)
fixtures=json.loads((ROOT/"packs/security-fixtures/b_s3_09_cases.json").read_text(encoding="utf-8")); ids={x["id"] for x in fixtures["cases"]}
for required in ("traversal","absolute-drive-unc","symlink-reparse","invalid-id-version","hash-byte-mismatch","signature-failure","oversized","unsupported-active","failed-upgrade","protection-failure","custom-trust-escalation"): need(required in ids,f"missing malicious fixture {required}")
packs=(ROOT/"src-tauri/src/packs/mod.rs").read_text(encoding="utf-8"); protection=(ROOT/"src-tauri/src/protection/mod.rs").read_text(encoding="utf-8"); ui=(ROOT/"src/components/PacksView.tsx").read_text(encoding="utf-8")
for token in ("safe_relative_path","reject_reparse","canonical_root","MAX_PACK_FILES","MAX_PACK_FILE_BYTES","MAX_PACK_TOTAL_BYTES","active_content","deny_unknown_fields","reject_trust_and_source_collisions",".staging-",".backup-","registry rollback","official protection failed","verify_pack_integrity"): need(token in packs,f"installer boundary missing {token}")
for token in ("CASEFORGE_PROTECTION_FORCE_FAIL","pack-security-report","transactional_upgrade","protection_verified"): need(token in protection+ui+packs,f"security proof/UI missing {token}")
proc=subprocess.run(["cargo","test","b_s3_09","--lib","--quiet"],cwd=ROOT/"src-tauri",capture_output=True,text=True,encoding="utf-8",errors="replace",timeout=300)
need(proc.returncode==0,"malicious pack regression failed\n"+(proc.stdout+proc.stderr)[-6000:])
report={"batch":"B-S3-09","generated_at":datetime.now(timezone.utc).isoformat(),"implementation_status":"PASS","formal_acceptance_status":"PASS","acceptance_gates":{f"AC-S3-9.{i}":"PASS" for i in range(1,6)},"fixture_count":len(fixtures["cases"]),"archive_transport":"UNSUPPORTED_AND_REJECTED","evidence_note":"Fixtures use synthetic passive text and inert exploit strings; no pack content is executed."}
out=ROOT/"eval/reports/b_s3_09_pack_security.json"; out.parent.mkdir(parents=True,exist_ok=True); out.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
print(f"PASS B-S3-09 malicious-pack hardening; fixtures={len(fixtures['cases'])}; report={out}")
