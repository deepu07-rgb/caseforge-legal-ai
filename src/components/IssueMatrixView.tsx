import { useCallback, useEffect, useState } from "react";
import type { CaseSummary, IssueMatrixState, MatrixIssue, MatrixLocator } from "../lib/ipc";
import { issueMatrixAdd, issueMatrixBuild, issueMatrixDecide, issueMatrixMemo, issueMatrixSetFiling, issueMatrixSetMatterType } from "../lib/ipc";

/**
 * B-S4-18: Issue Matrix. A neutral, side-by-side view of each issue the judge has accepted: what each party has pleaded,
 * which facts the filings agree on and which they dispute, and arguments one side never answered. The same rules run for
 * every party; nothing is ranked or scored and no answer to an issue is suggested. Proposed issues do nothing until the
 * judge accepts, edits, merges or rejects them.
 */
const TONE = { ok: "var(--ok, #2f7d4f)", warn: "var(--warn, #9a6a00)", muted: "var(--muted, #5b6470)" };
const ORIGIN: Record<string, string> = { TEMPLATE: "from an issue template", GRAPH_ISSUE: "found in the pleadings", JUDGE_AUTHORED: "written by the judge" };
const KIND: Record<string, string> = { RECORD: "statement of fact", LAW: "cites law", INFERENCE_FROM_RECORD: "contention" };
const AUDIT: Record<string, string> = {
  VERIFIED: "found in the installed law",
  VERIFIED_WITH_NOTE: "found in the installed law, with a note",
  MISQUOTED: "the quotation differs from the installed text",
  NOT_IN_CORPUS: "not in the installed law",
  SUPERSEDED: "the installed law says it has been superseded",
  NOT_APPLICABLE: "not applicable here",
  CUSTOM_SOURCE: "from a source you added",
};
const ERROR: Record<string, string> = {
  CF_ISSUE_MATRIX_PARTIES: "The matter needs at least two parties. Add them in Case Material & Review.",
  CF_ISSUE_MERGE_TARGET: "An issue can only be merged into an issue you have accepted, and an issue that others are merged into cannot be rejected until they are sent back.",
  CF_ISSUE_TEXT: "Write the issue first.",
  CF_ISSUE_DECISION: "That change is not possible for this issue.",
  CF_ISSUE_UNKNOWN: "That issue is no longer in the matter.",
  CF_ISSUE_DOC_UNKNOWN: "That document is no longer in the matter.",
  CF_ISSUE_PARTY_UNKNOWN: "That party is no longer in the matter.",
  CF_ISSUE_FILING_PARTY: "A document is assigned to a party that is not in the matter. Assign it again.",
  CF_ISSUE_MATTER_TYPE: "No installed template covers that kind of matter.",
};

function plain(e: unknown): string {
  const s = String(e);
  const key = Object.keys(ERROR).find((k) => s.includes(k));
  return key ? ERROR[key] : s;
}
function where(l: MatrixLocator): string {
  return `${l.doc_id}${l.page_index === null ? "" : `, page ${l.page_index + 1}`}, characters ${l.char_start}–${l.char_end}`;
}
function download(name: string, text: string) {
  const url = URL.createObjectURL(new Blob([text], { type: "text/plain;charset=utf-8" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

export default function IssueMatrixView({ activeCase }: { activeCase: CaseSummary | null }) {
  const caseId = activeCase?.case_id ?? null;
  const [state, setState] = useState<IssueMatrixState | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [editing, setEditing] = useState<Record<string, string>>({});
  const [mergeInto, setMergeInto] = useState<Record<string, string>>({});

  const refresh = useCallback(async () => {
    if (!caseId) return;
    try {
      setState(await issueMatrixBuild(caseId));
      setError(null);
    } catch (e) {
      setState(null);
      setError(plain(e));
    }
  }, [caseId]);
  useEffect(() => {
    setState(null);
    void refresh();
  }, [refresh]);

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

  if (!activeCase) return <div className="card"><p className="muted">Open a matter to build its issue matrix.</p></div>;
  const matrix = state?.matrix;
  const party = (id: string) => {
    const p = matrix?.parties.find((x) => x.party_id === id);
    return p ? `${p.display_name} (${p.role})` : id;
  };
  const live = (matrix?.issues ?? []).filter((i) => i.judge_decision === "ACCEPTED" || i.judge_decision === "EDITED");
  const proposed = (matrix?.issues ?? []).filter((i) => i.judge_decision === "PROPOSED");
  const set_aside = (matrix?.issues ?? []).filter((i) => i.judge_decision === "MERGED" || i.judge_decision === "REJECTED");
  const decide = (i: MatrixIssue, decision: string, text: string | null = null, into: string | null = null) => act(() => issueMatrixDecide(activeCase.case_id, i.issue_id, decision, text, into));

  const decisionRow = (i: MatrixIssue) => (
    <div className="row" style={{ gap: 6, flexWrap: "wrap", alignItems: "center", marginTop: 4 }}>
      {i.judge_decision === "PROPOSED" && <button type="button" disabled={busy} onClick={() => void decide(i, "ACCEPTED")}>Accept</button>}
      <input value={editing[i.issue_id] ?? ""} placeholder="Reword the issue…" aria-label={`Reworded text for: ${i.text}`} onChange={(e) => setEditing((s) => ({ ...s, [i.issue_id]: e.target.value }))} style={{ minWidth: 220 }} />
      <button type="button" disabled={busy || !(editing[i.issue_id] ?? "").trim()} onClick={() => void decide(i, "EDITED", editing[i.issue_id])}>Accept reworded</button>
      <select value={mergeInto[i.issue_id] ?? ""} onChange={(e) => setMergeInto((s) => ({ ...s, [i.issue_id]: e.target.value }))} aria-label={`Merge into another issue: ${i.text}`}>
        <option value="">Merge into…</option>
        {live.filter((x) => x.issue_id !== i.issue_id).map((x) => <option key={x.issue_id} value={x.issue_id}>{x.text.slice(0, 70)}</option>)}
      </select>
      <button type="button" disabled={busy || !mergeInto[i.issue_id]} onClick={() => void decide(i, "MERGED", null, mergeInto[i.issue_id])}>Merge</button>
      <button type="button" disabled={busy} onClick={() => void decide(i, "REJECTED")}>Reject</button>
    </div>
  );

  return (
    <div className="grid" style={{ gap: 12 }}>
      <div className="card" style={{ gridColumn: "1 / -1" }}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline" }}>
          <h4>Issues and the record</h4>
          <button type="button" disabled={busy || live.length === 0} onClick={() => void act(async () => download("issue-matrix-bench-memo.txt", await issueMatrixMemo(activeCase.case_id)))}>Download bench memo (.txt)</button>
        </div>
        <p className="muted" style={{ fontSize: "0.8rem" }}>
          For each issue you accept, this shows what each side has pleaded, the facts the filings agree on, the facts they dispute, and arguments the other side never answered, each with where it is on the record. The same rules are applied to every party. It ranks nothing, scores nothing and suggests no answer; every decision is the judge&apos;s. The lists come from fixed rules applied to the filings, so a reader should check each against the paragraph it points to.
        </p>
        {error && <div className="notice warn">{error}</div>}
        {state?.warnings.map((w) => <div className="notice warn" key={w}>{w}</div>)}
      </div>

      <div className="card" style={{ gridColumn: "1 / -1" }}>
        <h4>Filings</h4>
        <p className="muted" style={{ fontSize: "0.8rem" }}>Say whose pleading or brief each document is. Only assigned documents are read.</p>
        {state?.docs.length === 0 && <p className="muted">No documents in this matter yet.</p>}
        {state?.docs.map((d) => {
          const assigned = state.filings.find((f) => f.doc_id === d.doc_id)?.party_id ?? "";
          return (
            <div className="row" key={d.doc_id} style={{ gap: 6, alignItems: "center", marginBottom: 4, fontSize: "0.85rem" }}>
              <span style={{ minWidth: 240 }}>{d.filename}</span>
              <select value={assigned} disabled={busy} aria-label={`Whose filing is ${d.filename}`} onChange={(e) => void act(() => issueMatrixSetFiling(activeCase.case_id, d.doc_id, e.target.value || null))}>
                <option value="">Not a party&apos;s filing</option>
                {matrix?.parties.map((p) => <option key={p.party_id} value={p.party_id}>{p.display_name} ({p.role})</option>)}
              </select>
            </div>
          );
        })}
      </div>

      <div className="card" style={{ gridColumn: "1 / -1" }}>
        <h4>Issues</h4>
        {state && state.matter_types.length > 0 && (
          <div className="row" style={{ gap: 6, alignItems: "center", marginBottom: 6 }}>
            <label htmlFor="im-matter-type">Kind of matter (for issue templates)</label>
            <select id="im-matter-type" value={state.matter_type ?? ""} disabled={busy} onChange={(e) => void act(() => issueMatrixSetMatterType(activeCase.case_id, e.target.value || null))}>
              <option value="">None</option>
              {state.matter_types.map((m) => <option key={m} value={m}>{m.toLowerCase().replace(/_/g, " ")}</option>)}
            </select>
          </div>
        )}
        <div className="row" style={{ gap: 6, marginBottom: 8 }}>
          <input value={draft} onChange={(e) => setDraft(e.target.value)} placeholder="Write an issue…" aria-label="Write an issue" style={{ flex: 1 }} />
          <button type="button" disabled={busy || !draft.trim()} onClick={() => void act(async () => { await issueMatrixAdd(activeCase.case_id, draft); setDraft(""); })}>Add issue</button>
        </div>
        {proposed.length > 0 && <h5>Proposed — nothing below is used until you accept it</h5>}
        {proposed.map((i) => (
          <div key={i.issue_id} style={{ marginBottom: 10 }}>
            <div>{i.text}</div>
            <div className="muted" style={{ fontSize: "0.78rem" }}>{ORIGIN[i.origin]}{i.template_rule_id ? ` · rule ${i.template_rule_id}` : ""}</div>
            {decisionRow(i)}
          </div>
        ))}
        {proposed.length === 0 && live.length === 0 && <p className="muted">No issues yet. Write one above, or assign the pleadings and set the kind of matter to see proposals.</p>}
        {set_aside.length > 0 && (
          <details style={{ marginTop: 8 }}>
            <summary className="muted">Merged and rejected issues ({set_aside.length})</summary>
            {set_aside.map((i) => (
              <div key={i.issue_id} className="row" style={{ gap: 6, alignItems: "center", fontSize: "0.85rem", marginTop: 4 }}>
                <span>{i.text}</span>
                <span className="muted">— {i.judge_decision === "MERGED" ? `merged into “${matrix?.issues.find((x) => x.issue_id === i.merged_into)?.text.slice(0, 60) ?? i.merged_into}”` : "rejected"}</span>
                <button type="button" disabled={busy} onClick={() => void decide(i, i.origin === "JUDGE_AUTHORED" ? "ACCEPTED" : "PROPOSED")}>{i.origin === "JUDGE_AUTHORED" ? "Accept again" : "Send back to proposals"}</button>
              </div>
            ))}
          </details>
        )}
      </div>

      {live.map((issue, n) => (
        <div className="card" key={issue.issue_id} style={{ gridColumn: "1 / -1" }}>
          <h4>Issue {n + 1}: {issue.text}</h4>
          <div className="muted" style={{ fontSize: "0.78rem" }}>{ORIGIN[issue.origin]}{issue.judge_decision === "EDITED" ? " · reworded by the judge" : ""}</div>
          <div className="grid" style={{ gridTemplateColumns: `repeat(${Math.max(1, matrix?.parties.length ?? 2)}, minmax(0, 1fr))`, gap: 12, marginTop: 8 }}>
            {issue.positions.map((pos) => (
              <div key={pos.party_id}>
                <strong>{party(pos.party_id)}</strong>
                {pos.sentences.length === 0 && <p className="muted" style={{ fontSize: "0.85rem" }}>No paragraph of their assigned filings touches this issue.</p>}
                <ul style={{ paddingLeft: "1.1rem", margin: "4px 0", fontSize: "0.85rem" }}>
                  {pos.sentences.map((s, k) => (
                    <li key={k} style={{ marginBottom: 6 }}>
                      {s.text}
                      <div className="muted" style={{ fontSize: "0.72rem" }}>{KIND[s.classification]} · {where(s.locators[0] as unknown as MatrixLocator)}</div>
                    </li>
                  ))}
                </ul>
                {pos.evidence.length > 0 && (
                  <div style={{ fontSize: "0.8rem" }}>
                    Refers to: {pos.evidence.map((e) => e.excerpt).join(", ")}
                  </div>
                )}
                {pos.authorities.length > 0 && (
                  <ul style={{ paddingLeft: "1.1rem", margin: "4px 0", fontSize: "0.8rem" }}>
                    {pos.authorities.map((a) => <li key={a.audit_item_id}>Authority check: {AUDIT[a.result] ?? a.result}</li>)}
                  </ul>
                )}
              </div>
            ))}
          </div>
          <div className="grid" style={{ gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: 12, marginTop: 10 }}>
            <div>
              <strong style={{ color: TONE.ok }}>Agreed</strong>
              {issue.admitted_facts.length === 0 && <p className="muted" style={{ fontSize: "0.85rem" }}>None found.</p>}
              <ul style={{ paddingLeft: "1.1rem", margin: "4px 0", fontSize: "0.85rem" }}>
                {issue.admitted_facts.map((f) => (
                  <li key={f.fact_node_id}>{f.locators[0].excerpt}<div className="muted" style={{ fontSize: "0.72rem" }}>{f.party_ids.map(party).join(", ")} · {where(f.locators[0])}</div></li>
                ))}
              </ul>
            </div>
            <div>
              <strong style={{ color: TONE.warn }}>Disputed</strong>
              {issue.disputed_facts.length === 0 && <p className="muted" style={{ fontSize: "0.85rem" }}>None found.</p>}
              <ul style={{ paddingLeft: "1.1rem", margin: "4px 0", fontSize: "0.85rem" }}>
                {issue.disputed_facts.map((f) => (
                  <li key={f.fact_node_id}>{f.locators[0].excerpt}<div className="muted" style={{ fontSize: "0.72rem" }}>{f.party_ids.map(party).join(", ")} · {where(f.locators[0])}</div></li>
                ))}
              </ul>
            </div>
            <div>
              <strong>Not answered</strong>
              {issue.unanswered_arguments.length === 0 && <p className="muted" style={{ fontSize: "0.85rem" }}>None found.</p>}
              <ul style={{ paddingLeft: "1.1rem", margin: "4px 0", fontSize: "0.85rem" }}>
                {issue.unanswered_arguments.map((u) => (
                  <li key={`${u.argument_node_id}-${u.unanswered_by_party_id}`}>
                    {u.locator.excerpt}
                    <div className="muted" style={{ fontSize: "0.72rem" }}>Argument of {party(u.by_party_id)}; no answer found from {party(u.unanswered_by_party_id)} · {where(u.locator)}</div>
                  </li>
                ))}
              </ul>
            </div>
          </div>
          {decisionRow(issue)}
          {issue.origin !== "JUDGE_AUTHORED" && (
            <div className="row" style={{ marginTop: 4 }}>
              <button type="button" disabled={busy} onClick={() => void decide(issue, "PROPOSED")}>Send back to proposals</button>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
