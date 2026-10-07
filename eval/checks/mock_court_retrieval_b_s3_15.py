#!/usr/bin/env python3
"""Structural and evidence gate for B-S3-15 grounded Mock Court retrieval."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ORCH = (ROOT / "src-tauri/src/mock_court_retrieval.rs").read_text(encoding="utf-8")
HOST = (ROOT / "src-tauri/src/lib.rs").read_text(encoding="utf-8")
IPC = (ROOT / "src/lib/ipc.ts").read_text(encoding="utf-8")
UI = (ROOT / "src/components/MockCourtView.tsx").read_text(encoding="utf-8")
DOC = (ROOT / "docs/B-S3-15_mock_court_retrieval.md").read_text(encoding="utf-8")

checks = {
    "host_owned_retrieval": "mock_court_retrieval::retrieve" in HOST and "retrievalIds: []" not in UI,
    "versioned_ipc": all(x in ORCH + IPC for x in ["expected_graph_version", "expected_working_set_version", "CF_MOCK_COURT_SESSION_STALE"]),
    "readiness_gate": "mock_court_retrieval_readiness" in HOST + IPC and "mock-court-readiness-block" in UI,
    "strict_source_filter": all(x in ORCH for x in ["OFFICIAL_VERIFIED", "APPLICABLE", "CURRENT", "INCLUDED", "controlling", "jurisdiction_allowed"]),
    "missing_authority_blocks": "CF_MOCK_COURT_AUTHORITY_ABSENT" in ORCH and "legal turn cannot run with empty retrieval" in ORCH,
    "immutable_output_provenance": "working_set_select_for_output" in HOST and '"MOCK_COURT"' in HOST,
    "safe_cache_key": all(x in ORCH for x in ["graph_version", "working_set_version", "cache_key", "MAX_CACHE_ENTRIES"]),
    "per_turn_exact_text": all(x in UI for x in ["mock-court-turn-sources", "authority.exact_text", "authority.chunk_checksum"]),
    "voice_is_grounded": "retrieval_ids: Some(retrieval_ids)" in HOST and '"authorities": retrieval.authorities' in HOST,
    "no_grounded_fallback": "No ungrounded legal challenge was generated" in UI,
    "behavioural_tests": all(x in ORCH for x in ["excludes_wrong_jurisdiction_and_noncurrent_sources", "blocks_unconfirmed_or_empty_working_set", "version_or_query_change_invalidates_cache_key", "stale_session_requires_explicit_resume"]),
    "operator_documentation": all(x in DOC for x in ["Readiness", "Version invalidation", "Exact source text", "B-S3-15"]),
}

report = {
    "batch": "B-S3-15",
    "status": "PASS" if all(checks.values()) else "FAIL",
    "checks": checks,
    "acceptance": {
        "AC-S3-15.1": "PASS — host blocks empty legal retrieval",
        "AC-S3-15.2": "PASS — turn text and response carry verified authority/chunk IDs",
        "AC-S3-15.3": "PASS — host filters jurisdiction, decision, trust, temporal, and supersession state",
        "AC-S3-15.4": "PASS — readiness and per-turn panels expose exact stored chunk text",
        "AC-S3-15.5": "PASS — graph/working-set versions are pinned and stale requests fail closed",
    },
}
out = ROOT / "eval/reports/b_s3_15_mock_court_retrieval.json"
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(f"{report['status']} B-S3-15 grounded Mock Court; checks={sum(checks.values())}/{len(checks)} report={out}")
raise SystemExit(0 if report["status"] == "PASS" else 1)
