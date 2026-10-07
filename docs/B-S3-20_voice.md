# B-S3-20 — Production offline English speech

## Status

ENGINEERING IMPLEMENTED on 2026-08-21. PENDING G-AUDIO human-audio measurement. Voice is not marked controlled-test ready until rights-cleared human recordings pass the frozen WER and critical-term gates.

## Production path

The Windows development machine uses the locally cached `Systran/faster-whisper-small` adapter with `local_files_only=True`. The persistent host worker loads it once and retains the model between turns. The app never downloads speech assets while running and never uses a cloud endpoint.

Microphone capture is user-initiated. The interface exposes unavailable, idle, permission-request, recording, processing, denied, and error states. Cancel and component cleanup stop every media track. The capture is capped at 25 MiB, hashed, placed in a temporary decoder file, and deleted in `finally`.

The raw transcript and lexicon-adjusted staging text stay distinct. Editing clears confirmation. Counsel must tick an explicit review confirmation before commit. Confirmed transcript text, correction lineage, local backend/pipeline identifiers, and the audio SHA-256 are stored in the encrypted app vault; source audio is never stored. Lexicon correction changes staging only and does not modify quoted evidence or source documents.

Mock Court uses the same real-audio capture and staging flow. The desktop host rejects a “voice” turn unless it carries a production-eligible result and a valid audio SHA-256. The confirmed record is encrypted before the grounded turn proceeds. Text mode remains available on unsupported hardware.

Offline TTS uses Windows SAPI voices already installed with the operating system and returns an in-memory WAV for the bench, opposition, and witness role presets. No third-party TTS weights are bundled.

## Licence decision

OpenAI publishes Whisper code and model weights under MIT (`https://github.com/openai/whisper`), and the Faster-Whisper adapter consumes a local converted Small model. The runtime records this model choice in `contracts/speech_offline.json`.

The proposed Piper Lessac voice was rejected. Although its hosting repository is labelled MIT, its model card (`https://huggingface.co/rhasspy/piper-voices/raw/main/en/en_US/lessac/medium/MODEL_CARD`) points to underlying Lessac/Voice Factory material (`https://www.cstr.ed.ac.uk/projects/blizzard/2013/lessac_blizzard2013/license.html`) licensed for research only and excluding commercial speech products. CaseForge therefore does not download or redistribute that voice. Windows SAPI avoids bundling those restricted weights.

## Frozen G-AUDIO gate

`eval/fixtures/g_audio/manifest.json` deliberately contains no invented participant recordings. To close the gate:

1. Recruit at least five English speakers and record at least twenty clips through the app microphone on supported hardware.
2. Obtain explicit rights consent for controlled product evaluation and set `rights_attested: true` per clip.
3. Include labelled legal names, citations, amounts, provision numbers, and courtroom terms in every relevant reference transcript.
4. Mark the manifest `GOLDEN`, then run `python eval/tools/evaluate_g_audio.py`.
5. Required aggregate WER is at most 10%. Required labelled critical-term error is at most 2%.
6. Retain the generated latency/clip report and Stage 3 metric result files. Any failing metric keeps voice text-only for controlled testing.

Run `npm run test:s3:20` for the engineering gate. It verifies real-audio enforcement, offline model/TTS readiness, microphone cleanup, explicit staging confirmation, encrypted provenance, Mock Court proof, frozen metrics, and the honest pending human-audio state.
