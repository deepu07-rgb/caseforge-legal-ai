#!/usr/bin/env python3
"""Runs the real local model over the G-INDIC translation fixture and records what happened (B-S4-15 live evidence).

This is the only part of the translation gate that needs the model. It writes `eval/evidence/translation_live_b_s4_15.json`; the gate
(`eval/checks/translation_b_s4_15.py`) reads that file in CI, where there is no model, and refuses it if it was made with a different prompt or
model than the one in `contracts/translation_config.json`.

It measures three things, honestly:
  * marker survival - how often the model gives back each [F1], [F2] marker exactly once (the host depends on it);
  * what the host does when it does not - every lost, repeated or invented marker flags the field and the segment;
  * how close the English prose is to the reference (an indicative chrF score only: the references are one acceptable rendering written by the
    coding agent, not a legal translator's, and no limit is set on this number).
The prose quality of a 7B general model on Hindi legal text is modest; every segment is shown beside the original and labelled for that reason.

Usage: python eval/tools/translate_golden_live.py            (Ollama running with the model in contracts/translation_config.json)
"""
from __future__ import annotations

import collections
import hashlib
import json
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "sidecars" / "docpipe"))
sys.path.insert(0, str(ROOT / "sidecars" / "infer_llm"))

import infer_llm as L  # noqa: E402
import translate as T  # noqa: E402

GOLD = ROOT / "golden" / "G-INDIC" / "fixtures" / "translation_core.json"
OUT = ROOT / "eval" / "evidence" / "translation_live_b_s4_15.json"


def chrf(ref: str, hyp: str, n: int = 6, beta: float = 2.0) -> float:
    def grams(s: str, k: int) -> collections.Counter:
        s = s.replace(" ", "")
        return collections.Counter(s[i:i + k] for i in range(len(s) - k + 1))

    ps, rs = [], []
    for k in range(1, n + 1):
        r, h = grams(ref, k), grams(hyp, k)
        if not r or not h:
            continue
        ov = sum((r & h).values())
        ps.append(ov / max(1, sum(h.values())))
        rs.append(ov / max(1, sum(r.values())))
    if not ps:
        return 0.0
    p, r = sum(ps) / len(ps), sum(rs) / len(rs)
    return 0.0 if p + r == 0 else (1 + beta ** 2) * p * r / (beta ** 2 * p + r)


def main() -> int:
    status = L.handle({"jsonrpc": "2.0", "id": 1, "method": "llm.status", "params": {}})["result"]
    if not status.get("ready"):
        print("the local model is not available; start Ollama with the configured model", file=sys.stderr)
        return 2
    cfg = T.load_config()
    if status.get("model_digest") != cfg["model"]["digest"]:
        print(f"the running model ({status.get('model_digest')}) is not the configured one ({cfg['model']['digest']})", file=sys.stderr)
        return 2
    gold = json.loads(GOLD.read_text(encoding="utf-8"))

    def translator(masked: str) -> str:
        r = L.handle({"jsonrpc": "2.0", "id": 1, "method": "llm.translate", "params": {"text": masked}})
        if "error" in r:
            raise RuntimeError(r["error"])
        return r["result"]["content"]

    rows = []
    started = time.time()
    for page, segs in gold["pages"].items():
        for seg in segs:
            if not seg["translate"]:
                continue
            res = T.translate_segment(seg["source"], translator)
            expected = [(f["kind"], f["source_value"], f["english"]) for f in seg["fields"]]
            fields = []
            for f in res["fields"]:
                english, reason = res["rendered"][f["field_index"]]
                want = next((e[2] for e in expected if e[1] == f["source_value"]), None)
                state = res["field_states"][f["field_index"]]
                fields.append({"kind": f["field_kind"], "source_value": f["source_value"], "english": english, "expected": want, "marker": state, "reason": reason,
                               "matches_expected": (english == want) if f["field_kind"] != "NAME" else None})
            rows.append({"page": page, "source": seg["source"], "masked": res["masked"], "model_output": res["model_output"], "english": res["text"], "reference": seg["reference_english"],
                         "chrf": round(chrf(seg["reference_english"], res["text"]), 3), "segment_flagged": res["segment_flagged"], "invented": res["invented"], "fields": fields})
    all_fields = [f for r in rows for f in r["fields"]]
    non_name = [f for f in all_fields if f["kind"] != "NAME"]
    carried = sum(1 for f in all_fields if f["marker"] == "OK")
    wrong_unflagged = [f for f in non_name if f["matches_expected"] is False and f["marker"] == "OK" and not f["reason"]]
    summary = {
        "segments": len(rows), "segments_flagged": sum(r["segment_flagged"] for r in rows), "fields": len(all_fields), "markers_carried_intact": carried,
        "markers_lost_or_repeated": len(all_fields) - carried, "marker_survival": round(carried / max(1, len(all_fields)), 4),
        "non_name_fields": len(non_name), "non_name_fields_equal_to_expected": sum(1 for f in non_name if f["matches_expected"]),
        "non_name_fields_wrong_and_not_flagged": len(wrong_unflagged), "names": len(all_fields) - len(non_name),
        "chrf_mean": round(sum(r["chrf"] for r in rows) / max(1, len(rows)), 3), "seconds": round(time.time() - started),
    }
    evidence = {
        "batch": "B-S4-15", "tool": "eval/tools/translate_golden_live.py", "made_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "model": {"name": status["model"], "digest": status["model_digest"], "backend": status["backend"], "backend_version": status["backend_version"]},
        "prompt_sha256": hashlib.sha256((ROOT / cfg["prompt"]["file"]).read_bytes()).hexdigest(), "generation": cfg["generation"],
        "golden_sha256": hashlib.sha256(GOLD.read_bytes()).hexdigest(), "summary": summary, "segments": rows,
        "limits": "Reference translations are the coding agent's, not a legal translator's; chrF is indicative only and no limit is set on it. One local 7B general model; prose errors are expected and are why every segment is shown beside the original and labelled.",
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(evidence, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(summary))
    return 0


if __name__ == "__main__":
    sys.exit(main())
