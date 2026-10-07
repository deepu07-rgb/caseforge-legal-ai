# infer-llm sidecar

Local offline reasoner for CaseForge (Mock Court, argument building).

## Preferred setup (this machine)

[Ollama](https://ollama.com) is detected automatically:

```powershell
ollama pull qwen2.5:7b
# optional:
$env:CASEFORGE_LLM_MODEL = "qwen2.5:7b"
```

Smoke test:

```powershell
python sidecars/infer_llm/infer_llm.py --once "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"llm.status\",\"params\":{}}"
python sidecars/infer_llm/infer_llm.py --once "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"llm.chat\",\"params\":{\"user_text\":\"Give a 2-sentence counsel prep tip on evidence gaps.\",\"role\":\"counsel\",\"format\":\"oral_argument\"}}"
```

## Backends

| Backend | Env | Notes |
|---------|-----|--------|
| **ollama** (default) | `CASEFORGE_OLLAMA_HOST`, `CASEFORGE_LLM_MODEL` | Best DX; one model resident |
| openai_compat | `CASEFORGE_OPENAI_BASE` (e.g. LM Studio `http://127.0.0.1:1234/v1`) | Local OpenAI-compatible server |
| llama_cpp | `CASEFORGE_GGUF_PATH` + `pip install llama-cpp-python` | Direct GGUF |

Force backend: `CASEFORGE_LLM_BACKEND=ollama|openai_compat|llama_cpp`

## Product rules

- No cloud calls.
- No verdict prediction.
- Not a substitute for counsel; preparation only.
