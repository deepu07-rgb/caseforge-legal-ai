import { useState } from "react";
import { auditDraft, type AuditItem, type AuditReport, type AuditResult } from "../lib/ipc";
import { CertificatePanel, VerifyCertificatePanel } from "./CertificatePanel";
import { CurrencyStamp, TreatmentBlock } from "./CurrencyStamp";

/**
 * B-S4-04: audit every citation and quotation in the draft against the installed legal packs.
 * "Not in installed packs" is a repository gap, never a claim that the authority does not exist.
 */
export const RESULT_LABEL: Record<AuditResult, { text: string; tone: "ok" | "warn" | "bad" | "info" }> = {
  VERIFIED: { text: "Verified", tone: "ok" },
  VERIFIED_WITH_NOTE: { text: "Verified (see note)", tone: "ok" },
  MISQUOTED: { text: "Misquoted", tone: "bad" },
  NOT_IN_CORPUS: { text: "Not in installed packs", tone: "bad" },
  SUPERSEDED: { text: "Superseded", tone: "warn" },
  NOT_APPLICABLE: { text: "Not applicable / status unknown", tone: "warn" },
  CUSTOM_SOURCE: { text: "Custom source only", tone: "info" },
};
export const TONE = { ok: "var(--ok, #2f7d4f)", warn: "var(--warn, #9a6a00)", bad: "var(--danger, #a33a2b)", info: "var(--muted, #5b6470)" };
export const AUDIT_ORDER: AuditResult[] = ["MISQUOTED", "NOT_IN_CORPUS", "SUPERSEDED", "NOT_APPLICABLE", "CUSTOM_SOURCE", "VERIFIED_WITH_NOTE", "VERIFIED"];

function Diff({ item, whose }: { item: AuditItem; whose: string }) {
  if (!item.misquote) return null;
  return (
    <div style={{ marginTop: 4, fontSize: "0.8rem" }}>
      <span className="muted">Source wording vs. {whose} quotation: </span>
      {item.misquote.diff_ops.map((op, i) => (
        <span
          key={i}
          style={
            op.op === "DELETE"
              ? { background: "rgba(47,125,79,0.15)", textDecoration: "underline" }
              : op.op === "INSERT"
                ? { background: "rgba(163,58,43,0.15)", textDecoration: "line-through" }
                : undefined
          }
          title={op.op === "DELETE" ? `in the source, missing from ${whose} quotation` : op.op === "INSERT" ? `in ${whose} quotation, not in the source` : undefined}
        >
          {op.text}{" "}
        </span>
      ))}
    </div>
  );
}

export function AuditRow({ item, whose = "your" }: { item: AuditItem; whose?: string }) {
  const label = RESULT_LABEL[item.result];
  const loc = item.resolved;
  return (
    <li style={{ marginBottom: 6 }}>
      <span style={{ color: TONE[label.tone], fontWeight: 600 }}>{label.text}</span>
      <span className="muted"> · {item.kind === "QUOTATION" ? "Quotation" : "Citation"}</span>
      <div className="mono" style={{ fontSize: "0.8rem" }}>{item.kind === "QUOTATION" ? `“${item.draft_text}”` : item.draft_text}</div>
      {item.kind === "CITATION" && item.pinpoint.claimed && (
        <div style={{ fontSize: "0.78rem", color: item.pinpoint.status === "MISMATCH" ? TONE.bad : undefined }}>
          Pinpoint {item.pinpoint.claimed}: {item.pinpoint.status === "MATCH" ? "found in the source" : item.pinpoint.status === "MISMATCH" ? "not found in the installed text of this source" : "not checked"}
        </div>
      )}
      {loc && (
        <div className="muted" style={{ fontSize: "0.75rem" }}>
          {loc.source_id} · {loc.pack_id}{loc.page_index != null ? ` · p.${loc.page_index}` : ""}{loc.paragraph ? ` · ${loc.paragraph}` : ""}
          {item.result !== "MISQUOTED" && <div style={{ whiteSpace: "pre-wrap" }}>"{loc.excerpt.slice(0, 220)}{loc.excerpt.length > 220 ? "…" : ""}"</div>}
        </div>
      )}
      {loc && <CurrencyStamp packId={loc.pack_id} />}
      {item.notes.map((n, i) => (
        <div key={i} style={{ fontSize: "0.78rem", color: n.note_kind === "TREATMENT" ? TONE.warn : undefined, whiteSpace: "pre-wrap" }}>
          {n.note_kind === "TREATMENT" ? "Treatment note · " : n.note_kind === "CODE_BRIDGE" ? "Code note · " : ""}{n.text}
        </div>
      ))}
      {loc && item.kind === "CITATION" && item.citation_kind !== "STATUTE" && <TreatmentBlock sourceId={loc.source_id} chunkId={loc.chunk_id} />}
      <Diff item={item} whose={whose} />
    </li>
  );
}

export function AuditPanel({ text, caseId, filename }: { text: string; caseId?: string | null; filename?: string }) {
  const [report, setReport] = useState<AuditReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function run() {
    setBusy(true);
    setError(null);
    try {
      setReport(await auditDraft(text, filename ?? "draft.txt", caseId ?? null));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const items = report ? [...report.items].sort((a, b) => AUDIT_ORDER.indexOf(a.result) - AUDIT_ORDER.indexOf(b.result) || a.draft_char_start - b.draft_char_start) : [];
  return (
    <div className="card" style={{ gridColumn: "1 / -1" }}>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline" }}>
        <h4>Citation &amp; quote audit</h4>
        <button type="button" className="primary" onClick={() => void run()} disabled={busy || !text.trim()}>
          {busy ? "Auditing…" : report ? "Re-run audit" : "Audit citations & quotes"}
        </button>
      </div>
      <p className="muted" style={{ fontSize: "0.8rem" }}>
        Checks every citation and quotation against the legal packs installed on this computer. "Not in installed packs" means a gap in your library, not that the authority does not exist.
      </p>
      {error && <div className="notice warn">{error}</div>}
      {report && (
        <>
          <div style={{ fontSize: "0.85rem", marginBottom: 6 }}>
            {AUDIT_ORDER.filter((r) => report.counts[r] > 0).map((r) => (
              <span key={r} style={{ marginRight: 12, color: TONE[RESULT_LABEL[r].tone] }}>
                {RESULT_LABEL[r].text}: {report.counts[r]}
              </span>
            ))}
            {report.items.length === 0 && <span className="muted">No citations or quotations found.</span>}
          </div>
          {report.pack_snapshot.length === 0 && <div className="notice warn">No legal packs are installed, so nothing can be verified yet.</div>}
          {report.pack_snapshot.length > 0 && (
            <div className="muted" style={{ fontSize: "0.75rem", marginBottom: 6 }} data-testid="currency-stamp">
              Checked against: {report.pack_snapshot.map((p) => `${p.pack_id} ${p.currency_date ? `as of ${p.currency_date}` : "(no currency date stated)"}`).join("; ")}
            </div>
          )}
          <ul style={{ paddingLeft: "1.1rem", margin: 0 }}>
            {items.map((item) => (
              <AuditRow key={item.item_id} item={item} />
            ))}
          </ul>
          <div className="muted mono" style={{ fontSize: "0.7rem", marginTop: 6 }}>
            {report.pipeline_id} · {report.audit_id} · draft sha256 {report.subject.sha256.slice(0, 12)}… · deterministic, no AI model
          </div>
          {report.pack_snapshot.length > 0 && <CertificatePanel report={report} text={text} />}
        </>
      )}
      <VerifyCertificatePanel currentDraft={text} />
    </div>
  );
}
