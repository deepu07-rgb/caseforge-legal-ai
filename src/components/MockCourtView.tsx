/**
 * Mock Court — five text formats (B-S2-02 / PRD §2).
 * Adversary path: mockCourtTurnForCase → case_graph + retrieval only (AC-2.1).
 */

import { useEffect, useMemo, useRef, useState } from "react";
import { AnswerFeedbackPanel } from "./AnswerFeedbackPanel";
import {
  COURT_FORMATS,
  COURT_ROLES,
  coachReply,
  roleToMockCourtRole,
  welcomeMessage,
  type ChatMessage,
  type CourtFormat,
  type CourtRole,
} from "../lib/mockCourtCoach";
import {
  adaptiveBenchArchetypes,
  adaptiveBenchResolveCase,
  benchPrepPolicy,
  benchPrepRedteam,
  benchPrepTurnForCase,
  mockCourtPolicy,
  mockCourtRetrievalReadiness,
  mockCourtTurnForCase,
  voiceCourtGate,
  voiceCourtTurnForCase,
  type ArchetypeMeta,
  type BenchProfile,
  type BenchSliderSet,
  type CaseSummary,
  mockCourtDebriefV2Generate,
  preparationTasksList,
  preparationTaskUpdate,
  speechStatus,
  speechStt,
  speechWarmup,
  type DebriefTurnInput,
  type AnswerFeedback,
  type MockCourtDebriefV2,
  type PreparationTask,
  type PreparationTaskStatus,
  type LlmStatus,
  type MockCourtAuthority,
  type MockCourtReadiness,
  type PrepSessionReport,
  type SpeechStatus,
  type SttResult,
  type TtsResult,
  type PropositionLedger,
  type VoiceGate,
} from "../lib/ipc";
import { verifiedPropositionProvenance, type ProvenanceRecord } from "../lib/provenance";
import ProvenanceInspector from "./ProvenanceInspector";
import { useMicrophoneCapture } from "../lib/microphoneCapture";

type Props = {
  cases: CaseSummary[];
  activeCase: CaseSummary | null;
  llm: LlmStatus | null;
  onOpenCase: (caseId: string) => void;
  onRefreshLlm: () => void;
  initialInsightTab?: "bench" | "debrief";
};

const ROLE_TO_MODE: Record<CourtRole, string> = {
  counsel: "Argue",
  bench_prep: "Bench prep",
  adversary_drill: "Adversary drill",
};

const DEFAULT_SLIDERS: BenchSliderSet = {
  tempo: 50,
  formality: 50,
  opposition_aggression: 50,
  technicality: 50,
  interruption: 50,
};

const SLIDER_LABELS: { key: keyof BenchSliderSet; label: string }[] = [
  { key: "tempo", label: "Tempo" },
  { key: "formality", label: "Formality" },
  { key: "opposition_aggression", label: "Opposition aggression" },
  { key: "technicality", label: "Technicality" },
  { key: "interruption", label: "Interruption" },
];

export default function MockCourtView({
  cases,
  activeCase,
  llm,
  onOpenCase,
  onRefreshLlm,
  initialInsightTab = "bench",
}: Props) {
  const modelReady = Boolean(llm?.controlled_beta_ready);
  const [role, setRole] = useState<CourtRole>("adversary_drill");
  const [format, setFormat] = useState<CourtFormat>("Bail_Interim_Motion");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [lastMeta, setLastMeta] = useState<string | null>(null);
  const [policyNote, setPolicyNote] = useState<string | null>(null);
  const [insightTab, setInsightTab] = useState<"insights" | "weak" | "auth" | "bench" | "debrief">(
    "bench",
  );
  const [archetypes, setArchetypes] = useState<ArchetypeMeta[]>([]);
  const [archetypeId, setArchetypeId] = useState<string>("procedure_first");
  const [sliders, setSliders] = useState<BenchSliderSet>(DEFAULT_SLIDERS);
  const [benchProfile, setBenchProfile] = useState<BenchProfile | null>(null);
  const [inputMode, setInputMode] = useState<"text" | "voice">("text");
  const [voiceGate, setVoiceGate] = useState<VoiceGate | null>(null);
  const [debriefV2, setDebriefV2] = useState<MockCourtDebriefV2 | null>(null);
  const [sessionTurns, setSessionTurns] = useState<DebriefTurnInput[]>([]);
  // B-S4-00b: answer check per counsel message id.
  const [answerFeedback, setAnswerFeedback] = useState<Record<string, AnswerFeedback>>({});
  const [preparationTasks, setPreparationTasks] = useState<PreparationTask[]>([]);
  const [taskDrafts, setTaskDrafts] = useState<Record<string,string>>({});
  const [voiceRaw, setVoiceRaw] = useState("");
  const [voiceStaging, setVoiceStaging] = useState("");
  const [voiceReviewed, setVoiceReviewed] = useState(false);
  const [voiceSttResult, setVoiceSttResult] = useState<SttResult | null>(null);
  const [speech, setSpeech] = useState<SpeechStatus | null>(null);
  const [lastVoiceAudio, setLastVoiceAudio] = useState<TtsResult | null>(null);
  const [voiceTurnIndex, setVoiceTurnIndex] = useState(1);
  const [prepSide, setPrepSide] = useState<"side_a" | "side_b">("side_a");
  const [prepNotice, setPrepNotice] = useState<string | null>(null);
  const [redteamReport, setRedteamReport] = useState<PrepSessionReport | null>(null);
  const [retrievalReadiness, setRetrievalReadiness] = useState<MockCourtReadiness | null>(null);
  const [retrievalLoading, setRetrievalLoading] = useState(false);
  const [turnAuthorities, setTurnAuthorities] = useState<Array<{ turnId: string; authorities: MockCourtAuthority[]; ledger: PropositionLedger | null }>>([]);
  const microphone = useMicrophoneCapture();
  const logRef = useRef<HTMLDivElement | null>(null);
  const sessionId = useMemo(
    () => `ui-${format}-${activeCase?.case_id || "none"}-${Date.now()}`,
    // reset session identity when format/case changes
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [format, activeCase?.case_id],
  );
  const startedAt = useMemo(() => Date.now(), [role, format, activeCase?.case_id]);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    const t = window.setInterval(() => setTick((n) => n + 1), 1000);
    return () => window.clearInterval(t);
  }, []);

  useEffect(() => {
    setSessionTurns([]);
    setDebriefV2(null);
    setTurnAuthorities([]);
  }, [sessionId]);

  useEffect(() => {
    setInsightTab(initialInsightTab);
  }, [initialInsightTab]);

  useEffect(() => {
    if (!activeCase) { setPreparationTasks([]); return; }
    void preparationTasksList(activeCase.case_id).then(setPreparationTasks).catch(() => setPreparationTasks([]));
  }, [activeCase?.case_id]);

  useEffect(() => {
    void mockCourtPolicy()
      .then((p) => {
        setPolicyNote(
          `${String(p.pipeline_id)} · text_only · turn≤${String(p.turn_max_s)}s · Adaptive Bench · no named judges`,
        );
      })
      .catch(() => setPolicyNote("caseforge.mock_court.v1"));
    void adaptiveBenchArchetypes()
      .then((list) => {
        setArchetypes(list);
        if (list[0]) {
          setArchetypeId(list[0].id);
          setSliders(list[0].baseline);
        }
      })
      .catch(() => setArchetypes([]));
    void voiceCourtGate(null)
      .then(setVoiceGate)
      .catch(() => setVoiceGate(null));
    void speechStatus()
      .then((value) => {
        setSpeech(value);
        if (value.production_ready) void speechWarmup();
      })
      .catch(() => setSpeech(null));
    void benchPrepPolicy()
      .then((p) => setPrepNotice(String(p.non_decision_maker_notice || "")))
      .catch(() =>
        setPrepNotice(
          "Bench Preparation Mode: AI plays both counsel only. Never the decision-maker.",
        ),
      );
  }, []);

  useEffect(() => {
    if (!activeCase) {
      setBenchProfile(null);
      setRetrievalReadiness(null);
      return;
    }
    void adaptiveBenchResolveCase({
      caseId: activeCase.case_id,
      archetypeId,
      sliders,
    })
      .then(setBenchProfile)
      .catch(() => setBenchProfile(null));
    setRetrievalLoading(true);
    void mockCourtRetrievalReadiness(activeCase.case_id)
      .then(setRetrievalReadiness)
      .catch(() => setRetrievalReadiness(null))
      .finally(() => setRetrievalLoading(false));
  }, [activeCase?.case_id, archetypeId, sliders]);

  const elapsed = useMemo(() => {
    void tick;
    const s = Math.floor((Date.now() - startedAt) / 1000);
    const mm = String(Math.floor(s / 60)).padStart(2, "0");
    const ss = String(s % 60).padStart(2, "0");
    return `${mm}:${ss}`;
  }, [startedAt, tick]);

  useEffect(() => {
    setMessages([
      welcomeMessage(role, format, modelReady),
      {
        id: "hint-1",
        role: "assistant",
        speaker:
          role === "bench_prep"
            ? "Bench (prep)"
            : role === "adversary_drill"
              ? "Adversary (drill)"
              : "Prep coach",
        text: activeCase
          ? `Matter: “${activeCase.cause_title || activeCase.label}” · ${activeCase.jurisdiction}. Stage 2 uses case_graph only (no raw PDF). Format: ${COURT_FORMATS.find((f) => f.id === format)?.label}.`
          : "No case open — open a vault matter so adversary turns can load case_graph (AC-2.1).",
      },
    ]);
    setDraft("");
    setLastMeta(null);
    setTurnAuthorities([]);
  }, [role, format, activeCase?.case_id, modelReady, llm?.model, llm?.backend]);

  useEffect(() => {
    const el = logRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages, busy]);

  const formatMeta = useMemo(
    () => COURT_FORMATS.find((f) => f.id === format),
    [format],
  );

  const stopCourtRecording = async () => {
    try {
      const capture = await microphone.stop();
      setBusy(true);
      const result = await speechStt({
        audio_base64: capture.audio_base64,
        audio_format: capture.audio_format,
        transcript_stub: null,
        language: "English",
      });
      if (!result.production_eligible || result.input_kind !== "REAL_AUDIO") {
        throw new Error("Mock Court voice requires production-eligible real audio.");
      }
      setVoiceRaw(result.raw_transcript);
      setVoiceStaging(result.staged_transcript);
      setVoiceSttResult(result);
      setVoiceReviewed(false);
    } catch (e) {
      setMessages((items) => [...items, { id: `mic-${Date.now()}`, role: "system", text: e instanceof Error ? e.message : String(e) }]);
    } finally {
      setBusy(false);
    }
  };

  const sendVoice = async () => {
    const text = voiceStaging.trim();
    if (!text || busy) return;
    if (!voiceReviewed || !voiceSttResult?.production_eligible || !voiceSttResult.audio_sha256) {
      setMessages((items) => [...items, { id: `confirm-${Date.now()}`, role: "system", text: "Review and confirm the real-audio transcript before sending it as counsel input." }]);
      return;
    }
    if (!activeCase) {
      setMessages((m) => [
        ...m,
        {
          id: `sys-${Date.now()}`,
          role: "system",
          text: "Open a case with case_graph before voice-in-court (AC-2.1).",
        },
      ]);
      return;
    }
    if (voiceGate && !voiceGate.allowed) {
      setMessages((m) => [
        ...m,
        {
          id: `lite-${Date.now()}`,
          role: "system",
          text:
            voiceGate.honesty_notice ||
            voiceGate.messaging ||
            "Lite tier: live Mock Court voice simulation refused.",
        },
      ]);
      setLastMeta(voiceGate.refuse_code || "CF_VOICE_COURT_LITE_REFUSE");
      return;
    }
    setBusy(true);
    setDraft("");
    setMessages((m) => [
      ...m,
      { id: `u-v-${Date.now()}`, role: "user", text: `[Voice STT] ${text}` },
    ]);
    try {
      const res = await voiceCourtTurnForCase({
        sessionId,
        caseId: activeCase.case_id,
        format,
        counselText: text,
        confirmedRawTranscript: voiceRaw,
        confirmedAudioSha256: voiceSttResult.audio_sha256,
        productionEligible: voiceSttResult.production_eligible,
        courtRole: roleToMockCourtRole(role),
        turnIndex: voiceTurnIndex,
        benchArchetypeId: archetypeId,
        benchSliders: sliders,
        language: "English",
      });
      const turn = res.turn;
      setLastVoiceAudio(res.tts);
      setSessionTurns((prev) => [...prev, {
        turn_id: res.generation.turn.turn_id,
        counsel_text: turn.counsel_staged,
        assistant_text: res.generation.turn.assistant_text || turn.tts_spoken_text || "",
        proposition_ledger: res.generation.proposition_ledger ?? null,
        authorities: res.retrieval?.authorities || [],
      }]);
      setVoiceTurnIndex((n) => n + 1);
      setVoiceRaw("");
      setVoiceStaging("");
      setVoiceSttResult(null);
      setVoiceReviewed(false);
      setMessages((m) => [
        ...m,
        {
          id: `a-v-${Date.now()}`,
          role: "assistant",
          speaker: `TTS · ${turn.tts_role}`,
          text: turn.tts_spoken_text || turn.court_turn?.assistant_text || "(empty)",
          meta: `voice duplex · ${turn.elapsed_ms} ms · network=${turn.network_used} · lexicon=${turn.lexicon_applied} · AC-3.3=${turn.within_ac33}`,
        },
      ]);
      setLastMeta(
        `Voice turn ${turn.turn_index} · ${turn.elapsed_ms} ms · staged “${(turn.counsel_staged || "").slice(0, 40)}…” · offline`,
      );
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setMessages((m) => [
        ...m,
        {
          id: `err-v-${Date.now()}`,
          role: "system",
          text: msg.includes("LITE") || msg.includes("Lite")
            ? msg
            : `Voice court error: ${msg}`,
        },
      ]);
      setLastMeta("voice court error");
    } finally {
      setBusy(false);
    }
  };

  const runDebrief = async () => {
    if (!activeCase || sessionTurns.length === 0) {
      setMessages((m) => [
        ...m,
        {
          id: `sys-d-${Date.now()}`,
          role: "system",
          text: "Complete at least one grounded text or voice turn before Debrief v2.",
        },
      ]);
      return;
    }
    setBusy(true);
    try {
      const report = await mockCourtDebriefV2Generate({
        session_id: sessionId,
        case_id: activeCase.case_id,
        format,
        turns: sessionTurns,
      });
      setDebriefV2(report);
      setPreparationTasks(await preparationTasksList(activeCase.case_id));
      setInsightTab("debrief");
      setLastMeta(`Debrief v2 · ${report.findings.length} findings · ${report.tasks.length} source-specific tasks · ${report.source_fingerprint.slice(0,16)}`);
    } catch (e) {
      setLastMeta(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const updatePreparationTask = async (task:PreparationTask,status:PreparationTaskStatus) => {
    if (!activeCase) return;
    setBusy(true);
    try {
      const tasks=await preparationTaskUpdate({caseId:activeCase.case_id,taskId:task.task_id,status,title:taskDrafts[task.task_id]??task.title,counselNote:status==="DISMISSED"?"Dismissed by counsel; immutable source/debrief retained.":task.counsel_note});
      setPreparationTasks(tasks);
    } finally { setBusy(false); }
  };

  const sendBenchPrep = async () => {
    const text = draft.trim() || "Develop the limb from the case_graph only.";
    if (busy) return;
    if (!activeCase) {
      setMessages((m) => [
        ...m,
        {
          id: `sys-bp-${Date.now()}`,
          role: "system",
          text: "Open a case with case_graph for Bench Prep dual-counsel mode.",
        },
      ]);
      return;
    }
    setBusy(true);
    setDraft("");
    setMessages((m) => [
      ...m,
      {
        id: `u-bp-${Date.now()}`,
        role: "user",
        text: `[Bench Prep · ${prepSide}] ${text}`,
      },
    ]);
    try {
      const r = await benchPrepTurnForCase({
        sessionId,
        caseId: activeCase.case_id,
        format,
        side: prepSide,
        userCue: text,
        redteam: false,
      });
      setMessages((m) => [
        ...m,
        {
          id: `a-bp-${Date.now()}`,
          role: "assistant",
          speaker: r.side_label,
          text: r.text,
          meta: `non_decision_maker=${r.non_decision_maker} · g1_clean=${r.g1_clean} · refused=${r.refused} · ${r.elapsed_ms} ms`,
        },
      ]);
      setLastMeta(
        `Bench Prep · ${r.side_id} · G1 ${r.g1_clean ? "clean" : "hit"} · never decision-maker`,
      );
      setPrepSide((s) => (s === "side_a" ? "side_b" : "side_a"));
    } catch (e) {
      setMessages((m) => [
        ...m,
        {
          id: `err-bp-${Date.now()}`,
          role: "system",
          text: e instanceof Error ? e.message : String(e),
        },
      ]);
    } finally {
      setBusy(false);
    }
  };

  const runPrepRedteam = async () => {
    setBusy(true);
    try {
      const rep = await benchPrepRedteam({
        caseId: activeCase?.case_id ?? "ui-redteam",
      });
      setRedteamReport(rep);
      setMessages((m) => [
        ...m,
        {
          id: `rt-${Date.now()}`,
          role: "system",
          text: [
            "Bench Prep G1 red-team",
            `pass=${rep.pass} · hits=${rep.redteam_hits} · exchanges=${rep.exchanges.length}`,
            rep.notice,
            ...(rep.reasons || []),
          ].join("\n"),
        },
      ]);
      setLastMeta(
        rep.pass
          ? "Red-team PASS — no holding/order leaks"
          : `Red-team FAIL — hits ${rep.redteam_hits}`,
      );
    } catch (e) {
      setLastMeta(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const send = async () => {
    if (inputMode === "voice") {
      await sendVoice();
      return;
    }
    if (role === "bench_prep") {
      await sendBenchPrep();
      return;
    }
    const text = draft.trim();
    if (!text || busy) return;
    setBusy(true);
    setDraft("");
    const userMsg: ChatMessage = {
      id: `u-${Date.now()}`,
      role: "user",
      text,
    };
    setMessages((m) => [...m, userMsg]);

    try {
      if (activeCase) {
        if (!retrievalReadiness?.ready) {
          throw new Error(`CF_MOCK_COURT_NOT_READY: ${(retrievalReadiness?.blocking_reasons || ["load and confirm the matter legal working set"]).join(" | ")}`);
        }
        const grounded = await mockCourtTurnForCase({
          sessionId,
          caseId: activeCase.case_id,
          format,
          role: roleToMockCourtRole(role),
          userText: text,
          expectedGraphVersion: retrievalReadiness.graph_version,
          expectedWorkingSetVersion: retrievalReadiness.working_set_version,
          benchArchetypeId: archetypeId,
          benchSliders: sliders,
        });
        const turn = grounded.turn;
        const feedback = grounded.answer_feedback;
        if (feedback) setAnswerFeedback((prev) => ({ ...prev, [userMsg.id]: feedback }));
        setSessionTurns((prev) => [...prev, { turn_id:turn.turn_id,counsel_text:text,assistant_text:turn.assistant_text||"",proposition_ledger:grounded.proposition_ledger??null,authorities:grounded.authorities }]);
        setTurnAuthorities((items) => [...items, { turnId: turn.turn_id, authorities: grounded.authorities, ledger: grounded.proposition_ledger ?? null }]);
        const speaker =
          role === "adversary_drill" ? "Adversary" : "Coach";
        setMessages((m) => [
          ...m,
          {
            id: turn.turn_id || `a-${Date.now()}`,
            role: "assistant",
            speaker,
            text: turn.assistant_text || "(empty turn)",
            meta: `${grounded.generation_mode} · ${grounded.model || grounded.backend} · ${turn.elapsed_ms} ms · sources=${(turn.sources || []).join("+")}`,
          },
        ]);
        setLastMeta(
          `${turn.pipeline_stages_completed?.length || 0} stages · ${turn.elapsed_ms} ms · working set v${grounded.working_set_version} · ${grounded.cache_hit ? "retrieval cache" : "fresh retrieval"} · ${grounded.generation_mode}${grounded.schema_repair_used ? " · schema repaired" : ""}`,
        );
        if (grounded.generation_mode === "DEGRADED_DETERMINISTIC") {
          setMessages((m) => [...m, {
            id: `degraded-${Date.now()}`,
            role: "system",
            text: `Local adversarial model unavailable. This turn used the deterministic rehearsal template only. ${grounded.degraded_reason || ""}`,
          }]);
        }
      } else {
        await new Promise((r) => setTimeout(r, 200));
        const reply = coachReply({
          userText: text,
          role,
          format,
          caseTitle: undefined,
          jurisdiction: undefined,
          modelReady: false,
        });
        setMessages((m) => [...m, reply]);
        setLastMeta("offline scaffold (open a case for case_graph adversary)");
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setMessages((m) => [
        ...m,
        {
          id: `err-${Date.now()}`,
          role: "system",
          text: `Mock Court blocked: ${msg}\nNo ungrounded legal challenge was generated. Review the Authorities panel and resume against the current versions.`,
        },
      ]);
      if (msg.includes("CF_MOCK_COURT_SESSION_STALE")) {
        setRetrievalLoading(true);
        void mockCourtRetrievalReadiness(activeCase?.case_id || "")
          .then(setRetrievalReadiness)
          .finally(() => setRetrievalLoading(false));
        setLastMeta("Session invalidated: authority or graph version changed. Review Authorities, then send again to resume.");
      } else {
        setLastMeta("Mock Court blocked until its grounded matter inputs are ready");
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="page-grid" data-testid="mock-court-view">
      <div
        className="row"
        style={{
          justifyContent: "space-between",
          marginTop: 0,
          alignItems: "flex-start",
        }}
      >
        <div className="matter-chips" style={{ margin: 0 }}>
          {activeCase ? (
            <>
              <span className="chip matter">
                {activeCase.cause_title || activeCase.label}
              </span>
              <span className="chip matter">{activeCase.jurisdiction}</span>
              <span className="chip matter">{activeCase.court || "Court"}</span>
              <span className="chip matter">{activeCase.user_side || "Side"}</span>
              <span className={`chip ${retrievalReadiness?.ready ? "ok" : "warn"}`}>
                {retrievalLoading
                  ? "checking authorities"
                  : retrievalReadiness?.ready
                    ? `grounded · ${retrievalReadiness.usable_authority_count} authorities`
                    : "grounding blocked"}
              </span>
            </>
          ) : (
            <span className="chip warn">No matter linked</span>
          )}
          <span
            className="chip"
            style={{
              background: "var(--ink)",
              color: "#f4efe6",
              borderColor: "var(--ink)",
            }}
          >
            Practice · not a real hearing
          </span>
        </div>
        <div className="mode-tabs">
          {COURT_ROLES.map((r) => (
            <button
              key={r.id}
              type="button"
              className={role === r.id ? "active" : undefined}
              onClick={() => setRole(r.id)}
            >
              {ROLE_TO_MODE[r.id]}
            </button>
          ))}
        </div>
      </div>

      {activeCase && !retrievalLoading && retrievalReadiness && !retrievalReadiness.ready && (
        <div className="notice warn" data-testid="mock-court-readiness-block">
          <strong>Mock Court is not ready</strong>
          <p style={{ margin: "0.4rem 0 0" }}>
            {retrievalReadiness.blocking_reasons.join(" · ")}
          </p>
          <p className="muted" style={{ margin: "0.35rem 0 0", fontSize: "0.8rem" }}>
            Open Working Library, confirm the applicable-law regime, and mark at least one official current authority as included and controlling.
          </p>
        </div>
      )}

      {retrievalReadiness?.warnings.map((warning) => (
        <div className="notice warn" key={warning} data-testid="mock-court-readiness-warning">
          {warning}
        </div>
      ))}

      <div className="context-strip">
              <span className="chip matter">B-S3-15 · grounded court</span>
        <span className={`chip ${modelReady ? "ok" : "warn"}`}>
          {modelReady
            ? `${llm?.model || "local"} · verified adversarial mode`
            : "Degraded deterministic mode"}
        </span>
        <span className="chip muted">Turn · {elapsed}</span>
        {voiceGate && (
          <span className={`chip ${voiceGate.allowed ? "ok" : "warn"}`}>
            Voice {voiceGate.tier}
            {voiceGate.allowed ? " · duplex ok" : " · Lite refuse"}
          </span>
        )}
        <span className="muted" style={{ fontSize: "0.82rem", flex: 1, minWidth: 200 }}>
          {policyNote ||
            "No named judges. Adversary uses case_graph + retrieval only (AC-2.1)."}
        </span>
      </div>

      {voiceGate && !voiceGate.allowed && (
        <div className="notice warn" data-testid="lite-honesty-notice">
          <strong>Lite honesty notice</strong>
          <p style={{ margin: "0.4rem 0 0" }}>
            {voiceGate.honesty_notice || voiceGate.messaging}
          </p>
        </div>
      )}

      {role === "bench_prep" && (
        <div className="notice warn" data-testid="non-decision-maker-banner">
          <strong>Non-decision-maker (G1)</strong>
          <p style={{ margin: "0.4rem 0 0" }}>
            {prepNotice ||
              "AI plays both counsel only. CaseForge never issues holdings, court orders, or verdicts."}
          </p>
          <div className="row" style={{ marginTop: "0.65rem", gap: "0.5rem" }}>
            <button
              type="button"
              className={prepSide === "side_a" ? "primary" : undefined}
              onClick={() => setPrepSide("side_a")}
              data-testid="prep-side-a"
            >
              Side A · Applicant
            </button>
            <button
              type="button"
              className={prepSide === "side_b" ? "primary" : undefined}
              onClick={() => setPrepSide("side_b")}
              data-testid="prep-side-b"
            >
              Side B · Respondent
            </button>
            <button
              type="button"
              className="ghost"
              disabled={busy}
              onClick={() => void runPrepRedteam()}
              data-testid="bench-prep-redteam-btn"
            >
              Run G1 red-team
            </button>
          </div>
          {redteamReport && (
            <p className="mono" style={{ fontSize: "0.75rem", marginTop: "0.5rem" }} data-testid="bench-prep-redteam-result">
              redteam pass={String(redteamReport.pass)} hits={redteamReport.redteam_hits}
            </p>
          )}
        </div>
      )}

      <div className="court-layout">
        <div className="court-col">
          <div className="stat-label">Session setup</div>
          <div style={{ marginTop: "0.75rem" }}>
            <div
              className="row"
              style={{ marginTop: 0, justifyContent: "space-between" }}
            >
              <div>
                <div
                  className="muted"
                  style={{
                    fontSize: "0.68rem",
                    letterSpacing: "0.1em",
                    textTransform: "uppercase",
                  }}
                >
                  Side
                </div>
                <strong>{activeCase?.user_side || "Counsel"}</strong>
              </div>
              <div>
                <div
                  className="muted"
                  style={{
                    fontSize: "0.68rem",
                    letterSpacing: "0.1em",
                    textTransform: "uppercase",
                  }}
                >
                  Format order
                </div>
                <strong>{formatMeta?.order ?? "—"} / 5</strong>
              </div>
            </div>
            <p className="muted" style={{ marginTop: "0.75rem", fontSize: "0.88rem" }}>
              {formatMeta?.label ?? "Format"} (text-only · voice in B-S2-04)
            </p>
          </div>

          <div
            className="stat-label"
            style={{ marginTop: "1.25rem", marginBottom: "0.55rem" }}
          >
            Format (PRD sequential)
          </div>
          <div className="role-grid">
            {COURT_FORMATS.map((f) => (
              <button
                key={f.id}
                type="button"
                className={`role-card ${format === f.id ? "selected" : ""}`}
                onClick={() => setFormat(f.id)}
                data-testid={`format-${f.id}`}
              >
                <strong>
                  {f.order}. {f.label}
                </strong>
                <span>{f.blurb}</span>
              </button>
            ))}
          </div>

          {!activeCase && cases.length > 0 && (
            <div className="row" style={{ marginTop: "0.85rem" }}>
              {cases.slice(0, 3).map((c) => (
                <button key={c.case_id} type="button" onClick={() => onOpenCase(c.case_id)}>
                  {(c.cause_title || c.label || "Case").slice(0, 22)}
                </button>
              ))}
            </div>
          )}

          <div className="row" style={{ marginTop: "auto", paddingTop: "1rem" }}>
            <button type="button" onClick={onRefreshLlm}>
              Refresh LLM
            </button>
            <button
              type="button"
              onClick={() => setMessages([welcomeMessage(role, format, modelReady)])}
            >
              End session
            </button>
          </div>
        </div>

        <div className="court-col">
          <div className="row" style={{ justifyContent: "space-between", marginTop: 0 }}>
            <div className="stat-label">Chamber transcript</div>
            <button
              type="button"
              className="ghost"
              onClick={() => setMessages([welcomeMessage(role, format, modelReady)])}
            >
              Reset
            </button>
          </div>

          <div className="chamber-log" ref={logRef}>
            {messages.map((m) => (
              <div key={m.id} className={`transcript-block ${m.role}`}>
                {m.role === "assistant" && m.speaker && (
                  <span className="speaker">{m.speaker}</span>
                )}
                {m.role === "user" && <span className="speaker">Counsel (you)</span>}
                {m.role === "system" && <span className="speaker">System</span>}
                <div className="body">{m.text}</div>
                {m.role === "user" && answerFeedback[m.id] && <AnswerFeedbackPanel feedback={answerFeedback[m.id]} />}
                {m.meta && (
                  <div className="muted mono" style={{ fontSize: "0.7rem", marginTop: 4 }}>
                    {m.meta}
                  </div>
                )}
              </div>
            ))}
            {busy && (
              <div className="transcript-block system">
                <span className="speaker">Chamber</span>
                <div className="body">Running DECOMPOSE→…→RENDER on case_graph…</div>
              </div>
            )}
          </div>

          {lastMeta && (
            <div
              className="muted mono"
              style={{ fontSize: "0.72rem", marginBottom: "0.35rem" }}
            >
              {lastMeta}
            </div>
          )}
          {lastVoiceAudio?.audio_base64 && (
            <audio controls src={`data:audio/${lastVoiceAudio.audio_format || "wav"};base64,${lastVoiceAudio.audio_base64}`} data-testid="court-tts-audio">
              Offline Mock Court audio is unavailable in this renderer.
            </audio>
          )}

          <div className="chamber-composer">
            <div className="mode-tabs" style={{ marginLeft: 0, marginBottom: "0.55rem" }}>
              <button
                type="button"
                className={inputMode === "text" ? "active" : undefined}
                onClick={() => setInputMode("text")}
              >
                Text
              </button>
              <button
                type="button"
                className={inputMode === "voice" ? "active" : undefined}
                onClick={() => setInputMode("voice")}
                data-testid="voice-mode-tab"
              >
                Voice duplex
              </button>
            </div>
            {inputMode === "voice" ? (
              <div className="voice-court-capture" data-testid="voice-real-audio-input">
                <div className={`notice ${microphone.state === "RECORDING" ? "warn" : ""}`} aria-live="polite">
                  {speech?.production_ready
                    ? `Microphone: ${microphone.state.replaceAll("_", " ").toLowerCase()} · audio deleted after transcription`
                    : speech?.detail || "Production speech status unavailable; use Text mode."}
                </div>
                <div className="row" style={{ gap: "0.4rem", flexWrap: "wrap" }}>
                  <button type="button" disabled={busy || !speech?.production_ready || microphone.state === "RECORDING"} onClick={() => void microphone.start()} data-testid="court-mic-start">Start microphone</button>
                  <button type="button" disabled={busy || microphone.state !== "RECORDING"} onClick={() => void stopCourtRecording()} data-testid="court-mic-stop">Stop and transcribe</button>
                  <button type="button" disabled={microphone.state !== "RECORDING" && microphone.state !== "REQUESTING_PERMISSION"} onClick={microphone.cancel}>Cancel</button>
                </div>
                <textarea
                  value={voiceStaging}
                  onChange={(event) => { setVoiceStaging(event.target.value); setVoiceReviewed(false); }}
                  placeholder="Your local transcript appears here for review…"
                  aria-label="Reviewed voice transcript"
                />
                {voiceRaw && <small className="muted">Raw STT: {voiceRaw}</small>}
                {voiceSttResult && <small className="mono muted">real audio · sha256 {voiceSttResult.audio_sha256} · retained={String(voiceSttResult.audio_retained)}</small>}
                <label className="check-row">
                  <input type="checkbox" checked={voiceReviewed} disabled={!voiceStaging.trim()} onChange={(event) => setVoiceReviewed(event.target.checked)} />
                  I reviewed and confirm this transcript as my counsel submission.
                </label>
              </div>
            ) : (
              <textarea
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                placeholder="State your submission…"
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    void send();
                  }
                }}
              />
            )}
            <div className="composer-actions">
              <span className="chip muted">{inputMode === "voice" ? "STT→Court→TTS" : "Text"}</span>
              <span className="chip muted">English · offline</span>
              <button type="button" className="ghost" disabled={busy || sessionTurns.length === 0} onClick={() => void runDebrief()} data-testid="debrief-v2-btn">Generate Debrief v2</button>
              <button
                type="button"
                className="primary"
                disabled={
                  busy ||
                  (inputMode === "text" ? !draft.trim() : !voiceStaging.trim() || !voiceReviewed) ||
                  (Boolean(activeCase) && role !== "bench_prep" && !retrievalReadiness?.ready) ||
                  (inputMode === "voice" && Boolean(voiceGate && !voiceGate.allowed))
                }
                onClick={() => void send()}
                data-testid="court-send-btn"
              >
                {inputMode === "voice" ? "Send confirmed voice turn" : "Send"}
              </button>
            </div>
            <p className="muted" style={{ margin: 0, fontSize: "0.75rem" }}>
              {inputMode === "voice"
                ? "Full-duplex English: STT (lexicon) → grounded Mock Court → TTS roles. Lite tier is refused with honesty notice."
                : "Legal turns use host-verified working-set chunks plus case_graph; raw document text is never attached (AC-2.1)."}
            </p>
            {debriefV2 && (
              <div className="notice ok" style={{ marginTop: "0.55rem", fontSize: "0.78rem" }} data-testid="debrief-summary">
                Debrief v2 ready · {debriefV2.turns_completed} turns · {debriefV2.findings.length} findings · {debriefV2.tasks.length} tasks
              </div>
            )}
          </div>
        </div>

        <div className="court-col">
          <div className="mode-tabs" style={{ marginLeft: 0, marginBottom: "1rem" }}>
            <button
              type="button"
              className={insightTab === "bench" ? "active" : undefined}
              onClick={() => setInsightTab("bench")}
            >
              Adaptive Bench
            </button>
            <button
              type="button"
              className={insightTab === "insights" ? "active" : undefined}
              onClick={() => setInsightTab("insights")}
            >
              Insights
            </button>
            <button type="button" className={insightTab === "debrief" ? "active" : undefined} onClick={() => setInsightTab("debrief")}>Debrief &amp; tasks</button>
            <button
              type="button"
              className={insightTab === "weak" ? "active" : undefined}
              onClick={() => setInsightTab("weak")}
            >
              Weak points
            </button>
            <button
              type="button"
              className={insightTab === "auth" ? "active" : undefined}
              onClick={() => setInsightTab("auth")}
            >
              Authorities
            </button>
          </div>

          {insightTab === "bench" && (
            <div data-testid="adaptive-bench-panel">
              <div className="stat-label" style={{ marginBottom: "0.55rem" }}>
                Generic archetypes (never named judges)
              </div>
              <div className="role-grid" style={{ marginBottom: "0.85rem" }}>
                {(archetypes.length
                  ? archetypes
                  : [
                      {
                        id: "procedure_first",
                        label: "Procedure-first",
                        baseline: DEFAULT_SLIDERS,
                      },
                    ]
                ).map((a) => (
                  <button
                    key={a.id}
                    type="button"
                    className={`role-card ${archetypeId === a.id ? "selected" : ""}`}
                    onClick={() => {
                      setArchetypeId(a.id);
                      setSliders(a.baseline);
                    }}
                    data-testid={`archetype-${a.id}`}
                  >
                    <strong>{a.label}</strong>
                    <span className="mono" style={{ fontSize: "0.72rem" }}>
                      {a.id}
                    </span>
                  </button>
                ))}
              </div>
              <div className="stat-label" style={{ marginBottom: "0.45rem" }}>
                Dial-in sliders
              </div>
              {SLIDER_LABELS.map(({ key, label }) => (
                <label
                  key={key}
                  style={{
                    display: "grid",
                    gridTemplateColumns: "1fr auto",
                    gap: "0.35rem",
                    marginBottom: "0.55rem",
                    fontSize: "0.82rem",
                    textTransform: "none",
                    letterSpacing: 0,
                  }}
                >
                  <span>
                    {label}{" "}
                    <strong className="mono">{sliders[key]}</strong>
                  </span>
                  <input
                    type="range"
                    min={0}
                    max={100}
                    value={sliders[key]}
                    onChange={(e) =>
                      setSliders((s) => ({
                        ...s,
                        [key]: Number(e.target.value),
                      }))
                    }
                    style={{ gridColumn: "1 / -1", width: "100%" }}
                    data-testid={`slider-${key}`}
                  />
                </label>
              ))}
              {benchProfile ? (
                <div className="notice" style={{ fontSize: "0.8rem", marginTop: "0.5rem" }}>
                  <div className="tag">Case-order signals</div>
                  Suggested: <code>{benchProfile.signals.suggested_archetype}</code> · fp{" "}
                  <code>{benchProfile.signals.fingerprint}</code>
                  <div className="muted" style={{ marginTop: 6 }}>
                    gaps={benchProfile.signals.open_gap_count} · evidence=
                    {benchProfile.signals.evidence_count} · urgency=
                    {String(benchProfile.signals.urgency_hint)}
                  </div>
                </div>
              ) : (
                <div className="notice warn" style={{ fontSize: "0.8rem" }}>
                  Open a case with case_graph to load case-order signals (AC-10.2).
                </div>
              )}
              <p className="muted" style={{ fontSize: "0.75rem", marginTop: "0.65rem" }}>
                Named-judge profiles are refused (PRD §20 · AC-10.1).
              </p>
            </div>
          )}

          {insightTab === "insights" && (
            <>
              <div className="stat-label" style={{ marginBottom: "0.65rem" }}>
                Format pressure (generic)
              </div>
              <div className="contradiction-card">
                <div className="tag">{formatMeta?.label}</div>
                {formatMeta?.blurb}
              </div>
              <div className="contradiction-card">
                <div className="tag">AC-2.1</div>
                Uses case_graph + retrieval only — never re-parses PDFs.
              </div>
              <div className="contradiction-card">
                <div className="tag">AC-3.3</div>
                Turn budget ≤ 15 s for ~300-token adversary response when models ready.
              </div>
            </>
          )}

          {insightTab === "weak" && (
            <>
              <div className="stat-label" style={{ marginBottom: "0.55rem" }}>
                Preparation focus
              </div>
              <p className="muted" style={{ fontSize: "0.9rem" }}>
                Drill open gaps from the case_graph. Completeness only — never win
                probability (G6).
              </p>
              <p className="muted" style={{ fontSize: "0.85rem", marginTop: "0.75rem" }}>
                Use Argument Tree for VI / Missing Links (B-S2-01).
              </p>
            </>
          )}

          {insightTab === "debrief" && (
            <div data-testid="debrief-v2-panel">
              <div className="stat-label">Source-specific Debrief v2</div>
              {!debriefV2 ? <p className="muted">Complete a grounded turn, then generate Debrief v2. Existing accepted tasks remain below.</p> : <>
                <div className="notice" data-testid="session-comparison">
                  <strong>Session comparison — observed support only</strong>
                  <div>{debriefV2.comparison.observed_change}</div>
                  <div className="mono muted">outcome_prediction={String(debriefV2.comparison.outcome_prediction)} · {debriefV2.rubric_version}</div>
                </div>
                <p className="notice warn">{debriefV2.disclaimer}</p>
                {debriefV2.findings.map((finding) => <article key={finding.finding_id} className="contradiction-card" data-testid="debrief-finding">
                  <div className="tag">{finding.category.replaceAll("_"," ")} · {finding.epistemic_status} · {finding.severity}</div>
                  <strong>{finding.title}</strong>
                  <p className="muted">{finding.detail}</p>
                  <div className="mono muted">turns: {finding.turn_ids.join(" · ")}</div>
                  {finding.source_refs.length===0 ? <span className="chip warn">Explicit {finding.epistemic_status.toLowerCase()} — no verified source</span> : finding.source_refs.map((source)=><ProvenanceInspector key={`${finding.finding_id}-${source.stable_id}`} compact record={debriefSourceProvenance(source,finding.title,finding.epistemic_status)} />)}
                </article>)}
              </>}
              <h4 style={{marginTop:"1rem"}}>Preparation tasks</h4>
              {preparationTasks.length===0 ? <p className="muted">No stored tasks yet.</p> : preparationTasks.map((task)=><article key={task.task_id} className="contradiction-card" data-testid="preparation-task">
                <div className="tag">{task.status} · usefulness {task.usefulness_score}/4 · {task.epistemic_status}</div>
                <input aria-label={`Edit task ${task.task_id}`} value={taskDrafts[task.task_id]??task.title} onChange={(e)=>setTaskDrafts((d)=>({...d,[task.task_id]:e.target.value}))} />
                <p className="muted">{task.detail}</p>
                <div className="mono muted">turns: {task.turn_ids.join(" · ")} · findings: {task.finding_ids.join(" · ")}</div>
                {task.source_refs.map((source)=><ProvenanceInspector key={`${task.task_id}-${source.stable_id}`} compact record={debriefSourceProvenance(source,task.title,task.epistemic_status)} />)}
                <div className="row" style={{gap:".35rem",flexWrap:"wrap",marginTop:".5rem"}}>
                  <button disabled={busy} onClick={()=>void updatePreparationTask(task,"ACCEPTED")}>Accept / save edit</button>
                  <button disabled={busy} onClick={()=>void updatePreparationTask(task,"DISMISSED")}>Dismiss</button>
                  <button disabled={busy} onClick={()=>void updatePreparationTask(task,"COMPLETED")}>Mark complete</button>
                </div>
              </article>)}
            </div>
          )}

          {insightTab === "auth" && (
            <>
              <div className="chip-row" style={{ marginBottom: "0.85rem" }}>
                <span className={`chip ${retrievalReadiness?.ready ? "ok" : "warn"}`}>
                  {retrievalReadiness?.ready ? "READY" : "BLOCKED"}
                </span>
                <span className="chip">graph {retrievalReadiness?.graph_version.slice(0, 10) || "—"}</span>
                <span className="chip">working set v{retrievalReadiness?.working_set_version ?? "—"}</span>
              </div>
              <div className={`notice ${retrievalReadiness?.ready ? "ok" : "warn"}`} style={{ fontSize: "0.85rem" }}>
                {retrievalReadiness?.ready
                  ? `${retrievalReadiness.usable_authority_count} counsel-confirmed, official, current authority chunks are available. Each turn is pinned to these graph and working-set versions.`
                  : (retrievalReadiness?.blocking_reasons || ["Open a case to inspect readiness."]).join(" · ")}
              </div>
              {(retrievalReadiness?.authorities || []).map((authority) => (
                <details key={`ready-${authority.authority_id}-${authority.chunk_id}`} className="contradiction-card" style={{ marginTop: "0.55rem" }}>
                  <summary><strong>{authority.title}</strong> · {authority.section_label}</summary>
                  <ProvenanceInspector compact record={authorityProvenance(authority)} />
                  <div className="mono muted" style={{ fontSize: "0.7rem", marginTop: 6 }}>
                    {authority.citation} · {authority.authority_id} · {authority.chunk_id}
                  </div>
                  <p style={{ whiteSpace: "pre-wrap", fontSize: "0.82rem" }}>{authority.exact_text}</p>
                  <div className="muted" style={{ fontSize: "0.72rem" }}>{authority.source_reference}</div>
                </details>
              ))}
              {turnAuthorities.map((entry) => (
                <div key={entry.turnId} style={{ marginTop: "0.8rem" }} data-testid="mock-court-turn-sources">
                  <div className="stat-label">Turn {entry.turnId.slice(-10)}</div>
                  {(entry.ledger?.propositions || []).map((proposition) => {
                    const authority = entry.authorities.find((a) => proposition.verified_support_ids.some((id) => id.includes(a.authority_id) && id.includes(a.chunk_id)));
                    return <div key={`${entry.turnId}-${proposition.proposition_id}`}>
                      <ProvenanceInspector record={verifiedPropositionProvenance(proposition, authority ? authorityProvenance(authority) : {})} />
                    </div>;
                  })}
                  {entry.authorities.map((authority) => (
                    <details key={`${entry.turnId}-${authority.chunk_id}`} className="contradiction-card" style={{ marginTop: "0.4rem" }}>
                      <summary>{authority.citation} · {authority.section_label}</summary>
                      <ProvenanceInspector compact record={authorityProvenance(authority)} />
                      <p style={{ whiteSpace: "pre-wrap", fontSize: "0.82rem" }}>{authority.exact_text}</p>
                      <div className="mono muted" style={{ fontSize: "0.68rem" }}>
                        {authority.authority_id} · {authority.chunk_id} · {authority.chunk_checksum.slice(0, 12)}
                      </div>
                    </details>
                  ))}
                </div>
              ))}
            </>
          )}

          <div className="row" style={{ marginTop: "auto", paddingTop: "1.25rem" }}>
            <button type="button" className="ghost" onClick={onRefreshLlm}>
              Probe LLM
            </button>
          </div>
        </div>
      </div>

      <div className="system-pulse-bar">
        <span>
          Mock Court text formats · five sequential · no filing · no cloud · no named judges
        </span>
        <span>UI never freezes &gt; 200 ms</span>
      </div>
    </div>
  );
}

function authorityProvenance(authority: MockCourtAuthority): ProvenanceRecord {
  return {
    stableId: authority.chunk_id,
    propositionClass: "LEGAL_BASIS",
    origin: "LEGAL_AUTHORITY",
    supportState: authority.trust_level === "OFFICIAL_VERIFIED" && authority.temporal_status === "APPLICABLE" && authority.superseded_status === "CURRENT" ? "GREEN" : "AMBER",
    title: `${authority.citation} · ${authority.section_label}`,
    verification: "Mock Court working-set authority. Host verification is still required for each proposition.",
    version: authority.chunk_checksum,
    packId: authority.pack_id,
    sourceId: authority.source_id,
    chunkId: authority.chunk_id,
    section: authority.section_label,
    excerpt: authority.exact_text,
    supportIds: [`legal:${authority.authority_id}:${authority.chunk_id}`],
  };
}

function debriefSourceProvenance(source:{source_kind:string;stable_id:string;locator:string;title:string;excerpt?:string|null;verification_status:"GREEN"|"AMBER"|"RED"|"UNSUPPORTED"},title:string,epistemic:string):ProvenanceRecord{
  const legal=source.source_kind==="LEGAL_AUTHORITY";
  return { stableId:source.stable_id,propositionClass:legal?"LEGAL_BASIS":epistemic==="INFERENCE"?"INFERENCE":source.verification_status==="UNSUPPORTED"?"UNSUPPORTED_OR_GAP":"FACTUAL_BASIS",origin:legal?"LEGAL_AUTHORITY":epistemic==="INFERENCE"?"MODEL_INFERENCE":source.verification_status==="UNSUPPORTED"?"UNSUPPORTED_GAP":"MATTER_EVIDENCE",supportState:source.verification_status,title,verification:`Debrief source: ${source.title} · ${source.locator}`,excerpt:source.excerpt,supportIds:[source.stable_id] };
}
