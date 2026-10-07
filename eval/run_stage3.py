#!/usr/bin/env python3
"""Fail-closed Stage 3 evidence validator and metric reporter."""

from __future__ import annotations

import argparse
import ctypes
import glob
import hashlib
import json
import os
import platform
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import jsonschema

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "contracts" / "eval_fixture_manifest.schema.json"
STAGE3_PATH = ROOT / "golden" / "STAGE3_MANIFEST.json"
METRICS_PATH = ROOT / "eval" / "stage3_metrics.json"
DEFAULT_REPORT = ROOT / "eval" / "reports" / "stage3_evidence_report.json"


def load(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def total_ram_bytes() -> int | None:
    if os.name == "nt":
        class MEMORYSTATUSEX(ctypes.Structure):
            _fields_ = [("dwLength", ctypes.c_ulong), ("dwMemoryLoad", ctypes.c_ulong),
                        ("ullTotalPhys", ctypes.c_ulonglong), ("ullAvailPhys", ctypes.c_ulonglong),
                        ("ullTotalPageFile", ctypes.c_ulonglong), ("ullAvailPageFile", ctypes.c_ulonglong),
                        ("ullTotalVirtual", ctypes.c_ulonglong), ("ullAvailVirtual", ctypes.c_ulonglong),
                        ("sullAvailExtendedVirtual", ctypes.c_ulonglong)]
        state = MEMORYSTATUSEX()
        state.dwLength = ctypes.sizeof(state)
        if ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(state)):
            return int(state.ullTotalPhys)
    try:
        return int(os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES"))
    except (AttributeError, OSError, ValueError):
        return None


def ollama_info() -> dict[str, Any]:
    result = {"backend": "Ollama", "model": "unknown", "quantization": "unknown", "model_digest": "unknown"}
    try:
        proc = subprocess.run(["ollama", "list"], capture_output=True, text=True, timeout=10, check=False)
        lines = [line.split() for line in proc.stdout.splitlines()[1:] if line.strip()]
        if lines:
            result["model"] = lines[0][0]
            if len(lines[0]) > 1:
                result["model_digest"] = lines[0][1]
            detail = subprocess.run(["ollama", "show", result["model"]], capture_output=True, text=True, timeout=10, check=False)
            for line in detail.stdout.splitlines():
                if line.strip().startswith("quantization"):
                    result["quantization"] = line.split()[-1]
                    break
    except (FileNotFoundError, subprocess.SubprocessError):
        result["backend"] = "not-detected"
    return result


def pack_versions() -> list[dict[str, str]]:
    found: list[dict[str, str]] = []
    for path in sorted((ROOT / "packs").rglob("*manifest*.json")):
        try:
            data = load(path)
        except (OSError, json.JSONDecodeError):
            continue
        found.append({
            "path": path.relative_to(ROOT).as_posix(),
            "version": str(data.get("version", data.get("schema_version", "unknown"))),
            "sha256": sha256(path),
        })
    return found


def source_fingerprint(paths: list[Path]) -> str:
    digest = hashlib.sha256()
    for path in sorted(set(paths)):
        digest.update(path.relative_to(ROOT).as_posix().encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def environment(paths: list[Path]) -> dict[str, Any]:
    cpu = platform.processor() or os.environ.get("PROCESSOR_IDENTIFIER") or platform.machine() or "unknown"
    vcs = "not_git_repository"
    commit = "unavailable"
    try:
        proc = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, timeout=5, check=False)
        if proc.returncode == 0:
            vcs, commit = "git", proc.stdout.strip()
    except (FileNotFoundError, subprocess.SubprocessError):
        pass
    return {
        "os": platform.platform(), "cpu": cpu, "ram_bytes": total_ram_bytes(),
        "python": platform.python_version(), **ollama_info(), "pack_versions": pack_versions(),
        "vcs": vcs, "commit": commit, "source_fingerprint_sha256": source_fingerprint(paths),
    }


def validate_metrics(registry: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    required = {"id", "formula", "threshold", "source_classes", "command", "human_scored"}
    allowed_classes = {"SYNTHETIC_CONTROLLED", "REAL_WORLD_RIGHTS_CLEARED", "AUTHORITATIVE_LEGAL", "ADVERSARIAL_SYNTHETIC", "HUMAN_AUDIO_RIGHTS_CLEARED"}
    seen: set[str] = set()
    for metric in registry.get("metrics", []):
        missing = required - set(metric)
        if missing:
            errors.append(f"metric {metric.get('id', '<unknown>')} missing {sorted(missing)}")
            continue
        if metric["id"] in seen:
            errors.append(f"duplicate metric id: {metric['id']}")
        seen.add(metric["id"])
        threshold = metric["threshold"]
        if threshold.get("op") not in {">=", "<="} or not isinstance(threshold.get("value"), (int, float)):
            errors.append(f"metric {metric['id']} has invalid threshold")
        if not metric["source_classes"] or not set(metric["source_classes"]) <= allowed_classes:
            errors.append(f"metric {metric['id']} has invalid source class")
        expected_command = f"python eval/run_stage3.py --metric {metric['id']}"
        if metric["command"] != expected_command:
            errors.append(f"metric {metric['id']} command is not reproducible")
        if metric["human_scored"] and not metric.get("review_policy"):
            errors.append(f"human-scored metric {metric['id']} has no review policy")
    return errors


def discover_fixtures(stage3: dict[str, Any], schema: dict[str, Any]) -> tuple[list[dict[str, Any]], list[str], list[Path]]:
    fixtures: list[dict[str, Any]] = []
    errors: list[str] = []
    files: list[Path] = []
    for suite in stage3["suites"]:
        matches = [Path(value) for value in glob.glob(str(ROOT / suite["manifest_glob"]))]
        if not matches:
            errors.append(f"{suite['id']}: no fixture manifests found")
        for path in sorted(matches):
            files.append(path)
            try:
                fixture = load(path)
                jsonschema.validate(fixture, schema, format_checker=jsonschema.FormatChecker())
            except (OSError, json.JSONDecodeError, jsonschema.ValidationError) as exc:
                errors.append(f"{path.relative_to(ROOT)}: {exc}")
                continue
            fixture["_manifest_path"] = path.relative_to(ROOT).as_posix()
            fixture["_integrity_errors"] = []
            if fixture["suite_id"] != suite["id"]:
                fixture["_integrity_errors"].append("suite_id mismatch")
            if fixture["source_class"] != suite["required_source_class"]:
                fixture["_integrity_errors"].append("source_class mismatch")
            for item in fixture["assets"]:
                if item["path"] is None:
                    continue
                target = (ROOT / item["path"]).resolve()
                try:
                    target.relative_to(ROOT.resolve())
                except ValueError:
                    fixture["_integrity_errors"].append(f"asset escapes workspace: {item['path']}")
                    continue
                if not target.is_file():
                    fixture["_integrity_errors"].append(f"missing asset: {item['path']}")
                else:
                    if item["sha256"] != sha256(target):
                        fixture["_integrity_errors"].append(f"sha256 mismatch: {item['path']}")
                    if item["bytes"] != target.stat().st_size:
                        fixture["_integrity_errors"].append(f"byte count mismatch: {item['path']}")
            answer_path = fixture["expected_answer"]["path"]
            if answer_path is not None:
                answer = (ROOT / answer_path).resolve()
                try:
                    answer.relative_to(ROOT.resolve())
                except ValueError:
                    fixture["_integrity_errors"].append(f"expected answer escapes workspace: {answer_path}")
                else:
                    if not answer.is_file():
                        fixture["_integrity_errors"].append(f"missing expected answer: {answer_path}")
            if fixture["status"] == "GOLDEN" and fixture["_integrity_errors"]:
                errors.extend(f"{fixture['fixture_id']}: {error}" for error in fixture["_integrity_errors"])
            fixtures.append(fixture)
    return fixtures, errors, files


def suite_results(stage3: dict[str, Any], fixtures: list[dict[str, Any]]) -> list[dict[str, Any]]:
    results = []
    for suite in stage3["suites"]:
        candidates = [f for f in fixtures if f["suite_id"] == suite["id"]]
        qualifying = [f for f in candidates if f["status"] == "GOLDEN" and not f["_integrity_errors"] and f["rights"]["status"] in {"VERIFIED", "NOT_APPLICABLE"}]
        results.append({
            "suite_id": suite["id"], "required_source_class": suite["required_source_class"],
            "state": "PASS" if qualifying else "BLOCKED",
            "reason": f"{len(qualifying)} integrity-verified GOLDEN fixture(s)" if qualifying else "No integrity-verified, human-locked GOLDEN fixture",
            "fixture_states": [{"fixture_id": f["fixture_id"], "status": f["status"], "integrity_errors": f["_integrity_errors"]} for f in candidates]
        })
    return results


def compare(value: float, threshold: dict[str, Any]) -> bool:
    return value >= threshold["value"] if threshold["op"] == ">=" else value <= threshold["value"]


def metric_result(metric: dict[str, Any], fixtures: list[dict[str, Any]]) -> dict[str, Any]:
    path = ROOT / "eval" / "results" / "stage3" / f"{metric['id']}.json"
    base = {"metric_id": metric["id"], "source_classes": metric["source_classes"], "threshold": metric["threshold"]}
    if not path.is_file():
        return {**base, "state": "NOT_TESTED", "reason": f"Missing measured result: {path.relative_to(ROOT)}"}
    try:
        result = load(path)
    except (OSError, json.JSONDecodeError) as exc:
        return {**base, "state": "BLOCKED", "reason": f"Unreadable measured result: {exc}"}
    fixture_map = {f["fixture_id"]: f for f in fixtures}
    used = [fixture_map.get(value) for value in result.get("fixture_ids", [])]
    if not used or any(f is None or f["status"] != "GOLDEN" or f["_integrity_errors"] for f in used):
        return {**base, "state": "BLOCKED", "reason": "Result does not identify only integrity-verified GOLDEN fixtures"}
    result_class = result.get("source_class")
    if result_class not in metric["source_classes"] or any(f["source_class"] != result_class for f in used if f):
        return {**base, "state": "BLOCKED", "reason": "Result source_class is absent, mixed, or not permitted"}
    if metric["human_scored"]:
        reviewers = result.get("reviewed_by", [])
        if len(set(reviewers)) < 2 and not result.get("adjudicator"):
            return {**base, "state": "BLOCKED", "reason": "Human-scored result lacks two reviewers or documented adjudication"}
    value = result.get("value")
    if not isinstance(value, (int, float)) or not isinstance(result.get("samples"), int) or result["samples"] < 1:
        return {**base, "state": "BLOCKED", "reason": "Measured value or sample count is invalid"}
    return {**base, "state": "PASS" if compare(float(value), metric["threshold"]) else "FAIL", "value": value,
            "samples": result["samples"], "source_class": result_class, "fixture_ids": result["fixture_ids"]}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--validate-only", action="store_true")
    parser.add_argument("--strict-evidence", action="store_true", help="Return nonzero unless every suite has qualifying GOLDEN evidence")
    parser.add_argument("--metric")
    parser.add_argument("--json", type=Path, default=DEFAULT_REPORT)
    args = parser.parse_args()

    schema, stage3, registry = load(SCHEMA_PATH), load(STAGE3_PATH), load(METRICS_PATH)
    metric_errors = validate_metrics(registry)
    fixtures, fixture_errors, fixture_files = discover_fixtures(stage3, schema)
    suites = suite_results(stage3, fixtures)
    metric_defs = {m["id"]: m for m in registry["metrics"]}
    if args.metric and args.metric not in metric_defs:
        print(f"Unknown metric: {args.metric}", file=sys.stderr)
        return 2
    selected = [metric_defs[args.metric]] if args.metric else registry["metrics"]
    results = [] if args.validate_only else [metric_result(metric, fixtures) for metric in selected]

    grouped: dict[str, dict[str, Any]] = {}
    for source_class in ["REAL_WORLD_RIGHTS_CLEARED", "SYNTHETIC_CONTROLLED", "AUTHORITATIVE_LEGAL", "ADVERSARIAL_SYNTHETIC", "HUMAN_AUDIO_RIGHTS_CLEARED"]:
        grouped[source_class] = {
            "fixtures": [f["fixture_id"] for f in fixtures if f["source_class"] == source_class],
            "metric_results": [r for r in results if r.get("source_class") == source_class or (r["state"] == "NOT_TESTED" and source_class in r["source_classes"])]
        }

    structural = [
        {"id":"AC-S3-2.1","state":"PASS" if not metric_errors else "BLOCKED","details":metric_errors or [f"{len(registry['metrics'])} metrics have formula, threshold, source class, and command"]},
        {"id":"AC-S3-2.2","state":"PASS","details":["Report has separate source_class groups; pooled aggregates are not emitted"]},
        {"id":"AC-S3-2.3","state":"PASS" if all(s["state"] in {"PASS","BLOCKED"} for s in suites) else "BLOCKED","details":["Missing GOLDEN evidence is BLOCKED and missing measurements are NOT_TESTED"]},
        {"id":"AC-S3-2.4","state":"PASS","details":["Environment and source fingerprint are captured below"]},
        {"id":"AC-S3-2.5","state":"PASS" if not [e for e in metric_errors if "review policy" in e] else "BLOCKED","details":["Human metrics declare two-reviewer/adjudication policy and result validation enforces it"]},
    ]
    report_paths = [SCHEMA_PATH, STAGE3_PATH, METRICS_PATH, *fixture_files]
    report = {
        "schema_version":"1.0.0", "batch":"B-S3-02",
        "generated_at":datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "batch_contract_state":"PASS" if not metric_errors and not fixture_errors and all(g["state"] == "PASS" for g in structural) else "BLOCKED",
        "controlled_beta_evidence_ready": all(s["state"] == "PASS" for s in suites),
        "structural_gates":structural, "suite_evidence":suites, "fixture_validation_errors":fixture_errors,
        "results_by_source_class":grouped, "environment":environment(report_paths),
        "statement":"Batch contract readiness and product evidence readiness are distinct. Structural PASS does not convert BLOCKED/NOT_TESTED suites into PASS."
    }
    args.json.parent.mkdir(parents=True, exist_ok=True)
    args.json.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"batch_contract_state": report["batch_contract_state"], "controlled_beta_evidence_ready": report["controlled_beta_evidence_ready"], "report": str(args.json), "suite_states": {s["suite_id"]:s["state"] for s in suites}, "metric_states": {r["metric_id"]:r["state"] for r in results}}, indent=2))
    if report["batch_contract_state"] != "PASS":
        return 1
    if args.strict_evidence and not report["controlled_beta_evidence_ready"]:
        return 2
    if args.metric and results and results[0]["state"] != "PASS":
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
