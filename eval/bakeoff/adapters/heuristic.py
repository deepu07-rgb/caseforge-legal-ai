"""Baseline adapter: caseforge.triage.heuristic.v1 (sidecars/docpipe)."""

from __future__ import annotations

import sys
import time
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[3]
SIDECAR = ROOT / "sidecars" / "docpipe"
if str(SIDECAR) not in sys.path:
    sys.path.insert(0, str(SIDECAR))

from triage import CLASSIFIER_ID, pages_from_document, triage_pages  # type: ignore


def classify_fixture(fx: dict[str, Any]) -> list[dict[str, Any]]:
    """Return list of {page_index, triage_class, confidence, latency_ms}."""
    t0 = time.perf_counter()
    content_path = fx.get("content_path")
    if content_path and Path(content_path).exists():
        import base64

        raw = Path(content_path).read_bytes()
        pages = pages_from_document(
            content_base64=base64.b64encode(raw).decode("ascii"),
            filename=Path(content_path).name,
            media_type=Path(content_path).suffix.lstrip(".") or "bin",
        )
    else:
        pages = pages_from_document(
            text=fx.get("text") or "",
            media_type="text/plain",
            filename="page_stub.txt",
        )
    results = triage_pages(pages)
    elapsed_ms = (time.perf_counter() - t0) * 1000.0
    out = []
    for r in results:
        out.append(
            {
                "page_index": int(r["page_index"]),
                "triage_class": r["triage_class"],
                "confidence": r.get("confidence"),
                "classifier_id": CLASSIFIER_ID,
                "latency_ms": elapsed_ms / max(len(results), 1),
            }
        )
    return out
