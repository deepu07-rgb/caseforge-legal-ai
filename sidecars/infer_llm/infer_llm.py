#!/usr/bin/env python3
"""CaseForge local inference sidecar (B-S3-13 controlled-beta model freeze)."""
from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import sys
import time
import urllib.request
from pathlib import Path
from typing import Any

VERSION, BATCH = "0.4.0", "B-S4-00"
ROOT = Path(__file__).resolve().parents[2]
MANIFEST_PATH = ROOT / "contracts" / "supported_models.json"
ADDON_MANIFEST_PATH = ROOT / "contracts" / "supported_models_addon.json"
DEFAULT_OLLAMA = os.environ.get("CASEFORGE_OLLAMA_HOST", "http://127.0.0.1:11434").rstrip("/")
OPENAI_BASE = os.environ.get("CASEFORGE_OPENAI_BASE", "").rstrip("/")
GGUF_PATH = os.environ.get("CASEFORGE_GGUF_PATH", "").strip()
FORCED_BACKEND = os.environ.get("CASEFORGE_LLM_BACKEND", "").strip().lower()
ALLOW_UNVALIDATED = os.environ.get("CASEFORGE_ALLOW_UNVALIDATED_MODEL", "") == "1"
# B-S4-00 tiered models: "default" ships in the base install; "addon" is the optional larger
# model, used only when the user explicitly selects it and the machine meets the Full tier.
CONTRACTS = {"default": json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))}
if ADDON_MANIFEST_PATH.is_file():
    CONTRACTS["addon"] = json.loads(ADDON_MANIFEST_PATH.read_text(encoding="utf-8"))
REQUESTED_TIER = (os.environ.get("CASEFORGE_LLM_TIER", "default").strip().lower() or "default")
TIER = REQUESTED_TIER if REQUESTED_TIER in CONTRACTS else "default"
TIER_ERROR = None if REQUESTED_TIER in CONTRACTS else f"unknown model tier {REQUESTED_TIER!r}; available: {sorted(CONTRACTS)}"
CONTRACT = CONTRACTS[TIER]
DEFAULT_MODEL = os.environ.get("CASEFORGE_LLM_MODEL", CONTRACT["model"]["name"])
LICENSE_NAMES = {"apache-2.0": "Apache-2.0", "mit": "MIT"}
ADDON_MIN_RAM_GB = 8  # constants.hardware_tiers.full.min_ram_gb
GENERATION = CONTRACT["generation"]
PROMPT_PATH = ROOT / CONTRACT["prompt"]["file"]
SYSTEM_CORE = PROMPT_PATH.read_text(encoding="utf-8").strip()
SYSTEM_PROMPT_HASH = hashlib.sha256(PROMPT_PATH.read_bytes()).hexdigest()
MOCK_COURT_REGISTRY = json.loads((ROOT / "contracts" / "mock_court_prompt_registry.json").read_text(encoding="utf-8"))
MOCK_COURT_PROMPT_PATH = ROOT / MOCK_COURT_REGISTRY["file"]
MOCK_COURT_PROMPT = MOCK_COURT_PROMPT_PATH.read_text(encoding="utf-8").strip()
MOCK_COURT_PROMPT_HASH = hashlib.sha256(MOCK_COURT_PROMPT_PATH.read_bytes()).hexdigest()
MOCK_COURT_PROMPT_VALID = MOCK_COURT_PROMPT_HASH == MOCK_COURT_REGISTRY["sha256"]
MOCK_COURT_TURN_SCHEMA = json.loads((ROOT / MOCK_COURT_REGISTRY["turn_schema"]).read_text(encoding="utf-8"))


def ok(id_: Any, result: Any) -> dict[str, Any]:
    return {"jsonrpc": "2.0", "result": result, "id": id_}


def err(id_: Any, code: int, message: str, data: Any = None) -> dict[str, Any]:
    body: dict[str, Any] = {"code": code, "message": message}
    if data is not None:
        body["data"] = data
    return {"jsonrpc": "2.0", "error": body, "id": id_}


def http_json(url: str, payload: dict[str, Any] | None = None, timeout: float = 180.0) -> Any:
    raw_payload = json.dumps(payload).encode("utf-8") if payload is not None else None
    headers = {"Accept": "application/json"}
    if raw_payload is not None:
        headers["Content-Type"] = "application/json"
    request = urllib.request.Request(url, data=raw_payload, headers=headers, method="POST" if raw_payload else "GET")
    with urllib.request.urlopen(request, timeout=timeout) as response:
        raw = response.read().decode("utf-8")
        return json.loads(raw) if raw else {}


def _ollama_version() -> str:
    try:
        return str(http_json(f"{DEFAULT_OLLAMA}/api/version", timeout=3.0).get("version") or "unknown")
    except Exception:
        try:
            return subprocess.check_output(["ollama", "--version"], text=True, timeout=3).strip().rsplit(" ", 1)[-1]
        except Exception:
            return "unknown"


def total_ram_gb() -> float | None:
    try:
        if os.name == "nt":
            import ctypes

            class MEMORYSTATUSEX(ctypes.Structure):
                _fields_ = [("dwLength", ctypes.c_ulong), ("dwMemoryLoad", ctypes.c_ulong), ("ullTotalPhys", ctypes.c_ulonglong),
                            ("ullAvailPhys", ctypes.c_ulonglong), ("ullTotalPageFile", ctypes.c_ulonglong), ("ullAvailPageFile", ctypes.c_ulonglong),
                            ("ullTotalVirtual", ctypes.c_ulonglong), ("ullAvailVirtual", ctypes.c_ulonglong), ("sullAvailExtendedVirtual", ctypes.c_ulonglong)]
            state = MEMORYSTATUSEX()
            state.dwLength = ctypes.sizeof(state)
            if ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(state)):
                return state.ullTotalPhys / 1024 ** 3
            return None
        return os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES") / 1024 ** 3
    except (AttributeError, OSError, ValueError):
        return None


def tier_info() -> dict[str, Any]:
    ram = total_ram_gb()
    addon_blocked = None
    if TIER == "addon" and ram is not None and ram < ADDON_MIN_RAM_GB:
        addon_blocked = f"optional add-on model requires >= {ADDON_MIN_RAM_GB} GB RAM; this machine has {ram:.1f} GB"
    return {"tier": TIER, "requested_tier": REQUESTED_TIER, "available_tiers": {name: c["model"]["name"] for name, c in CONTRACTS.items()},
            "total_ram_gb": round(ram, 1) if ram is not None else None, "tier_error": TIER_ERROR or addon_blocked}


def probe_ollama() -> dict[str, Any]:
    try:
        installed = http_json(f"{DEFAULT_OLLAMA}/api/tags", timeout=3.0).get("models") or []
        exact = next((model for model in installed if model.get("name") == DEFAULT_MODEL), None)
        entry = exact or (installed[0] if installed else None)
        selected = str((entry or {}).get("name") or DEFAULT_MODEL)
        show = http_json(f"{DEFAULT_OLLAMA}/api/show", {"name": selected}, timeout=5.0) if entry else {}
        details = show.get("details") or (entry or {}).get("details") or {}
        model_info = show.get("model_info") or {}
        digest = str((entry or {}).get("digest") or "")
        digest = f"sha256:{digest}" if digest and not digest.startswith("sha256:") else digest
        license_id = str(model_info.get("general.license") or "unknown")
        license_text = str(show.get("license") or "").replace("\r\n", "\n").strip()
        license_text_sha256 = hashlib.sha256(license_text.encode("utf-8")).hexdigest() if license_text else None
        pinned_license_hash = CONTRACT["model"].get("license_text_sha256")
        if license_id == "unknown" and pinned_license_hash and license_text_sha256 == pinned_license_hash:
            # No metadata tag, but the exact licence text shipped with the model matches the pinned text.
            license_id = CONTRACT["model"]["license"]
        architecture = str(model_info.get("general.architecture") or details.get("family") or "")
        info: dict[str, Any] = {
            "backend": "ollama", "backend_version": _ollama_version(), "available": entry is not None,
            "host": DEFAULT_OLLAMA, "model": selected,
            "models": [model.get("name") for model in installed if model.get("name")], "digest": digest,
            "family": str(details.get("family") or "unknown"), "parameters": str(details.get("parameter_size") or "unknown"),
            "quantization": str(details.get("quantization_level") or "unknown"),
            "context_length": int(model_info.get(f"{architecture}.context_length") or details.get("context_length") or 0),
            "license": LICENSE_NAMES.get(license_id.lower(), license_id),
            "classification": "stock-instruct + quantized + CaseForge-prompted",
            "legally_fine_tuned": False, "checkpoint_evidence": False,
        }
        expected, expected_backend = CONTRACT["model"], CONTRACT["backend"]
        checks = {
            "backend": info["backend"] == expected_backend["name"],
            "backend_version": info["backend_version"] == expected_backend["version"],
            "model": info["model"] == expected["name"], "digest": info["digest"] == expected["digest"],
            "family": info["family"] == expected["family"], "parameters": info["parameters"] == expected["parameters"],
            "quantization": info["quantization"] == expected["quantization"],
            "context_length": info["context_length"] == expected["context_length"], "license": info["license"] == expected["license"],
            "prompt_hash": SYSTEM_PROMPT_HASH == CONTRACT["prompt"]["sha256"],
        }
        if pinned_license_hash:
            checks["license_text"] = license_text_sha256 == pinned_license_hash
        info["license_text_sha256"] = license_text_sha256
        tiers = tier_info()
        checks["tier"] = tiers["tier_error"] is None
        info.update(tiers)
        info["allowlist_checks"] = checks
        info["controlled_beta_ready"] = bool(info["available"] and all(checks.values()))
        info["readiness_reason"] = "exact allowlisted configuration" if info["controlled_beta_ready"] else (tiers["tier_error"] or "unsupported or drifted model configuration")
        return info
    except Exception as exc:
        return {"backend": "ollama", "backend_version": _ollama_version(), "available": False, "model": DEFAULT_MODEL, "controlled_beta_ready": False, "error": str(exc)}


def probe_openai_compat() -> dict[str, Any]:
    if not OPENAI_BASE:
        return {"backend": "openai_compat", "available": False, "controlled_beta_ready": False, "reason": "CASEFORGE_OPENAI_BASE not set"}
    try:
        data = http_json(f"{OPENAI_BASE}/models", timeout=3.0)
        models = [str(model.get("id")) for model in data.get("data", []) if model.get("id")]
        return {"backend": "openai_compat", "available": True, "controlled_beta_ready": False, "model": DEFAULT_MODEL if DEFAULT_MODEL in models else (models[0] if models else "local-model"), "models": models, "readiness_reason": "backend is not in the controlled-beta allowlist"}
    except Exception as exc:
        return {"backend": "openai_compat", "available": False, "controlled_beta_ready": False, "error": str(exc)}


def probe_llama_cpp() -> dict[str, Any]:
    available = bool(GGUF_PATH and Path(GGUF_PATH).is_file())
    return {"backend": "llama_cpp", "available": available, "controlled_beta_ready": False, "model": GGUF_PATH, "readiness_reason": "GGUF backend has not passed the controlled-beta evaluation"}


def resolve_backend() -> dict[str, Any]:
    probes = {"ollama": probe_ollama, "openai_compat": probe_openai_compat, "llama_cpp": probe_llama_cpp}
    if FORCED_BACKEND:
        probe = probes.get(FORCED_BACKEND)
        return probe() if probe else {"available": False, "controlled_beta_ready": False, "backend": FORCED_BACKEND, "error": "unknown backend"}
    for name in ("ollama", "openai_compat", "llama_cpp"):
        info = probes[name]()
        if info.get("available"):
            return info
    return {"available": False, "controlled_beta_ready": False, "backend": "none", "hint": "Install the exact allowlisted Ollama model configuration."}


def _mode_instruction(role: str) -> str:
    return {
        "counsel": "Mode: Counsel preparation. Strengthen the argument while identifying gaps.",
        "bench_prep": "Mode: Bench preparation. Ask hard, neutral judicial questions and demand source precision.",
        "adversary_drill": "Mode: Opposition drill. Press the weakest fact, evidence, law, and exposure links without conceding them.",
        "argument_builder": "Mode: Argument Builder. Give a relief root and 1-3 limbs, score Fact/Evidence/Law/Exposure, calculate VI, identify Missing Links below 60, and rank strengthening actions.",
    }.get(role, "Mode: Counsel preparation.")


def build_messages(params: dict[str, Any]) -> tuple[list[dict[str, str]], dict[str, Any]]:
    role, fmt = str(params.get("role") or "counsel").strip(), str(params.get("format") or "oral_argument").strip()
    case_context, authority_context = str(params.get("case_context") or "").strip(), str(params.get("authority_context") or "").strip()
    authority_ids = [str(value) for value in (params.get("authority_ids") or []) if str(value).strip()]
    support_sources = params.get("support_sources") or []
    structured = bool(params.get("structured_propositions"))
    mock_court_v2 = bool(params.get("mock_court_schema_v2"))
    working_set_version = str(params.get("working_set_version") or "").strip()
    user_text, history = str(params.get("user_text") or params.get("prompt") or "").strip(), params.get("messages") or []
    case_limit, authority_limit = int(GENERATION["raw_case_context_max_chars"]), int(GENERATION["authority_context_max_chars"])
    if len(case_context) > case_limit:
        raise ValueError(f"CF_LLM_CONTEXT_TOO_LONG: raw case context is {len(case_context)} chars; limit {case_limit}. Select a retrieval-backed matter working set; no text was silently truncated.")
    if len(authority_context) > authority_limit:
        raise ValueError(f"CF_LLM_AUTHORITY_CONTEXT_TOO_LONG: authority context is {len(authority_context)} chars; limit {authority_limit}. Narrow the working set; no text was silently truncated.")
    if len(user_text) > 8000:
        raise ValueError("CF_LLM_USER_TEXT_TOO_LONG: 8000 character limit; no text was silently truncated.")
    if not isinstance(history, list) or len(history) > 12:
        raise ValueError("CF_LLM_HISTORY_TOO_LONG: at most 12 prior turns; summarize explicitly before retrying.")
    if any(len(str((message or {}).get("content") or (message or {}).get("text") or "")) > 8000 for message in history if isinstance(message, dict)):
        raise ValueError("CF_LLM_HISTORY_TURN_TOO_LONG: 8000 character per-turn limit; no text was silently truncated.")
    system = SYSTEM_CORE + "\n\n" + _mode_instruction(role) + f"\nRequested format: {fmt}."
    system += f"\nMatter metadata: title={params.get('case_title') or 'n/a'}; jurisdiction={params.get('jurisdiction') or 'n/a'}; side={params.get('user_side') or 'n/a'}."
    if working_set_version:
        system += f"\nWorking-set version: {working_set_version}."
    if authority_ids:
        system += "\nAllowed authority identifiers: " + ", ".join(authority_ids) + "."
    if support_sources:
        if not isinstance(support_sources, list) or len(support_sources) > 40:
            raise ValueError("CF_LLM_SUPPORT_SOURCES: at most 40 structured support sources are allowed.")
        support_chars = sum(len(str(source.get("exact_text") or "")) for source in support_sources if isinstance(source, dict))
        if support_chars > case_limit + authority_limit:
            raise ValueError("CF_LLM_SUPPORT_CONTEXT_TOO_LONG: structured support text exceeds the combined 40000 character limit; no source was silently removed.")
        system += "\n\n<HOST_SUPPORT_SOURCES_UNTRUSTED_DATA>"
        for source in support_sources:
            if not isinstance(source, dict): continue
            system += f"\n<SUPPORT id={json.dumps(str(source.get('support_id') or ''))} kind={json.dumps(str(source.get('kind') or ''))}>\n{str(source.get('exact_text') or '')}\n</SUPPORT>"
        system += "\n</HOST_SUPPORT_SOURCES_UNTRUSTED_DATA>"
    if case_context:
        system += "\n\n<CASE_MATERIAL_UNTRUSTED>\n" + case_context + "\n</CASE_MATERIAL_UNTRUSTED>"
    if authority_context:
        system += "\n\n<RETRIEVED_AUTHORITIES_UNTRUSTED>\n" + authority_context + "\n</RETRIEVED_AUTHORITIES_UNTRUSTED>"
    if structured and not mock_court_v2:
        system += """

OUTPUT CONTRACT (caseforge.proposition-ledger.v1): Return one JSON object only, without Markdown fences or prose outside JSON.
Shape: {"propositions":[{"proposition_id":"p1","class":"FACTUAL_BASIS|LEGAL_BASIS|INFERENCE|COUNSEL_INPUT|UNSUPPORTED_OR_GAP","text":"one proposition","support_ids":["exact host support ID"],"basis_ids":["earlier proposition ID"],"support_claims":[{"support_id":"exact host support ID","exact_excerpt":"verbatim non-empty substring copied from that support"}],"quotation":null}]}
Rules: Split every important factual, legal, inferential, and counsel proposition. FACTUAL_BASIS and LEGAL_BASIS require support_ids plus verbatim support_claims. INFERENCE requires earlier basis_ids. If no permitted source supports a legal proposition, use UNSUPPORTED_OR_GAP. For a direct quotation, set quotation to its support_id and exact quoted text. Never invent IDs. Do not output verification status; only the host assigns trust.
"""
    if mock_court_v2:
        system += "\n\n" + MOCK_COURT_PROMPT
    messages: list[dict[str, str]] = [{"role": "system", "content": system}]
    for message in history:
        if not isinstance(message, dict): continue
        msg_role, content = message.get("role"), message.get("content") or message.get("text")
        if msg_role in ("user", "assistant") and content:
            messages.append({"role": str(msg_role), "content": str(content)})
    if user_text and (messages[-1].get("role") != "user" or messages[-1].get("content") != user_text):
        messages.append({"role": "user", "content": user_text})
    bounded_excerpt = "[CASEFORGE_BOUNDED_CASE_EXCERPT:" in case_context
    if bounded_excerpt and not working_set_version:
        raise ValueError("CF_LLM_WORKING_SET_REQUIRED: bounded case excerpt detected; select a retrieval-backed matter working set before generation.")
    return messages, {"case_context_chars": len(case_context), "authority_context_chars": len(authority_context), "structured_support_chars": sum(len(str(source.get("exact_text") or "")) for source in support_sources if isinstance(source, dict)), "authority_ids": authority_ids, "working_set_version": working_set_version or None, "context_truncated": bounded_excerpt, "proposition_schema_version": "caseforge.proposition-ledger.v1" if structured else None, "mock_court_prompt_version": MOCK_COURT_REGISTRY["version"] if mock_court_v2 else None, "mock_court_prompt_hash": f"sha256:{MOCK_COURT_PROMPT_HASH}" if mock_court_v2 else None}


# B-S4-28 best-of-n reasoning samples. Variant 0 is exactly the contract's settings. Variants 1-7 keep top_p and num_ctx
# and use a different seed and a higher temperature, so candidates differ while each stays reproducible.
VARIANT_TEMPERATURE = (None, 0.5, 0.7, 0.9, 0.6, 0.8, 0.4, 1.0)


def _variant(value: Any) -> int:
    try: return max(0, min(int(value or 0), 7))
    except (TypeError, ValueError): return 0


def _options(max_tokens: int, variant: int = 0) -> dict[str, Any]:
    variant = _variant(variant)
    temperature = float(GENERATION["temperature"]) if variant == 0 else VARIANT_TEMPERATURE[variant]
    return {"temperature": temperature, "top_p": float(GENERATION["top_p"]), "seed": int(GENERATION["seed"]) + 1000 * variant, "num_ctx": int(GENERATION["num_ctx"]), "num_predict": max_tokens}


def chat_ollama(model: str, messages: list[dict[str, str]], max_tokens: int, response_format: dict[str, Any] | None = None, variant: int = 0) -> dict[str, Any]:
    started = time.time()
    payload: dict[str, Any] = {"model": model, "messages": messages, "stream": False, "options": _options(max_tokens, variant)}
    if "think" in GENERATION: payload["think"] = bool(GENERATION["think"])
    if response_format is not None: payload["format"] = response_format
    data = http_json(f"{DEFAULT_OLLAMA}/api/chat", payload, timeout=300.0)
    eval_count, eval_duration = int(data.get("eval_count") or 0), int(data.get("eval_duration") or 0)
    return {"content": strip_reasoning(str((data.get("message") or {}).get("content") or "")), "model": data.get("model") or model, "backend": "ollama", "elapsed_ms": int((time.time() - started) * 1000), "done_reason": data.get("done_reason"), "eval_count": eval_count, "tokens_per_second": round(eval_count / (eval_duration / 1_000_000_000), 2) if eval_duration else None}


def strip_reasoning(content: str) -> str:
    """Hidden model reasoning is never product output: drop <think> blocks, including an unterminated one."""
    without_closed = re.sub(r"<think>.*?</think>", "", content, flags=re.S | re.I)
    return re.sub(r"<think>.*\Z", "", without_closed, flags=re.S | re.I).strip()


def chat_openai_compat(model: str, messages: list[dict[str, str]], max_tokens: int, variant: int = 0) -> dict[str, Any]:
    started = time.time()
    opts = _options(max_tokens, variant)
    data = http_json(f"{OPENAI_BASE}/chat/completions", {"model": model, "messages": messages, "temperature": opts["temperature"], "top_p": opts["top_p"], "seed": opts["seed"], "max_tokens": max_tokens, "stream": False}, timeout=300.0)
    choices = data.get("choices") or []
    return {"content": str(((choices[0].get("message") or {}).get("content")) if choices else "").strip(), "model": data.get("model") or model, "backend": "openai_compat", "elapsed_ms": int((time.time() - started) * 1000)}


_LLAMA_MODEL = None


def chat_llama_cpp(messages: list[dict[str, str]], max_tokens: int, variant: int = 0) -> dict[str, Any]:
    global _LLAMA_MODEL
    from llama_cpp import Llama  # type: ignore
    started = time.time()
    if _LLAMA_MODEL is None:
        _LLAMA_MODEL = Llama(model_path=GGUF_PATH, n_ctx=int(GENERATION["num_ctx"]), verbose=False)
    opts = _options(max_tokens, variant)
    data = _LLAMA_MODEL.create_chat_completion(messages=messages, temperature=opts["temperature"], top_p=opts["top_p"], seed=opts["seed"], max_tokens=max_tokens)
    choices = data.get("choices") or []
    return {"content": str(((choices[0].get("message") or {}).get("content")) if choices else "").strip(), "model": GGUF_PATH, "backend": "llama_cpp", "elapsed_ms": int((time.time() - started) * 1000)}


# --- B-S4-00 untrusted-citation guard (deterministic; no model involved) -------------------------------------------
# A citation that appears ONLY in untrusted matter material (case text, matter sources) and NOT in a permitted legal
# authority must never come out of the model as a citation: injected or planted authorities are neutralised in the raw
# output before anything is parsed. This enforces G2 ("no citation bypassing retrieval") at the model boundary; the host
# proposition verifier (B-S3-14) remains the trust authority for everything that is rendered.
GUARD_VERSION = "caseforge.untrusted-citation-guard.v1"
GUARD_LABEL = "[unverified citation from case material removed]"
PERMITTED_SOURCE_KINDS = {"LEGAL_AUTHORITY"}
TRUSTED_NON_AUTHORITY_KINDS = {"COUNSEL_INPUT"}  # counsel's own words are not scanned (and are never authority either)
_REPORTER_CITATION = re.compile(r"""(?ix)
      \(\d{4}\)\s*\d{1,3}\s*SCC\s+(?:OnLine\s+[A-Za-z.]+\s+)?\d{1,5}             # (2101) 3 SCC 77
    | \d{4}\s+SCC\s+OnLine\s+[A-Za-z.]+\s+\d{1,6}                                  # 2023 SCC OnLine Del 12
    | AIR\s+\d{4}\s+[A-Za-z.]{2,12}\s+\d{1,5}                                      # AIR 2105 SC 77
    | \d{4}\s+INSC\s+\d{1,5}                                                       # 2101 INSC 42
    | \[\d{4}\]\s*\d{0,3}\s*[A-Z][A-Za-z.]{1,10}\s+\d{1,5}                         # [2024] 1 SCR 5, [2024] UKSC 12
    | \d{4}\s+\(\d{1,3}\)\s+[A-Z][A-Za-z.]{1,12}\s+\d{1,5}                         # 2102 (4) Mh.L.J. 9
    | \d{1,4}\s+(?:U\.\s?S\.|S\.\s?Ct\.|L\.\s?Ed\.(?:\s?2d)?|F\.\s?(?:2d|3d|4th)|F\.\s?Supp\.(?:\s?(?:2d|3d))?)\s+\d{1,5}(?:\s*\(\d{4}\))?
""")
# Case name immediately before a reporter citation ("Doe v. Roe, 999 U.S. 123"); bare "X v Y" (e.g. the cause title) is NOT
# treated as a citation, so the matter's own parties are never touched.
_CASE_NAME_BEFORE = re.compile(r"([A-Z][\w'&-]*)\s+(?:v\.?|vs\.?|versus)\s+([A-Z][\w'&-]*(?:\s+(?:of|and|&)\s+[A-Z][\w'&-]*)*)[\s,]*$")
_SEP = r"[\s.,()\[\]]*"


def _norm(text: str) -> str:
    return re.sub(r"[^0-9a-z]+", "", text.lower())


def _flexible(tokens: list[str]) -> str:
    return _SEP.join(re.escape(t) for t in tokens if t)


def untrusted_citation_patterns(untrusted: list[str], permitted: list[str]) -> list[tuple[str, re.Pattern[str]]]:
    """Citations found in untrusted text that are absent from permitted authority text, as tolerant output matchers."""
    permitted_norm = _norm(" ".join(permitted))
    found: dict[str, re.Pattern[str]] = {}
    for text in untrusted:
        for m in _REPORTER_CITATION.finditer(text):
            cite = re.sub(r"\s*\(\d{4}\)\s*$", "", m.group(0)) if re.search(r"U\.\s?S\.|S\.\s?Ct\.|F\.|L\.\s?Ed", m.group(0)) else m.group(0)
            if _norm(cite) and _norm(cite) in permitted_norm:
                continue  # the same authority is in the counsel-confirmed working set: keep it
            found.setdefault(cite, re.compile(r"(?<![0-9A-Za-z])\(?" + _flexible(re.findall(r"[0-9A-Za-z]+", cite)) + r"\)?", re.I))
            name = _CASE_NAME_BEFORE.search(text[max(0, m.start() - 120):m.start()])
            if name:
                left, right = name.group(1), name.group(2)
                label = f"{left} v {right}"
                if _norm(label) in permitted_norm:
                    continue
                right_tokens = re.findall(r"[0-9A-Za-z'&-]+", right)
                found.setdefault(label, re.compile(
                    r"\b" + re.escape(left) + r"[\s.,]*(?:v\.?|vs\.?|versus)[\s.,]*" + re.escape(right_tokens[0])
                    + "".join(r"(?:\s+" + re.escape(t) + ")?" for t in right_tokens[1:]), re.I))
    return sorted(found.items(), key=lambda item: -len(item[0]))


def guard_untrusted_citations(content: str, params: dict[str, Any]) -> tuple[str, dict[str, Any]]:
    sources = [s for s in (params.get("support_sources") or []) if isinstance(s, dict)]
    untrusted = [str(params.get("case_context") or "")] + [str(s.get("exact_text") or "") for s in sources
                                                          if str(s.get("kind") or "") not in PERMITTED_SOURCE_KINDS | TRUSTED_NON_AUTHORITY_KINDS]
    permitted = [str(params.get("authority_context") or "")] + [str(s.get("exact_text") or "") for s in sources
                                                                if str(s.get("kind") or "") in PERMITTED_SOURCE_KINDS]
    patterns = untrusted_citation_patterns(untrusted, permitted)
    neutralised = 0
    for _, pattern in patterns:
        content, count = pattern.subn(GUARD_LABEL, content)
        neutralised += count
    return content, {"version": GUARD_VERSION, "untrusted_citations_detected": len(patterns), "occurrences_neutralised": neutralised}


def _provenance(info: dict[str, Any], messages: list[dict[str, str]], context: dict[str, Any], max_tokens: int, variant: int = 0) -> dict[str, Any]:
    prompt_hash = hashlib.sha256(json.dumps(messages, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()
    return {"model_digest": info.get("digest") or "unverified", "prompt_version": CONTRACT["prompt"]["version"], "system_prompt_hash": f"sha256:{SYSTEM_PROMPT_HASH}", "prompt_hash": f"sha256:{prompt_hash}", "generation": _options(max_tokens, variant), "sample_variant": _variant(variant), **context}


def _parse_propositions(content: str) -> list[dict[str, Any]]:
    candidate = content.strip()
    if candidate.startswith("```"):
        candidate = candidate.split("\n", 1)[1] if "\n" in candidate else candidate[3:]
        candidate = candidate.rsplit("```", 1)[0].strip()
    if not candidate.startswith("{"):
        start, end = candidate.find("{"), candidate.rfind("}")
        candidate = candidate[start:end + 1] if start >= 0 and end > start else ""
    try:
        parsed = json.loads(candidate)
    except (json.JSONDecodeError, TypeError):
        return []
    propositions = parsed.get("propositions") if isinstance(parsed, dict) else None
    if not isinstance(propositions, list): return []
    clean: list[dict[str, Any]] = []
    allowed = {"FACTUAL_BASIS", "LEGAL_BASIS", "INFERENCE", "COUNSEL_INPUT", "UNSUPPORTED_OR_GAP"}
    for index, prop in enumerate(propositions[:40]):
        if not isinstance(prop, dict) or str(prop.get("class") or "") not in allowed or not str(prop.get("text") or "").strip(): continue
        support_ids = [str(value) for value in (prop.get("support_ids") or [])]
        support_claims = [{"support_id": str(claim.get("support_id") or ""), "exact_excerpt": str(claim.get("exact_excerpt") or "")} for claim in (prop.get("support_claims") or []) if isinstance(claim, dict)]
        if not support_claims and str(prop["class"]) in {"FACTUAL_BASIS", "LEGAL_BASIS", "COUNSEL_INPUT"}:
            support_claims = [{"support_id": support_id, "exact_excerpt": str(prop["text"]).strip()} for support_id in support_ids]
        clean.append({"proposition_id": str(prop.get("proposition_id") or f"p{index + 1}"), "class": str(prop["class"]), "text": str(prop["text"]).strip(), "support_ids": support_ids, "basis_ids": [str(value) for value in (prop.get("basis_ids") or [])], "support_claims": support_claims, "quotation": prop.get("quotation") if isinstance(prop.get("quotation"), dict) else None})
    return clean


def _parse_mock_court_turn(content: str) -> dict[str, Any] | None:
    candidate = content.strip()
    if candidate.startswith("```"):
        candidate = candidate.split("\n", 1)[1] if "\n" in candidate else candidate[3:]
        candidate = candidate.rsplit("```", 1)[0].strip()
    if not candidate.startswith("{"):
        start, end = candidate.find("{"), candidate.rfind("}")
        candidate = candidate[start:end + 1] if start >= 0 and end > start else ""
    try: parsed = json.loads(candidate)
    except (json.JSONDecodeError, TypeError): return None
    if not isinstance(parsed, dict): return None
    propositions = _parse_propositions(candidate)
    def groups(name: str, maximum: int) -> list[dict[str, Any]] | None:
        raw = parsed.get(name)
        if not isinstance(raw, list) or len(raw) > maximum: return None
        clean = []
        for group in raw:
            ids = group.get("proposition_ids") if isinstance(group, dict) else None
            if not str((group or {}).get("id") or "").strip() or not isinstance(ids, list) or not ids or len(ids) > 8: return None
            clean.append({"id": str(group["id"]), "proposition_ids": [str(value) for value in ids]})
        return clean
    challenges, unanswered = groups("challenges", 6), groups("unanswered_questions", 6)
    focus = parsed.get("next_focus")
    focus_ids = focus.get("proposition_ids") if isinstance(focus, dict) else None
    if not challenges or unanswered is None or not propositions or not isinstance(focus_ids, list) or not focus_ids or len(focus_ids) > 8: return None
    return {"schema_version": "caseforge.mock-court-turn.v2", "challenges": challenges, "propositions": propositions, "unanswered_questions": unanswered, "next_focus": {"id": str(focus.get("id") or "focus"), "proposition_ids": [str(value) for value in focus_ids]}}


def handle(req: dict[str, Any]) -> dict[str, Any]:
    if req.get("jsonrpc") != "2.0": return err(req.get("id"), -32600, "Invalid Request: jsonrpc must be 2.0")
    method, params, rid = req.get("method"), req.get("params") or {}, req.get("id")
    if isinstance(params, list): params = params[0] if params else {}
    if method in ("llm.ping", "infer.ping"): return ok(rid, {"ok": True, "pong": "pong", "sidecar": "infer-llm", "batch": BATCH})
    if method in ("llm.version", "infer.version"): return ok(rid, {"product": "CaseForge", "sidecar": "infer-llm", "version": VERSION, "batch": BATCH, "scope": "controlled_beta_local_reasoner"})
    if method in ("llm.status", "infer.status"):
        info = resolve_backend()
        return ok(rid, {"ready": bool(info.get("available")), "controlled_beta_ready": bool(info.get("controlled_beta_ready")), "backend": info.get("backend"), "backend_version": info.get("backend_version") or "unknown", "model": info.get("model"), "model_digest": info.get("digest") or "unverified", "quantization": info.get("quantization") or "unknown", "parameters": info.get("parameters") or "unknown", "context_length": int(info.get("context_length") or 0), "classification": info.get("classification") or "unvalidated", "license": info.get("license") or "unknown", "legally_fine_tuned": bool(info.get("legally_fine_tuned")), "checkpoint_evidence": bool(info.get("checkpoint_evidence")), "prompt_version": CONTRACT["prompt"]["version"], "system_prompt_hash": f"sha256:{SYSTEM_PROMPT_HASH}", "mock_court_prompt_version": MOCK_COURT_REGISTRY["version"], "mock_court_prompt_hash": f"sha256:{MOCK_COURT_PROMPT_HASH}", "mock_court_prompt_valid": MOCK_COURT_PROMPT_VALID, "generation": GENERATION, "tier": TIER, "requested_tier": REQUESTED_TIER, "available_tiers": {name: c["model"]["name"] for name, c in CONTRACTS.items()}, "tier_error": info.get("tier_error"), "detail": info, "product_role": "Primary local reasoner / argument builder / Mock Court replier", "cloud": False})
    if method in ("llm.tiers", "infer.tiers"):
        return ok(rid, {"active": TIER, "tiers": [{"tier": name, "model": c["model"]["name"], "parameters": c["model"]["parameters"], "quantization": c["model"]["quantization"], "license": c["model"]["license"], "install_policy": c["model"].get("install_policy", "BASE_INSTALL" if name == "default" else "OPTIONAL_ADDON")} for name, c in CONTRACTS.items()], "auto_download": False})
    if method in ("llm.chat", "infer.chat", "llm.complete"):
        info = resolve_backend()
        if not info.get("available"): return err(rid, -32000, "No local LLM backend available", {"code": "CF_LLM_NOT_READY", "detail": info})
        if not info.get("controlled_beta_ready") and not ALLOW_UNVALIDATED: return err(rid, -32002, "Unsupported model configuration cannot claim controlled-beta readiness", {"code": "CF_LLM_CONFIG_NOT_ALLOWLISTED", "detail": info})
        if params.get("mock_court_schema_v2") and not MOCK_COURT_PROMPT_VALID: return err(rid, -32003, "Mock Court prompt registry hash mismatch", {"code": "CF_MOCK_COURT_PROMPT_DRIFT"})
        try: messages, context = build_messages(params)
        except ValueError as exc: return err(rid, -32602, str(exc), {"code": str(exc).split(":", 1)[0]})
        if len(messages) < 2: return err(rid, -32602, "user_text or messages required", {"code": "CF_LLM_BAD_PARAMS"})
        max_tokens = max(32, min(int(params.get("max_tokens") or GENERATION["num_predict_default"]), 1024))
        variant = _variant(params.get("variant"))
        try:
            backend, model = info.get("backend"), str(info.get("model") or DEFAULT_MODEL)
            if backend == "ollama": result = chat_ollama(model, messages, max_tokens, MOCK_COURT_TURN_SCHEMA if params.get("mock_court_schema_v2") else None, variant)
            elif backend == "openai_compat": result = chat_openai_compat(model, messages, max_tokens, variant)
            elif backend == "llama_cpp": result = chat_llama_cpp(messages, max_tokens, variant)
            else: return err(rid, -32000, f"Unsupported backend: {backend}")
        except Exception as exc: return err(rid, -32001, f"LLM generation failed: {exc}", {"code": "CF_LLM_GENERATE_FAILED"})
        if not result.get("content"): return err(rid, -32001, "Empty model response", {"code": "CF_LLM_EMPTY"})
        # Guard runs on the raw output BEFORE parsing, so raw text, propositions and Mock Court turns are all covered.
        result["content"], guard = guard_untrusted_citations(result["content"], params)
        context["untrusted_citation_guard"] = guard
        propositions = _parse_propositions(result["content"]) if params.get("structured_propositions") else []
        structured_turn = _parse_mock_court_turn(result["content"]) if params.get("mock_court_schema_v2") else None
        return ok(rid, {**result, "propositions": propositions, "mock_court_turn": structured_turn, "role": params.get("role") or "counsel", "format": params.get("format") or "oral_argument", "controlled_beta_ready": bool(info.get("controlled_beta_ready")), "provenance": _provenance(info, messages, context, max_tokens, variant), "disclaimer": "Local preparation output only. Not legal advice. Inspect every cited source before use."})
    if method == "llm.translate":
        # B-S4-15: one masked Hindi segment in, English prose out. A fixed translation prompt (its hash is recorded in contracts/translation_config.json),
        # greedy decoding, no matter context: the reasoner's legal prompt and case context are not involved.
        info = resolve_backend()
        if not info.get("available"): return err(rid, -32000, "No local LLM backend available", {"code": "CF_LLM_NOT_READY", "detail": info})
        text = str(params.get("text") or "")
        if not text.strip(): return err(rid, -32602, "text required", {"code": "CF_LLM_BAD_PARAMS"})
        cfg = json.loads((ROOT / "contracts" / "translation_config.json").read_text(encoding="utf-8"))
        prompt_bytes = (ROOT / cfg["prompt"]["file"]).read_bytes()
        if hashlib.sha256(prompt_bytes).hexdigest() != cfg["prompt"]["sha256"]: return err(rid, -32050, "translation prompt hash mismatch", {"code": "CF_TRANSLATE_PROMPT_CHECKSUM"})
        gen = cfg["generation"]
        max_tokens = max(32, min(int(gen["num_predict_max"]), int(len(text) * float(gen["num_predict_per_source_char"])) + 64))
        messages = [{"role": "system", "content": prompt_bytes.decode("utf-8")}, {"role": "user", "content": text}]
        model = str(info.get("model") or DEFAULT_MODEL)
        try:
            if info.get("backend") != "ollama": return err(rid, -32000, "translation currently needs the Ollama backend", {"code": "CF_TRANSLATE_BACKEND"})
            started = time.time()
            data = http_json(f"{DEFAULT_OLLAMA}/api/chat", {"model": model, "messages": messages, "stream": False, "options": {"temperature": float(gen["temperature"]), "top_p": float(gen["top_p"]), "seed": int(gen["seed"]), "num_ctx": int(gen["num_ctx"]), "num_predict": max_tokens}}, timeout=300.0)
        except Exception as exc: return err(rid, -32001, f"LLM generation failed: {exc}", {"code": "CF_LLM_GENERATE_FAILED"})
        content = strip_reasoning(str((data.get("message") or {}).get("content") or ""))
        if not content: return err(rid, -32001, "Empty model response", {"code": "CF_LLM_EMPTY"})
        return ok(rid, {"content": content, "model": data.get("model") or model, "backend": "ollama", "elapsed_ms": int((time.time() - started) * 1000), "done_reason": data.get("done_reason"), "prompt_sha256": cfg["prompt"]["sha256"], "eval_count": int(data.get("eval_count") or 0)})
    return err(rid, -32601, f"Method not found: {method}")


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv and argv[0] == "--once":
        raw = argv[1] if len(argv) > 1 else sys.stdin.read()
        try: request = json.loads(raw)
        except json.JSONDecodeError as exc: print(json.dumps(err(None, -32700, f"Parse error: {exc}"))); return 1
        print(json.dumps(handle(request), ensure_ascii=False)); return 0
    for line in sys.stdin:
        if not line.strip(): continue
        try: response = handle(json.loads(line))
        except json.JSONDecodeError as exc: response = err(None, -32700, f"Parse error: {exc}")
        print(json.dumps(response, ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__": raise SystemExit(main())
