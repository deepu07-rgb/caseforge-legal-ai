# infer-speech — B-S3-20

Production offline English speech worker for CaseForge.

| Method | Purpose |
|---|---|
| `speech.ping` | Offline worker health |
| `speech.status` | Production readiness, local adapters, retention policy |
| `speech.warmup` | Load the local STT model once in the persistent worker |
| `speech.stt` | Transcribe real microphone audio with local Faster-Whisper Small |
| `speech.tts` | Generate an offline Windows SAPI WAV for bench/opposition/witness |

The default path requires `audio_base64`; it never downloads a model and never calls a network endpoint. `CASEFORGE_STT_MODEL_PATH` may point to an installer-bundled Faster-Whisper directory. Otherwise the worker uses an already cached `Systran/faster-whisper-small` snapshot with `local_files_only=True`.

Transcript stubs are rejected unless `CASEFORGE_SPEECH_DEV_STUB=1`. That flag makes `production_ready=false`, and every stub result is marked `production_eligible=false`.

Captured audio is capped at 25 MiB, written to a temporary file only for decoding, hashed for provenance, and deleted in `finally`. The host retains the raw/staged transcript and correction lineage according to the speech contract, not the waveform.

Whisper source code and model weights are MIT licensed. No third-party voice weights are bundled: the Windows build uses locally installed operating-system voices.
