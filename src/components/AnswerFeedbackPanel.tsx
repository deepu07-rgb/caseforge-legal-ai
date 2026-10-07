import type { AnswerFeedback, AnswerFeedbackItem } from "../lib/ipc";

/**
 * B-S4-00b: shows what the record and the working set say about counsel's answer to the previous challenge.
 * Deterministic checks only — never "right/wrong", never an outcome. Every line names its record reference.
 */
const STATUS_LABEL: Record<AnswerFeedbackItem["status"], { text: string; tone: "ok" | "warn" | "bad" | "info" }> = {
  SUPPORTED_BY_RECORD: { text: "Supported by record", tone: "ok" },
  VERIFIED_IN_WORKING_SET: { text: "In working set", tone: "ok" },
  CONFLICTS_WITH_RECORD: { text: "Conflicts with record", tone: "bad" },
  NOT_IN_WORKING_SET: { text: "Not verified", tone: "bad" },
  NOT_FOUND_IN_RECORD: { text: "Not found in record", tone: "warn" },
  AVAILABLE_NOT_USED: { text: "Available, not used", tone: "info" },
};

const ENGAGEMENT_LABEL: Record<AnswerFeedback["engagement"], string> = {
  ADDRESSED: "Engages the challenge",
  PARTIAL: "Partly engages the challenge",
  NOT_ADDRESSED: "Does not engage the challenge's specifics",
  NO_PRIOR_CHALLENGE: "Opening statement (no earlier challenge)",
};

const TONE_COLOR = { ok: "var(--ok, #2f7d4f)", warn: "var(--warn, #9a6a00)", bad: "var(--danger, #a33a2b)", info: "var(--muted, #5b6470)" };

function locatorText(item: AnswerFeedbackItem): string {
  const loc = item.locator;
  if (!loc || !loc.doc_id) return item.record_ref ? `ref ${item.record_ref}` : "";
  return `${loc.doc_id}${loc.page != null ? ` p.${loc.page}` : ""}`;
}

export function AnswerFeedbackPanel({ feedback }: { feedback: AnswerFeedback }) {
  if (feedback.engagement === "NO_PRIOR_CHALLENGE" && feedback.items.length === 0) return null;
  return (
    <div className="answer-feedback" style={{ marginTop: 6, padding: "6px 8px", borderLeft: "3px solid var(--line, #c9ccd1)", fontSize: "0.82rem" }}>
      <div className="muted" style={{ marginBottom: 4 }}>
        Answer check · {ENGAGEMENT_LABEL[feedback.engagement]}
        {feedback.engagement_terms_total > 0 && ` (${feedback.engagement_terms_matched}/${feedback.engagement_terms_total} specific terms)`}
      </div>
      {feedback.items.length === 0 ? (
        <div className="muted">No dates, amounts or authorities in this answer to check against the record.</div>
      ) : (
        <ul style={{ margin: 0, paddingLeft: "1.1rem" }}>
          {feedback.items.map((item, i) => {
            const label = STATUS_LABEL[item.status];
            return (
              <li key={`${item.kind}-${i}`} style={{ marginBottom: 2 }}>
                <span style={{ color: TONE_COLOR[label.tone], fontWeight: 600 }}>{label.text}</span>
                {item.value && <span className="mono"> · {item.value}</span>}
                <span> — {item.message}</span>
                {locatorText(item) && <span className="muted mono"> [{locatorText(item)}]</span>}
              </li>
            );
          })}
        </ul>
      )}
      <div className="muted" style={{ marginTop: 4, fontSize: "0.7rem" }}>
        Deterministic record check ({feedback.engine_version}); no AI model, no outcome prediction.
      </div>
    </div>
  );
}
