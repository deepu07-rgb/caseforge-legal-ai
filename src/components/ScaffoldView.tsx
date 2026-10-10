import { useCallback, useEffect, useState } from "react";
import type { AuditReport, CaseSummary, ScaffoldSection, ScaffoldState } from "../lib/ipc";
import { scaffoldAuditText, scaffoldExport, scaffoldGet, scaffoldPrefill, scaffoldRemoveSentence, scaffoldSetText } from "../lib/ipc";
import { AUDIT_ORDER, AuditRow } from "./AuditPanel";

/**
 * B-S4-21: Judgment scaffold. Seven sections can be filled from the record, each sentence the record's own words with the
 * place it comes from. Analysis, findings and the operative order are written only by the judge: there is no button that
 * drafts them and nothing typed in them is sent to any model. Spellcheck is the browser's; the citation check is the
 * existing audit; export carries the footer naming which sections were filled from the record.
 */
const HEADINGS: Record<string, string> = {
  CAUSE_TITLE: "Cause title", APPEARANCES: "Appearances", BRIEF_FACTS: "Brief facts (what the filings agree on)", PROCEDURAL_HISTORY: "Procedural history", SUBMISSIONS: "Submissions of each party",
  ISSUES: "Issues (as you accepted them)", EVIDENCE_SUMMARY: "Evidence summary", ANALYSIS: "Analysis", FINDINGS: "Findings", OPERATIVE_ORDER: "Operative order",
};
const ERROR: Record<string, string> = {
  CF_SCAFFOLD_NONE: "Create the scaffold from the record first.",
  CF_EDITION_NOT_ENTITLED: "The judgment scaffold belongs to the Chambers Edition.",
  CF_EDITION_PILOT_DISABLED: "This court’s configuration has switched this off.",
};
function plain(e: unknown): string {
  const s = String(e);
  const k = Object.keys(ERROR).find((x) => s.includes(x));
  return k ? ERROR[k] : s;
}
function mine(section: ScaffoldSection): string {
  return section.sentences.filter((s) => s.origin === "JUDGE_AUTHORED" && s.locators.length === 0).map((s) => s.text).join(" ");
}

export default function ScaffoldView({ activeCase }: { activeCase: CaseSummary | null }) {
  const caseId = activeCase?.case_id ?? null;
  const [state, setState] = useState<ScaffoldState | null>(null);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [checks, setChecks] = useState<Record<string, AuditReport>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!caseId) return;
    try {
      const s = await scaffoldGet(caseId);
      setState(s);
      setDrafts(Object.fromEntries((s.scaffold?.sections ?? []).map((x) => [x.section_kind, mine(x)])));
      setError(null);
    } catch (e) {
      setError(plain(e));
    }
  }, [caseId]);
  useEffect(() => {
    setState(null);
    void load();
  }, [load]);

  async function act(fn: () => Promise<ScaffoldState | void>) {
    setBusy(true);
    setError(null);
    try {
      const s = await fn();
      if (s) {
        setState(s);
        setDrafts(Object.fromEntries((s.scaffold?.sections ?? []).map((x) => [x.section_kind, mine(x)])));
      }
    } catch (e) {
      setError(plain(e));
    } finally {
      setBusy(false);
    }
  }
  if (!activeCase) return <div className="card"><p className="muted">Open a matter to prepare its judgment scaffold.</p></div>;
  const docName = (id: unknown) => state?.documents[String(id)] ?? String(id ?? "");
  const download = async () => {
    const text = await scaffoldExport(activeCase.case_id);
    const url = URL.createObjectURL(new Blob([text], { type: "text/plain;charset=utf-8" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = "judgment-scaffold.txt";
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="grid" style={{ gap: 12 }}>
      <div className="card" style={{ gridColumn: "1 / -1" }}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline", flexWrap: "wrap", gap: 8 }}>
          <h4>Judgment scaffold</h4>
          <div className="row" style={{ gap: 6 }}>
            <button type="button" disabled={busy} onClick={() => void act(() => scaffoldPrefill(activeCase.case_id))}>{state?.exists ? "Refresh from the record" : "Fill from the record"}</button>
            <button type="button" disabled={busy || !state?.exists} onClick={() => void act(download)}>Download (.txt)</button>
          </div>
        </div>
        <p className="muted" style={{ fontSize: "0.8rem" }}>
          The sections above Analysis are filled from the record, in the record’s own words, each with where it comes from; you can remove any sentence or add your own. <strong>Analysis, Findings and the Operative order are yours alone</strong>: CaseForge cannot draft them and nothing you type there is sent to any model. Once the scaffold exists, record-only mode stays on for this matter.
        </p>
        {error && <div className="notice warn">{error}</div>}
        {state && state.problems.length > 0 && <div className="notice warn">Some pre-filled lines no longer match the documents (a document may have changed). Refresh from the record: {state.problems.slice(0, 3).join("; ")}</div>}
      </div>

      {state?.scaffold?.sections.map((sec) => {
        const locked = sec.assist_policy === "LOCKED_JUDGE_ONLY";
        const check = checks[sec.section_kind];
        return (
          <div className="card" key={sec.section_kind} style={{ gridColumn: "1 / -1" }}>
            <h4>{HEADINGS[sec.section_kind]}</h4>
            {locked ? (
              <div className="muted" style={{ fontSize: "0.78rem" }}>Written by the judge only. There is no way to ask CaseForge to draft this section.</div>
            ) : (
              <>
                {sec.sentences.filter((s) => s.origin === "MACHINE_PREFILL" || s.locators.some((l) => l.kind === "ISSUE_DECISION")).length === 0 && <div className="muted" style={{ fontSize: "0.82rem" }}>Nothing found on the record for this section.</div>}
                <ul style={{ paddingLeft: "1.1rem", margin: "4px 0" }}>
                  {sec.sentences.map((s, i) =>
                    s.origin === "MACHINE_PREFILL" || s.locators.some((l) => l.kind === "ISSUE_DECISION") ? (
                      <li key={i} style={{ fontSize: "0.86rem", marginBottom: 4 }}>
                        <span style={{ whiteSpace: "pre-wrap" }}>{s.text}</span>{" "}
                        <span className="muted" style={{ fontSize: "0.72rem" }}>
                          {s.locators.map((l, k) => <span key={k}>{k > 0 ? "; " : ""}{l.kind === "MATTER" ? `${docName(l.doc_id)}, page ${l.page}` : l.kind === "ISSUE_DECISION" ? `accepted by you (${String(l.decision).toLowerCase()})` : "matter details"}</span>)}
                        </span>{" "}
                        <button type="button" className="no-print" disabled={busy} onClick={() => void act(() => scaffoldRemoveSentence(activeCase.case_id, sec.section_kind, i))}>Remove</button>
                      </li>
                    ) : null,
                  )}
                </ul>
              </>
            )}
            <textarea
              value={drafts[sec.section_kind] ?? ""}
              onChange={(e) => setDrafts((d) => ({ ...d, [sec.section_kind]: e.target.value }))}
              rows={locked ? 7 : 3}
              spellCheck
              lang="en"
              style={{ width: "100%" }}
              aria-label={`Your text for ${HEADINGS[sec.section_kind]}`}
              placeholder={locked ? "Write this section yourself…" : "Add your own sentences…"}
            />
            <div className="row" style={{ gap: 6, marginTop: 4, flexWrap: "wrap" }}>
              <button type="button" disabled={busy} onClick={() => void act(() => scaffoldSetText(activeCase.case_id, sec.section_kind, drafts[sec.section_kind] ?? ""))}>Save my text</button>
              <button type="button" disabled={busy || !(drafts[sec.section_kind] ?? "").trim()} onClick={() => void act(async () => { await scaffoldSetText(activeCase.case_id, sec.section_kind, drafts[sec.section_kind] ?? ""); const r = await scaffoldAuditText(activeCase.case_id, sec.section_kind); setChecks((c) => ({ ...c, [sec.section_kind]: r })); return scaffoldGet(activeCase.case_id); })}>Check the citations in my text</button>
            </div>
            {check && (
              <div style={{ marginTop: 6 }}>
                {check.items.length === 0 && <div className="muted" style={{ fontSize: "0.8rem" }}>No citations or quotations found in your text.</div>}
                <ul style={{ paddingLeft: "1.1rem", margin: 0 }}>
                  {[...check.items].sort((a, b) => AUDIT_ORDER.indexOf(a.result) - AUDIT_ORDER.indexOf(b.result) || a.draft_char_start - b.draft_char_start).map((item) => <AuditRow key={item.item_id} item={item} whose="your" />)}
                </ul>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
