#!/usr/bin/env python3
"""B-S3-06 graph-v2 implementation and controlled-fixture evidence gate."""
from __future__ import annotations

import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REPORT = ROOT / "eval" / "reports" / "b_s3_06_case_graph.json"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


schema = json.loads((ROOT / "contracts" / "case_graph.schema.json").read_text(encoding="utf-8"))
node_types = set(schema["$defs"]["GraphNode"]["properties"]["node_type"]["enum"])
edge_types = set(schema["$defs"]["GraphEdge"]["properties"]["edge_type"]["enum"])
required_nodes = {
    "party", "claim", "cause_or_ground", "defence", "counterclaim", "issue", "sub_issue",
    "relief", "fact", "evidence", "exhibit", "document", "witness", "statement", "event",
    "procedure", "application", "hearing", "order", "statute", "rule", "authority",
    "provision", "argument", "interpretation",
}
required_edges = {
    "fact_to_document", "fact_to_evidence", "issue_to_fact", "issue_to_law",
    "argument_to_support", "contradicts", "alternative_to",
}
require(required_nodes <= node_types, f"schema misses node concepts: {sorted(required_nodes-node_types)}")
require(required_edges <= edge_types, f"schema misses relationships: {sorted(required_edges-edge_types)}")

prov_required = set(schema["$defs"]["Provenance"]["required"])
require({"sources", "confidence", "origin", "pipeline_version", "recorded_at", "counsel_confirmed", "correction_lineage"} <= prov_required, "provenance contract incomplete")

source = (ROOT / "src-tauri" / "src" / "docpipe" / "skeleton.rs").read_text(encoding="utf-8")
require("same date divergent" not in source.lower(), "legacy same-date contradiction scorer remains")
require("explicit_negation" in source and "left_source" in source and "right_source" in source, "semantic contradiction evidence missing")
require('"ambiguous"' in source and '"relative"' in source, "ambiguity-aware date path missing")
require("inferences_are_never_document_facts" in source, "inference boundary missing")

sidecar_source = (ROOT / "sidecars" / "docpipe" / "skeleton.py").read_text(encoding="utf-8")
require("caseforge.graph-extractor.deterministic.v2" in sidecar_source, "sidecar graph-v2 route missing")
require("same date divergent" not in sidecar_source.lower() and "same-date multi-event" not in sidecar_source.lower(), "retired sidecar contradiction scorer remains")

migration = (ROOT / "src-tauri" / "migrations" / "case_013_case_graph_v2.sql").read_text(encoding="utf-8")
require("v1_retained_reextract_for_v2" in migration and "graph_contract_version" in migration, "migration strategy not encoded")

ui = (ROOT / "src" / "components" / "DigestView.tsx").read_text(encoding="utf-8")
for token in ("graph-v2-audit", "graph-concept-coverage", "ambiguous-date", "contradiction-candidate", "left_source", "right_source"):
    require(token in ui, f"review UI missing {token}")

cmd = ["cargo", "test", "graph_golden_metrics", "--lib", "--", "--nocapture"]
proc = subprocess.run(cmd, cwd=ROOT / "src-tauri", text=True, encoding="utf-8", errors="replace", capture_output=True, timeout=180)
combined = proc.stdout + "\n" + proc.stderr
require(proc.returncode == 0, "controlled Rust graph golden failed\n" + combined[-4000:])
match = re.search(r"B_S3_06_METRICS=(\{.*\})", combined)
require(match is not None, "controlled metrics marker missing")
metrics = json.loads(match.group(1))

sys.path.insert(0, str(ROOT / "sidecars" / "docpipe"))
from skeleton import assert_every_extracted_has_source, extract_from_pages  # type: ignore
sidecar_metrics: dict[str, dict[str, object]] = {}
for fixture_file in ("india_controlled.json", "us_controlled.json"):
    fixture = json.loads((ROOT / "golden" / "G-CASE-GRAPH" / "fixtures" / fixture_file).read_text(encoding="utf-8"))
    graph = extract_from_pages(case_id=fixture["fixture_id"], jurisdiction=fixture["jurisdiction"], pages=fixture["pages"])
    observed: dict[str, int] = {}
    for node in graph["nodes"]:
        observed[node["node_type"]] = observed.get(node["node_type"], 0) + 1
    require(observed == fixture["expected_counts"], f"sidecar concept mismatch for {fixture_file}: {observed}")
    defects = assert_every_extracted_has_source(graph)
    require(not defects, f"sidecar provenance defects for {fixture_file}: {defects}")
    sidecar_metrics[fixture["fixture_id"]] = {"nodes": len(graph["nodes"]), "edges": len(graph["edges"]), "concepts": observed, "provenance_defects": len(defects)}

concept_metrics: dict[str, dict[str, float | int]] = {}
for fixture_name, result in metrics.items():
    fixture_file = "india_controlled.json" if fixture_name == "india" else "us_controlled.json"
    fixture = json.loads((ROOT / "golden" / "G-CASE-GRAPH" / "fixtures" / fixture_file).read_text(encoding="utf-8"))
    expected = fixture["expected_counts"]
    observed = result["observed"]
    for concept in sorted(set(expected) | set(observed)):
        tp = min(expected.get(concept, 0), observed.get(concept, 0))
        precision = tp / observed.get(concept, 1) if observed.get(concept, 0) else (1.0 if expected.get(concept, 0) == 0 else 0.0)
        recall = tp / expected.get(concept, 1) if expected.get(concept, 0) else (1.0 if observed.get(concept, 0) == 0 else 0.0)
        concept_metrics[f"{fixture_name}:{concept}"] = {"expected": expected.get(concept, 0), "observed": observed.get(concept, 0), "precision": precision, "recall": recall}
    require(result["provenance_defects"] == 0, f"{fixture_name} provenance defects")

threshold_contract = ROOT / "contracts" / "case_graph_evaluation.json"
thresholds = json.loads(threshold_contract.read_text(encoding="utf-8")) if threshold_contract.exists() else {}
approved = thresholds.get("status") == "FROZEN_OWNER_APPROVED" and bool(thresholds.get("per_concept_thresholds"))

report = {
    "batch": "B-S3-06",
    "generated_at": datetime.now(timezone.utc).isoformat(),
    "implementation_status": "PASS",
    "formal_acceptance_status": "PASS" if approved else "BLOCKED",
    "formal_blockers": [] if approved else [
        "Owner-approved per-concept precision/recall thresholds are absent.",
        "Controlled synthetic fixtures are not representative held-out India/US production goldens.",
    ],
    "schema_version": schema["schema_version"],
    "controlled_metrics": metrics,
    "sidecar_parity_metrics": sidecar_metrics,
    "controlled_per_concept_count_metrics": concept_metrics,
    "limitations": "Count metrics prove deterministic controlled coverage only; they must not be represented as real-document extraction quality.",
    "acceptance_gates": {
        "AC-S3-6.1": "PASS",
        "AC-S3-6.2": "PASS",
        "AC-S3-6.3": "BLOCKED" if not approved else "PASS",
        "AC-S3-6.4": "PASS",
        "AC-S3-6.5": "PASS",
        "AC-S3-6.6": "PASS",
    },
}
REPORT.parent.mkdir(parents=True, exist_ok=True)
REPORT.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(f"PASS B-S3-06 implementation; formal evidence {report['formal_acceptance_status']}; report={REPORT}")
