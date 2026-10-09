import { useState } from "react";
import type { CaseSummary, CauseListBriefs, CauseListEntryIn, CauseListField } from "../lib/ipc";
import { causeListBriefs, causeListImport } from "../lib/ipc";
import { EXPORT_FOOTER_LINES } from "../lib/exportDraft";

/**
 * B-S4-20: Daily Cause-List Brief. Import the day's list from a file (or type it), match each listing to a matter, and print
 * one page per matter. Every line on a page cites the order or document it comes from, or says it was not found in the
 * record. Nothing is summarised or inferred beyond the wording of the orders.
 */
const STATUS: Record<string, { text: string; tone: string }> = {
  MATCHED: { text: "Matter found", tone: "var(--ok, #2f7d4f)" },
  AMBIGUOUS_CONFIRMED: { text: "Matter chosen by you", tone: "var(--ok, #2f7d4f)" },
  AMBIGUOUS_UNCONFIRMED: { text: "Could be more than one matter — choose", tone: "var(--warn, #9a6a00)" },
  NO_LOCAL_RECORD: { text: "No local record", tone: "var(--muted, #5b6470)" },
};
const ERROR: Record<string, string> = {
  CF_CAUSELIST_EMPTY: "No listed matter was found. Put each matter on its own line starting with its serial number, for example “1. Ravi Kumar v. Anil Kumar CS 112/2023”, or use a CSV with serial, case number and cause title.",
  CF_CAUSELIST_FILE: "That file could not be read.",
  CF_EDITION_NOT_ENTITLED: "The cause-list brief belongs to the Chambers Edition.",
  CF_EDITION_PILOT_DISABLED: "This court’s configuration has switched this off.",
};
const PRINT_CSS = `@media print { body * { visibility: hidden !important; } .cause-print, .cause-print * { visibility: visible !important; } .cause-print { position: absolute; left: 0; top: 0; width: 100%; } .brief-page { page-break-after: always; } .no-print { display: none !important; } }`;

function plain(e: unknown): string {
  const s = String(e);
  const k = Object.keys(ERROR).find((x) => s.includes(x));
  return k ? ERROR[k] : s;
}
function readBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onerror = () => reject(r.error);
    r.onloadend = () => resolve(String(r.result).split(",", 2)[1] ?? "");
    r.readAsDataURL(file);
  });
}

export default function CauseListView({ activeCase: _activeCase }: { activeCase: CaseSummary | null }) {
  const [court, setCourt] = useState("");
  const [date, setDate] = useState(new Date().toISOString().slice(0, 10));
  const [typed, setTyped] = useState("");
  const [imported, setImported] = useState<{ format: string; sha256: string | null; entries: CauseListEntryIn[] } | null>(null);
  const [result, setResult] = useState<CauseListBriefs | null>(null);
  const [confirm, setConfirm] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function prepare(entries: CauseListEntryIn[], format: string, sha: string | null, conf: Record<string, string>) {
    setResult(await causeListBriefs(court || "Court", date, format, sha, entries, conf));
  }
  async function run<T>(fn: () => Promise<T>) {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(plain(e));
    } finally {
      setBusy(false);
    }
  }
  const fromFile = (file: File) =>
    run(async () => {
      const lower = file.name.toLowerCase();
      const format = lower.endsWith(".csv") ? "CSV" : "PDF";
      const imp = await causeListImport({ format, file_name: file.name, content_base64: await readBase64(file) });
      setImported(imp);
      setConfirm({});
      await prepare(imp.entries, imp.format, imp.sha256, {});
    });
  const fromTyped = () =>
    run(async () => {
      const imp = await causeListImport({ format: "MANUAL", text: typed });
      setImported(imp);
      setConfirm({});
      await prepare(imp.entries, imp.format, imp.sha256, {});
    });
  const choose = (serial: string, caseId: string) =>
    run(async () => {
      const next = { ...confirm, [serial]: caseId };
      setConfirm(next);
      if (imported) await prepare(imported.entries, imported.format, imported.sha256, next);
    });

  const doc = result?.brief_list;
  const title = (id: string | null) => (id && result?.matter_titles[id]) || id || "";
  const where = (f: CauseListField) => (f.locator ? `${result?.documents[f.locator.doc_id] ?? f.locator.doc_id}, page ${f.locator.page}` : "");
  const FieldLine = ({ label, f }: { label: string; f: CauseListField }) => (
    <div style={{ marginBottom: 6 }}>
      <strong>{label}: </strong>
      {f.state === "FOUND" ? (
        <>
          <span style={{ whiteSpace: "pre-wrap" }}>{f.value}</span>
          <div className="muted" style={{ fontSize: "0.72rem" }}>{where(f)}</div>
        </>
      ) : (
        <span className="muted">not found in the record</span>
      )}
    </div>
  );

  return (
    <div className="grid" style={{ gap: 12 }}>
      <style>{PRINT_CSS}</style>
      <div className="card no-print" style={{ gridColumn: "1 / -1" }}>
        <h4>Daily cause list</h4>
        <p className="muted" style={{ fontSize: "0.8rem" }}>
          Bring in the day’s list as a file (CSV, PDF or text) or type it, one matter per line. Each listing is matched to a matter in this app; you print one page per matter. Every line cites the order or document it comes from, or says it was not found in the record. Nothing is fetched from any court website.
        </p>
        {error && <div className="notice warn">{error}</div>}
        <div className="row" style={{ gap: 8, flexWrap: "wrap", alignItems: "center" }}>
          <input value={court} onChange={(e) => setCourt(e.target.value)} placeholder="Court / bench" aria-label="Court or bench" />
          <input type="date" value={date} onChange={(e) => setDate(e.target.value)} aria-label="Date of the list" />
          <label className="button">
            <input type="file" accept=".csv,.pdf,.txt,.docx" style={{ display: "none" }} disabled={busy} onChange={(e) => { const f = e.target.files?.[0]; if (f) void fromFile(f); e.target.value = ""; }} />
            Choose a file…
          </label>
        </div>
        <textarea value={typed} onChange={(e) => setTyped(e.target.value)} rows={4} style={{ width: "100%", marginTop: 6 }} aria-label="Typed cause list" placeholder={"1. Ravi Kumar v. Anil Kumar CS 112/2023\n2. State v. Dev Singh"} />
        <button type="button" disabled={busy || !typed.trim()} onClick={() => void fromTyped()}>Use the typed list</button>
      </div>

      {doc && (
        <div className="card no-print" style={{ gridColumn: "1 / -1" }}>
          <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline" }}>
            <h4>{doc.entries.length} listed · {doc.entries.filter((e) => e.brief).length} briefs</h4>
            <button type="button" onClick={() => window.print()} disabled={!doc.entries.some((e) => e.brief)}>Print one page per matter</button>
          </div>
          {result.matters_not_open.length > 0 && <div className="notice warn">These matters are not open, so their orders could not be read: {result.matters_not_open.map(title).join("; ")}. Open them (Matters) and prepare the briefs again.</div>}
          <ul style={{ paddingLeft: "1.1rem", margin: 0 }}>
            {doc.entries.map((e) => (
              <li key={e.serial} style={{ marginBottom: 6, fontSize: "0.88rem" }}>
                <strong>{e.serial}.</strong> {e.listed_title} — <span style={{ color: STATUS[e.match.status].tone, fontWeight: 600 }}>{STATUS[e.match.status].text}</span>
                {e.match.case_id && <span className="muted"> · {title(e.match.case_id)}</span>}
                {e.match.status === "AMBIGUOUS_UNCONFIRMED" && (
                  <div className="row" style={{ gap: 6, marginTop: 2 }}>
                    {(result.candidates[e.serial] ?? []).map((id) => (
                      <button key={id} type="button" disabled={busy} onClick={() => void choose(e.serial, id)}>It is: {title(id)}</button>
                    ))}
                  </div>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}

      {doc && (
        <div className="cause-print" style={{ gridColumn: "1 / -1" }}>
          {doc.entries.filter((e) => e.brief).map((e) => {
            const b = e.brief!;
            return (
              <div key={e.serial} className="card brief-page" style={{ fontSize: "0.88rem" }}>
                <h4>{doc.court_label} · {doc.list_date} · Item {e.serial}</h4>
                <div style={{ marginBottom: 6 }}><strong>{e.listed_title}</strong> <span className="muted">({title(e.match.case_id)})</span></div>
                <FieldLine label="Stage" f={b.stage} />
                <FieldLine label="Last order" f={b.last_order} />
                <FieldLine label="Date of last order" f={b.last_order_date} />
                <div style={{ marginBottom: 6 }}>
                  <strong>Directions in the last order: </strong>
                  {b.compliance.length === 0 && <span className="muted">none found in the record</span>}
                  <ul style={{ margin: "2px 0", paddingLeft: "1.1rem" }}>
                    {b.compliance.map((c, i) => (
                      <li key={i}>
                        {c.directed.value} <span className="muted">({where(c.directed)})</span>
                        <div>{c.record_status === "DONE_ON_RECORD" ? <>Shown done on the record: “{c.done_locator?.excerpt}” <span className="muted">({result.documents[c.done_locator?.doc_id ?? ""] ?? c.done_locator?.doc_id}, page {c.done_locator?.page})</span></> : <span className="muted">Not shown on the record as done</span>}</div>
                      </li>
                    ))}
                  </ul>
                </div>
                <div style={{ marginBottom: 6 }}>
                  <strong>Adjournments on the record: {b.adjournments.count}</strong>
                  <ul style={{ margin: "2px 0", paddingLeft: "1.1rem" }}>
                    {b.adjournments.entries.map((a, i) => (
                      <li key={i}>
                        Order of {a.order_date}: {a.sought_by_party_id ? `sought by ${result.parties[a.sought_by_party_id] ?? a.sought_by_party_id}` : "the order does not say who sought it"}
                        <span className="muted"> ({result.documents[a.locator.doc_id] ?? a.locator.doc_id}, page {a.locator.page})</span>
                      </li>
                    ))}
                  </ul>
                </div>
                <div style={{ marginBottom: 6 }}>
                  <strong>Applications not disposed of in the orders: </strong>
                  {b.pending_applications.length === 0 && <span className="muted">none found in the record</span>}
                  <ul style={{ margin: "2px 0", paddingLeft: "1.1rem" }}>
                    {b.pending_applications.map((p, i) => <li key={i}>{p.value} <span className="muted">({where(p)})</span></li>)}
                  </ul>
                </div>
                <div className="muted" style={{ fontSize: "0.66rem", marginTop: 8 }}>{EXPORT_FOOTER_LINES.join(" | ")}</div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
