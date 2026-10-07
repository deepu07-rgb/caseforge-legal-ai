/**
 * B-S1-16 Voice dictation + self-correcting lexicon (PRD §9).
 * STT → editable staging → commit; TTS roles; lexicon export/import.
 */

import { useCallback, useEffect, useState } from "react";
import type {
  LexiconEntry,
  SpeechStatus,
  SttResult,
  TtsResult,
} from "../lib/ipc";
import {
  isTauriRuntime,
  lexiconDelete,
  lexiconExport,
  lexiconImport,
  lexiconList,
  lexiconUpsert,
  speechStatus,
  speechStt,
  speechTts,
  speechTranscriptCommit,
  speechWarmup,
} from "../lib/ipc";
import { useMicrophoneCapture, type CapturedAudio } from "../lib/microphoneCapture";

const TTS_ROLES = ["bench", "opposition", "witness"] as const;

export default function VoiceView({
  vaultUnlocked,
  onError,
  onNotice,
}: {
  vaultUnlocked: boolean;
  onError: (msg: string) => void;
  onNotice: (msg: string) => void;
}) {
  const tauri = isTauriRuntime();
  const [status, setStatus] = useState<SpeechStatus | null>(null);
  const [stubInput, setStubInput] = useState(
    "the court shall hear the application under section one thirty eight",
  );
  const [rawTranscript, setRawTranscript] = useState("");
  const [staging, setStaging] = useState("");
  const [reviewConfirmed, setReviewConfirmed] = useState(false);
  const [lastStt, setLastStt] = useState<SttResult | null>(null);
  const [warming, setWarming] = useState(false);
  const [committed, setCommitted] = useState<string[]>([]);
  const [ttsText, setTtsText] = useState("Section 138 of the Negotiable Instruments Act.");
  const [ttsRole, setTtsRole] = useState<(typeof TTS_ROLES)[number]>("bench");
  const [ttsResult, setTtsResult] = useState<TtsResult | null>(null);
  const [entries, setEntries] = useState<LexiconEntry[]>([]);
  const [busy, setBusy] = useState(false);
  const [newSurface, setNewSurface] = useState("");
  const [newCorrected, setNewCorrected] = useState("");
  const [newKind, setNewKind] = useState<"stt_correction" | "tts_pronunciation_override">(
    "stt_correction",
  );
  const [newPron, setNewPron] = useState("");
  const [importJson, setImportJson] = useState("");
  const microphone = useMicrophoneCapture();

  const refreshStatus = useCallback(async () => {
    if (!tauri) return;
    try {
      setStatus(await speechStatus());
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    }
  }, [tauri, onError]);

  const refreshLexicon = useCallback(async () => {
    if (!tauri || !vaultUnlocked) {
      setEntries([]);
      return;
    }
    try {
      setEntries(await lexiconList(null));
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    }
  }, [tauri, vaultUnlocked, onError]);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  useEffect(() => {
    if (!tauri || !status?.production_ready) return;
    setWarming(true);
    void speechWarmup()
      .catch((e) => onError(e instanceof Error ? e.message : String(e)))
      .finally(() => setWarming(false));
  }, [tauri, status?.production_ready, onError]);

  useEffect(() => {
    void refreshLexicon();
  }, [refreshLexicon]);

  const runStt = async (capture?: CapturedAudio) => {
    if (!tauri) {
      onError("Voice requires the Tauri shell.");
      return;
    }
    setBusy(true);
    try {
      const r: SttResult = await speechStt({
        transcript_stub: capture ? null : stubInput,
        audio_base64: capture?.audio_base64 ?? null,
        audio_format: capture?.audio_format ?? null,
        language: "English",
      });
      if (r.network_used || r.cloud) {
        onError("AC-9.3 fail: network used on voice path");
        return;
      }
      setRawTranscript(r.raw_transcript);
      setStaging(r.staged_transcript);
      setLastStt(r);
      setReviewConfirmed(false);
      onNotice(
        r.lexicon_applied
          ? "STT complete — lexicon corrections applied to staging"
          : "STT complete — edit staging box, then commit",
      );
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const stopAndTranscribe = async () => {
    try {
      const capture = await microphone.stop();
      await runStt(capture);
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    }
  };

  const commitStaging = async () => {
    const t = staging.trim();
    if (!t || !reviewConfirmed) {
      onError("Review the transcript and tick the confirmation before committing it.");
      return;
    }
    if (!lastStt?.production_eligible || lastStt.input_kind !== "REAL_AUDIO" || !lastStt.audio_sha256) {
      onError("Only a confirmed real-audio transcript can be committed as counsel input.");
      return;
    }
    setBusy(true);
    try {
      await speechTranscriptCommit({
        input_kind: "REAL_AUDIO",
        audio_sha256: lastStt.audio_sha256,
        raw_transcript: lastStt.raw_transcript,
        staged_transcript: t,
        correction_applied: t !== lastStt.raw_transcript,
        counsel_confirmed: true,
        backend: lastStt.backend,
        pipeline_id: lastStt.pipeline_id,
      });
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
      setBusy(false);
      return;
    }
    setCommitted((prev) => [...prev, t]);
    onNotice(`Encrypted transcript commit ${committed.length + 1} saved; source audio was deleted.`);
    setRawTranscript("");
    setStaging("");
    setLastStt(null);
    setReviewConfirmed(false);
    setBusy(false);
  };

  const saveCorrectionFromStaging = async () => {
    if (!vaultUnlocked) {
      onError("Unlock vault to save lexicon corrections.");
      return;
    }
    const surface = rawTranscript.trim() || stubInput.trim();
    const corrected = staging.trim();
    if (!surface || !corrected || surface === corrected) {
      onError("Need different raw vs staging text to save an STT correction.");
      return;
    }
    setBusy(true);
    try {
      await lexiconUpsert({
        entry_kind: "stt_correction",
        surface_form: surface,
        corrected_form: corrected,
        pattern_type: "custom",
        source: "user",
      });
      await refreshLexicon();
      onNotice("STT correction saved permanently (AC-9.2)");
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const runTts = async () => {
    if (!tauri) return;
    setBusy(true);
    try {
      const r = await speechTts({
        text: ttsText,
        role: ttsRole,
        language: "English",
      });
      setTtsResult(r);
      onNotice(`TTS ${r.role}: offline audio ready`);
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const addEntry = async () => {
    if (!vaultUnlocked) {
      onError("Unlock vault first.");
      return;
    }
    setBusy(true);
    try {
      await lexiconUpsert({
        entry_kind: newKind,
        surface_form: newSurface,
        corrected_form: newKind === "stt_correction" ? newCorrected : null,
        pronunciation: newKind === "tts_pronunciation_override" ? newPron : null,
        pattern_type: "custom",
        source: "user",
      });
      setNewSurface("");
      setNewCorrected("");
      setNewPron("");
      await refreshLexicon();
      onNotice("Lexicon entry saved");
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const doExport = async () => {
    if (!vaultUnlocked) {
      onError("Unlock vault first.");
      return;
    }
    try {
      const env = await lexiconExport();
      const blob = new Blob([JSON.stringify(env, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `caseforge_lexicon_${env.entry_count}.cflex.json`;
      a.click();
      URL.revokeObjectURL(url);
      onNotice(`Exported ${env.entry_count} lexicon entries`);
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    }
  };

  const doImport = async () => {
    if (!vaultUnlocked) {
      onError("Unlock vault first.");
      return;
    }
    setBusy(true);
    try {
      const n = await lexiconImport(importJson);
      await refreshLexicon();
      onNotice(`Imported ${n} lexicon entries`);
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="page-grid voice-root" data-testid="voice-panel">
      <div className="matter-chips">
        <span className="chip matter">Voice · English primary</span>
        <span className="chip muted">B-S3-20</span>
        {status && (
          <>
            <span className={`chip ${status.production_ready ? "ok" : "warn"}`}>
              {status.production_ready ? "real audio ready" : "text-only fallback"}
            </span>
            <span className="chip muted">
              whisper {status.whisper_size} · {status.whisper_status}
            </span>
            <span className="chip muted">cloud={String(status.cloud)}</span>
          </>
        )}
        {!vaultUnlocked && <span className="chip warn">Vault locked — lexicon write disabled</span>}
      </div>

      <p className="muted g6-banner" data-testid="voice-g6">
        Microphone → local Whisper → editable staging → explicit counsel confirmation. Audio is
        deleted after transcription; voice stays offline and English-only.
      </p>

      <div className="digest-layout">
        <div className="card">
          <span className="eyebrow">STT · staging</span>
          <h3 className="digest-section-title">Dictation path</h3>
          <p className="muted" style={{ fontSize: "0.88rem" }}>
            CaseForge requests microphone permission only when you start recording. Stop to
            transcribe locally, review every word, then confirm before it becomes counsel input.
          </p>

          <div className={`notice ${microphone.state === "RECORDING" ? "warn" : ""}`} aria-live="polite">
            Microphone: {microphone.state.replaceAll("_", " ").toLowerCase()}
            {warming ? " · preparing local speech model" : ""}
            {microphone.error ? ` · ${microphone.error}` : ""}
          </div>

          <div className="row" style={{ marginTop: "0.75rem", gap: "0.4rem", flexWrap: "wrap" }}>
            <button
              type="button"
              className="primary"
              data-testid="mic-start"
              disabled={busy || !tauri || !status?.production_ready || microphone.state === "RECORDING"}
              onClick={() => void microphone.start()}
            >
              Start microphone
            </button>
            <button
              type="button"
              data-testid="mic-stop"
              disabled={busy || microphone.state !== "RECORDING"}
              onClick={() => void stopAndTranscribe()}
            >
              Stop and transcribe
            </button>
            <button
              type="button"
              disabled={microphone.state !== "RECORDING" && microphone.state !== "REQUESTING_PERMISSION"}
              onClick={microphone.cancel}
            >
              Cancel recording
            </button>
            <button type="button" disabled={busy} onClick={() => void refreshStatus()}>
              Refresh speech status
            </button>
          </div>

          {status?.developer_stub_enabled && (
            <details className="advanced-processing" style={{ marginTop: "0.75rem" }}>
              <summary>Developer transcript stub — excluded from readiness</summary>
              <textarea
                id="stt-stub"
                className="draft-editor"
                rows={3}
                data-testid="stt-stub"
                value={stubInput}
                onChange={(e) => setStubInput(e.target.value)}
                disabled={busy}
              />
              <button type="button" disabled={busy || !stubInput.trim()} onClick={() => void runStt()}>
                Run ineligible developer stub
              </button>
            </details>
          )}

          {!status?.production_ready && status && (
            <p className="notice warn">{status.detail} Voice controls remain disabled; use text input.</p>
          )}

          <label className="field-label" htmlFor="staging-box">
            Editable staging box
          </label>
          <textarea
            id="staging-box"
            className="draft-editor"
            rows={5}
            data-testid="staging-box"
            value={staging}
            onChange={(e) => {
              setStaging(e.target.value);
              setReviewConfirmed(false);
            }}
            placeholder="STT output appears here for edit before commit"
            disabled={busy}
          />
          {rawTranscript && (
            <p className="mono muted" style={{ fontSize: "0.72rem" }} data-testid="stt-raw">
              raw: {rawTranscript}
            </p>
          )}

          {lastStt && (
            <div className="mono muted" style={{ fontSize: "0.72rem" }} data-testid="speech-provenance">
              input={lastStt.input_kind} · production_eligible={String(lastStt.production_eligible)} ·
              audio_sha256={lastStt.audio_sha256 || "none"} · bytes={lastStt.audio_bytes} ·
              retained={String(lastStt.audio_retained)} · {lastStt.elapsed_ms} ms
            </div>
          )}

          <label className="check-row" style={{ marginTop: "0.65rem" }}>
            <input
              type="checkbox"
              checked={reviewConfirmed}
              onChange={(event) => setReviewConfirmed(event.target.checked)}
              disabled={!staging.trim()}
              data-testid="stt-reviewed"
            />
            I reviewed this transcript and confirm it as counsel input.
          </label>

          <div className="row" style={{ marginTop: "0.65rem", gap: "0.4rem", flexWrap: "wrap" }}>
            <button
              type="button"
              className="primary"
              data-testid="stt-commit"
              disabled={busy || !staging.trim() || !reviewConfirmed}
              onClick={() => void commitStaging()}
            >
              Commit staging
            </button>
            <button
              type="button"
              data-testid="stt-save-correction"
              disabled={busy || !vaultUnlocked}
              onClick={() => void saveCorrectionFromStaging()}
            >
              Save STT correction permanently
            </button>
          </div>

          {committed.length > 0 && (
            <div style={{ marginTop: "0.85rem" }} data-testid="committed-turns">
              <h4>Committed turns (voice-only rehearsal)</h4>
              <ol className="gap-list">
                {committed.map((c, i) => (
                  <li key={`${i}-${c.slice(0, 12)}`}>
                    <span className="chip ok">turn {i + 1}</span> {c}
                  </li>
                ))}
              </ol>
              <p className="mono muted" style={{ fontSize: "0.7rem" }}>
                turns={committed.length} · target for full rehearsal eval ≥ 10 (AC-9.3)
              </p>
            </div>
          )}
        </div>

        <div className="card">
          <span className="eyebrow">TTS · roles</span>
          <h3 className="digest-section-title">Role voices</h3>
          <p className="muted" style={{ fontSize: "0.88rem" }}>
            Presets: bench · opposition · witness. Pronunciation overrides from lexicon apply before
            TTS.
          </p>
          <label className="field-label" htmlFor="tts-role">
            Role
          </label>
          <select
            id="tts-role"
            data-testid="tts-role"
            value={ttsRole}
            onChange={(e) => setTtsRole(e.target.value as (typeof TTS_ROLES)[number])}
          >
            {TTS_ROLES.map((r) => (
              <option key={r} value={r}>
                {r}
              </option>
            ))}
          </select>
          <label className="field-label" htmlFor="tts-text">
            Text to speak
          </label>
          <textarea
            id="tts-text"
            className="draft-editor"
            rows={3}
            data-testid="tts-text"
            value={ttsText}
            onChange={(e) => setTtsText(e.target.value)}
          />
          <button
            type="button"
            className="primary"
            data-testid="tts-run"
            style={{ marginTop: "0.65rem" }}
            disabled={busy || !tauri}
            onClick={() => void runTts()}
          >
            Run TTS
          </button>
          {ttsResult && (
            <div style={{ marginTop: "0.65rem" }} data-testid="tts-result">
              {ttsResult.audio_base64 && (
                <audio controls src={`data:audio/${ttsResult.audio_format || "wav"};base64,${ttsResult.audio_base64}`}>
                  Offline speech audio is unavailable in this renderer.
                </audio>
              )}
              <div className="mono muted" style={{ fontSize: "0.75rem" }}>
                role={ttsResult.role} · backend={ttsResult.backend} · network_used={String(ttsResult.network_used)} · cloud={String(ttsResult.cloud)}
              </div>
            </div>
          )}
        </div>
      </div>

      <div className="card" data-testid="lexicon-panel">
        <span className="eyebrow">Lexicon · permanent</span>
        <h3 className="digest-section-title">Self-correcting lexicon</h3>
        <p className="muted" style={{ fontSize: "0.88rem" }}>
          STT corrections and TTS pronunciation overrides apply forever (AC-9.2). Pre-seeded with
          legal English, statute names, and proper-name patterns. Export/import{" "}
          <code>.cflex.json</code>.
        </p>

        <div className="row" style={{ gap: "0.4rem", flexWrap: "wrap", marginBottom: "0.75rem" }}>
          <button type="button" disabled={busy || !vaultUnlocked} onClick={() => void refreshLexicon()}>
            Refresh
          </button>
          <button type="button" data-testid="lexicon-export" disabled={!vaultUnlocked} onClick={() => void doExport()}>
            Export .cflex.json
          </button>
        </div>

        <div className="lexicon-add">
          <select
            value={newKind}
            onChange={(e) =>
              setNewKind(e.target.value as "stt_correction" | "tts_pronunciation_override")
            }
            data-testid="lexicon-kind"
          >
            <option value="stt_correction">STT correction</option>
            <option value="tts_pronunciation_override">TTS pronunciation</option>
          </select>
          <input
            placeholder="surface form"
            value={newSurface}
            onChange={(e) => setNewSurface(e.target.value)}
            data-testid="lexicon-surface"
          />
          {newKind === "stt_correction" ? (
            <input
              placeholder="corrected form"
              value={newCorrected}
              onChange={(e) => setNewCorrected(e.target.value)}
              data-testid="lexicon-corrected"
            />
          ) : (
            <input
              placeholder="pronunciation"
              value={newPron}
              onChange={(e) => setNewPron(e.target.value)}
              data-testid="lexicon-pron"
            />
          )}
          <button
            type="button"
            className="primary"
            data-testid="lexicon-add"
            disabled={busy || !vaultUnlocked || !newSurface.trim()}
            onClick={() => void addEntry()}
          >
            Add
          </button>
        </div>

        <ul className="gap-list lexicon-list" data-testid="lexicon-list">
          {entries.slice(0, 40).map((e) => (
            <li key={e.entry_id}>
              <span className={`chip ${e.active ? "ok" : "muted"}`}>{e.entry_kind}</span>{" "}
              <strong>{e.surface_form}</strong>
              {e.corrected_form && <> → {e.corrected_form}</>}
              {e.pronunciation && <> · say: {e.pronunciation}</>}
              <span className="mono muted" style={{ fontSize: "0.7rem" }}>
                {" "}
                · {e.pattern_type} · {e.source}
              </span>
              <button
                type="button"
                className="linkish"
                style={{ marginLeft: "0.5rem" }}
                disabled={!vaultUnlocked}
                onClick={() => {
                  const confirmed = window.confirm(
                    `Delete the local lexicon entry “${e.surface_form}”?\n\nThis does not change matter documents. Recreate the entry manually or import a saved lexicon to recover it.`,
                  );
                  if (!confirmed) return;
                  void lexiconDelete(e.entry_id)
                    .then(async () => {
                      await refreshLexicon();
                      onNotice("Lexicon entry deleted. Matter documents were not changed; it can be recreated or imported.");
                    })
                    .catch((err) => onError(err instanceof Error ? err.message : String(err)));
                }}
              >
                delete
              </button>
            </li>
          ))}
          {entries.length === 0 && (
            <li className="muted">
              {vaultUnlocked
                ? "No entries yet — unlock triggers pre-seed on first list."
                : "Unlock vault to load pre-seed lexicon."}
            </li>
          )}
        </ul>
        <p className="mono muted" style={{ fontSize: "0.7rem" }}>
          showing {Math.min(entries.length, 40)} / {entries.length}
        </p>

        <label className="field-label" htmlFor="lex-import">
          Import .cflex.json
        </label>
        <textarea
          id="lex-import"
          className="draft-editor"
          rows={4}
          data-testid="lexicon-import-json"
          value={importJson}
          onChange={(e) => setImportJson(e.target.value)}
          placeholder='{"schema_version":"1.0.0","kind":"lexicon_export","entries":[...]}'
        />
        <button
          type="button"
          data-testid="lexicon-import"
          style={{ marginTop: "0.5rem" }}
          disabled={busy || !vaultUnlocked || !importJson.trim()}
          onClick={() => void doImport()}
        >
          Import
        </button>
      </div>
    </div>
  );
}
