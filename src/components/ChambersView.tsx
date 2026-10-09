import { useCallback, useEffect, useState } from "react";
import type { CaseSummary, ChambersReasonResult, ChambersStatus, EditionStatus } from "../lib/ipc";
import { chambersReason, chambersSetRecordOnly, chambersStatus } from "../lib/ipc";
import { CurrencyStamp } from "./CurrencyStamp";

/**
 * B-S4-17: Chambers Edition shell. Everything on this screen comes from the filings, the evidence and orders on the record,
 * or installed law. Each statement the model produces is verified word for word against its source by the app, shown with
 * its class and where it comes from, and anything not so supported is withheld (counted, never shown). The tool writes no
 * analysis, findings or orders; the decisions are the judge's.
 */
const CLASS: Record<string, { label: string; color: string }> = {
  RECORD: { label: "From the record", color: "var(--ok, #2f7d4f)" },
  LAW: { label: "Installed law", color: "var(--ok, #2f7d4f)" },
  INFERENCE_FROM_RECORD: { label: "Follows from the record", color: "var(--warn, #9a6a00)" },
  EXTRA_RECORD: { label: "Not from the record", color: "var(--danger, #a33a2b)" },
};
const WHY: Record<string, string> = {
  NO_SUPPORT: "had no source",
  NOT_VERIFIED: "did not match its source word for word",
  NOT_FROM_THE_RECORD: "came from the question, not the record",
  RESTS_ON_EXTRA_RECORD: "rested on something that is not on the record",
};
const ERROR: Record<string, string> = {
  CF_EDITION_NOT_ENTITLED: "This belongs to the Chambers Edition, which needs a Judiciary licence.",
  CF_G7_LOCKED: "A judgment scaffold exists for this matter, so record-only mode stays on.",
  CF_EDITION_PILOT_DISABLED: "This court’s configuration has switched this off.",
  CF_EDITION_G7_FREE_GENERATION: "In the Chambers Edition the model can be reached only through this record-only screen.",
};

function plain(e: unknown): string {
  const s = String(e);
  const k = Object.keys(ERROR).find((x) => s.includes(x));
  return k ? ERROR[k] : s;
}
/** cfchunk:<pack id>:<hash> -> pack id */
function packOf(chunkId: unknown): string | null {
  const m = /^cfchunk:(.+):[0-9a-f]{16}$/.exec(String(chunkId ?? ""));
  return m ? m[1] : null;
}

export default function ChambersView({ activeCase, edition, onOpenIssues }: { activeCase: CaseSummary | null; edition: EditionStatus | null; onOpenIssues: () => void }) {
  const caseId = activeCase?.case_id ?? null;
  const [status, setStatus] = useState<ChambersStatus | null>(null);
  const [question, setQuestion] = useState("");
  const [result, setResult] = useState<ChambersReasonResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!caseId || !edition?.chambers) return;
    try {
      setStatus(await chambersStatus(caseId));
    } catch (e) {
      setError(plain(e));
    }
  }, [caseId, edition?.chambers]);
  useEffect(() => {
    setStatus(null);
    setResult(null);
    void refresh();
  }, [refresh]);

  if (!edition?.chambers) {
    return (
      <div className="card">
        <h4>Chambers Edition</h4>
        <p className="muted">This screen is part of the Chambers Edition, which needs a Judiciary licence issued to the court. Activate one under Licence.</p>
        {edition?.problem && <div className="notice warn">{plain(edition.problem)}</div>}
      </div>
    );
  }
  if (!activeCase) return <div className="card"><p className="muted">Open a matter to work on its record.</p></div>;

  async function act(fn: () => Promise<unknown>) {
    setBusy(true);
    setError(null);
    try {
      await fn();
      await refresh();
    } catch (e) {
      setError(plain(e));
    } finally {
      setBusy(false);
    }
  }
  async function ask() {
    if (!caseId) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setResult(await chambersReason(caseId, question, "CAREFUL"));
      await refresh();
    } catch (e) {
      setError(plain(e));
    } finally {
      setBusy(false);
    }
  }
  const docName = (id: unknown) => result?.record_coverage?.documents?.[String(id)] ?? String(id ?? "record");

  return (
    <div className="grid" style={{ gap: 12 }}>
      <div className="card" style={{ gridColumn: "1 / -1" }}>
        <h4>Record-only mode</h4>
        <p className="muted" style={{ fontSize: "0.8rem" }}>
          Everything below comes from the filings, the evidence and orders on the record, and installed law. Each statement is checked word for word against its source and shown with where it comes from. Anything else is withheld, and this tool writes no analysis, findings or orders. The app is kept offline under this licence.
        </p>
        {error && <div className="notice warn">{error}</div>}
        {status && (
          <div className="row" style={{ gap: 12, alignItems: "center", flexWrap: "wrap", fontSize: "0.85rem" }}>
            <strong style={{ color: status.record_only ? CLASS.RECORD.color : CLASS.EXTRA_RECORD.color }}>Record-only is {status.record_only ? "on" : "off"}</strong>
            {status.locked ? (
              <span className="muted">A judgment scaffold exists for this matter, so it stays on.</span>
            ) : (
              <button type="button" disabled={busy} onClick={() => void act(() => chambersSetRecordOnly(activeCase.case_id, !status.record_only))}>
                {status.record_only ? "Switch off" : "Switch on"}
              </button>
            )}
            <span className="muted">{status.withheld_total} statement{status.withheld_total === 1 ? "" : "s"} withheld so far · {status.filings_assigned} filing{status.filings_assigned === 1 ? "" : "s"} assigned to a party</span>
            <button type="button" onClick={onOpenIssues}>Open the Issue Matrix</button>
          </div>
        )}
        {status && !status.record_only && <div className="notice warn" style={{ marginTop: 8 }}>With record-only off, statements that are not supported by the record are shown, marked “Not from the record”. Switch it back on before relying on anything here.</div>}
        {edition.pilot?.court && <div className="muted" style={{ fontSize: "0.78rem" }}>Configured for {edition.pilot.court}.</div>}
        {status && Object.values(status.capabilities).some((on) => !on) && (
          <div className="muted" style={{ fontSize: "0.78rem" }}>Switched off by this court’s configuration: {Object.entries(status.capabilities).filter(([, on]) => !on).map(([c]) => c.replace(/_/g, " ")).join(", ")}.</div>
        )}
      </div>

      <div className="card" style={{ gridColumn: "1 / -1" }}>
        <h4>Ask the record</h4>
        <p className="muted" style={{ fontSize: "0.8rem" }}>Ask where the record or installed law speaks to a point. The answer is a list of statements, each tied to a passage you can read. It does not say how the matter should be decided.</p>
        <textarea value={question} onChange={(e) => setQuestion(e.target.value)} rows={3} maxLength={2000} style={{ width: "100%" }} aria-label="Question about the record" placeholder="e.g. What do the pleadings say about when the plaintiff came to know of the possession?" />
        <div className="row" style={{ gap: 8, alignItems: "center", marginTop: 6 }}>
          <button type="button" disabled={busy || !question.trim()} onClick={() => void ask()}>{busy ? "Reading the record…" : "Ask the record"}</button>
          {busy && <span className="muted" style={{ fontSize: "0.8rem" }}>This can take several minutes on a local model.</span>}
        </div>
        {result && (
          <div style={{ marginTop: 10 }}>
            {result.statements.length === 0 && <div className="notice warn">Nothing could be shown from the record or installed law for this question.</div>}
            <ol style={{ paddingLeft: "1.2rem", margin: "8px 0" }}>
              {result.statements.map((s) => {
                const c = CLASS[s.class] ?? CLASS.EXTRA_RECORD;
                return (
                  <li key={s.proposition_id} style={{ marginBottom: 10, fontSize: "0.88rem" }}>
                    <span style={{ color: c.color, fontWeight: 600, fontSize: "0.75rem" }}>{c.label}</span>
                    <span className="mono muted" style={{ fontSize: "0.68rem" }}> · {s.class}</span>
                    <div>{s.text}</div>
                    {s.class === "INFERENCE_FROM_RECORD" && <div className="muted" style={{ fontSize: "0.75rem" }}>Rests on: {s.bases.join(", ")}</div>}
                    {s.locators.map((l, i) => (
                      <div key={i} style={{ fontSize: "0.78rem" }}>
                        <blockquote className="muted" style={{ margin: "2px 0 0 0", borderLeft: `2px solid ${c.color}`, paddingLeft: 6 }}>“{String(l.excerpt ?? "")}”</blockquote>
                        <div className="muted" style={{ fontSize: "0.72rem" }}>
                          {l.kind === "MATTER" ? `${docName(l.doc_id)}${l.page ? `, page ${l.page}` : ""}${l.paragraph ? `, ${l.paragraph}` : ""}` : `Authority ${String(l.authority_id ?? "")}${l.paragraph ? `, ${String(l.paragraph)}` : ""}`}
                        </div>
                        {l.kind === "LAW" && <CurrencyStamp packId={packOf(l.chunk_id)} />}
                      </div>
                    ))}
                  </li>
                );
              })}
            </ol>
            {result.withheld_count > 0 && (
              <div className="muted" style={{ fontSize: "0.8rem" }}>
                {result.withheld_count} statement{result.withheld_count === 1 ? "" : "s"} withheld because {Object.entries(result.withheld_by_reason).map(([r, n]) => `${n} ${WHY[r] ?? r}`).join("; ")}. Only a hash of each is kept in the matter’s log.
              </div>
            )}
            <div className="muted" style={{ fontSize: "0.72rem", marginTop: 4 }}>
              {result.record_coverage.passages_offered_to_the_model} of {result.record_coverage.passages_in_record} passages of the record were offered to the model (those sharing words with the question{result.record_coverage.passages_shortened ? `; ${result.record_coverage.passages_shortened} shortened` : ""}). {result.disclaimer}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
