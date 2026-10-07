#!/usr/bin/env python3
"""B-S3-19 controlled workflow, safety, and usability-readiness gate."""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
read = lambda path: (ROOT / path).read_text(encoding="utf-8")

contract = json.loads(read("contracts/lawyer_workflow_usability.json"))
workflow = read("src/lib/lawyerWorkflow.ts")
journey = read("src/components/WorkflowJourney.tsx")
app = read("src/App.tsx")
court = read("src/components/MockCourtView.tsx")
styles = read("src/styles.css")
docs = read("docs/B-S3-19_lawyer_workflow.md")

expected = contract["journey"]
states = contract["state_vocabulary"]
primary_labels = [
    "Preparation Journey",
    "Matters",
    "Case Material & Review",
    "Case Digest",
    "Legal Authorities",
    "Argument Analysis",
    "Mock Court",
]
advanced_labels = ["Triage", "Text layer", "Skeleton", "Batch", "model status"]

checks = {
    "frozen_nine_step_journey": all(label in workflow for label in expected) and len(expected) == 9,
    "complete_state_vocabulary": all(f'"{state}"' in workflow for state in states),
    "persisted_inputs": all(token in workflow for token in ["documents", "pageSources", "graph", "readiness", "argumentTree", "tasks", "batchJob"]),
    "stale_from_saved_timestamps": "ingested_at_unix_ms" in workflow and "generated_at" in workflow,
    "obvious_next_action": "nextSafeStage" in journey and "workflow-next" in journey,
    "state_not_colour_only": "STATE_LABEL" in journey and "aria-label" in journey and "data-workflow-state" in journey,
    "primary_information_architecture": all(label in app for label in primary_labels) and "PRIMARY_NAV" in app,
    "diagnostics_under_details": all(token in app for token in ["advanced-navigation", "advanced-processing-details", "advanced-system-details"]),
    "implementation_terms_not_primary_tabs": all(f'[\"{token.lower().replace(" layer", "")}\", \"{token}\"]' not in app for token in advanced_labels[:4]),
    "host_backed_parallel_refresh": "Promise.allSettled" in app and all(token in app for token in ["mockCourtRetrievalReadiness", "preparationTasksList", "argumentTreeGet"]),
    "unsafe_mock_court_blocked": 'stageId==="mock_court"&&!workflowReadiness?.ready' in app and "blocking_reasons" in app,
    "preparation_task_handoff": 'setCourtInitialPanel("debrief")' in app and "initialInsightTab" in court,
    "destructive_scope_and_recovery": "Remove knowledge pack" in app and "Matter files are not changed" in app and "window.confirm" in app,
    "keyboard_focus_visible": "focus-visible" in styles and "outline: 3px" in styles,
    "long_text_readable": "overflow-wrap: anywhere" in styles and ".provenance-excerpt" in styles and "line-height: 1.55" in styles,
    "responsive_journey": "@media (max-width: 720px)" in styles and ".workflow-state" in styles,
    "usability_threshold_frozen": contract["thresholds"]["screen_purpose_and_action_within_seconds"] == 15 and contract["thresholds"]["controlled_journey_completion_rate_min"] == 0.8,
    "human_gate_not_fabricated": "PENDING HUMAN SIGN-OFF" in docs and "five unfamiliar participants" in docs,
}

report = {
    "batch": "B-S3-19",
    "checks": checks,
    "automated_gate_passed": all(checks.values()),
    "human_usability_gate": "pending",
}
(ROOT / "eval/reports/b_s3_19_lawyer_workflow.json").write_text(
    json.dumps(report, indent=2) + "\n", encoding="utf-8"
)
for key, value in checks.items():
    print(f"{'PASS' if value else 'FAIL'} {key}")
print(f"B-S3-19 automated gate: {sum(checks.values())}/{len(checks)} checks passed")
print("Human usability sign-off remains pending; no participant results were fabricated.")
raise SystemExit(0 if report["automated_gate_passed"] else 1)
