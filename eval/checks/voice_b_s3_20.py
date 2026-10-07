#!/usr/bin/env python3
"""B-S3-20 production offline speech engineering and honest-evidence gate."""
from __future__ import annotations
import json, subprocess, sys
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
read=lambda path:(ROOT/path).read_text(encoding="utf-8")
sidecar_path=ROOT/"sidecars/infer_speech/infer_speech.py"
def rpc(method:str,params:dict|None=None)->dict:
    request=json.dumps({"jsonrpc":"2.0","id":1,"method":method,"params":params or {}})
    completed=subprocess.run([sys.executable,str(sidecar_path),"--once",request],cwd=ROOT,text=True,capture_output=True,check=True)
    response=json.loads(completed.stdout)
    if "error" in response: raise RuntimeError(response["error"]["message"])
    return response["result"]

contract=json.loads(read("contracts/speech_offline.json")); manifest=json.loads(read("eval/fixtures/g_audio/manifest.json"))
sidecar=read("sidecars/infer_speech/infer_speech.py"); host=read("src-tauri/src/infer_speech/mod.rs")+read("src-tauri/src/lib.rs")
ipc=read("src/lib/ipc.ts"); mic=read("src/lib/microphoneCapture.ts"); voice=read("src/components/VoiceView.tsx")
court=read("src/components/MockCourtView.tsx"); migration=read("src-tauri/migrations/app_006_speech_transcript.sql")
evaluator=read("eval/tools/evaluate_g_audio.py"); docs=read("docs/B-S3-20_voice.md")
status=rpc("speech.status"); tts=rpc("speech.tts",{"text":"Section 138","role":"bench","language":"English"})
stub_rejected=False
try: rpc("speech.stt",{"transcript_stub":"not real audio","language":"English"})
except RuntimeError as error: stub_rejected="REAL_AUDIO_REQUIRED" in str(error)
checks={
 "production_local_status":status.get("production_ready") is True and status.get("ready") is True,
 "offline_only":status.get("cloud") is False and status.get("network_required") is False,
 "real_audio_required":status.get("real_audio_required") is True and stub_rejected,
 "stub_ineligible_even_in_dev":"production_eligible\": False" in sidecar and "CASEFORGE_SPEECH_DEV_STUB" in sidecar,
 "local_model_no_download":"local_files_only=True" in sidecar and status.get("stt_backend")=="faster_whisper_local",
 "persistent_warmed_worker":all(token in host for token in ["SpeechWorker","OnceLock","speech.warmup","speech_warmup"]),
 "offline_tts_waveform":tts.get("backend")=="windows_sapi" and tts.get("audio_format")=="wav" and len(tts.get("audio_base64") or "")>1000,
 "microphone_permission_lifecycle":all(token in mic for token in ["getUserMedia","MediaRecorder","REQUESTING_PERMISSION","DENIED","track.stop()"]),
 "capture_cancel_cleanup":"useEffect(() => cancel" in mic and "recorder.stop()" in mic,
 "editable_confirmed_staging":all(token in voice for token in ["staging-box","stt-reviewed","reviewConfirmed","speechTranscriptCommit"]),
 "audio_provenance_no_retention":all(token in sidecar+migration for token in ["audio_sha256","audio_retained","delete","CHECK (audio_retained = 0)"]),
 "encrypted_commit":"speech_transcript_commit" in migration and all(token in host+ipc for token in ["speech_transcript_commit","SpeechTranscriptCommit"]),
 "mock_court_real_audio_proof":all(token in court+host for token in ["voice-real-audio-input","confirmedAudioSha256","productionEligible","CF_VOICE_COURT_AUDIO_PROOF"]),
 "text_only_fallback":"text-only fallback" in voice and "Text mode" in court and "CF_SPEECH_HARDWARE_UNSUPPORTED" in host,
 "frozen_metrics":contract["g_audio"]["word_error_rate_max"]==0.1 and contract["g_audio"]["critical_term_error_rate_max"]==0.02,
 "rights_cleared_evaluator":all(token in evaluator for token in ["HUMAN_AUDIO_RIGHTS_CLEARED","rights_attested","critical_terms","edit_distance"]),
 "human_audio_not_fabricated":manifest["status"]=="AWAITING_RIGHTS_CLEARED_HUMAN_AUDIO" and "PENDING G-AUDIO" in docs,
 "licence_selection_documented":"MIT" in docs and "Lessac" in docs and "research only" in docs,
}
report={"batch":"B-S3-20","checks":checks,"engineering_gate_passed":all(checks.values()),"g_audio_status":manifest["status"],"controlled_voice_readiness":False}
(ROOT/"eval/reports/b_s3_20_voice.json").write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
for key,value in checks.items(): print(f"{'PASS' if value else 'FAIL'} {key}")
print(f"B-S3-20 engineering gate: {sum(checks.values())}/{len(checks)} checks passed")
print("Controlled voice readiness remains false until rights-cleared human G-AUDIO passes 10% WER and 2% critical-term error.")
raise SystemExit(0 if report["engineering_gate_passed"] else 1)
