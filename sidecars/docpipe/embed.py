#!/usr/bin/env python3
"""Offline text embeddings for CaseForge legal search (B-S4-27).

Model: intfloat/multilingual-e5-small (MIT), portable int8 ONNX built by eval/tools/make_embedding_model.py; files are
verified against model_manifest.json before use. Runs on CPU, no network. English and Indian languages.

Library:  embed(texts, kind="passage"|"query") -> list[list[float]]  (L2-normalised, 384 dims)
          quantise(vectors) -> bytes  (int8, scale 127, row-major) — the on-disk format in packs
JSON-RPC: python embed.py --once '{"jsonrpc":"2.0","id":1,"method":"embed.query","params":{"text":"..."}}'
Model dir: $CASEFORGE_EMBED_MODEL, else ../models/e5-small next to this file.
"""
from __future__ import annotations

import hashlib
import json
import os
import sys
from functools import lru_cache
from pathlib import Path

MODEL_ID = "intfloat/multilingual-e5-small@614241f"
# Built by .github/workflows/models.yml (release models-v1; fp32-vs-int8 min cosine 0.9871). A file with any other hash
# is refused, whatever its manifest says.
PINNED = {
    "model_int8.onnx": "8da4c9ba0ad59f58e8566839425d7fd6339d31414d0ce5cba2d7d0afb75dd8b6",
    "tokenizer.json": "0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39",
}


def model_dir() -> Path:
    env = os.environ.get("CASEFORGE_EMBED_MODEL")
    return Path(env) if env else Path(__file__).resolve().parent.parent / "models" / "e5-small"


@lru_cache(maxsize=1)
def _load():
    import onnxruntime as ort
    from tokenizers import Tokenizer

    d = model_dir()
    manifest = json.loads((d / "model_manifest.json").read_text(encoding="utf-8"))
    for name, digest in PINNED.items():
        h = hashlib.sha256((d / name).read_bytes()).hexdigest()
        if h != digest or manifest["files"].get(name) != digest:
            raise RuntimeError(f"CF_EMBED_MODEL_TAMPERED: {name} sha256 {h[:12]} is not the pinned {digest[:12]}")
    tok = Tokenizer.from_file(str(d / "tokenizer.json"))
    tok.enable_truncation(manifest.get("max_tokens", 512))
    opts = ort.SessionOptions()
    opts.intra_op_num_threads = max(1, min(4, (os.cpu_count() or 2) - 1))
    sess = ort.InferenceSession(str(d / "model_int8.onnx"), sess_options=opts, providers=["CPUExecutionProvider"])
    return tok, sess, manifest


def embed(texts: list[str], kind: str = "passage", batch: int = 32) -> list[list[float]]:
    import numpy as np

    tok, sess, manifest = _load()
    prefix = manifest["prefixes"]["query" if kind == "query" else "passage"]
    needs_types = any(i.name == "token_type_ids" for i in sess.get_inputs())
    # Batches of similar length waste little work on padding; results are returned in input order.
    order = sorted(range(len(texts)), key=lambda k: len(texts[k]))
    out: list = [None] * len(texts)
    for i in range(0, len(order), batch):
        part = order[i: i + batch]
        enc = tok.encode_batch([prefix + texts[k] for k in part])
        n = max(len(e.ids) for e in enc)
        ids = np.array([e.ids + [0] * (n - len(e.ids)) for e in enc], dtype=np.int64)
        mask = np.array([e.attention_mask + [0] * (n - len(e.attention_mask)) for e in enc], dtype=np.int64)
        feeds = {"input_ids": ids, "attention_mask": mask}
        if needs_types:
            feeds["token_type_ids"] = np.zeros_like(ids)
        hidden = sess.run(None, feeds)[0]
        m = mask[..., None].astype(np.float32)
        v = (hidden * m).sum(1) / np.clip(m.sum(1), 1e-9, None)
        v = v / np.linalg.norm(v, axis=1, keepdims=True)
        for k, row in zip(part, v.tolist()):
            out[k] = row
    return out


def quantise(vectors: list[list[float]]) -> bytes:
    import numpy as np

    return np.clip(np.rint(np.asarray(vectors, dtype=np.float32) * 127.0), -127, 127).astype(np.int8).tobytes()


def handle(req: dict) -> dict:
    rid, method, params = req.get("id"), req.get("method"), req.get("params") or {}
    try:
        if method == "embed.status":
            _, _, manifest = _load()
            return {"jsonrpc": "2.0", "id": rid, "result": {"ready": True, "model": MODEL_ID, "dims": manifest["dims"]}}
        if method == "embed.query":
            v = embed([str(params.get("text") or "")], kind="query")[0]
            return {"jsonrpc": "2.0", "id": rid, "result": {"model": MODEL_ID, "vector": [round(x, 6) for x in v]}}
        return {"jsonrpc": "2.0", "id": rid, "error": {"code": -32601, "message": f"unknown method {method}"}}
    except Exception as e:  # noqa: BLE001 - reported to the host, which falls back to lexical search
        return {"jsonrpc": "2.0", "id": rid, "error": {"code": -32000, "message": str(e), "data": {"code": "CF_EMBED_UNAVAILABLE"}}}


if __name__ == "__main__":
    if len(sys.argv) >= 3 and sys.argv[1] == "--once":
        print(json.dumps(handle(json.loads(sys.argv[2]))))
    else:
        for line in sys.stdin:
            if line.strip():
                print(json.dumps(handle(json.loads(line))), flush=True)
