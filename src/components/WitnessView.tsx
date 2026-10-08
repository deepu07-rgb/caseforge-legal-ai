import { useEffect, useMemo, useState } from "react";
import {
  impeachFindingStatus,
  impeachModelScan,
  witnessDecisionAdd,
  witnessDecisionRevoke,
  witnessMatrixBuild,
  witnessPropose,
  witnessRegisterList,
  witnessRegisterRemove,
  witnessRegisterSave,
  type CaseSummary,
  type DocumentMeta,
  type ImpeachFinding,
  type MatrixWitness,
  type WitnessBuildResult,
  type WitnessRegisterEntry,
  type WitnessRegisterState,
} from "../lib/ipc";
import { downloadBytes, renderDocx, renderPdf } from "../lib/exportDraft";
import { DIMENSION_LABEL, KIND_LABEL, crossOutline, kitDocument, locatorLabel, oneLine, statementLabel } from "../lib/impeachKit";

/**
 * B-S4-07/08: Witnesses. Register each statement on record against its witness, line the statements up by topic,
 * and review the contradictions and omissions found between them. Rule-based; the optional local-AI pass only adds
 * findings whose quotations match the stored statements exactly.
 */
const TONE = { bad: "var(--danger, #a33a2b)", warn: "var(--warn, #9a6a00)", ok: "var(--ok, #2f7d4f)" };

export default function WitnessView({
  activeCase,
  documents,
  modelReady,
  onRehearse,
}: {
  activeCase: CaseSummary | null;
  documents: DocumentMeta[];
  modelReady: boolean;
  onRehearse: (draft: string) => void;
}) {
  const caseId = activeCase?.case_id ?? null;
  const [state, setState] = useState<WitnessRegisterState | null>(null);
  const [built, setBuilt] = useState<WitnessBuildResult | null>(null);
  const [selected, setSelected] = useState<string>("");
  const [edits, setEdits] = useState<Record<string, WitnessRegisterEntry>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [splitName, setSplitName] = useState<Record<string, string>>({});
  const [openOutline, setOpenOutline] = useState<Record<string, boolean>>({});

  const docName = useMemo(() => {
    const names = new Map(documents.map((d) => [d.doc_id, d.original_filename]));
    return (id: string) => names.get(id) ?? id;
  }, [documents]);

  async function refresh(rebuild = true) {
    if (!caseId) return;
    const s = await witnessRegisterList(caseId);
    setState(s);
    setEdits({});
    if (rebuild && s.register.length > 0) {
      const b = await witnessMatrixBuild(caseId);
      setBuilt(b);
      setSelected((cur) => (b.matrix.witnesses.some((w) => w.witness_id === cur) ? cur : b.matrix.witnesses[0]?.witness_id ?? ""));
    } else if (s.register.length === 0) {
      setBuilt(null);
    }
  }

  async function run(label: string, fn: () => Promise<unknown>, after = true) {
    setBusy(label);
    setError(null);
    setNotice(null);
    try {
      await fn();
      if (after) await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  }

  useEffect(() => {
    setState(null);
    setBuilt(null);
    setSelected("");
    if (caseId) void run("Loading", () => refresh(), false);
  }, [caseId]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!activeCase || !caseId) {
    return (
      <div className="card">
        <p className="muted">Open a matter first. Witness statements are read from the documents uploaded to that matter.</p>
      </div>
    );
  }

  const register = state?.register ?? [];
  const unregistered = documents.filter((d) => !register.some((r) => r.doc_id === d.doc_id));
  const kinds = state?.statement_kinds.filter((k) => k.applies) ?? [];
  const witness: MatrixWitness | undefined = built?.matrix.witnesses.find((w) => w.witness_id === selected);
  const kit = built?.kits.find((k) => k.witness_id === selected);
  const others = built?.matrix.witnesses.filter((w) => w.witness_id !== selected) ?? [];
  const row = (e: WitnessRegisterEntry) => edits[e.doc_id] ?? e;
  const setRow = (e: WitnessRegisterEntry, patch: Partial<WitnessRegisterEntry>) => setEdits((cur) => ({ ...cur, [e.doc_id]: { ...row(e), ...patch } }));

  function exportKit(kind: "docx" | "pdf") {
    if (!kit || !witness) return;
    const doc = kitDocument({ kit, witness, activeCase, docName });
    const base = `Impeachment_kit_${witness.display_name.replace(/[^\w]+/g, "_")}`;
    if (kind === "docx") downloadBytes(`${base}.docx`, renderDocx(doc), "application/vnd.openxmlformats-officedocument.wordprocessingml.document");
    else downloadBytes(`${base}.pdf`, renderPdf(doc), "application/pdf");
  }

  function rehearse() {
    if (!kit || !witness) return;
    const lines = kit.findings
      .filter((f) => f.status !== "COUNSEL_REJECTED")
      .slice(0, 3)
      .flatMap((f) => crossOutline(f, witness, docName));
    onRehearse(`Cross-examination of ${witness.display_name}:\n${lines.join("\n")}`);
  }

  function FindingCard({ f, w }: { f: ImpeachFinding; w: MatrixWitness }) {
    const rejected = f.status === "COUNSEL_REJECTED";
    return (
      <li style={{ marginBottom: 10, opacity: rejected ? 0.55 : 1 }}>
        <div>
          <span style={{ fontWeight: 600, color: f.finding_kind === "OMISSION" ? TONE.warn : TONE.bad }}>
            {f.confrontation_order}. {DIMENSION_LABEL[f.dimension] ?? f.dimension}
          </span>
          {f.finding_kind === "OMISSION" && <span className="muted"> · for your judgement</span>}
          {f.detection === "MODEL_VERIFIED" && <span className="muted"> · suggested by the local AI, quotes checked word for word</span>}
          {f.status === "COUNSEL_ACCEPTED" && <span style={{ color: TONE.ok }}> · accepted</span>}
        </div>
        <div style={{ fontSize: "0.82rem", marginTop: 2 }}>
          <span className="muted">Earlier · {statementLabel(w, f.earlier.statement_id)}{f.earlier.locator ? ` · ${locatorLabel(f.earlier.locator, docName)}` : ""}: </span>
          {f.earlier.quote != null ? `“${oneLine(f.earlier.quote)}”` : <em>{f.earlier.absence_note}</em>}
        </div>
        <div style={{ fontSize: "0.82rem" }}>
          <span className="muted">Later · {statementLabel(w, f.later.statement_id)} · {locatorLabel(f.later.locator, docName)}: </span>“{oneLine(f.later.quote ?? "")}”
        </div>
        <div className="row" style={{ gap: 6, marginTop: 4 }}>
          <button type="button" disabled={!!busy} onClick={() => void run("Saving", () => impeachFindingStatus(caseId!, f.finding_id, f.status === "COUNSEL_ACCEPTED" ? "PROPOSED" : "COUNSEL_ACCEPTED"))}>
            {f.status === "COUNSEL_ACCEPTED" ? "Undo accept" : "Accept"}
          </button>
          <button type="button" disabled={!!busy} onClick={() => void run("Saving", () => impeachFindingStatus(caseId!, f.finding_id, rejected ? "PROPOSED" : "COUNSEL_REJECTED"))}>
            {rejected ? "Restore" : "Not useful"}
          </button>
          <button type="button" onClick={() => setOpenOutline((o) => ({ ...o, [f.finding_id]: !o[f.finding_id] }))}>
            {openOutline[f.finding_id] ? "Hide questions" : "Suggested questions"}
          </button>
        </div>
        {openOutline[f.finding_id] && (
          <ol style={{ fontSize: "0.8rem", marginTop: 4 }}>
            {crossOutline(f, w, docName).map((l, i) => (
              <li key={i}>{l}</li>
            ))}
          </ol>
        )}
      </li>
    );
  }

  return (
    <div className="grid" style={{ gap: 12 }}>
      <div className="card">
        <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline", flexWrap: "wrap", gap: 8 }}>
          <h4>Statements on record ({register.length})</h4>
          <button type="button" className="primary" disabled={!!busy} onClick={() => void run("Finding statements", () => witnessPropose(caseId))}>
            {busy === "Finding statements" ? "Reading documents…" : "Find witness statements in uploaded documents"}
          </button>
        </div>
        <p className="muted" style={{ fontSize: "0.8rem" }}>
          FIR, statements to police or the Magistrate, examination-in-chief and cross; declarations, affidavits and deposition transcripts (page:line). Check each row and save it: your corrections are kept every time the matrix is rebuilt.
        </p>
        {unregistered.length > 0 && (
          <div style={{ fontSize: "0.85rem", marginBottom: 6 }}>
            Add a document as a statement:{" "}
            <select value="" onChange={(e) => e.target.value && void run("Adding", () => witnessPropose(caseId, [e.target.value]))}>
              <option value="">Choose…</option>
              {unregistered.map((d) => (
                <option key={d.doc_id} value={d.doc_id}>
                  {d.original_filename}
                </option>
              ))}
            </select>
          </div>
        )}
        {register.length > 0 && (
          <table className="table" style={{ width: "100%", fontSize: "0.82rem" }}>
            <thead>
              <tr>
                <th>Document</th>
                <th>Witness</th>
                <th>Kind</th>
                <th>Recorded on</th>
                <th>Order</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {register.map((e) => {
                const r = row(e);
                const dirty = Boolean(edits[e.doc_id]);
                return (
                  <tr key={e.doc_id}>
                    <td title={e.doc_id}>{docName(e.doc_id)}{r.locator_kind === "PAGE_LINE" ? " (transcript)" : ""}</td>
                    <td><input value={r.witness_name} onChange={(ev) => setRow(e, { witness_name: ev.target.value })} /></td>
                    <td>
                      <select value={r.statement_kind} onChange={(ev) => setRow(e, { statement_kind: ev.target.value })}>
                        {kinds.map((k) => (
                          <option key={k.kind} value={k.kind}>{k.label}</option>
                        ))}
                        <option value="OTHER">Other</option>
                      </select>
                    </td>
                    <td><input type="date" value={r.recorded_on ?? ""} onChange={(ev) => setRow(e, { recorded_on: ev.target.value || null })} /></td>
                    <td><input type="number" style={{ width: 56 }} title="Tie-break when two statements share a date" value={r.sequence_override ?? ""} onChange={(ev) => setRow(e, { sequence_override: ev.target.value === "" ? null : Number(ev.target.value) })} /></td>
                    <td style={{ whiteSpace: "nowrap" }}>
                      <span className="muted" style={{ marginRight: 6 }}>{e.origin === "COUNSEL_CONFIRMED" ? "confirmed" : "proposed"}</span>
                      <button type="button" disabled={!!busy} onClick={() => void run("Saving", () => witnessRegisterSave(caseId, r))}>{dirty ? "Save" : e.origin === "COUNSEL_CONFIRMED" ? "Saved" : "Confirm"}</button>{" "}
                      <button type="button" disabled={!!busy} onClick={() => void run("Removing", () => witnessRegisterRemove(caseId, e.doc_id))}>Remove</button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
        {error && <div className="notice warn">{error}</div>}
        {notice && <div className="notice">{notice}</div>}
        {built && built.stale_decisions.length > 0 && <div className="notice warn">Some earlier merge/split decisions no longer apply: {built.stale_decisions.join("; ")}</div>}
        {built && built.unreadable.length > 0 && <div className="notice warn">Could not read: {built.unreadable.map((u) => docName(u.doc_id)).join(", ")}</div>}
      </div>

      {built && built.matrix.witnesses.length > 0 && (
        <div className="card">
          <h4>Witnesses</h4>
          <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
            {built.matrix.witnesses.map((w) => {
              const k = built.kits.find((x) => x.witness_id === w.witness_id);
              const n = k?.findings.filter((f) => f.status !== "COUNSEL_REJECTED").length ?? 0;
              return (
                <button key={w.witness_id} type="button" className={w.witness_id === selected ? "primary" : undefined} onClick={() => setSelected(w.witness_id)}>
                  {w.display_name} · {w.statements.length} statement{w.statements.length === 1 ? "" : "s"}{n ? ` · ${n} point${n === 1 ? "" : "s"}` : ""}
                </button>
              );
            })}
          </div>
          {witness && (
            <div style={{ marginTop: 8, fontSize: "0.82rem" }}>
              <span className="muted">{witness.resolution === "COUNSEL_CONFIRMED" ? "Confirmed by you" : "Grouped automatically by name — check it"}{witness.alias_node_ids.length ? ` · also recorded as ${witness.alias_node_ids.join(", ")}` : ""}</span>
              {others.length > 0 && (
                <div style={{ marginTop: 4 }}>
                  Same person as:{" "}
                  <select value="" onChange={(e) => e.target.value && void run("Merging", () => witnessDecisionAdd(caseId, { action: "MERGE", witness_id: e.target.value, into_witness_id: witness.witness_id }))}>
                    <option value="">Merge another witness into {witness.display_name}…</option>
                    {others.map((o) => (
                      <option key={o.witness_id} value={o.witness_id}>{o.display_name}</option>
                    ))}
                  </select>
                </div>
              )}
              {witness.statements.length > 1 && (
                <div style={{ marginTop: 4 }}>
                  A statement belongs to a different person:{" "}
                  {witness.statements.map((s) => (
                    <span key={s.statement_id} style={{ marginRight: 10 }}>
                      {docName(s.doc_id)}{" "}
                      <input placeholder="that person's name" style={{ width: 150 }} value={splitName[s.statement_id] ?? ""} onChange={(e) => setSplitName((c) => ({ ...c, [s.statement_id]: e.target.value }))} />{" "}
                      <button type="button" disabled={!!busy || !(splitName[s.statement_id] ?? "").trim()} onClick={() => void run("Splitting", () => witnessDecisionAdd(caseId, { action: "SPLIT", statement_id: s.statement_id, new_display_name: splitName[s.statement_id].trim() }))}>Split</button>
                    </span>
                  ))}
                </div>
              )}
              {(state?.decisions.filter((d) => d.active).length ?? 0) > 0 && (
                <details style={{ marginTop: 4 }}>
                  <summary>Your merge/split decisions</summary>
                  <ul>
                    {state!.decisions.filter((d) => d.active).map((d) => (
                      <li key={d.key}>
                        {d.decision.action === "MERGE" ? `Merged ${d.decision.witness_id} into ${d.decision.into_witness_id}` : `Split ${d.decision.statement_id} → ${d.decision.new_display_name}`}{" "}
                        <button type="button" disabled={!!busy} onClick={() => void run("Undoing", () => witnessDecisionRevoke(caseId, d.key))}>Undo</button>
                      </li>
                    ))}
                  </ul>
                </details>
              )}
            </div>
          )}
        </div>
      )}

      {witness && (
        <div className="card" style={{ overflowX: "auto" }}>
          <h4>Statement matrix — {witness.display_name}</h4>
          <p className="muted" style={{ fontSize: "0.8rem" }}>Each row is one matter the witness speaks about; each column one statement, earliest first. Every cell is the exact text, with its page (and line for transcripts).</p>
          <table className="table" style={{ fontSize: "0.78rem", minWidth: 480 }}>
            <thead>
              <tr>
                <th>Topic</th>
                {witness.statements.map((s) => (
                  <th key={s.statement_id}>{KIND_LABEL[s.statement_kind] ?? s.statement_kind}<br /><span className="muted">{s.recorded_on ?? "undated"}</span></th>
                ))}
              </tr>
            </thead>
            <tbody>
              {witness.topics.map((t) => (
                <tr key={t.topic_id}>
                  <td><span className="muted">{t.topic_kind}</span><br />{t.label}</td>
                  {witness.statements.map((s) => {
                    const cell = witness.cells.find((c) => c.topic_id === t.topic_id && c.statement_id === s.statement_id);
                    return (
                      <td key={s.statement_id} style={{ verticalAlign: "top" }}>
                        {cell?.assertions.map((a) => (
                          <div key={a.assertion_id} title={locatorLabel(a.locator, docName)} style={{ marginBottom: 4 }}>{a.text}</div>
                        )) ?? <span className="muted">—</span>}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {witness && kit && (
        <div className="card">
          <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline", flexWrap: "wrap", gap: 8 }}>
            <h4>Impeachment kit — {witness.display_name} ({kit.findings.filter((f) => f.status !== "COUNSEL_REJECTED").length})</h4>
            <div className="row" style={{ gap: 6 }}>
              <button type="button" disabled={!kit.findings.length} onClick={() => exportKit("docx")}>Export DOCX</button>
              <button type="button" disabled={!kit.findings.length} onClick={() => exportKit("pdf")}>Export PDF</button>
              <button type="button" disabled={!kit.findings.length} onClick={rehearse}>Rehearse in Mock Court</button>
            </div>
          </div>
          <p className="muted" style={{ fontSize: "0.8rem" }}>
            Each later statement is compared with the most recent earlier statement on the same matter. Ordered as a suggested confrontation sequence (identity and presence first). Omissions are candidates for your judgement.
          </p>
          {kit.findings.length === 0 && <p className="muted">No contradictions or omissions found between this witness's statements.</p>}
          <ul style={{ paddingLeft: "1.1rem", margin: 0 }}>
            {kit.findings.map((f) => (
              <FindingCard key={f.finding_id} f={f} w={witness} />
            ))}
          </ul>
          {witness.statements.length > 1 && (
            <div style={{ marginTop: 8 }}>
              <button
                type="button"
                disabled={!!busy || !modelReady}
                title={modelReady ? undefined : "Start the local model first"}
                onClick={() =>
                  void run("Asking the local AI", async () => {
                    const r = await impeachModelScan(caseId, witness.witness_id);
                    await refresh();
                    setNotice(`Local AI: ${r.rendered} contradiction${r.rendered === 1 ? "" : "s"} in meaning kept (quotes match the statements exactly); ${r.discarded.length} discarded because a quote did not match.`);
                  }, false)
                }
              >
                {busy === "Asking the local AI" ? "Asking the local AI…" : "Look for contradictions in meaning (local AI)"}
              </button>
            </div>
          )}
          <div className="muted mono" style={{ fontSize: "0.7rem", marginTop: 6 }}>
            {built?.pipeline_id} · {kit.kit_id} · matrix {kit.matrix_id}{kit.model_fingerprint ? ` · model ${kit.model_fingerprint}` : " · rule-based, no AI model"}
          </div>
        </div>
      )}
    </div>
  );
}
