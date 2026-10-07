/**
 * B-S1-09 Guided Intake Interview
 * AC-6.5: ≤12 questions → ≥90% skeleton completion
 * AC-6.6: resumable, skippable, re-openable
 * Questions only fill skeleton gaps — no legal strategy.
 */

import { useMemo, useState } from "react";
import type { InterviewSession } from "../lib/ipc";

export default function InterviewView({
  session,
  busy,
  onOpen,
  onPause,
  onResume,
  onAnswer,
  onSkip,
}: {
  session: InterviewSession | null;
  busy: boolean;
  onOpen: () => Promise<void>;
  onPause: () => Promise<void>;
  onResume: () => Promise<void>;
  onAnswer: (questionIndex: number, text: string) => Promise<void>;
  onSkip: (questionIndex: number) => Promise<void>;
}) {
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);

  const current = useMemo(() => {
    if (!session?.questions?.length) return null;
    const pending = session.questions.find((q) => q.status === "pending");
    if (pending) return pending;
    // fall back to cursor
    return session.questions[session.cursor_index] ?? session.questions[0] ?? null;
  }, [session]);

  const answeredCount =
    session?.questions.filter((q) => q.status === "answered" || q.status === "skipped")
      .length ?? 0;

  const startOrResume = async () => {
    setError(null);
    try {
      if (session && (session.status === "paused" || session.status === "open")) {
        await onResume();
      } else {
        await onOpen();
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const submit = async () => {
    if (!current || current.status !== "pending") return;
    if (!draft.trim()) {
      setError("Enter an answer, or skip.");
      return;
    }
    setError(null);
    try {
      await onAnswer(current.question_index, draft.trim());
      setDraft("");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const skip = async () => {
    if (!current || current.status !== "pending") return;
    setError(null);
    try {
      await onSkip(current.question_index);
      setDraft("");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  if (!session) {
    return (
      <div className="card empty">
        <h3>Guided Intake Interview</h3>
        <p>
          Targeted questions from skeleton gaps only. Answers become{" "}
          <span className="mono">SOURCED_BY_COUNSEL</span> facts — not legal strategy.
        </p>
        <button
          type="button"
          className="primary"
          disabled={busy}
          onClick={() => void startOrResume()}
          data-testid="interview-open"
        >
          {busy ? "Opening…" : "Start interview"}
        </button>
        {error && (
          <p className="notice warn" style={{ marginTop: "0.75rem" }}>
            {error}
          </p>
        )}
      </div>
    );
  }

  const maxQ = session.max_questions ?? 12;
  const minDone = session.min_completion_percent ?? 90;
  const done = session.status === "completed";

  return (
    <div className="interview-layout" data-testid="interview-session">
      <div className="row" style={{ justifyContent: "space-between", marginTop: 0 }}>
        <span className="eyebrow">DIGEST · guided intake</span>
        <span className={`chip ${done ? "ok" : session.status === "paused" ? "muted" : "ok"}`}>
          {session.status}
        </span>
      </div>
      <h3>Skeleton completion interview</h3>
      <p className="muted" style={{ fontSize: "0.85rem" }}>
        Max {maxQ} questions (AC-6.5). Target ≥ {minDone}% prep completeness. Resumable / skippable /
        re-openable (AC-6.6).
      </p>

      <div className="meter" aria-hidden>
        <span
          style={{
            width: `${Math.min(100, session.completion_percent || 0)}%`,
          }}
        />
      </div>
      <p className="mono muted" style={{ fontSize: "0.75rem" }}>
        prep {session.completion_percent}% · answered/skipped {answeredCount}/
        {session.questions.length} · cursor {session.cursor_index}
      </p>

      <div className="row" style={{ margin: "0.75rem 0", flexWrap: "wrap", gap: "0.4rem" }}>
        {session.status === "open" && (
          <button type="button" disabled={busy} onClick={() => void onPause()} data-testid="interview-pause">
            Pause
          </button>
        )}
        {(session.status === "paused" || session.status === "completed") && (
          <button
            type="button"
            className="primary"
            disabled={busy}
            onClick={() => void startOrResume()}
            data-testid="interview-resume"
          >
            {session.status === "completed" ? "Re-open interview" : "Resume"}
          </button>
        )}
        {!session.status && (
          <button type="button" className="primary" disabled={busy} onClick={() => void onOpen()}>
            Start
          </button>
        )}
      </div>

      {done ? (
        <div className="notice ok" data-testid="interview-complete">
          Interview completed at {session.completion_percent}% skeleton prep (≥ {minDone}% target).
          Counsel facts are stored with SOURCED_BY_COUNSEL provenance.
        </div>
      ) : current ? (
        <div className="interview-card" data-testid="interview-question">
          <div className="stat-label">
            Question {(current.question_index ?? 0) + 1} · {current.topic}
          </div>
          <p style={{ margin: "0.5rem 0 0.85rem" }}>{current.prompt}</p>
          {current.status === "pending" ? (
            <>
              <label>
                Your answer (counsel)
                <textarea
                  rows={4}
                  value={draft}
                  onChange={(e) => setDraft(e.target.value)}
                  disabled={busy || session.status !== "open"}
                  data-testid="interview-answer-input"
                  placeholder="Factual skeleton detail only — not strategy or outcome prediction"
                />
              </label>
              <div className="row" style={{ marginTop: "0.65rem", gap: "0.45rem" }}>
                <button
                  type="button"
                  className="primary"
                  disabled={busy || session.status !== "open"}
                  onClick={() => void submit()}
                  data-testid="interview-answer-save"
                >
                  Save answer
                </button>
                <button
                  type="button"
                  disabled={busy || session.status !== "open"}
                  onClick={() => void skip()}
                  data-testid="interview-skip"
                >
                  Skip
                </button>
              </div>
            </>
          ) : (
            <p className="mono muted" style={{ fontSize: "0.8rem" }}>
              {current.status}: {current.answer_text || "—"}
            </p>
          )}
        </div>
      ) : (
        <p className="muted">No pending questions. Pause/resume or re-open if needed.</p>
      )}

      <div className="stat-label" style={{ marginTop: "1.25rem" }}>
        Question log
      </div>
      <ul className="interview-log">
        {session.questions.map((q) => (
          <li key={q.question_index} className={`interview-log-item ${q.status}`}>
            <span className="mono">
              Q{q.question_index + 1} · {q.status}
            </span>
            <span>{q.prompt}</span>
          </li>
        ))}
      </ul>

      {error && (
        <p className="notice warn" style={{ marginTop: "0.75rem" }}>
          {error}
        </p>
      )}
    </div>
  );
}
