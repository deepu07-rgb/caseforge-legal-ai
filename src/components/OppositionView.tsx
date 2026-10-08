import { useEffect, useState } from "react";
import {
  docpipeCaseGraphGet,
  docUpdateTypeTag,
  oppositionScan,
  type CaseSummary,
  type DocumentMeta,
  type FilingKind,
  type FilingLocator,
  type OppositionScanResult,
} from "../lib/ipc";
import { AUDIT_ORDER, AuditRow, RESULT_LABEL, TONE } from "./AuditPanel";

/**
 * B-S4-06: Opposing Filing Scan. The other side's filing is treated as an untrusted adverse assertion:
 * it is checked against the record and the installed packs, and never becomes one of counsel's own facts.
 */
const EXTERNAL = "EXTERNAL_FILING";
const KINDS: { id: FilingKind; label: string }[] = [
  { id: "REPLY", label: "Reply" },
  { id: "PLEADING", label: "Pleading" },
  { id: "WRITTEN_SUBMISSION", label: "Written submission" },
  { id: "BRIEF", label: "Brief" },
  { id: "AFFIDAVIT", label: "Affidavit" },
  { id: "OTHER", label: "Other" },
];
const BASIS: Record<string, string> = {
  DATE: "Date differs from the record",
  AMOUNT: "Amount differs from the record",
  PRESENCE: "Denies something the record shows",
  NAME: "Name differs from the record",
  OTHER: "Differs from the record",
};
const ORIGIN: Record<string, string> = {
  SOURCED_BY_COUNSEL: "confirmed by counsel",
  EXTRACTED: "extracted from your documents",
  USER_EDIT: "edited by you",
};

function Where({ loc, label }: { loc: FilingLocator; label: string }) {
  return (
    <div style={{ fontSize: "0.8rem", marginTop: 2 }}>
      <span className="muted">
        {label} · {loc.doc_id}
        {loc.page_index != null ? ` · page ${loc.page_index + (label === "Their filing" ? 1 : 0)}` : ""}:{" "}
      </span>
      “{loc.excerpt.slice(0, 260)}
      {loc.excerpt.length > 260 ? "…" : ""}”
    </div>
  );
}

export default function OppositionView({
  activeCase,
  documents,
  onDocumentsChanged,
  onNavigate,
}: {
  activeCase: CaseSummary | null;
  documents: DocumentMeta[];
  onDocumentsChanged: () => void;
  onNavigate: (nav: "builder" | "court") => void;
}) {
  const [parties, setParties] = useState<{ party_id: string; display_name: string; role: string }[]>([]);
  const [partyId, setPartyId] = useState("");
  const [kind, setKind] = useState<FilingKind>("REPLY");
  const [source, setSource] = useState<"doc" | "paste">("doc");
  const [docId, setDocId] = useState("");
  const [text, setText] = useState("");
  const [result, setResult] = useState<OppositionScanResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const caseId = activeCase?.case_id ?? null;
  const external = documents.filter((d) => d.type_tag === EXTERNAL);
  const others = documents.filter((d) => d.type_tag !== EXTERNAL);

  useEffect(() => {
    setResult(null);
    setParties([]);
    if (!caseId) return;
    docpipeCaseGraphGet(caseId)
      .then((g) => {
        const list = (g?.parties ?? []).map((p) => ({ party_id: p.party_id, display_name: p.display_name, role: p.role }));
        setParties(list);
        setPartyId((cur) => cur || list.find((p) => /respond|defend|opposite|accused/i.test(p.role))?.party_id || "");
      })
      .catch(() => setParties([]));
  }, [caseId]);

  useEffect(() => {
    if (!external.some((d) => d.doc_id === docId)) setDocId(external[0]?.doc_id ?? "");
  }, [documents]); // eslint-disable-line react-hooks/exhaustive-deps

  async function markExternal(id: string) {
    if (!caseId || !id) return;
    setError(null);
    try {
      await docUpdateTypeTag(caseId, id, EXTERNAL);
      setDocId(id);
      onDocumentsChanged();
    } catch (e) {
      setError(String(e));
    }
  }

  async function run() {
    if (!caseId) return;
    setBusy(true);
    setError(null);
    try {
      setResult(
        await oppositionScan({
          caseId,
          filingPartyId: partyId || "opposing-party",
          filingKind: kind,
          docId: source === "doc" ? docId : null,
          text: source === "paste" ? text : null,
        }),
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  if (!activeCase) {
    return (
      <div className="card">
        <p className="muted">Open a matter first. The scan compares the other side's filing with that matter's record and Argument Tree.</p>
      </div>
    );
  }

  const ready = source === "doc" ? Boolean(docId) : text.trim().length > 0;
  const filing = result?.filing;
  const assertion = (id: string) => filing?.assertions.find((a) => a.assertion_id === id);
  const items = result ? [...result.audit.items].sort((a, b) => AUDIT_ORDER.indexOf(a.result) - AUDIT_ORDER.indexOf(b.result) || a.draft_char_start - b.draft_char_start) : [];
  const fresh = filing?.scan.new_arguments.filter((a) => !a.matched_tree_node_id) ?? [];
  const answered = filing?.scan.new_arguments.filter((a) => a.matched_tree_node_id) ?? [];

  return (
    <div className="grid" style={{ gap: 12 }}>
      <div className="card">
        <h4>The other side's filing</h4>
        <p className="muted" style={{ fontSize: "0.8rem" }}>
          Treated as their assertion, never as your fact: nothing in it is added to your case record, your drafts or Mock Court. The scan is rule-based (no AI model), so instructions hidden in their text cannot change the result.
        </p>
        <div className="row" style={{ gap: 12, flexWrap: "wrap" }}>
          <label>
            Filed by{" "}
            {parties.length > 0 ? (
              <select value={partyId} onChange={(e) => setPartyId(e.target.value)}>
                <option value="">Opposing party</option>
                {parties.map((p) => (
                  <option key={p.party_id} value={p.party_id}>
                    {p.display_name} ({p.role})
                  </option>
                ))}
              </select>
            ) : (
              <span className="muted">opposing party</span>
            )}
          </label>
          <label>
            Kind{" "}
            <select value={kind} onChange={(e) => setKind(e.target.value as FilingKind)}>
              {KINDS.map((k) => (
                <option key={k.id} value={k.id}>
                  {k.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            <input type="radio" checked={source === "doc"} onChange={() => setSource("doc")} /> Uploaded document
          </label>
          <label>
            <input type="radio" checked={source === "paste"} onChange={() => setSource("paste")} /> Paste text
          </label>
        </div>

        {source === "doc" ? (
          <div style={{ marginTop: 8 }}>
            {external.length > 0 ? (
              <select value={docId} onChange={(e) => setDocId(e.target.value)}>
                {external.map((d) => (
                  <option key={d.doc_id} value={d.doc_id}>
                    {d.original_filename}
                  </option>
                ))}
              </select>
            ) : (
              <span className="muted" style={{ fontSize: "0.85rem" }}>No document is marked as the other side's filing yet.</span>
            )}
            {others.length > 0 && (
              <div style={{ marginTop: 6, fontSize: "0.85rem" }}>
                Mark an uploaded document as the other side's filing:{" "}
                <select value="" onChange={(e) => void markExternal(e.target.value)}>
                  <option value="">Choose…</option>
                  {others.map((d) => (
                    <option key={d.doc_id} value={d.doc_id}>
                      {d.original_filename}
                    </option>
                  ))}
                </select>
              </div>
            )}
          </div>
        ) : (
          <textarea
            style={{ width: "100%", minHeight: 160, marginTop: 8 }}
            placeholder="Paste their reply, written statement or submissions. Numbered paragraphs (1., 2., …) give the clearest results."
            value={text}
            onChange={(e) => setText(e.target.value)}
          />
        )}
        <div className="row" style={{ marginTop: 8 }}>
          <button type="button" className="primary" disabled={busy || !ready} onClick={() => void run()}>
            {busy ? "Scanning…" : result ? "Re-scan" : "Scan their filing"}
          </button>
        </div>
        {error && <div className="notice warn">{error}</div>}
        {result?.stale_graph && (
          <div className="notice warn">
            Your case record was built while this document was still treated as your own material. Rebuild the case record (Case Material &amp; Review → skeleton) so their assertions are removed from your facts.
          </div>
        )}
      </div>

      {filing && result && (
        <>
          <div className="card">
            <h4>Their citations and quotations</h4>
            <div style={{ fontSize: "0.85rem", marginBottom: 6 }}>
              {AUDIT_ORDER.filter((r) => result.audit.counts[r] > 0).map((r) => (
                <span key={r} style={{ marginRight: 12, color: TONE[RESULT_LABEL[r].tone] }}>
                  {RESULT_LABEL[r].text}: {result.audit.counts[r]}
                </span>
              ))}
              {items.length === 0 && <span className="muted">No citations or quotations found.</span>}
            </div>
            {result.audit.pack_snapshot.length === 0 && <div className="notice warn">No legal packs are installed, so none of their authorities can be checked yet.</div>}
            <ul style={{ paddingLeft: "1.1rem", margin: 0 }}>
              {items.map((item) => (
                <AuditRow key={item.item_id} item={item} whose="their" />
              ))}
            </ul>
          </div>

          <div className="card">
            <h4>Where they contradict your record ({filing.scan.record_conflicts.length})</h4>
            {filing.scan.record_conflicts.length === 0 && <p className="muted">No date, amount or denial in their filing contradicts your case record.</p>}
            <ul style={{ paddingLeft: "1.1rem", margin: 0 }}>
              {filing.scan.record_conflicts.map((c) => (
                <li key={c.conflict_id} style={{ marginBottom: 8 }}>
                  <span style={{ color: TONE.bad, fontWeight: 600 }}>{BASIS[c.basis] ?? c.basis}</span>
                  <span className="muted"> · their para {c.assertion_id.replace(/^p/, "")} · your record {ORIGIN[c.record_origin]}</span>
                  <Where loc={c.filing_locator} label="Their filing" />
                  <Where loc={c.record_locator} label="Your record" />
                </li>
              ))}
            </ul>
          </div>

          <div className="card">
            <h4>Factual claims with no record reference ({filing.scan.unsupported_assertion_ids.length})</h4>
            <p className="muted" style={{ fontSize: "0.8rem" }}>
              Paragraphs stating facts without pointing to an annexure, exhibit or page. Worth putting to them: "where is this in the record?"
            </p>
            <ul style={{ paddingLeft: "1.1rem", margin: 0 }}>
              {filing.scan.unsupported_assertion_ids.map((id) => (
                <li key={id} style={{ fontSize: "0.85rem", marginBottom: 4 }}>
                  <span className="muted">Para {id.replace(/^p/, "")}: </span>
                  {assertion(id)?.text}
                </li>
              ))}
            </ul>
          </div>

          <div className="card">
            <h4>Their arguments</h4>
            {filing.scan.new_arguments.length === 0 && <p className="muted">No argument paragraphs found (for example "It is submitted that…").</p>}
            {fresh.length > 0 && (
              <>
                <div style={{ fontWeight: 600, color: TONE.warn }}>Not answered in your Argument Tree ({fresh.length})</div>
                <ul style={{ paddingLeft: "1.1rem" }}>
                  {fresh.map((a) => (
                    <li key={a.argument_id} style={{ fontSize: "0.85rem", marginBottom: 4 }}>
                      {a.summary}
                    </li>
                  ))}
                </ul>
                <div className="row" style={{ gap: 8 }}>
                  <button type="button" onClick={() => onNavigate("builder")}>Add a reply in Argument Builder</button>
                  <button type="button" onClick={() => onNavigate("court")}>Rehearse in Mock Court</button>
                </div>
              </>
            )}
            {answered.length > 0 && (
              <>
                <div style={{ fontWeight: 600, marginTop: 8 }}>Touches a limb you already argue ({answered.length})</div>
                <ul style={{ paddingLeft: "1.1rem" }}>
                  {answered.map((a) => (
                    <li key={a.argument_id} style={{ fontSize: "0.85rem", marginBottom: 4 }}>
                      {a.summary} <span className="muted">→ your limb {a.matched_tree_node_id}</span>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </div>

          <div className="muted mono" style={{ fontSize: "0.7rem" }}>
            {result.pipeline_id} · {result.audit.audit_id} · trust {filing.trust} · deterministic, no AI model
          </div>
        </>
      )}
    </div>
  );
}
