import { useState } from "react";
import { codeBridgeAnnotate, codeBridgeSetKeyDate, type CodeBridgeResult, type DateKind, type StatuteMapping } from "../lib/ipc";

/**
 * B-S4-09: Code Bridge. Each statutory reference with its successor/predecessor and the governing code, computed only
 * from installed, legal-editor signed succession tables and the key dates counsel enters. No table → "not mapped".
 */
const CLASS_LABEL: Record<StatuteMapping["equivalence"], string> = {
  IDENTICAL: "Same text",
  RENUMBERED: "Renumbered",
  MODIFIED: "Changed",
  NEW: "New provision",
  OMITTED: "Omitted",
  UNKNOWN: "Not mapped",
};
const QUESTION: Record<DateKind, string> = {
  OFFENCE_DATE: "When was the offence committed?",
  INSTITUTION_DATE: "When were the proceedings instituted?",
  FILING_DATE: "When was it filed?",
  CAUSE_OF_ACTION_DATE: "When did the cause of action arise?",
  EVENT_DATE: "When did the relevant event happen?",
};
const TONE = { ok: "var(--ok, #2f7d4f)", warn: "var(--warn, #9a6a00)", bad: "var(--danger, #a33a2b)", muted: "var(--muted, #5b6470)" };

export function CodeBridgePanel({ caseId, text }: { caseId: string; text: string }) {
  const [result, setResult] = useState<CodeBridgeResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dateDraft, setDateDraft] = useState<Partial<Record<DateKind, string>>>({});

  async function run(fn?: () => Promise<unknown>) {
    setBusy(true);
    setError(null);
    try {
      if (fn) await fn();
      setResult(await codeBridgeAnnotate(caseId, text, "DRAFT"));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const asked = new Set(
    (result?.annotations ?? []).map((a) => a.annotation.governing).filter((g) => g.status === "NEEDS_COUNSEL_INPUT" && g.basis_date_kind).map((g) => g.basis_date_kind as DateKind),
  );
  const known = new Map((result?.key_dates ?? []).map((d) => [d.kind, d.date]));
  const dateKinds = Array.from(new Set<DateKind>([...asked, ...(Array.from(known.keys()) as DateKind[])]));

  return (
    <div className="card" style={{ gridColumn: "1 / -1" }}>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline" }}>
        <h4>Code Bridge — old and new codes</h4>
        <button type="button" onClick={() => void run()} disabled={busy || !text.trim()}>
          {busy ? "Checking…" : result ? "Re-check" : "Check statutory references"}
        </button>
      </div>
      <p className="muted" style={{ fontSize: "0.8rem" }}>
        For every section cited: its successor or predecessor, and which code governs this case by its dates. Only signed succession tables in installed legal packs are used; nothing is guessed.
      </p>
      {error && <div className="notice warn">{error}</div>}
      {result && result.succession_rows === 0 && (
        <div className="notice warn">No succession table is installed for this jurisdiction yet, so every reference shows "Not mapped". Install a legal pack that carries a signed succession table.</div>
      )}
      {result && result.signed_rows < result.succession_rows && (
        <div className="notice warn">Some installed succession rows are not signed by a legal editor; they are shown for reference only and are never treated as verified.</div>
      )}
      {dateKinds.length > 0 && (
        <div style={{ margin: "6px 0", fontSize: "0.85rem" }}>
          {dateKinds.map((k) => (
            <div key={k} className="row" style={{ gap: 6, alignItems: "center", marginBottom: 4 }}>
              <span style={{ minWidth: 260 }}>{QUESTION[k]}</span>
              <input type="date" value={dateDraft[k] ?? known.get(k) ?? ""} onChange={(e) => setDateDraft((d) => ({ ...d, [k]: e.target.value }))} />
              <button type="button" disabled={busy || !(dateDraft[k] ?? "").trim()} onClick={() => void run(() => codeBridgeSetKeyDate(caseId, k, dateDraft[k] ?? null))}>
                Save
              </button>
              {known.has(k) && (
                <button type="button" disabled={busy} onClick={() => void run(() => codeBridgeSetKeyDate(caseId, k, null))}>
                  Clear
                </button>
              )}
            </div>
          ))}
        </div>
      )}
      {result && (
        <ul style={{ paddingLeft: "1.1rem", margin: 0 }}>
          {result.annotations.length === 0 && <li className="muted">No statutory references found in this draft.</li>}
          {result.annotations.map(({ annotation: a, trust }) => {
            const g = a.governing;
            const mismatch = g.status === "DETERMINED" && g.governing_code && g.governing_code !== a.code;
            return (
              <li key={a.reference_id} style={{ marginBottom: 8, fontSize: "0.85rem" }}>
                <span className="mono">{a.ref_text}</span>{" "}
                <span style={{ fontWeight: 600, color: a.equivalence === "UNKNOWN" ? TONE.muted : a.equivalence === "IDENTICAL" ? TONE.ok : TONE.warn }}>{CLASS_LABEL[a.equivalence]}</span>
                {a.counterpart && (
                  <span> · {a.counterpart.direction === "SUCCESSOR" ? "now" : "earlier"} {a.counterpart.code} {a.counterpart.provision}</span>
                )}
                {trust === "DEVELOPMENT_ONLY" && <span style={{ color: TONE.warn }}> · unsigned table (reference only)</span>}
                <div className="muted">
                  {g.status === "DETERMINED" && (
                    <span style={{ color: mismatch ? TONE.bad : undefined }}>
                      Governing code: {g.governing_code} ({g.basis_date_kind?.toLowerCase().replace(/_/g, " ")} {g.basis_date}){mismatch ? ` — this reference cites ${a.code}` : ""}
                    </span>
                  )}
                  {g.status === "NEEDS_COUNSEL_INPUT" && <span style={{ color: TONE.warn }}>Governing code needs your input: {g.basis_date_kind ? QUESTION[g.basis_date_kind] : "the transition rules disagree"}</span>}
                  {g.status === "UNKNOWN" && <span>Governing code: no transition rule installed</span>}
                </div>
                {a.side_by_side && (
                  <div className="row" style={{ gap: 8, marginTop: 4, alignItems: "stretch" }}>
                    <div style={{ flex: 1, fontSize: "0.78rem", whiteSpace: "pre-wrap" }}><div className="muted">Old text</div>{a.side_by_side.old.excerpt}</div>
                    <div style={{ flex: 1, fontSize: "0.78rem", whiteSpace: "pre-wrap" }}><div className="muted">New text</div>{a.side_by_side.new.excerpt}</div>
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
      {result && <div className="muted mono" style={{ fontSize: "0.7rem", marginTop: 6 }}>{result.pipeline_id} · {result.signed_rows}/{result.succession_rows} signed succession rows · deterministic, no AI model</div>}
    </div>
  );
}
