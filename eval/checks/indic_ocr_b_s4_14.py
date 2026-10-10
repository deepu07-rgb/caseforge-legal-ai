#!/usr/bin/env python3
"""B-S4-14 Hindi (Devanagari) ingestion gate.

AC-S4-09.1  Devanagari OCR character error rate on G-INDIC, measured through the product's own recognizer (script detection, routing,
            Hindi+English model, the preprocessing the product uses). The threshold is PROPOSED, not frozen (S4.indic.ocr_cer_max has no
            value; the owner freezes it after the bake-off, Q-S4-04), so the criterion is reported BLOCKED even when the measured error
            is under the proposal. The implementation checks below must pass for it to be reported at all.

Also checked, because the batch promises them:
  * script detection - the page script and each line's script, against hand-written labels;
  * critical fields - a date, amount, section, case number or name that OCR gets wrong is never accepted automatically: it is held for review
    (the safety property), and how many fields are read exactly is reported;
  * calibration - the Hindi confidence map is bound to its dataset, reproducible, trained and tested on different pages, and shares no line with
    the G-INDIC pages; held-out ECE/Brier reported (the owner's limits are not set, so it stays BLOCKED as the English one does);
  * the English path is unchanged - the three English samples give the same error rates as the committed B-S3-04 report;
  * integrity - the Hindi model is the one the config names (hash) and a model that fails the check is not used.

Golden: golden/G-INDIC/fixtures/*, pinned before the engine (cd65be6).

Usage: python eval/checks/indic_ocr_b_s4_14.py
"""
from __future__ import annotations

import difflib
import hashlib
import importlib.util
import json
import re
import sys
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "sidecars" / "docpipe"))
sys.path.insert(0, str(ROOT / "eval" / "tools"))

import indic_fields  # noqa: E402
import recognize as R  # noqa: E402
import script_detect as S  # noqa: E402
from confidence import INDIC_ARTIFACT_PATH, INDIC_CALIBRATION_ID, INDIC_DATASET_PATH, INDIC_ENGINE_CLASS, calibrate, load_indic_calibration, validate_critical_fields  # noqa: E402

FIXTURES = ROOT / "golden" / "G-INDIC" / "fixtures"
BAKEOFF = ROOT / "eval" / "bakeoff" / "indic_ocr_b_s4_14.json"
REPORT = ROOT / "eval" / "reports" / "b_s4_14_indic_ocr.json"
ENGLISH_REPORT = ROOT / "eval" / "reports" / "b_s3_04_ocr.json"
THRESHOLDS = ROOT / "contracts" / "stage4_thresholds.json"
HINDI_MODEL_MAX_BYTES = 20 * 1024 * 1024


def norm(s: str) -> str:
    return " ".join(unicodedata.normalize("NFC", s).split())


def same_field(a: str, b: str) -> bool:
    """Equal as a field value: punctuation inside a label (a comma read for a full stop) does not change the date, amount, section or number."""
    strip = lambda t: re.sub(r"[.,:;]", "", norm(t))  # noqa: E731
    return strip(a) == strip(b)


def lev(a, b) -> int:
    prev = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        cur = [i]
        for j, y in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (x != y)))
        prev = cur
    return prev[-1]


def cer_wer(ref: str, hyp: str) -> tuple[float, float]:
    ref, hyp = norm(ref), norm(hyp)
    return lev(ref, hyp) / max(1, len(ref)), lev(ref.split(), hyp.split()) / max(1, len(ref.split()))


def pair_fields(truth: list[dict], found: list[dict]) -> list[tuple[dict, dict | None]]:
    """One-to-one pairing of each truth field with the recognized field of the same kind that looks most like it (similarity at least 0.5)."""
    used: set[int] = set()
    pairs: list[tuple[dict, dict | None]] = []
    for t in truth:
        best, best_ratio = None, 0.0
        for i, f in enumerate(found):
            if i in used or f["field_type"] != t["kind"]:
                continue
            ratio = difflib.SequenceMatcher(None, norm(t["text"]), norm(f["text"])).ratio()
            if ratio > best_ratio:
                best, best_ratio = i, ratio
        if best is not None and best_ratio >= 0.5:
            used.add(best)
            pairs.append((t, found[best]))
        else:
            pairs.append((t, None))
    return pairs


def words_for(text: str, raw: float | None = 0.99) -> list[dict]:
    calibrated, state = calibrate(raw, INDIC_ENGINE_CLASS)
    return [{"word_index": i, "text": m.group(), "char_start": m.start(), "char_end": m.end(), "raw_engine_confidence": raw, "calibrated_confidence": calibrated,
             "confidence_state": state, "bbox": {"x": .1, "y": .1, "w": .1, "h": .02}} for i, m in enumerate(re.finditer(r"\S+", text))]


def ece_brier(predicted: list[float], actual: list[int]) -> tuple[float, float]:
    brier = sum((p - y) ** 2 for p, y in zip(predicted, actual)) / len(actual)
    ece = 0.0
    for low, high in ((0, .25), (.25, .5), (.5, .7), (.7, .85), (.85, .95), (.95, 1.01)):
        idx = [i for i, p in enumerate(predicted) if low <= p < high]
        if idx:
            ece += len(idx) / len(actual) * abs(sum(predicted[i] for i in idx) / len(idx) - sum(actual[i] for i in idx) / len(idx))
    return ece, brier


def load_english_module():
    spec = importlib.util.spec_from_file_location("ocr_b_s3_04", ROOT / "eval" / "checks" / "ocr_b_s3_04.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules["ocr_b_s3_04"] = mod
    spec.loader.exec_module(mod)
    return mod


def main() -> int:
    errors: list[str] = []
    cfg = R.load_indic_config()
    thresholds_file = json.loads(THRESHOLDS.read_text(encoding="utf-8"))["thresholds"]
    frozen_cer = next(t for t in thresholds_file if t["id"] == "S4.indic.ocr_cer_max")
    field_min = next(t for t in thresholds_file if t["id"] == "S4.indic.critical_field_accuracy_min")["proposed"]
    proposal = cfg["quality_gate"]["proposed_by_agent"]

    # ---- integrity of the model and the config ----
    model_path = R.find_tesseract().parent / "tessdata" / cfg["model"]["filename"]
    model_sha = hashlib.sha256(model_path.read_bytes()).hexdigest()
    if model_sha != cfg["model"]["sha256"]:
        errors.append("the Hindi model does not match the hash in its config")
    if model_path.stat().st_size > HINDI_MODEL_MAX_BYTES:
        errors.append("the Hindi model is larger than the size this batch allows")
    if cfg["model"]["license"] != "Apache-2.0" or cfg["engine"]["source_license"] != "Apache-2.0":
        errors.append("model or engine licence is not recorded as Apache-2.0")
    wrong = json.loads(json.dumps(cfg))
    wrong["model"]["sha256"] = "0" * 64
    if R.indic_model_ready(wrong):
        errors.append("a model that fails its hash check was accepted")
    if not R.indic_model_ready(cfg):
        errors.append("the vendored Hindi model is not ready")
    bake = json.loads(BAKEOFF.read_text(encoding="utf-8"))
    if bake["selected"] != "hin+eng-best" or bake["models"]["tessdata_best hin"] != cfg["model"]["sha256"]:
        errors.append("the bake-off report does not select the model in the config")
    best = bake["summary"]["hin+eng-best"]
    for other in ("hin-fast", "hin-best", "hin+eng-fast"):
        if bake["summary"][other]["degraded_cer"] < best["degraded_cer"]:
            errors.append(f"bake-off: {other} beats the chosen model on degraded pages")

    # ---- the pages ----
    pages = sorted(d for d in FIXTURES.iterdir() if d.is_dir())
    rows: dict[str, dict] = {}
    pair_totals = {"truth": 0, "exact": 0, "wrong_held": 0, "wrong_unheld": 0, "undetected": 0}
    wrong_unheld: list[dict] = []
    line_total = line_ok = 0
    for d in pages:
        truth = json.loads((d / "truth.json").read_text(encoding="utf-8"))
        meta = json.loads((d / "meta.json").read_text(encoding="utf-8"))
        content = (d / "page.png").read_bytes()
        if hashlib.sha256(content).hexdigest() != meta["page_sha256"]:
            errors.append(f"{d.name}: page changed since it was pinned")
        result = R.recognize_image_bytes(content)
        notes = json.loads(result["notes"])
        cer, wer = cer_wer(truth["text"], result["full_text"])
        expected_route = R.ROUTE_ENGLISH if truth["script"] == S.LATIN else R.ROUTE_INDIC
        if notes["route"] != expected_route:
            errors.append(f"{d.name}: routed to {notes['route']}, expected {expected_route}")
        page_script = notes["text_script"]
        if page_script != truth["script"]:
            errors.append(f"{d.name}: page script {page_script}, expected {truth['script']}")
        ocr_lines = [x for x in notes["line_scripts"]]
        if len(ocr_lines) == len(truth["line_scripts"]):
            ok = sum(1 for got, want in zip(ocr_lines, truth["line_scripts"]) if got["script"] == want)
            line_total += len(truth["line_scripts"])
            line_ok += ok
            lines_comparable = True
        else:
            lines_comparable = False
            errors.append(f"{d.name}: {len(ocr_lines)} lines read, {len(truth['line_scripts'])} expected, so line scripts cannot be compared")
        pairs = pair_fields(truth["critical_fields"], result["critical_fields"])
        held = unheld = exact = undetected = 0
        for t, f in pairs:
            if f is None:
                undetected += 1
            elif same_field(f["text"], t["text"]):
                exact += 1
                if not f["syntactically_valid"] and "ambiguous" not in " ".join(f["validation_reasons"]):
                    errors.append(f"{d.name}: correctly read {t['kind']} '{t['text']}' failed validation: {f['validation_reasons']}")
            elif f["force_review"] and f["suppress_from_auto"]:
                held += 1
            else:
                unheld += 1
                wrong_unheld.append({"page": d.name, "truth": t["text"], "read": f["text"], "kind": t["kind"]})
        for k, v in (("truth", len(pairs)), ("exact", exact), ("wrong_held", held), ("wrong_unheld", unheld), ("undetected", undetected)):
            pair_totals[k] += v
        # fields the recognizer invented that are not in the truth: they must not be accepted automatically either
        matched = {id(f) for _t, f in pairs if f is not None}
        extras = [f for f in result["critical_fields"] if id(f) not in matched]
        rows[d.name] = {
            "route": notes["route"], "route_reason": notes["route_reason"], "recognizer_id": result["recognizer_id"], "cer": round(cer, 4), "wer": round(wer, 4),
            "page_script": page_script, "lines_compared": lines_comparable, "fields": {"truth": len(pairs), "exact": exact, "wrong_and_held": held, "wrong_and_not_held": unheld, "undetected": undetected},
            "extra_fields_found": [{"kind": f["field_type"], "text": f["text"], "held": f["force_review"]} for f in extras],
            "mean_confidence": round(result["mean_confidence"], 4), "duration_ms": notes["duration_ms"], "peak_engine_rss_mb": round(notes["peak_engine_rss_bytes"] / 1e6),
            "orientation_correction_degrees": notes["orientation_correction_degrees"], "osd": notes["osd"],
        }
    clean = [r["cer"] for n, r in rows.items() if n.startswith(("hi_", "mixed")) and "degraded" not in n]
    degraded = [r["cer"] for n, r in rows.items() if "degraded" in n]
    clean_cer, degraded_cer = sum(clean) / len(clean), sum(degraded) / len(degraded)
    if clean_cer > proposal["clean_scan_cer_max"] or degraded_cer > proposal["degraded_scan_cer_max"]:
        errors.append(f"measured CER is above the proposed limits (clean {clean_cer:.4f}, degraded {degraded_cer:.4f})")
    if rows["latin_control_08"]["orientation_correction_degrees"] != 0:
        errors.append("the English control page was rotated on a low-confidence orientation guess")
    if pair_totals["wrong_unheld"]:
        errors.append(f"{pair_totals['wrong_unheld']} misread critical fields were not held for review: {wrong_unheld}")
    unheld_extras = [(n, e) for n, r in rows.items() for e in r["extra_fields_found"] if not e["held"] and not n.startswith("latin")]
    if unheld_extras:
        errors.append(f"fields not in the truth were accepted automatically: {unheld_extras}")
    safe = (pair_totals["exact"] + pair_totals["wrong_held"]) / pair_totals["truth"]
    if safe < field_min:
        errors.append(f"critical fields read exactly or held for review: {safe:.3f} < proposed {field_min}")
    line_accuracy = line_ok / line_total if line_total else 0.0
    if line_accuracy < 1.0:
        errors.append(f"line script accuracy {line_accuracy:.3f} (all line labels were hand-written and are expected to match)")

    # ---- validators on values OCR can corrupt (all must be held, none accepted) ----
    corrupt = {
        "date": ["३२/१३/२०२४", "१२/०३/२०२४", "१२/०३/२०24", "१० मार्च २०O४", "३० फरवरी २०२४"],
        "money": ["₹ १,२५,००O", "₹ १,२५००,०००", "₹ ०", "रुपये ७५,०O०"],
        "provision": ["धारा", "धारा O"],
        "case_number": ["वाद संख्या १४५/", "वाद संख्या १४५/२०२३५"],
    }
    corrupt_held = {}
    for kind, values in corrupt.items():
        for v in values:
            fields = [f for f in validate_critical_fields(v, words_for(v)) if f["field_type"] == kind]
            held_all = all(f["force_review"] and f["suppress_from_auto"] for f in fields)
            corrupt_held[v] = held_all
            if fields and not held_all:
                errors.append(f"corrupted {kind} '{v}' was not blocked: {[f['validation_reasons'] for f in fields]}")
    good = {"date": ["१० मार्च २०२४", "१५/०४/२०२४"], "money": ["₹ १,२५,०००", "रुपये ७५,०००", "₹ २ लाख ५० हजार"], "provision": ["धारा ३०२/३४", "धारा ८०"], "case_number": ["वाद संख्या १४५/२०२३", "अपराध क्रमांक ०४५६/२०२३"], "name": ["श्री रमेश कुमार"]}
    for kind, values in good.items():
        for v in values:
            fields = [f for f in validate_critical_fields(v, words_for(v)) if f["field_type"] == kind]
            if not fields or not all(f["syntactically_valid"] and not f["suppress_from_auto"] for f in fields):
                errors.append(f"well-formed {kind} '{v}' was not accepted: {[f['validation_reasons'] for f in fields]}")
    amb = [f for f in validate_critical_fields("दिनांक १२/०३/२०२४", words_for("दिनांक १२/०३/२०२४")) if f["field_type"] == "date"]
    if not amb or "locale-ambiguous" not in " ".join(amb[0]["validation_reasons"]):
        errors.append("a numeric date with day and month both up to 12 was not flagged as ambiguous")
    if indic_fields.parse_amount("₹ २ लाख ५० हजार") != 250000 or indic_fields.parse_date("२८ फरवरी २०२४") != "2024-02-28" or indic_fields.parse_sections("धारा ३०२/३४") != ["302", "34"]:
        errors.append("Devanagari digit parsing is wrong")

    # ---- script detection on text ----
    cases = {"धारा ३०२ के अंतर्गत": S.DEVANAGARI, "Section 302 of the Code": S.LATIN, "आरोपी के विरुद्ध Section 420 IPC तथा धारा ३४": S.MIXED, "१२३ ४५६": None, "उसने बयान दिया कि रकम Rs. 5 थी": S.DEVANAGARI}
    for text, want in cases.items():
        if S.script_of_text(text) != want:
            errors.append(f"script_of_text({text!r}) = {S.script_of_text(text)}, expected {want}")
    low = S.parse_osd("Page number: 0\nOrientation in degrees: 180\nRotate: 180\nOrientation confidence: 0.71\nScript: Latin\nScript confidence: 2.5\n")
    if low["rotation"] != 0 or low["rotation_trusted"]:
        errors.append("a 0.71-confidence orientation guess was trusted")
    sure = S.parse_osd("Rotate: 180\nOrientation confidence: 9.0\nScript: Devanagari\nScript confidence: 5.0\n")
    if sure["rotation"] != 180 or sure["script"] != S.DEVANAGARI:
        errors.append("a confident orientation report was not used")

    # ---- calibration ----
    artifact = load_indic_calibration()
    dataset = json.loads(INDIC_DATASET_PATH.read_text(encoding="utf-8"))
    cal = {"status": "MISSING"}
    if artifact is None:
        errors.append("the Hindi calibration map is missing")
    else:
        if hashlib.sha256(INDIC_DATASET_PATH.read_bytes()).hexdigest() != artifact["dataset_sha256"]:
            errors.append("calibration dataset fingerprint mismatch")
        if artifact["calibration_id"] != INDIC_CALIBRATION_ID or artifact["engine_class"] != INDIC_ENGINE_CLASS:
            errors.append("calibration identity mismatch")
        if dataset["hindi_model_sha256"] != cfg["model"]["sha256"]:
            errors.append("the calibration was made with a different Hindi model than the one shipped")
        train_ids = {r["id"] for r in dataset["training"]}
        held_ids = {r["id"] for r in dataset["held_out"]}
        if train_ids & held_ids:
            errors.append("training and held-out ids overlap")
        page_groups = {p["id"]: p["group"] for p in dataset["pages"]}
        train_lines = {l for p in dataset["pages"] if p["group"] == "training" for l in p["lines"]}
        held_lines = {l for p in dataset["pages"] if p["group"] == "held_out" for l in p["lines"]}
        golden_lines = {l for d in pages for l in json.loads((d / "truth.json").read_text(encoding="utf-8"))["lines"]}
        golden_shas = {json.loads((d / "meta.json").read_text(encoding="utf-8"))["page_sha256"] for d in pages}
        if train_lines & held_lines:
            errors.append("a line appears in both the training and held-out pages")
        if (train_lines | held_lines) & golden_lines:
            errors.append("a calibration line is also a G-INDIC evaluation line")
        if {p["page_sha256"] for p in dataset["pages"]} & golden_shas or artifact["evaluation_set_overlap"] is not False:
            errors.append("calibration pages overlap the evaluation pages")
        if len(page_groups) != len(dataset["pages"]):
            errors.append("duplicate calibration page ids")
        bins = artifact["bins"]
        if any(bins[i]["calibrated"] > bins[i + 1]["calibrated"] for i in range(len(bins) - 1)) or bins[0]["raw_min"] != 0.0 or bins[-1]["raw_max"] < 1.0:
            errors.append("calibration bins are not monotonic or do not cover 0..1")
        pred = [calibrate(r["raw"], INDIC_ENGINE_CLASS)[0] or 0.0 for r in dataset["held_out"]]
        act = [r["correct"] for r in dataset["held_out"]]
        ece, brier = ece_brier(pred, act)
        raw_ece, raw_brier = ece_brier([r["raw"] for r in dataset["held_out"]], act)
        if abs(ece - artifact["held_out_metrics"]["ece"]) > 1e-5 or abs(brier - artifact["held_out_metrics"]["brier"]) > 1e-5:
            errors.append("held-out calibration metrics are not reproducible from the dataset")
        if ece >= raw_ece:
            errors.append("calibration does not reduce held-out ECE")
        # a word never seen with a score is explicit, not invented
        if calibrate(None, INDIC_ENGINE_CLASS) != (None, "missing"):
            errors.append("a missing confidence was not explicit")
        cal = {"status": "BLOCKED", "reason": "the owner has not set ECE/Brier limits or per-field error costs (approval.missing in the artifact)", "training_words": len(train_ids), "held_out_words": len(held_ids),
               "held_out_ece": round(ece, 6), "held_out_brier": round(brier, 6), "uncalibrated_ece": round(raw_ece, 6), "uncalibrated_brier": round(raw_brier, 6),
               "word_accuracy": artifact["word_accuracy"], "bins": [(b["raw_min"], b["raw_max"], b["calibrated"], b["training_words"]) for b in bins],
               "note": "ECE improves; Brier is not better than the raw score on these pages (calibration trades sharpness for honesty), and the low-confidence bins hold few words"}

    # ---- the English path is unchanged ----
    eng = load_english_module()
    committed = json.loads(ENGLISH_REPORT.read_text(encoding="utf-8"))
    source_pdf = (eng.ASSETS / "complaint_for_a_civil_case.pdf").read_bytes()
    truth_en = eng.parse_pdf_bytes(source_pdf)["pages"][0]["native_text"]
    english: dict[str, dict] = {}
    for name in ("page_1.png", "page_1.jpg", "page_1_rotated_degraded.jpg"):
        r = R.recognize_image_bytes((eng.ASSETS / name).read_bytes())
        c, w = eng.rates(truth_en, r["full_text"])
        n = json.loads(r["notes"])
        got = {"cer": round(c, 6), "wer": round(w, 6), "words": len(r["words"]), "mean_confidence": round(r["mean_confidence"], 6)}
        want = {k: committed["samples"][name][k] for k in got}
        english[name] = {"now": got, "committed": want, "route": n["route"], "recognizer_id": r["recognizer_id"]}
        if got != want:
            errors.append(f"English sample {name} changed: {got} vs committed {want}")
        if r["recognizer_id"] != R.RECOGNIZER_ID:
            errors.append(f"English sample {name} was not read by the English recognizer ({r['recognizer_id']})")

    # ---- sources: no network, no stored source bytes, orientation floor ----
    src = (ROOT / "sidecars" / "docpipe" / "recognize.py").read_text(encoding="utf-8")
    sd = (ROOT / "sidecars" / "docpipe" / "script_detect.py").read_text(encoding="utf-8")
    inf = (ROOT / "sidecars" / "docpipe" / "indic_fields.py").read_text(encoding="utf-8")
    for name, text in (("recognize.py", src), ("script_detect.py", sd), ("indic_fields.py", inf)):
        if re.search(r"requests\.|urllib\.request|http://|https://|socket\.", text):
            errors.append(f"{name} names a network facility")
    if "OSD_ORIENTATION_MIN_CONFIDENCE = 2.0" not in sd or "parse_osd" not in src:
        errors.append("the orientation confidence floor is not in force")
    rs = (ROOT / "src-tauri" / "src" / "docpipe" / "mod.rs").read_text(encoding="utf-8")
    if rs.count("calibration_id_for(&p.recognizer_id)") != 2 or "tesseract-hin-eng.synthetic-isotonic.v1" not in rs:
        errors.append("the host does not record the Hindi calibration id for Hindi pages")
    # Found by running the real app (2026-10-11): the calibration maps are read at run time, and the packaged app did not ship them, so English
    # recognition failed outright and every Hindi word was held. They must be bundled.
    conf = json.loads((ROOT / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8"))
    if "../eval/calibration/**/*" not in conf["bundle"]["resources"]:
        errors.append("the confidence calibration maps (eval/calibration) are not bundled with the app")
    if "confidence calibration shipped" not in (ROOT / "scripts" / "smoke_installed_layout.ps1").read_text(encoding="utf-8"):
        errors.append("the installed-layout smoke test does not check that the calibration maps shipped")
    ui = (ROOT / "src" / "components" / "ReviewView.tsx").read_text(encoding="utf-8")
    if "pageScriptLabel(sourceLayer)" not in ui:
        errors.append("the review screen does not show the page script")

    cer_status = "FAIL" if errors else ("BLOCKED" if frozen_cer["proposed"] is None else "PASS")
    report = {
        "batch": "B-S4-14",
        "implementation_status": "FAIL" if errors else "PASS",
        "acs": {"AC-S4-09.1": {
            "status": cer_status,
            "reason": None if errors else "S4.indic.ocr_cer_max has no frozen value (owner freezes it after the bake-off, Q-S4-04); the measured error is under the agent's proposal but a proposal is not a threshold",
            "evidence": {"clean_cer_mean": round(clean_cer, 4), "degraded_cer_mean": round(degraded_cer, 4), "proposal": proposal, "frozen_value": frozen_cer["proposed"],
                         "engine": R.runtime_manifest()["engine_version"], "model": cfg["model"]["family"] + " hin.traineddata " + model_sha[:16]},
            "limits": "Rendered pages are cleaner than photographed scans; one typeface family (Nirmala UI). This is a floor, not what a court's scans will give.",
        }},
        "pages": rows,
        "script_detection": {"page_scripts_correct": sum(1 for r in rows.values() if r["page_script"]), "line_script_accuracy": round(line_accuracy, 4), "lines": line_total},
        "critical_fields": {**pair_totals, "exact_rate": round(pair_totals["exact"] / pair_totals["truth"], 4), "exact_or_held_rate": round(safe, 4), "proposed_min": field_min,
                            "wrong_not_held": wrong_unheld, "corrupted_values_held": corrupt_held,
                            "meaning": "a misread date, amount, section, case number or name is never accepted automatically; it is held for counsel to confirm"},
        "calibration": cal,
        "english_path": english,
        "bakeoff": {"selected": bake["selected"], "summary": bake["summary"], "models": bake["models"]},
        "errors": errors,
    }
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"{'PASS' if not errors else 'FAIL'} B-S4-14 implementation; AC-S4-09.1 {cer_status}; CER clean {clean_cer:.4f} degraded {degraded_cer:.4f}; "
          f"fields exact {pair_totals['exact']}/{pair_totals['truth']}, misread-and-held {pair_totals['wrong_held']}, misread-and-not-held {pair_totals['wrong_unheld']}, undetected {pair_totals['undetected']}")
    for e in errors:
        print("  ERROR:", e)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
