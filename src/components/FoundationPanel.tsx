import { useCallback, useEffect, useState } from "react";
import { foundationCheck, foundationLink, foundationSetType, type FoundationResult, type FoundationCheck, type FoundationElement } from "../lib/ipc";

/**
 * B-S4-10: Admissibility & foundation gaps. Each exhibit is matched to the installed evidence-law rules for its type, and
 * every required foundation element is shown as on file, missing, or "to link". Missing elements lead the list of Missing
 * Links, each with the rule and the text of the section it comes from. These are prompts for counsel, not rulings on
 * admissibility, and they change no score.
 */
const TONE = { ok: "var(--ok, #2f7d4f)", warn: "var(--warn, #9a6a00)", bad: "var(--danger, #a33a2b)", muted: "var(--muted, #5b6470)" };
const STATE: Record<FoundationElement["status"], { label: string; color: string }> = {
  PRESENT: { label: "On file", color: TONE.ok },
  UNKNOWN: { label: "A possible document — link it", color: TONE.warn },
  ABSENT: { label: "Not on file", color: TONE.bad },
};

export function FoundationPanel({ caseId, v1Panel }: { caseId: string; v1Panel: unknown[] }) {
  const [data, setData] = useState<FoundationResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setData(await foundationCheck(caseId, v1Panel as Record<string, unknown>[]));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [caseId, v1Panel]);
  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function act(fn: () => Promise<unknown>) {
    setBusy(true);
    try {
      await fn();
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  if (!data) return error ? <div className="notice warn">{error}</div> : null;
  const label = (id: string) => data.evidence.find((e) => e.evidence_id === id)?.label ?? id;
  const checksByItem = new Map<string, FoundationCheck[]>();
  data.checks.forEach((c) => checksByItem.set(c.evidence_node_id, [...(checksByItem.get(c.evidence_node_id) ?? []), c]));
  const foundation = data.panel.filter((p) => p.kind === "FOUNDATION");
  const unresolved = data.gaps.filter((g) => g.status === "UNKNOWN");

  return (
    <div style={{ marginTop: 12 }}>
      <h4 style={{ marginBottom: 4 }}>Evidence foundation — what each exhibit still needs</h4>
      <p className="muted" style={{ fontSize: "0.8rem" }}>
        Each exhibit is matched to the evidence-law requirement for its kind (for example a certificate for an electronic record). Gaps are prompts for you to resolve, not rulings on admissibility, and they change no score.
      </p>
      {error && <div className="notice warn">{error}</div>}
      {data.rules_installed === 0 && <div className="notice warn">No admissibility rules are installed for this jurisdiction. Install a legal pack that carries them (Legal Authorities).</div>}
      {data.unsigned_rules && data.rules_installed > 0 && <div className="notice warn">These requirements have not been signed off by a legal editor; treat them as a checklist to confirm.</div>}

      {foundation.length > 0 && (
        <ol style={{ paddingLeft: "1.2rem", margin: "6px 0" }}>
          {foundation.map((g) => (
            <li key={String(g.panel_id)} style={{ marginBottom: 8, fontSize: "0.88rem" }}>
              <strong>{String(g.evidence_label)}</strong> — {String(g.element_label)}
              <div style={{ color: TONE.bad, fontSize: "0.78rem", fontWeight: 600 }}>
                Not on file{g.evidence_type_status === "PROPOSED" ? " · the kind of evidence is a proposal — confirm it below" : ""}
              </div>
              <div className="muted" style={{ fontSize: "0.78rem" }}>
                {String((g.authority as Record<string, unknown>).provision_label ?? g.rule_id)}
                {g.authority_excerpt ? `: “${String(g.authority_excerpt)}”` : ""}
              </div>
            </li>
          ))}
        </ol>
      )}
      {foundation.length === 0 && data.rules_installed > 0 && <div className="muted" style={{ fontSize: "0.85rem" }}>No missing foundation element found for the exhibits recognised so far.</div>}
      {unresolved.length > 0 && (
        <div className="muted" style={{ fontSize: "0.8rem" }}>
          {unresolved.length} element{unresolved.length === 1 ? "" : "s"} could belong to more than one exhibit — link the right document below.
        </div>
      )}

      <details style={{ marginTop: 6 }}>
        <summary className="muted" style={{ fontSize: "0.8rem" }}>All exhibits ({data.evidence.length}) — confirm the kind, link documents</summary>
        {data.evidence.map((e) => (
          <div key={e.evidence_id} style={{ margin: "8px 0", fontSize: "0.85rem" }}>
            <div className="row" style={{ gap: 6, alignItems: "center", flexWrap: "wrap" }}>
              <strong>{e.label}</strong>
              <select
                value={e.evidence_type ?? ""}
                disabled={busy}
                onChange={(ev) => void act(() => foundationSetType(caseId, e.evidence_id, ev.target.value || null))}
                aria-label={`Kind of evidence: ${e.label}`}
              >
                <option value="">{e.evidence_type ? "Clear my choice" : "Not one of the listed kinds"}</option>
                {data.types.map((t) => (
                  <option key={t.evidence_type} value={t.evidence_type}>{t.label}</option>
                ))}
              </select>
              {e.evidence_type && <span className="muted" style={{ fontSize: "0.75rem" }}>{e.type_status === "COUNSEL_CONFIRMED" ? "confirmed by you" : "proposed from the words"}</span>}
            </div>
            {(checksByItem.get(e.evidence_id) ?? []).map((c) => (
              <div key={c.check_id} style={{ marginLeft: 12, marginTop: 4 }}>
                <div className="muted mono" style={{ fontSize: "0.7rem" }}>{c.rule_id}</div>
                {c.elements.map((el) => (
                  <div key={el.element_id} className="row" style={{ gap: 6, alignItems: "center", flexWrap: "wrap" }}>
                    <span style={{ color: STATE[el.status].color, fontWeight: 600, fontSize: "0.78rem" }}>{STATE[el.status].label}</span>
                    <span>{el.label}</span>
                    {el.locators.length > 0 && el.status === "PRESENT" && <span className="muted" style={{ fontSize: "0.75rem" }}>({el.locators.map((l) => l.excerpt).join("; ")})</span>}
                    {el.status !== "PRESENT" && (
                      <select disabled={busy} value="" onChange={(ev) => ev.target.value && void act(() => foundationLink(caseId, c.evidence_node_id, el.element_id, ev.target.value))} aria-label={`Link a document for: ${el.label}`}>
                        <option value="">Link a document…</option>
                        {data.docs.map((d) => (
                          <option key={d.doc_id} value={d.doc_id}>{d.filename}</option>
                        ))}
                      </select>
                    )}
                    {el.status === "PRESENT" && el.locators.some((l) => l.excerpt.includes("linked by counsel")) && (
                      <button type="button" disabled={busy} onClick={() => void act(() => foundationLink(caseId, c.evidence_node_id, el.element_id, null))}>Unlink</button>
                    )}
                  </div>
                ))}
              </div>
            ))}
            {!checksByItem.has(e.evidence_id) && <div className="muted" style={{ marginLeft: 12, fontSize: "0.78rem" }}>{e.evidence_type ? `No installed rule applies to ${label(e.evidence_id)} for this date.` : "No foundation rule applies unless you set a kind."}</div>}
          </div>
        ))}
      </details>
      <div className="muted mono" style={{ fontSize: "0.7rem", marginTop: 6 }}>checked on this computer from installed rules · dates checked: {data.as_of.join(", ")} · no AI model</div>
    </div>
  );
}
