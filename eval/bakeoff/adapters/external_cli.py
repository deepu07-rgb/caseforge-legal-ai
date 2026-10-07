"""External CLI adapter: one JSON request/response per fixture batch.

CLI contract (stdin → stdout, single JSON object):

Request:
{
  "pages": [
    {"page_index": 0, "text": "...", "content_base64": null, "filename": "p.png"}
  ]
}

Response:
{
  "pages": [
    {"page_index": 0, "triage_class": "typed_english", "confidence": 0.8}
  ]
}
"""

from __future__ import annotations

import json
import subprocess
import time
from pathlib import Path
from typing import Any


def classify_fixture(fx: dict[str, Any], command: list[str]) -> list[dict[str, Any]]:
    import base64

    page: dict[str, Any] = {
        "page_index": 0,
        "text": fx.get("text") or "",
        "filename": "page",
        "content_base64": None,
    }
    content_path = fx.get("content_path")
    if content_path and Path(content_path).exists():
        raw = Path(content_path).read_bytes()
        page["content_base64"] = base64.b64encode(raw).decode("ascii")
        page["filename"] = Path(content_path).name

    payload = json.dumps({"pages": [page]}, ensure_ascii=False)
    t0 = time.perf_counter()
    proc = subprocess.run(
        command,
        input=payload,
        capture_output=True,
        text=True,
        timeout=300,
        check=False,
    )
    elapsed_ms = (time.perf_counter() - t0) * 1000.0
    if proc.returncode != 0:
        raise RuntimeError(
            f"external_cli failed rc={proc.returncode}: {proc.stderr or proc.stdout}"
        )
    data = json.loads(proc.stdout)
    out = []
    for r in data.get("pages") or []:
        out.append(
            {
                "page_index": int(r.get("page_index", 0)),
                "triage_class": r.get("triage_class") or r.get("class"),
                "confidence": r.get("confidence"),
                "classifier_id": r.get("classifier_id") or "external_cli",
                "latency_ms": elapsed_ms,
            }
        )
    return out
