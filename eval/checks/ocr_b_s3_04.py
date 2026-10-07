#!/usr/bin/env python3
"""B-S3-04 offline OCR implementation and real-scan evidence gate."""

from __future__ import annotations

import json
import re
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DOCPIPE = ROOT / "sidecars" / "docpipe"
sys.path.insert(0, str(DOCPIPE))

from pdf_parser import parse_pdf_bytes  # noqa: E402
from recognize import load_ocr_config, recognize_document, recognize_image_bytes, runtime_manifest  # noqa: E402

FIXTURE = ROOT / "golden" / "G-DOCS-REAL" / "fixtures" / "uscourts_pro_se_1"
ASSETS = FIXTURE / "assets"
REPORT = ROOT / "eval" / "reports" / "b_s3_04_ocr.json"


def normalize(text: str) -> str:
    return " ".join(re.findall(r"[a-z0-9]+", text.lower()))


def distance(left: list[str] | str, right: list[str] | str) -> int:
    previous = list(range(len(right) + 1))
    for index, left_value in enumerate(left, 1):
        current = [index]
        for other_index, right_value in enumerate(right, 1):
            current.append(min(current[-1] + 1, previous[other_index] + 1, previous[other_index - 1] + (left_value != right_value)))
        previous = current
    return previous[-1]


def rates(reference: str, prediction: str) -> tuple[float, float]:
    ref, pred = normalize(reference), normalize(prediction)
    return distance(ref, pred) / max(len(ref), 1), distance(ref.split(), pred.split()) / max(len(ref.split()), 1)


def main() -> int:
    errors: list[str] = []
    config = load_ocr_config()
    runtime = runtime_manifest()
    source_pdf = (ASSETS / "complaint_for_a_civil_case.pdf").read_bytes()
    truth = parse_pdf_bytes(source_pdf)["pages"][0]["native_text"]
    samples = {}
    for filename, quality in (("page_1.png", "clean"), ("page_1.jpg", "clean"), ("page_1_rotated_degraded.jpg", "degraded")):
        started = time.perf_counter()
        result = recognize_image_bytes((ASSETS / filename).read_bytes())
        cer, wer = rates(truth, result["full_text"])
        diagnostics = json.loads(result["notes"])
        boxes_valid = bool(result["words"]) and all(
            word["bbox"] and 0 <= word["bbox"]["x"] <= 1 and 0 <= word["bbox"]["y"] <= 1
            and 0 < word["bbox"]["w"] <= 1 and 0 < word["bbox"]["h"] <= 1
            and word["char_end"] > word["char_start"]
            for word in result["words"]
        )
        samples[filename] = {
            "quality": quality, "cer": round(cer, 6), "wer": round(wer, 6),
            "words": len(result["words"]), "mean_confidence": round(result["mean_confidence"], 6),
            "requires_review": diagnostics["requires_review"], "automatic_extraction_allowed": diagnostics["automatic_extraction_allowed"],
            "duration_ms": diagnostics["duration_ms"], "wall_ms": round((time.perf_counter() - started) * 1000),
            "peak_engine_rss_bytes": diagnostics["peak_engine_rss_bytes"], "boxes_valid": boxes_valid,
        }
        if not boxes_valid:
            errors.append(f"{filename}: invalid word geometry or char spans")
        if quality == "clean" and (cer > config["quality_gate"]["clean_scan_cer_max"] or wer > config["quality_gate"]["clean_scan_wer_max"]):
            errors.append(f"{filename}: clean scan threshold missed")
        if quality == "degraded" and (cer > config["quality_gate"]["degraded_scan_cer_max"] or wer > config["quality_gate"]["degraded_scan_wer_max"]):
            if not diagnostics["requires_review"] or diagnostics["automatic_extraction_allowed"]:
                errors.append(f"{filename}: below-threshold page was not blocked for review")

    scanned_layers = recognize_document(
        content_base64=__import__("base64").b64encode((ASSETS / "complaint_scanned_derivative.pdf").read_bytes()).decode("ascii"),
        media_type="application/pdf", filename="complaint_scanned_derivative.pdf",
    )
    if len(scanned_layers) != 5 or any(not layer["words"] for layer in scanned_layers):
        errors.append("scanned PDF did not produce five non-empty production text layers")

    source = (DOCPIPE / "recognize.py").read_text(encoding="utf-8")
    forbidden = [token for token in ("requests.", "urllib.request", "http://", "https://") if token in source]
    if forbidden:
        errors.append(f"network-capable OCR source tokens present: {forbidden}")

    fixture = json.loads((FIXTURE / "manifest.json").read_text(encoding="utf-8"))
    expected = json.loads((FIXTURE / "expected_answer.json").read_text(encoding="utf-8"))
    human_locked = fixture["status"] == "GOLDEN" and expected["status"] == "GOLDEN" and bool(expected.get("critical_fields"))
    formal_status = "PASS" if human_locked and not errors else ("FAIL" if errors else "BLOCKED")
    report = {
        "batch": "B-S3-04", "implementation_status": "PASS" if not errors else "FAIL",
        "formal_acceptance_status": formal_status, "recognizer_id": runtime["recognizer_id"],
        "engine_version": runtime["engine_version"], "model_sha256": runtime["model_sha256"],
        "offline": True, "samples": samples,
        "scanned_pdf": {"pages": len(scanned_layers), "non_empty_pages": sum(bool(layer["words"]) for layer in scanned_layers)},
        "hardware_profiles": {
            "lite": {"threads": 1, "observed_peak_rss_bytes": max(v["peak_engine_rss_bytes"] for v in samples.values()), "timing_basis": "single-page sequential"},
            "full": {"threads": 2, "observed_peak_rss_bytes": max(v["peak_engine_rss_bytes"] for v in samples.values()), "timing_basis": "single-page sequential; model concurrency remains disabled"},
        },
        "critical_field_gate": {"threshold": config["quality_gate"]["critical_field_accuracy_min"], "label_count": len(expected.get("critical_fields") or []), "status": "BLOCKED" if not human_locked else "READY"},
        "blocking_reason": None if human_locked else "Rights-cleared fixture is READY_FOR_REVIEW with draft expected answers and no human-locked critical-field labels.",
        "errors": errors,
    }
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(("PASS" if not errors else "FAIL") + f" B-S3-04 implementation; formal evidence {formal_status}")
    print(json.dumps({"samples": samples, "scanned_pdf_pages": len(scanned_layers)}, indent=2))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
