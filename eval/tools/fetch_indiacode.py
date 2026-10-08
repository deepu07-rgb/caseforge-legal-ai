"""Fetch the six official criminal-law Acts from India Code (Ministry of Law and Justice), retrying while the site
returns 5xx. Handle pages are read only to discover the official bitstream PDF link. Output: <out>/<slug>.pdf and
<out>/fetch_ledger.json (url, sha256, bytes, fetched_at). Run with: python -I fetch_indiacode.py <out_dir>"""
import hashlib
import json
import re
import sys
import time
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

OUT = Path(sys.argv[1])
OUT.mkdir(parents=True, exist_ok=True)
BASE = "https://www.indiacode.nic.in"
ACTS = [
    # slug, known official bitstream (or None), handle pages to discover it
    ("bns_2023", None, ["/handle/123456789/20062?view_type=browse", "/handle/123456789/20062"]),
    ("bnss_2023", "/bitstream/123456789/20099/3/a2023-46.pdf", ["/handle/123456789/20099"]),
    ("bsa_2023", "/bitstream/123456789/20063/1/aa202347.pdf", ["/handle/123456789/20063"]),
    ("ipc_1860", None, ["/handle/123456789/12850?view_type=browse", "/handle/123456789/12850", "/handle/123456789/2263"]),
    ("crpc_1973", "/bitstream/123456789/13624/1/the_code_of_criminal_procedure%2c_1973.pdf", ["/handle/123456789/12831"]),
    ("evidence_1872", None, ["/handle/123456789/12846?view_type=browse", "/handle/123456789/12846"]),
]
DEADLINE = time.time() + 40 * 60


def get(path: str, timeout: int = 120) -> bytes:
    req = urllib.request.Request(BASE + path, headers={"User-Agent": "Mozilla/5.0 (CaseForge pack builder; official-text acquisition)"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read()


def discover(handles: list[str]) -> str | None:
    for h in handles:
        try:
            html = get(h, 90).decode("utf-8", "replace")
        except Exception as e:  # noqa: BLE001
            print(f"  handle {h}: {e}", flush=True)
            continue
        links = sorted(set(re.findall(r'href="(/bitstream/123456789/[^"]+?\.pdf)[^"]*"', html, re.I)))
        english = [l for l in links if "manipuri" not in l.lower() and "hindi" not in l.lower()]
        print(f"  handle {h}: {links}", flush=True)
        if english:
            return english[0]
    return None


ledger_path = OUT / "fetch_ledger.json"
ledger = json.loads(ledger_path.read_text()) if ledger_path.exists() else {}
pending = [a for a in ACTS if a[0] not in ledger]
while pending and time.time() < DEADLINE:
    for slug, known, handles in list(pending):
        url = known or discover(handles)
        if not url:
            continue
        try:
            data = get(url, 180)
        except Exception as e:  # noqa: BLE001
            print(f"{slug}: {url}: {e}", flush=True)
            continue
        if not data.startswith(b"%PDF"):
            print(f"{slug}: {url}: not a PDF ({len(data)} bytes)", flush=True)
            continue
        (OUT / f"{slug}.pdf").write_bytes(data)
        ledger[slug] = {"url": BASE + url, "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data), "fetched_at": datetime.now(timezone.utc).isoformat()}
        ledger_path.write_text(json.dumps(ledger, indent=2))
        pending.remove((slug, known, handles))
        print(f"OK {slug} {len(data)} bytes", flush=True)
    if pending:
        time.sleep(60)
print("DONE" if not pending else f"GAVE UP: {[p[0] for p in pending]}", flush=True)
