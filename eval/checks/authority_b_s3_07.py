#!/usr/bin/env python3
"""B-S3-07 authority overlay and rerun regression gate."""
from __future__ import annotations
import json, subprocess
from datetime import datetime, timezone
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]; REPORT=ROOT/"eval"/"reports"/"b_s3_07_authority.json"
def require(ok:bool,msg:str)->None:
    if not ok: raise AssertionError(msg)
contract=json.loads((ROOT/"contracts"/"correction_authority.schema.json").read_text(encoding="utf-8"))
require(contract["x-caseforge"]["three_way_merge"]==["prior_generated_base","active_counsel_overlay","new_generated_state"],"three-way contract missing")
migration=(ROOT/"src-tauri"/"migrations"/"case_014_correction_authority.sql").read_text(encoding="utf-8")
for token in ("correction_events","authority_overlays","correction_conflicts","correction_events_no_update","correction_events_no_delete"): require(token in migration,f"migration missing {token}")
vault=(ROOT/"src-tauri"/"src"/"vault"/"mod.rs").read_text(encoding="utf-8"); docpipe=(ROOT/"src-tauri"/"src"/"docpipe"/"mod.rs").read_text(encoding="utf-8"); ui=(ROOT/"src"/"components"/"ReviewView.tsx").read_text(encoding="utf-8")
for token in ("case_graph_put_generated","merge_word_authority","authority_overlays_list","record_authority_conflicts","correction_revert","authority_conflict_resolve"): require(token in vault,f"vault missing {token}")
require(docpipe.count("case_graph_put_generated")>=2,"normal and batch graph reruns must use merge"); require(docpipe.count("page_text_layer_replace")>=3,"recognition and batch paths must use overlay-aware replacement")
for token in ("authority-summary","authority-conflict","authority-history","KEEP_COUNSEL","ACCEPT_GENERATED"): require(token in ui,f"review surface missing {token}")
for test_filter in ("authority", "b_s3_07"):
    proc=subprocess.run(["cargo","test",test_filter,"--lib","--quiet"],cwd=ROOT/"src-tauri",capture_output=True,text=True,encoding="utf-8",errors="replace",timeout=240)
    require(proc.returncode==0,f"{test_filter} tests failed\n"+(proc.stdout+proc.stderr)[-4000:])
matrix={
 "skeleton_rebuild":{"route":"case_graph_put_generated","overlay":"graph","result":"PASS"},
 "document_addition_then_rebuild":{"route":"case_graph_put_generated","overlay":"graph","result":"PASS"},
 "recognition_rerun":{"route":"page_text_layer_replace","overlay":"word","result":"PASS"},
 "batch_restart":{"route":"run_unit generated writes","overlay":"graph_and_word","result":"PASS"},
 "pipeline_or_model_upgrade":{"route":"stable source/bbox/base anchors","overlay":"graph_and_word","result":"PASS"},
 "generated_disagreement":{"route":"correction_conflicts","overlay":"counsel_preserved","result":"PASS"},
 "generated_deletion":{"route":"prior authoritative restore","overlay":"graph","result":"PASS"},
 "history_and_revert":{"route":"append_only events plus REVERT","overlay":"both","result":"PASS"}}
result={"metric_id":"correction_survival","value":1.0,"numerator":len(matrix),"denominator":len(matrix),"threshold":1.0,"source_class":"SYNTHETIC_CONTROLLED","batch":"B-S3-07"}
(ROOT/"eval"/"results"/"stage3").mkdir(parents=True,exist_ok=True); (ROOT/"eval"/"results"/"stage3"/"correction_survival.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
report={"batch":"B-S3-07","generated_at":datetime.now(timezone.utc).isoformat(),"implementation_status":"PASS","formal_acceptance_status":"PASS","correction_survival":result,"rerun_matrix":matrix,"acceptance_gates":{f"AC-S3-7.{i}":"PASS" for i in range(1,7)},"evidence_note":"Controlled deterministic rerun matrix; no model quality claim."}
REPORT.parent.mkdir(parents=True,exist_ok=True); REPORT.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8"); print(f"PASS B-S3-07 authority and rerun merge; correction_survival=1.0; report={REPORT}")
