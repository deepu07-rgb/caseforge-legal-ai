# B-S2-04 — Voice-in-court + structured debrief

**Status:** DONE (2026-08-08)  
**Pipeline:** `caseforge.voice_court.v1`

## Goal

Full-duplex **English** voice practice on Mock Court formats + structured debrief.  
Lexicon applies. Network off. **Lite tier refuses live simulation** with honesty notice.

## Duplex path

```
Counsel STT (stub/audio, English pin)
  → lexicon_apply_stt
  → mock_court turn (case_graph + retrieval only)
  → TTS role (bench | opposition | witness) + lexicon_apply_tts
  → structured debrief after multi-turn
```

## Gates

| Gate | Rule |
|------|------|
| Language | English only (`CF_SPEECH_LANGUAGE_PINNED`) |
| Network | `network_used=false`, `cloud=false` (AC-9.3) |
| AC-3.3 | Turn ≤ 15 s |
| Lite | `CF_VOICE_COURT_LITE_REFUSE` + honesty notice |
| AC-2.1 | No raw document parse |

## Debrief fields

strengths · weaknesses · missing_links_focus · recommended_actions · lexicon_hits · max_turn_ms · network_used_any

## IPC

| Command | Purpose |
|---------|---------|
| `voice_court_policy` | Frozen policy |
| `voice_court_gate` | Tier + Lite honesty |
| `voice_court_turn` | Offline duplex turn |
| `voice_court_turn_for_case` | Vault graph + speech + lexicon |
| `voice_court_run_script` | Multi-turn scripted |
| `voice_court_debrief` | Structured debrief |

## UI

Mock Court → **Voice duplex** tab · Speak turn · Debrief

## Eval

```powershell
npm run test:voicecourt
```
