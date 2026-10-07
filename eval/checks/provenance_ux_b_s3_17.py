#!/usr/bin/env python3
"""B-S3-17 deterministic acceptance gate for universal provenance UX."""
from __future__ import annotations
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
read = lambda p: (ROOT / p).read_text(encoding="utf-8")

schema = json.loads(read("contracts/provenance_record.schema.json"))
lib = read("src/lib/provenance.ts")
ui = read("src/components/ProvenanceInspector.tsx")
css = read("src/styles.css")
export = read("src/lib/exportDraft.ts")
docs = read("docs/B-S3-17_provenance_ux.md")
surface_files = {
    "Digest": "src/components/DigestView.tsx",
    "Drafting": "src/components/DraftingView.tsx",
    "Integrity": "src/components/ArgumentTreeView.tsx",
    "Review": "src/components/ReviewView.tsx",
    "Retrieval": "src/components/RetrievalView.tsx",
    "Mock Court": "src/components/MockCourtView.tsx",
}

checks = {
    "contract_vocabulary": schema.get("$id") == "caseforge.provenance-record.v1" and all(x in json.dumps(schema) for x in ["FACTUAL_BASIS", "LEGAL_BASIS", "INFERENCE", "COUNSEL_INPUT", "UNSUPPORTED_OR_GAP"]),
    "shared_inspector": "Where did CaseForge get that?" in ui and "provenanceLocator(record)" in ui,
    "matter_deep_locator": "matter://" in lib and "page=${p.pageIndex + 1}" in lib and "chars=${p.charStart}-${p.charEnd}" in lib,
    "legal_deep_locator": "legal://" in lib and all(x in lib for x in ["p.packId", "p.sourceId", "p.chunkId"]),
    "counsel_and_inference_distinct": all(x in lib + export for x in ["COUNSEL_INPUT", "MODEL_INFERENCE", "user_edited"]),
    "unsafe_states_not_green": "UNSUPPORTED_GAP" in lib and "REPOSITORY_GAP" in lib and "provenance-unsupported" in css,
    "keyboard_and_screen_reader": all(x in ui for x in ['aria-haspopup="dialog"', 'aria-expanded={open}', 'role="dialog"', 'aria-modal="true"', 'event.key === "Escape"', 'event.key === "Tab"', 'trigger.current?.focus()']),
    "not_color_alone": "ICON:" in ui and "record.supportState" in ui and "record.origin.replaceAll" in ui,
    "all_named_surfaces": all("ProvenanceInspector" in read(path) for path in surface_files.values()),
    "export_retains_provenance": all(x in export for x in ["source_records", "provenanceExportLine", "renderPlainText", "renderPdf", "renderDocx"]),
    "copy_retains_provenance": "Copy with provenance" in ui and "Preparation aid only" in ui,
    "preparation_disclaimer": "verify before filing" in ui.lower() and "EXPORT_FOOTER" in export,
    "operator_documentation": "AC-S3-17" not in docs and "IMPLEMENTED" in docs and "npm run test:s3:17" in docs,
}

report = {"batch": "B-S3-17", "checks": checks, "surface_files": surface_files, "passed": all(checks.values())}
out = ROOT / "eval/reports/b_s3_17_provenance_ux.json"
out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
for name, passed in checks.items():
    print(f"{'PASS' if passed else 'FAIL'} {name}")
print(f"B-S3-17: {sum(checks.values())}/{len(checks)} checks passed")
raise SystemExit(0 if report["passed"] else 1)
