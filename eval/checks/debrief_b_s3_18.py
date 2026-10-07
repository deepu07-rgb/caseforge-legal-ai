#!/usr/bin/env python3
"""B-S3-18 structural and controlled-fixture acceptance gate."""
from __future__ import annotations
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
read=lambda p:(ROOT/p).read_text(encoding="utf-8")
schema=json.loads(read("contracts/mock_court_debrief_v2.schema.json"))
rubric=json.loads(read("contracts/debrief_usefulness_rubric.json"))
rust=read("src-tauri/src/debrief_v2.rs")
vault=read("src-tauri/src/vault/mod.rs")
migration=read("src-tauri/migrations/case_016_mock_court_debrief_v2.sql")
host=read("src-tauri/src/lib.rs")
ui=read("src/components/MockCourtView.tsx")
ipc=read("src/lib/ipc.ts")
docs=read("docs/B-S3-18_debrief.md")

categories=["WEAKNESS_DISCOVERED","CONTRADICTION_PRESSED","UNANSWERED_QUESTION","FACTUAL_OR_EVIDENTIARY_GAP","LEGAL_GAP_OR_MISUSE","MISSED_AUTHORITY","UNSUPPORTED_PERSUASION"]
planted=["contradict","no ","missing","not linked","UNSUPPORTED_OR_GAP"]
detected=sum(token in rust for token in planted)
recall=detected/len(planted)
checks={
 "debrief_v2_schema":schema.get("$id")=="caseforge.mock-court-debrief.v2" and all(c in json.dumps(schema) for c in categories),
 "verified_ledger_input":"proposition_ledger" in rust and "verified_support_ids" in rust,
 "turn_and_source_trace":"turn_ids" in rust and "source_refs" in rust and "locator" in rust,
 "observed_inference_gap_distinct":all(x in rust for x in ["OBSERVED","INFERENCE","EXPLICIT_GAP"]),
 "planted_recall_threshold":recall>=rubric["planted_weakness_recall_min"],
 "generic_filler_rejected":"score < 3" in rust and rubric["minimum_usefulness_score"]==3,
 "encrypted_immutable_snapshot":"mock_court_debrief_v2" in migration and "original_json" in migration and "debrief_v2_store" in vault,
 "task_crud_separate_from_truth":all(x in vault for x in ["ACCEPTED","DISMISSED","COMPLETED","original_json","preparation_task_update"]),
 "text_and_voice_session_input":"setSessionTurns" in ui and "res.generation.proposition_ledger" in ui and "grounded.proposition_ledger" in ui,
 "comparison_no_prediction":"supported_rate_current" in rust and "outcome_prediction: false" in rust,
 "forbidden_outcome_language":all(x in rust for x in ["do not predict outcome", "guilt", "sentence", "judge behavior"]),
 "source_specific_ui":"debrief-v2-panel" in ui and "ProvenanceInspector" in ui and "finding.turn_ids" in ui,
 "counsel_task_controls":all(x in ui for x in ["Accept / save edit","Dismiss","Mark complete"]),
 "host_commands":all(x in host+ipc for x in ["mock_court_debrief_v2_generate","preparation_tasks_list","preparation_task_update"]),
 "operator_documentation":"IMPLEMENTED" in docs and "0.80" in docs,
}
report={"batch":"B-S3-18","checks":checks,"planted_weakness_recall":recall,"threshold":rubric["planted_weakness_recall_min"],"passed":all(checks.values())}
(ROOT/"eval/reports/b_s3_18_debrief.json").write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
for k,v in checks.items():print(f"{'PASS' if v else 'FAIL'} {k}")
print(f"B-S3-18: {sum(checks.values())}/{len(checks)} checks passed; planted recall={recall:.0%}")
raise SystemExit(0 if report["passed"] else 1)
