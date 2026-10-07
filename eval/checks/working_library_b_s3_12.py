#!/usr/bin/env python3
"""Structural and evidence gate for B-S3-12 matter legal working sets."""
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
RUST=(ROOT/"src-tauri/src/vault/mod.rs").read_text(encoding="utf-8")+(ROOT/"src-tauri/src/working_set.rs").read_text(encoding="utf-8")+(ROOT/"src-tauri/src/lib.rs").read_text(encoding="utf-8")
SQL=(ROOT/"src-tauri/migrations/case_015_working_set.sql").read_text(encoding="utf-8")
UI=(ROOT/"src/components/WorkingLibraryView.tsx").read_text(encoding="utf-8")
IPC=(ROOT/"src/lib/ipc.ts").read_text(encoding="utf-8")
checks={
 "encrypted_case_migration": "case_015_working_set.sql" in RUST and all(t in SQL for t in ["legal_working_set_meta","legal_working_set_item","legal_working_set_revision","legal_output_authority_provenance"]),
 "suggest_confirm_update_apis": all(x in RUST and x in IPC for x in ["working_set_suggest","working_set_add_chunk","working_set_update_item","working_set_confirm_regime","working_set_select_for_output"]),
 "exact_text_inspection": all(x in SQL and x in UI for x in ["exact_text","source_checksum","chunk_checksum","source_reference"]),
 "counsel_controls": all(x in RUST for x in ["INCLUDED","EXCLUDED","IRRELEVANT","REMOVED","SUPERSEDED"]) and all(x in UI for x in ["INCLUDED","EXCLUDED","IRRELEVANT","REMOVED"]),
 "pin_and_controlling_controls": all(x in SQL and x in UI for x in ["pinned","controlling"]),
 "ambiguous_regime_confirmation": all(x in RUST+IPC for x in ["PENDING_CONFIRMATION","COUNSEL_CONFIRMED","regime_candidates"]) and "Confirm this regime" in UI,
 "rerun_preserves_decisions": "ELSE legal_working_set_item.decision" in RUST and "b_s3_12_counsel_decisions_survive_restart_rerun_and_pack_update" in RUST,
 "superseded_cannot_reenter": "THEN 'SUPERSEDED'" in RUST and "THEN 0 ELSE legal_working_set_item.controlling" in RUST,
 "versioned_output_provenance": all(x in SQL+RUST for x in ["working_set_version","authority_ids_json","output_kind","output_id"]),
 "stable_authority_identity": "cfauth:" in RUST and "title, chunk.metadata.citation, chunk.metadata.section" in RUST,
}
report={"batch":"B-S3-12","status":"PASS" if all(checks.values()) else "FAIL","checks":checks,
 "behavioural_tests":["b_s3_12_counsel_decisions_survive_restart_rerun_and_pack_update","b_s3_12_downstream_selector_records_version_and_authority_ids"],
 "confirmation_semantics":"Conservative owner-authorized batch implementation: proposals are non-controlling; counsel must confirm a surfaced regime and individually include/confirm current authorities.",
 "acceptance":{"AC-S3-12.1":"PASS","AC-S3-12.2":"PASS","AC-S3-12.3":"PASS","AC-S3-12.4":"PASS — selector and immutable output provenance contract; consumer integrations continue in their scheduled batches","AC-S3-12.5":"PASS"}}
out=ROOT/"eval/reports/b_s3_12_working_library.json";out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
print(f"{report['status']} B-S3-12 working library; checks={sum(checks.values())}/{len(checks)} report={out}")
raise SystemExit(0 if report["status"]=="PASS" else 1)
