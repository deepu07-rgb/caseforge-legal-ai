# B-S1-16 — Voice dictation + self-correcting lexicon

**Status:** DONE (2026-08-08)

## Goal

English voice dictation: **Whisper STT → editable staging box → commit**, plus **TTS role presets** and a **permanent, exportable lexicon** (PRD §9).

## Freezes

| Item | Value |
|------|--------|
| Pipeline | `caseforge.speech.provisional.v1` |
| Whisper size (PRD §21 #2) | **small** — `PROVISIONAL_STAGE1` |
| Language | **English** primary (AC-9.4 pin) |
| Network | **disabled** for voice eval (AC-9.3) |
| Lexicon schema | `lexicon_entry.schema.json` v1.0.0 (Q-LX-01) |
| Export | `.cflex.json` envelope |
| TTS roles | `bench` · `opposition` · `witness` |

## AC gates

| AC | Result for this batch |
|----|------------------------|
| **AC-9.1** WER | **SKIP** numeric until `G-AUDIO` fixture is `GOLDEN` with `wer_target` (PRD has no Layer-1 WER number). STT pipeline wiring PASS. |
| **AC-9.2** | Lexicon STT/TTS corrections persist in encrypted app vault and re-apply forever. |
| **AC-9.3** | Offline voice-only path: sidecar `cloud:false`, `network_used:false`; 10-turn dictation smoke. |
| **AC-9.4** | Non-English `language` rejected (`CF_SPEECH_LANGUAGE_PINNED`). |

## Architecture

```
UI staging box  →  speech_stt (infer-speech)  →  lexicon_apply_stt (app vault)
                →  user edits staging
                →  commit (in-session turns) / save correction permanently

TTS text        →  lexicon_apply_tts  →  speech_tts (role preset)
```

| Layer | Path |
|-------|------|
| Sidecar | `sidecars/infer_speech/infer_speech.py` |
| Host | `src-tauri/src/infer_speech/mod.rs` |
| Vault | `app_003_lexicon.sql` + `VaultManager::lexicon_*` |
| Pre-seed | `contracts/voice_preseed_lexicon.json` |
| UI | **Voice** nav → `VoiceView` |

## Provisional honesty

- Default STT input is **transcript stub** (offline, no invented WER).
- Full whisper.cpp audio path can attach via `CASEFORGE_WHISPER_CMD` later; size remains **small** until G-AUDIO GOLDEN re-opens the decision.
- Provisional TTS returns **spoken_text** (pronunciation-applied); waveform may be null.

## Eval

```powershell
npm run test:voice
python sidecars/infer_speech/infer_speech.py --once "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"speech.status\",\"params\":{}}"
```

## Out of scope

- Full voice Mock Court (Stage 2 / B-S2-04)
- Non-English primary
- Firm shared-lexicon merge (PRD §21 #5)
- Invented numeric WER threshold without GOLDEN audio
