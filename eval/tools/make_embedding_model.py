#!/usr/bin/env python3
"""B-S4-27: build the offline embedding model CaseForge ships: intfloat/multilingual-e5-small (MIT), pinned commit,
quantised to portable int8 (dynamic quantisation, runs on any x86-64 CPU), with a fidelity check against fp32.

Usage: python eval/tools/make_embedding_model.py <out_dir>
Output: model_int8.onnx, tokenizer.json, model_manifest.json (source commit, file SHA-256s, licence, fidelity).
"""
from __future__ import annotations

import hashlib
import json
import sys
import urllib.request
from pathlib import Path

REPO = "intfloat/multilingual-e5-small"
COMMIT = "614241f622f53c4eeff9890bdc4f31cfecc418b3"
FILES = ["onnx/model.onnx", "onnx/tokenizer.json"]
CHECK = [
    ("query: punishment for murder", "passage: Whoever commits murder shall be punished with death or imprisonment for life, and shall also be liable to fine."),
    ("query: anticipatory bail when apprehending arrest", "passage: When any person has reason to believe that he may be arrested on an accusation of having committed a non-bailable offence, he may apply to the High Court or the Court of Session for a direction."),
    ("query: हत्या के लिए दंड", "passage: Whoever commits murder shall be punished with death or imprisonment for life."),
    ("query: certificate for electronic record", "passage: Any information contained in an electronic record shall be deemed to be also a document, if the conditions mentioned are satisfied."),
]


def sha256(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def embed(session, tok, texts):
    import numpy as np

    enc = tok.encode_batch(texts)
    n = max(len(e.ids) for e in enc)
    ids = np.array([e.ids + [0] * (n - len(e.ids)) for e in enc], dtype=np.int64)
    mask = np.array([e.attention_mask + [0] * (n - len(e.attention_mask)) for e in enc], dtype=np.int64)
    feeds = {"input_ids": ids, "attention_mask": mask}
    if any(i.name == "token_type_ids" for i in session.get_inputs()):
        feeds["token_type_ids"] = np.zeros_like(ids)
    hidden = session.run(None, feeds)[0]
    m = mask[..., None].astype(np.float32)
    v = (hidden * m).sum(1) / np.clip(m.sum(1), 1e-9, None)
    return v / np.linalg.norm(v, axis=1, keepdims=True)


def main() -> int:
    import numpy as np
    import onnxruntime as ort
    from onnxruntime.quantization import QuantType, quantize_dynamic
    from tokenizers import Tokenizer

    out = Path(sys.argv[1])
    raw = out / "raw"
    raw.mkdir(parents=True, exist_ok=True)
    for f in FILES:
        dest = raw / Path(f).name
        if not dest.exists():
            urllib.request.urlretrieve(f"https://huggingface.co/{REPO}/resolve/{COMMIT}/{f}", dest)
    quantize_dynamic(str(raw / "model.onnx"), str(out / "model_int8.onnx"), weight_type=QuantType.QInt8)
    (out / "tokenizer.json").write_bytes((raw / "tokenizer.json").read_bytes())
    tok = Tokenizer.from_file(str(out / "tokenizer.json"))
    tok.enable_truncation(512)
    fp = ort.InferenceSession(str(raw / "model.onnx"), providers=["CPUExecutionProvider"])
    q8 = ort.InferenceSession(str(out / "model_int8.onnx"), providers=["CPUExecutionProvider"])
    texts = [t for pair in CHECK for t in pair]
    a, b = embed(fp, tok, texts), embed(q8, tok, texts)
    fidelity = float(np.min((a * b).sum(1)))
    pair_sims = [float((b[2 * i] * b[2 * i + 1]).sum()) for i in range(len(CHECK))]
    cross = float((b[0] * b[3]).sum())  # murder query vs bail passage: should be lower than the matching pairs
    manifest = {
        "model": REPO, "commit": COMMIT, "licence": "MIT", "dims": int(b.shape[1]), "pooling": "mean", "normalised": True,
        "prefixes": {"query": "query: ", "passage": "passage: "}, "max_tokens": 512, "quantisation": "onnxruntime dynamic QInt8 (portable)",
        "files": {p.name: sha256(p) for p in (out / "model_int8.onnx", out / "tokenizer.json")},
        "source_files": {f: sha256(raw / Path(f).name) for f in FILES},
        "fidelity_min_cosine_fp32_vs_int8": round(fidelity, 4), "matching_pair_similarity": [round(x, 4) for x in pair_sims], "non_matching_similarity": round(cross, 4),
    }
    (out / "model_manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    print(json.dumps(manifest, indent=2))
    if fidelity < 0.98:
        raise SystemExit(f"int8 model drifts from fp32 (min cosine {fidelity:.4f} < 0.98)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
