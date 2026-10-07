#!/usr/bin/env python3
"""CaseForge production offline English speech sidecar (B-S3-20).

STT uses a locally installed/cached Faster-Whisper model with network access disabled.
TTS uses Windows SAPI voices installed with the operating system. Audio is held only
in memory and a restricted temporary file, which is deleted in finally. Transcript
stubs require CASEFORGE_SPEECH_DEV_STUB=1 and are never production eligible.
"""

from __future__ import annotations

import base64
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import sys
import tempfile
import time
from typing import Any

VERSION = "2.0.0"
BATCH = "B-S3-20"
PIPELINE_ID = "caseforge.speech.offline-en.v2"
WHISPER_SIZE = os.environ.get("CASEFORGE_WHISPER_SIZE", "small").strip().lower() or "small"
PRIMARY_LANGUAGE = "English"
ALLOWED_LANGS = {"english", "en", "en-us", "en-gb", "en-in"}
TTS_ROLES = ("bench", "opposition", "witness")
CLOUD = False
DEV_STUB = os.environ.get("CASEFORGE_SPEECH_DEV_STUB", "").strip() == "1"
MAX_AUDIO_BYTES = 25 * 1024 * 1024
MAX_TTS_CHARS = 4000
LEGAL_PROMPT = (
    "Legal dictation in English. Preserve names, dates, monetary amounts, citations, "
    "section numbers, exhibit labels, and courtroom terminology exactly as spoken."
)
_STT_MODEL: Any = None


def ok(id_: Any, result: Any) -> dict[str, Any]:
    return {"jsonrpc": "2.0", "result": result, "id": id_}


def err(id_: Any, code: int, message: str, data: Any = None) -> dict[str, Any]:
    body: dict[str, Any] = {"code": code, "message": message}
    if data is not None:
        body["data"] = data
    return {"jsonrpc": "2.0", "error": body, "id": id_}


def pin_language(lang: str | None) -> str | None:
    if lang is None or not str(lang).strip():
        return None
    key = str(lang).strip().lower().replace("_", "-")
    if key in ALLOWED_LANGS or key.startswith("en-"):
        return None
    return f"CF_SPEECH_LANGUAGE_PINNED: English is required; got language={lang!r}"


def _resolve_stt_model() -> str | None:
    configured = os.environ.get("CASEFORGE_STT_MODEL_PATH", "").strip()
    if configured:
        path = Path(configured).expanduser().resolve()
        return str(path) if path.is_dir() else None
    if importlib.util.find_spec("faster_whisper") is None:
        return None
    try:
        from huggingface_hub import snapshot_download
        return snapshot_download(
            repo_id=f"Systran/faster-whisper-{WHISPER_SIZE}",
            local_files_only=True,
        )
    except Exception:
        return None


def _tts_available() -> bool:
    return platform.system() == "Windows" and importlib.util.find_spec("win32com.client") is not None


def status_result() -> dict[str, Any]:
    model_path = _resolve_stt_model()
    stt_ready = bool(model_path)
    tts_ready = _tts_available()
    production_ready = stt_ready and tts_ready and not DEV_STUB
    missing = []
    if not stt_ready:
        missing.append("local Faster-Whisper model/package")
    if not tts_ready:
        missing.append("offline Windows SAPI voice")
    return {
        "ready": production_ready,
        "production_ready": production_ready,
        "backend": "faster_whisper_local+sapi" if production_ready else "unavailable",
        "stt_backend": "faster_whisper_local" if stt_ready else "unavailable",
        "tts_backend": "windows_sapi" if tts_ready else "unavailable",
        "pipeline_id": PIPELINE_ID,
        "whisper_size": WHISPER_SIZE,
        "whisper_status": "PRODUCTION_LOCAL" if stt_ready else "MODEL_MISSING",
        "stt_model_path": model_path,
        "primary_language": PRIMARY_LANGUAGE,
        "language_pinning": True,
        "tts_roles": list(TTS_ROLES),
        "cloud": CLOUD,
        "network_required": False,
        "real_audio_required": True,
        "microphone_capture_required": True,
        "developer_stub_enabled": DEV_STUB,
        "retention_policy": "Audio is transient and deleted after local transcription; raw/staged text and correction lineage are committed separately.",
        "batch": BATCH,
        "version": VERSION,
        "detail": (
            "Production offline English speech ready; real microphone audio required."
            if production_ready
            else f"Text-only fallback: missing {', '.join(missing) or 'production mode disabled by developer stub flag'}."
        ),
        "product_role": "Production offline English STT/TTS",
    }


def _decode_audio(value: str) -> bytes:
    try:
        raw = base64.b64decode(value, validate=True)
    except Exception as exc:
        raise ValueError(f"CF_SPEECH_AUDIO_DECODE: {exc}") from exc
    if len(raw) < 256:
        raise ValueError("CF_SPEECH_AUDIO_TOO_SHORT: capture contains too little audio")
    if len(raw) > MAX_AUDIO_BYTES:
        raise ValueError(f"CF_SPEECH_AUDIO_TOO_LARGE: maximum is {MAX_AUDIO_BYTES} bytes")
    return raw


def _transcribe(raw_audio: bytes, audio_format: str) -> tuple[str, float | None, int]:
    global _STT_MODEL
    model_path = _resolve_stt_model()
    if not model_path:
        raise ValueError("CF_SPEECH_MODEL_MISSING: local Faster-Whisper is unavailable; use text-only fallback")
    from faster_whisper import WhisperModel

    suffix = "." + (audio_format if audio_format in {"wav", "webm", "ogg", "mp4", "m4a"} else "webm")
    temp_path = ""
    try:
        with tempfile.NamedTemporaryFile(prefix="caseforge-speech-", suffix=suffix, delete=False) as handle:
            handle.write(raw_audio)
            temp_path = handle.name
        started = time.perf_counter()
        if _STT_MODEL is None:
            _STT_MODEL = WhisperModel(model_path, device="cpu", compute_type="int8", local_files_only=True)
        segments, _ = _STT_MODEL.transcribe(
            temp_path,
            language="en",
            beam_size=5,
            vad_filter=True,
            condition_on_previous_text=True,
            initial_prompt=LEGAL_PROMPT,
        )
        collected = list(segments)
        transcript = " ".join(segment.text.strip() for segment in collected if segment.text.strip()).strip()
        if not transcript:
            raise ValueError("CF_SPEECH_EMPTY: no English speech was detected")
        weighted = [(segment.avg_logprob, max(0.01, segment.end - segment.start)) for segment in collected]
        confidence = None
        if weighted:
            confidence = max(0.0, min(1.0, math.exp(sum(v * w for v, w in weighted) / sum(w for _, w in weighted))))
        return transcript, confidence, int((time.perf_counter() - started) * 1000)
    finally:
        if temp_path:
            try:
                Path(temp_path).unlink(missing_ok=True)
            except OSError:
                pass


def stt(params: dict[str, Any]) -> dict[str, Any]:
    language_error = pin_language(params.get("language"))
    if language_error:
        raise ValueError(language_error)
    stub = str(params.get("transcript_stub") or "").strip()
    audio_b64 = str(params.get("audio_base64") or "").strip()
    audio_format = str(params.get("audio_format") or "webm").strip().lower()
    if stub and not DEV_STUB:
        raise ValueError("CF_SPEECH_REAL_AUDIO_REQUIRED: transcript stubs are disabled in production")
    if stub:
        return {
            "raw_transcript": stub, "language": PRIMARY_LANGUAGE, "language_pinned": True,
            "backend": "developer_stub", "pipeline_id": PIPELINE_ID, "whisper_size": WHISPER_SIZE,
            "cloud": False, "network_used": False, "elapsed_ms": 0, "confidence": None,
            "input_kind": "DEVELOPER_STUB", "production_eligible": False, "audio_sha256": None,
            "audio_bytes": 0, "audio_retained": False,
            "notes": "Explicit developer/test stub; excluded from readiness and G-AUDIO metrics.",
            "disclaimer": "Developer input only. Do not use as a controlled-test voice result.",
        }
    if not audio_b64:
        raise ValueError("CF_SPEECH_REAL_AUDIO_REQUIRED: capture microphone audio before transcription")
    raw_audio = _decode_audio(audio_b64)
    transcript, confidence, elapsed_ms = _transcribe(raw_audio, audio_format)
    return {
        "raw_transcript": transcript, "language": PRIMARY_LANGUAGE, "language_pinned": True,
        "backend": "faster_whisper_local", "pipeline_id": PIPELINE_ID, "whisper_size": WHISPER_SIZE,
        "cloud": False, "network_used": False, "elapsed_ms": elapsed_ms, "confidence": confidence,
        "input_kind": "REAL_AUDIO", "production_eligible": True,
        "audio_sha256": hashlib.sha256(raw_audio).hexdigest(), "audio_bytes": len(raw_audio),
        "audio_retained": False,
        "notes": "Local Faster-Whisper transcription; temporary audio deleted after inference.",
        "disclaimer": "Review and explicitly confirm the staged transcript before counsel input.",
    }


def _sapi_wave(text: str, role: str) -> bytes:
    if not _tts_available():
        raise ValueError("CF_SPEECH_TTS_UNAVAILABLE: offline Windows SAPI voice is unavailable")
    import win32com.client

    temp_path = ""
    try:
        with tempfile.NamedTemporaryFile(prefix="caseforge-tts-", suffix=".wav", delete=False) as handle:
            temp_path = handle.name
        speaker = win32com.client.Dispatch("SAPI.SpVoice")
        voices = speaker.GetVoices()
        speaker.Voice = voices.Item({"bench": 0, "opposition": 1, "witness": 2}[role] % max(1, voices.Count))
        speaker.Rate = {"bench": -1, "opposition": 1, "witness": 0}[role]
        stream = win32com.client.Dispatch("SAPI.SpFileStream")
        stream.Open(temp_path, 3, False)
        speaker.AudioOutputStream = stream
        speaker.Speak(text)
        stream.Close()
        return Path(temp_path).read_bytes()
    finally:
        if temp_path:
            try:
                Path(temp_path).unlink(missing_ok=True)
            except OSError:
                pass


def tts(params: dict[str, Any]) -> dict[str, Any]:
    language_error = pin_language(params.get("language"))
    if language_error:
        raise ValueError(language_error)
    text = str(params.get("text") or "").strip()
    spoken = str(params.get("spoken_text") or text).strip()
    if not text:
        raise ValueError("CF_SPEECH_TTS_EMPTY: text required")
    if len(spoken) > MAX_TTS_CHARS:
        raise ValueError(f"CF_SPEECH_TTS_TOO_LONG: maximum is {MAX_TTS_CHARS} characters")
    role = str(params.get("role") or "bench").strip().lower()
    if role not in TTS_ROLES:
        raise ValueError(f"CF_SPEECH_TTS_ROLE: role must be one of {list(TTS_ROLES)}")
    started = time.perf_counter()
    wave = _sapi_wave(spoken, role)
    return {
        "role": role, "text": text, "spoken_text": spoken,
        "audio_base64": base64.b64encode(wave).decode("ascii"), "audio_format": "wav",
        "backend": "windows_sapi", "pipeline_id": PIPELINE_ID, "cloud": False,
        "network_used": False, "elapsed_ms": int((time.perf_counter() - started) * 1000),
        "notes": "Offline operating-system voice; waveform returned in memory and not retained.",
        "disclaimer": "Synthetic preparation audio, not a court recording.",
    }


def warmup() -> dict[str, Any]:
    global _STT_MODEL
    model_path = _resolve_stt_model()
    if not model_path:
        raise ValueError("CF_SPEECH_MODEL_MISSING: local Faster-Whisper is unavailable")
    from faster_whisper import WhisperModel
    started = time.perf_counter()
    if _STT_MODEL is None:
        _STT_MODEL = WhisperModel(model_path, device="cpu", compute_type="int8", local_files_only=True)
    return {
        "ready": True,
        "backend": "faster_whisper_local",
        "elapsed_ms": int((time.perf_counter() - started) * 1000),
        "network_used": False,
    }


def handle(req: dict[str, Any]) -> dict[str, Any]:
    request_id = req.get("id")
    params = req.get("params") or {}
    if not isinstance(params, dict):
        params = {}
    try:
        method = req.get("method")
        if method == "speech.ping":
            return ok(request_id, {"ok": True, "sidecar": "infer-speech", "batch": BATCH, "cloud": False})
        if method == "speech.status":
            return ok(request_id, status_result())
        if method == "speech.warmup":
            return ok(request_id, warmup())
        if method == "speech.stt":
            return ok(request_id, stt(params))
        if method == "speech.tts":
            return ok(request_id, tts(params))
        return err(request_id, -32601, f"Method not found: {method}")
    except ValueError as exc:
        return err(request_id, -32000, str(exc))
    except Exception as exc:
        return err(request_id, -32000, f"CF_SPEECH_INTERNAL: {exc}")


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if args and args[0] == "--once":
        raw = args[1] if len(args) > 1 else sys.stdin.read()
        print(json.dumps(handle(json.loads(raw)), ensure_ascii=False), flush=True)
        return 0
    for line in sys.stdin:
        if not line.strip():
            continue
        try:
            response = handle(json.loads(line))
        except json.JSONDecodeError as exc:
            response = err(None, -32700, f"Parse error: {exc}")
        print(json.dumps(response, ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
