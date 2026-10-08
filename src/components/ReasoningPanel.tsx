import { useState } from "react";
import { reasoningRun, type ReasoningDepth, type ReasoningResult, type VerifiedProposition } from "../lib/ipc";

/**
 * B-S4-28: step-by-step reasoning with best-of-n verification. The local model proposes numbered steps; the host checks
 * every factual and legal step against the exact text of a supplied source and every inference against the steps it rests
 * on. Several attempts are made and the best-verified one is kept. Steps the host could not verify are shown as gaps.
 */
const DEPTHS: { value: ReasoningDepth; label: string; hint: string }[] = [
  { value: "QUICK", label: "Quick", hint: "one attempt" },
  { value: "CAREFUL", label: "Careful", hint: "up to 3 attempts, one repair" },
  { value: "THOROUGH", label: "Thorough", hint: "5 attempts, one repair" },
];
const TONE = { ok: "var(--ok, #2f7d4f)", warn: "var(--warn, #9a6a00)", bad: "var(--danger, #a33a2b)", muted: "var(--muted, #5b6470)" };
const CLASS_LABEL: Record<VerifiedProposition["class"], string> = {
  FACTUAL_BASIS: "Fact",
  LEGAL_BASIS: "Law",
  INFERENCE: "Inference",
  COUNSEL_INPUT: "Your instruction",
  UNSUPPORTED_OR_GAP: "Gap",
};

function stepState(p: VerifiedProposition): { label: string; color: string } {
  switch (p.render_action) {
    case "TRUSTED":
      return { label: "Checked against a supplied source", color: TONE.ok };
    case "LABELLED":
      return { label: p.class === "INFERENCE" ? "Inference from the steps above" : "Your instruction", color: TONE.warn };
    case "REPOSITORY_GAP":
      return { label: "Not verified — source not supplied", color: TONE.bad };
    default:
      return { label: "Not supported — not relied on", color: TONE.bad };
  }
}

const BANNER: Record<ReasoningResult["status"], { title: string; text: string; color: string }> = {
  VERIFIED: { title: "Verified", text: "Every step is checked against a supplied source or shown as an inference from earlier steps.", color: TONE.ok },
  PARTIAL: { title: "Partly verified", text: "Some steps could not be verified and are shown as gaps. Rely only on the steps marked as checked.", color: TONE.warn },
  NO_VERIFIED_ANSWER: { title: "Nothing verified", text: "No answer could be verified from the supplied sources. Do not rely on anything below; add the missing authority or facts to the working set and try again.", color: TONE.bad },
};

export function ReasoningPanel({ caseId }: { caseId: string }) {
  const [question, setQuestion] = useState("");
  const [depth, setDepth] = useState<ReasoningDepth>("CAREFUL");
  const [result, setResult] = useState<ReasoningResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function run() {
    setBusy(true);
    setError(null);
    try {
      setResult(await reasoningRun(caseId, question, depth));
    } catch (e) {
      setResult(null);
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const banner = result ? BANNER[result.status] : null;
  return (
    <div className="card" style={{ gridColumn: "1 / -1" }}>
      <h4>Reason it through — checked step by step</h4>
      <p className="muted" style={{ fontSize: "0.8rem" }}>
        Ask a legal question about this matter. The local model sets out numbered steps; each fact and rule must copy exact text from the matter or from an authority in your working set, and the app checks that copy. It tries several times and keeps the best-checked attempt. Preparation material, not legal advice.
      </p>
      <textarea
        value={question}
        onChange={(e) => setQuestion(e.target.value)}
        rows={3}
        maxLength={2000}
        placeholder="e.g. Is the 14-month lease enforceable without registration?"
        style={{ width: "100%" }}
        aria-label="Question to reason through"
      />
      <div className="row" style={{ gap: 8, alignItems: "center", marginTop: 6 }}>
        <select value={depth} onChange={(e) => setDepth(e.target.value as ReasoningDepth)} aria-label="How thoroughly to reason" disabled={busy}>
          {DEPTHS.map((d) => (
            <option key={d.value} value={d.value}>
              {d.label} — {d.hint}
            </option>
          ))}
        </select>
        <button type="button" onClick={() => void run()} disabled={busy || !question.trim()}>
          {busy ? "Reasoning…" : "Reason it through"}
        </button>
        {busy && <span className="muted" style={{ fontSize: "0.8rem" }}>This can take several minutes on a local model; every attempt is checked before it is shown.</span>}
      </div>
      {error && <div className="notice warn" style={{ marginTop: 8 }}>{error}</div>}
      {result && banner && (
        <div style={{ marginTop: 10 }}>
          <div className="notice" style={{ borderLeft: `4px solid ${banner.color}`, paddingLeft: 8 }}>
            <strong style={{ color: banner.color }}>{banner.title}</strong> — {banner.text}
          </div>
          <ol style={{ paddingLeft: "1.2rem", margin: "8px 0" }}>
            {(result.ledger?.propositions ?? []).map((p) => {
              const s = stepState(p);
              return (
                <li key={p.proposition_id} style={{ marginBottom: 8, fontSize: "0.88rem" }}>
                  <span className="muted" style={{ fontSize: "0.75rem" }}>{CLASS_LABEL[p.class]} · </span>
                  <span style={{ color: s.color, fontSize: "0.75rem", fontWeight: 600 }}>{s.label}</span>
                  <div>{p.text}</div>
                  {p.render_action === "TRUSTED" &&
                    p.support_claims.map((c) => (
                      <blockquote key={c.support_id} className="muted" style={{ margin: "2px 0 0 0", fontSize: "0.78rem", borderLeft: `2px solid ${TONE.ok}`, paddingLeft: 6 }}>
                        “{c.exact_excerpt}”
                      </blockquote>
                    ))}
                  {p.render_action === "LABELLED" && p.basis_ids.length > 0 && <div className="muted" style={{ fontSize: "0.75rem" }}>Rests on: {p.basis_ids.join(", ")}</div>}
                  {(p.render_action === "REPOSITORY_GAP" || p.render_action === "BLOCKED") && <div className="muted" style={{ fontSize: "0.75rem" }}>{p.reason}</div>}
                </li>
              );
            })}
          </ol>
          <details>
            <summary className="muted" style={{ fontSize: "0.8rem" }}>
              How this was reached — {result.candidates_run} attempt{result.candidates_run === 1 ? "" : "s"}
              {result.repair_attempted ? (result.repair_accepted ? ", one repair kept" : ", one repair tried, not kept") : ""}
              {result.stopped_early ? ", stopped at the first fully checked attempt" : ""}
              {result.stopped_for_time ? ", stopped to stay within the time allowed" : ""}
            </summary>
            <table style={{ fontSize: "0.78rem", marginTop: 4 }}>
              <thead>
                <tr>
                  <th>Attempt</th>
                  <th>Checked</th>
                  <th>Inferences</th>
                  <th>Gaps</th>
                  <th>Conclusion stands</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {result.candidates.map((c) => (
                  <tr key={`${c.variant}-${c.repair}`}>
                    <td>{c.repair ? "Repair" : c.variant + 1}</td>
                    <td>{c.trusted}</td>
                    <td>{c.labelled}</td>
                    <td>{c.red}</td>
                    <td>{c.conclusion_ok ? "yes" : "no"}</td>
                    <td>{c.variant === result.chosen_variant ? "kept" : ""}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {result.discarded.length > 0 && (
              <ul className="muted" style={{ fontSize: "0.75rem" }}>
                {result.discarded.map((d) => (
                  <li key={d.variant}>
                    {d.variant === 100 ? "Repair" : `Attempt ${d.variant + 1}`} produced nothing usable: {d.reason}
                  </li>
                ))}
              </ul>
            )}
          </details>
          <div className="muted mono" style={{ fontSize: "0.7rem", marginTop: 6 }}>
            {result.model.model} ({result.model.backend}) · {(result.elapsed_ms / 1000).toFixed(0)} s · steps checked by the app, not by the model · {result.disclaimer}
          </div>
        </div>
      )}
    </div>
  );
}
