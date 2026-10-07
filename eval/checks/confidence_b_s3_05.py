#!/usr/bin/env python3
"""B-S3-05 calibrated-confidence and legal critical-field gates."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "sidecars" / "docpipe"))

from confidence import ARTIFACT_PATH, DATASET_PATH, calibrate, validate_critical_fields  # noqa: E402

REPORT = ROOT / "eval" / "reports" / "b_s3_05_confidence.json"


def words(text: str, raw: float | None = 0.99) -> list[dict]:
    out = []
    calibrated, state = calibrate(raw, "tesseract_eng_fast_scan")
    for index, match in enumerate(re.finditer(r"\S+", text)):
        out.append({"word_index":index,"text":match.group(),"char_start":match.start(),"char_end":match.end(),
                    "raw_engine_confidence":raw,"calibrated_confidence":calibrated,"confidence_state":state,"bbox":{"x":.1,"y":.1,"w":.1,"h":.02}})
    return out


def metrics(rows: list[dict]) -> tuple[float, float]:
    predicted = [calibrate(float(row["raw"]), "tesseract_eng_fast_scan")[0] or 0.0 for row in rows]
    actual = [int(row["correct"]) for row in rows]
    brier = sum((p-y)**2 for p,y in zip(predicted,actual))/len(rows)
    ece = 0.0
    for low, high in ((0,.25),(.25,.5),(.5,.7),(.7,.85),(.85,.95),(.95,1.01)):
        indexes = [i for i,p in enumerate(predicted) if low <= p < high]
        if indexes:
            ece += len(indexes)/len(rows)*abs(sum(predicted[i] for i in indexes)/len(indexes)-sum(actual[i] for i in indexes)/len(indexes))
    return ece, brier


def main() -> int:
    errors: list[str] = []
    artifact = json.loads(ARTIFACT_PATH.read_text(encoding="utf-8"))
    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    fingerprint = hashlib.sha256(DATASET_PATH.read_bytes()).hexdigest()
    if fingerprint != artifact["dataset_sha256"]:
        errors.append("calibration dataset fingerprint mismatch")
    train_ids = {row["id"] for row in dataset["training"]}
    held_ids = {row["id"] for row in dataset["held_out"]}
    if train_ids & held_ids:
        errors.append("calibration and held-out IDs overlap")
    ece, brier = metrics(dataset["held_out"])
    if abs(ece-artifact["held_out_metrics"]["ece"]) > 1e-9 or abs(brier-artifact["held_out_metrics"]["brier"]) > 1e-9:
        errors.append("held-out calibration metrics are not reproducible")

    good = "On March 12, 2024 claimant sought $10,000 under Section 10 and cited 410 U.S. 113."
    bad = "On 32/13/2024 claimant sought $1O,000 under Section 1O and cited 4IO U.S. 113."
    good_fields = validate_critical_fields(good, words(good))
    bad_fields = validate_critical_fields(bad, words(bad))
    for required in ("date", "money", "citation", "provision"):
        if not any(field["field_type"] == required for field in good_fields):
            errors.append(f"good sample missing {required}")
        hits = [field for field in bad_fields if field["field_type"] == required]
        if not hits or not all(field["force_review"] and field["suppress_from_auto"] for field in hits):
            errors.append(f"OCR-corrupted {required} was not blocked")
    missing = validate_critical_fields(good, words(good, None))
    if not missing or not all(field["confidence_state"] == "missing" and field["force_review"] for field in missing):
        errors.append("missing confidence was not explicit/forced to review")

    rust = (ROOT / "src-tauri" / "src" / "docpipe" / "recognize.rs").read_text(encoding="utf-8")
    production_body = rust.split("pub fn recognize_document_bytes",1)[1].split("#[cfg(test)]",1)[0]
    if "score_word" in production_body or "recognize_text" in production_body:
        errors.append("former surface-form scorer remains reachable from production entrypoint")
    host = (ROOT / "src-tauri" / "src" / "docpipe" / "mod.rs").read_text(encoding="utf-8")
    if "page_quality_warn || layer.words.iter().any(|word| word.force_review)" not in host:
        errors.append("trusted graph boundary does not block warned/force-review layers")
    ui = (ROOT / "src" / "components" / "ReviewView.tsx").read_text(encoding="utf-8")
    for token in ("raw OCR:", "calibrated:", "validation:", "counsel:", "source region"):
        if token not in ui:
            errors.append(f"review UI missing {token}")

    contract = json.loads((ROOT / "contracts" / "confidence_calibration.schema.json").read_text(encoding="utf-8"))
    gates = contract["properties"]["quality_gates"]["properties"]
    owner_blocked = gates["ece_max"]["type"] == ["number", "null"] and artifact["status"] == "PROPOSED_AWAITING_OWNER_APPROVAL"
    formal = "FAIL" if errors else ("BLOCKED" if owner_blocked else "PASS")
    report = {"batch":"B-S3-05","implementation_status":"FAIL" if errors else "PASS","formal_acceptance_status":formal,
              "calibration_id":artifact["calibration_id"],"dataset_sha256":fingerprint,"train_count":len(train_ids),"held_out_count":len(held_ids),
              "held_out_metrics":{"ece":ece,"brier":brier},"critical_field_examples":{"valid":good_fields,"ocr_corrupted":bad_fields},
              "blocking_reason":"Owner approval is absent for ECE/Brier limits and per-field error costs; existing 0.99 accuracy and 0.55 downstream block thresholds remain enforced." if owner_blocked else None,
              "errors":errors}
    REPORT.parent.mkdir(parents=True,exist_ok=True)
    REPORT.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
    print(("PASS" if not errors else "FAIL")+f" B-S3-05 implementation; formal evidence {formal}; ECE={ece:.6f} Brier={brier:.6f}")
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
