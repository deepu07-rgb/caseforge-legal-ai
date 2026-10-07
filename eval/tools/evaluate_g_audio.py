#!/usr/bin/env python3
"""Measure B-S3-20 WER and labelled critical-term error on rights-cleared human audio."""
from __future__ import annotations

import argparse
import base64
import importlib.util
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_MANIFEST = ROOT / "eval/fixtures/g_audio/manifest.json"
TOKEN = re.compile(r"[a-z0-9]+(?:'[a-z0-9]+)?")


def words(text: str) -> list[str]:
    return TOKEN.findall(text.casefold())


def edit_distance(reference: list[str], hypothesis: list[str]) -> int:
    previous = list(range(len(hypothesis) + 1))
    for row, expected in enumerate(reference, 1):
        current = [row]
        for column, actual in enumerate(hypothesis, 1):
            current.append(min(current[-1] + 1, previous[column] + 1, previous[column - 1] + (expected != actual)))
        previous = current
    return previous[-1]


def load_sidecar():
    path = ROOT / "sidecars/infer_speech/infer_speech.py"
    spec = importlib.util.spec_from_file_location("caseforge_infer_speech", path)
    if not spec or not spec.loader:
        raise RuntimeError("Unable to load infer-speech sidecar")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    clips = manifest.get("clips", [])
    speakers = {clip.get("speaker_id") for clip in clips if clip.get("speaker_id")}
    if manifest.get("status") != "GOLDEN" or manifest.get("source_class") != "HUMAN_AUDIO_RIGHTS_CLEARED":
        raise SystemExit("G-AUDIO is not GOLDEN rights-cleared human audio; measurement halted.")
    if len(clips) < int(manifest.get("minimum_clips", 20)) or len(speakers) < int(manifest.get("minimum_speakers", 5)):
        raise SystemExit("G-AUDIO does not meet the frozen clip/speaker minimum.")
    sidecar = load_sidecar()
    word_errors = reference_words = critical_errors = critical_total = 0
    latency_ms: list[int] = []
    rows = []
    for clip in clips:
        if clip.get("rights_attested") is not True:
            raise SystemExit(f"Missing rights attestation for {clip.get('clip_id')}")
        audio_path = (args.manifest.parent / clip["audio_file"]).resolve()
        if not audio_path.is_file() or args.manifest.parent.resolve() not in audio_path.parents:
            raise SystemExit(f"Invalid audio path for {clip.get('clip_id')}")
        result = sidecar.stt({
            "audio_base64": base64.b64encode(audio_path.read_bytes()).decode("ascii"),
            "audio_format": audio_path.suffix.lstrip("."),
            "language": "English",
        })
        reference = words(clip["reference_text"])
        hypothesis = words(result["raw_transcript"])
        errors = edit_distance(reference, hypothesis)
        word_errors += errors
        reference_words += len(reference)
        hypothesis_text = " ".join(hypothesis)
        labelled = clip.get("critical_terms", [])
        missed = [term for term in labelled if " ".join(words(term["text"])) not in hypothesis_text]
        critical_errors += len(missed)
        critical_total += len(labelled)
        latency_ms.append(int(result["elapsed_ms"]))
        rows.append({"clip_id": clip["clip_id"], "errors": errors, "words": len(reference), "missed_critical_terms": missed})
    wer = word_errors / max(1, reference_words)
    critical_rate = critical_errors / max(1, critical_total)
    report = {
        "batch": "B-S3-20",
        "source_class": "HUMAN_AUDIO_RIGHTS_CLEARED",
        "clips": len(clips),
        "speakers": len(speakers),
        "word_error_rate": wer,
        "critical_term_error_rate": critical_rate,
        "word_error_rate_max": 0.1,
        "critical_term_error_rate_max": 0.02,
        "median_latency_ms": sorted(latency_ms)[len(latency_ms) // 2],
        "passed": wer <= 0.1 and critical_rate <= 0.02,
        "rows": rows,
    }
    (ROOT / "eval/reports/b_s3_20_g_audio.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    for metric_id, value, threshold in [
        ("voice_word_error_rate", wer, 0.1),
        ("voice_critical_term_error_rate", critical_rate, 0.02),
    ]:
        (ROOT / f"eval/results/stage3/{metric_id}.json").write_text(json.dumps({
            "metric_id": metric_id, "value": value, "threshold": threshold,
            "source_class": "HUMAN_AUDIO_RIGHTS_CLEARED", "batch": "B-S3-20",
        }, indent=2) + "\n", encoding="utf-8")
    print(f"G-AUDIO WER={wer:.2%} (≤10%); critical-term error={critical_rate:.2%} (≤2%)")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
