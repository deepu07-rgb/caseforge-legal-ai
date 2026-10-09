import { useCallback, useEffect, useState } from "react";
import { CurrencyStamp } from "./CurrencyStamp";
import type { CaseSummary } from "../lib/ipc";
import { deadlinesAssign, deadlinesCompute, deadlinesExportIcs, deadlinesFactRemove, deadlinesFactSave, deadlinesUnassign, type DeadlinesState, type DeadlineRecord } from "../lib/ipc";

/**
 * B-S4-11: Deadline & Limitation Watch. Every date is computed from an installed rule, a start event counsel picks from
 * the matter's chronology, and the court calendar - and shows its working so a lawyer can redo it by hand. Anything
 * uncertain is a range with the reason. Every result carries a permanent VERIFY label: a computation to check, not advice.
 */
const TONE = { ok: "var(--ok, #2f7d4f)", warn: "var(--warn, #9a6a00)", bad: "var(--danger, #a33a2b)", muted: "var(--muted, #5b6470)" };
const REASON: Record<string, string> = {
  TRIGGER_DATE_IMPRECISE: "The start date is only known roughly (a month, a year or a range).",
  TRIGGER_DATE_CONFLICT: "The documents give different start dates.",
  CALENDAR_MISSING: "The court calendar for this period is not installed, so closed days could not be applied.",
  EXCLUSION_FACT_UNCONFIRMED: "A time exclusion has been recorded but not confirmed.",
  RULE_WINDOW_AMBIGUOUS: "The rule changed during the possible start dates.",
};
const ERROR: Record<string, string> = {
  CF_DEADLINE_TRIGGER_UNKNOWN: "The start date of that event is unknown or unresolved. Fix it in the chronology first.",
  CF_DEADLINE_NO_LOCATOR: "That event has no source document and excerpt, so no deadline can be shown from it.",
  CF_DEADLINE_NO_AUTHORITY: "The rule cites no installed authority, so no deadline is shown.",
  CF_DEADLINE_NO_RULE: "No rule is in force on the start date.",
  CF_DEADLINE_TRIGGER_TYPE: "This rule runs from a different kind of event.",
  CF_DEADLINE_EXCLUSION_DAYS_MISSING: "An exclusion fact is recorded without its number of days.",
  CF_DEADLINE_ASSIGNMENT_STALE: "The rule or the chronology event is no longer available.",
};
const UNIT: Record<string, string> = { DAYS: "day", MONTHS: "month", YEARS: "year" };

function plain(code: string): string {
  const key = Object.keys(ERROR).find((k) => code.startsWith(k));
  return key ? ERROR[key] : code;
}
function when(r: DeadlineRecord): string {
  return r.result.kind === "DATE" ? r.result.date ?? "" : `${r.result.range_from} to ${r.result.range_to}`;
}

export default function DeadlinesView({ activeCase }: { activeCase: CaseSummary | null }) {
  const caseId = activeCase?.case_id ?? null;
  const [state, setState] = useState<DeadlinesState | null>(null);
  const [pick, setPick] = useState<Record<string, string>>({});
  const [fact, setFact] = useState<Record<string, { days: string; doc: string; excerpt: string; confirmed: boolean }>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!caseId) return;
    try {
      setState(await deadlinesCompute(caseId));
      setError(null);
    } catch (e) {
      setError(String(e));
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
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function exportIcs() {
    if (!caseId) return;
    try {
      const text = await deadlinesExportIcs(caseId);
      const url = URL.createObjectURL(new Blob([text], { type: "text/calendar;charset=utf-8" }));
      const a = document.createElement("a");
      a.href = url;
      a.download = "caseforge-deadlines.ics";
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      setError(String(e));
    }
  }

  if (!activeCase) return <div className="card"><p className="muted">Open a matter to compute its deadlines.</p></div>;
  const eventLabel = (id: string) => {
    const e = state?.events.find((x) => x.event_id === id);
    return e ? `${e.date || "undated"} — ${e.summary}` : id;
  };
  const exclusionTypes = Array.from(new Set((state?.rules ?? []).flatMap((r) => r.exclusions.map((x) => x.fact_type))));

  return (
    <div className="grid" style={{ gap: 12 }}>
      <div className="card" style={{ gridColumn: "1 / -1" }}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline" }}>
          <h4>Deadlines and limitation — {state?.jurisdiction ?? activeCase.jurisdiction}</h4>
          <button type="button" onClick={() => void exportIcs()} disabled={busy || !state?.records.length}>Download calendar file (.ics)</button>
        </div>
        <p className="muted" style={{ fontSize: "0.8rem" }}>
          Choose which event in the chronology starts each period. The app counts from that event using the installed rule and court calendar and shows every step, so you can check it by hand. Where the start date, a time exclusion or the calendar is uncertain you get a range and the reason, never a single date. These are computations to verify, not legal advice. The calendar file is saved on this computer and contacts nothing.
        </p>
        {error && <div className="notice warn">{error}</div>}
        {state && state.rules.length === 0 && (
          <div className="notice warn">No limitation or deadline rules are installed for {state.jurisdiction}. Install a legal pack that carries them (Legal Authorities).</div>
        )}
        {state?.unsigned_rules && state.rules.length > 0 && (
          <div className="notice warn">Some rules have not been signed off by a legal editor. Dates computed from them are for reference only.</div>
        )}
        {state && state.rejected_tables.length > 0 && <div className="notice warn">Some rule tables were rejected: {state.rejected_tables.join("; ")}</div>}
      </div>

      {state?.records.length ? (
        <div className="card" style={{ gridColumn: "1 / -1" }}>
          <h4>Computed</h4>
          <ul style={{ paddingLeft: "1.1rem", margin: 0 }}>
            {state.records.map((r) => (
              <li key={r.deadline_id} style={{ marginBottom: 12 }}>
                <div>
                  <strong>{r.label}</strong> — <span style={{ color: r.result.kind === "DATE" ? TONE.ok : TONE.warn, fontWeight: 600 }}>{r.result.kind === "DATE" ? when(r) : `somewhere between ${when(r)}`}</span>{" "}
                  <span className="mono" style={{ fontSize: "0.72rem", border: `1px solid ${TONE.warn}`, color: TONE.warn, padding: "0 4px", borderRadius: 3 }}>VERIFY</span>
                  {r.authority.rule_trust_state !== "SIGNED_OFF" && <span style={{ color: TONE.warn, fontSize: "0.78rem" }}> · rule not yet signed off</span>}
                </div>
                {r.result.uncertainty_reasons.length > 0 && (
                  <ul className="muted" style={{ fontSize: "0.8rem", margin: "2px 0" }}>
                    {r.result.uncertainty_reasons.map((u) => <li key={u}>{REASON[u] ?? u}</li>)}
                    {r.result.uncertainty_reasons.includes("CALENDAR_MISSING") && <li>The earliest date shown is the safe one to work to.</li>}
                  </ul>
                )}
                <details style={{ fontSize: "0.8rem" }}>
                  <summary className="muted">Show the working</summary>
                  <ol style={{ margin: "4px 0", paddingLeft: "1.2rem" }}>
                    <li>
                      Start: {r.trigger.event_type.toLowerCase().replace(/_/g, " ")}{" "}
                      {r.trigger.date ? `on ${r.trigger.date}` : r.trigger.date_range ? `between ${r.trigger.date_range.from} and ${r.trigger.date_range.to}` : ""} — recorded in <span className="mono">{r.trigger.locator.doc_id}</span>: “{r.trigger.locator.excerpt}”
                    </li>
                    <li>
                      Period: {r.computation.period.value} {UNIT[r.computation.period.unit]}
                      {r.computation.period.value === 1 ? "" : "s"}, {r.computation.exclude_trigger_day ? "the first day is not counted" : "the first day is counted as day 1"}.
                    </li>
                    {r.computation.exclusions_applied.map((x) => (
                      <li key={x.exclusion_id}>
                        Add {x.days_excluded} day{x.days_excluded === 1 ? "" : "s"} ({x.exclusion_id.replace(/-/g, " ")}) — “{x.locator.excerpt}” ({x.locator.doc_id})
                      </li>
                    ))}
                    {r.computation.rolled_forward_from && <li>The last day, {r.computation.rolled_forward_from}, was a day the court is closed, so the date moves to the next open day.</li>}
                    {r.computation.calendar_rule_ids.length > 0 && <li>Court calendar used: {r.computation.calendar_rule_ids.join(", ")}.</li>}
                    <li>
                      Rule <span className="mono">{r.rule_id}</span> — authority: {(r.authority.provision_label as string | undefined) ?? r.authority.chunk_id}
                      <CurrencyStamp packId={r.authority.pack_id} />
                    </li>
                  </ol>
                </details>
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      {state?.errors.length ? (
        <div className="card" style={{ gridColumn: "1 / -1" }}>
          <h4>No deadline could be shown</h4>
          <ul style={{ paddingLeft: "1.1rem", margin: 0, fontSize: "0.85rem" }}>
            {state.errors.map((e, i) => (
              <li key={i}>
                {state.rules.find((r) => r.rule_id === e.rule_id)?.label ?? e.rule_id}: {plain(e.error)}
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      {state?.rules.map((rule) => {
        const mine = state.assignments.filter((a) => a.rule_id === rule.rule_id);
        return (
          <div className="card" key={rule.rule_id}>
            <h4>{rule.label}</h4>
            <div className="muted" style={{ fontSize: "0.8rem" }}>
              {rule.period.value} {UNIT[rule.period.unit]}
              {rule.period.value === 1 ? "" : "s"} from a {rule.trigger_event_type.toLowerCase().replace(/_/g, " ")} · {rule.exclude_trigger_day ? "first day not counted" : "first day counted"}
              {rule.roll_forward ? " · moves to the next open day if the court is closed" : ""} · {rule.provision ?? rule.rule_id}
              {rule.trust_state !== "SIGNED_OFF" ? " · not yet signed off" : ""}
            </div>
            {mine.map((a) => (
              <div key={a.event_id} className="row" style={{ gap: 6, alignItems: "center", marginTop: 4, fontSize: "0.85rem" }}>
                <span>Starts from: {eventLabel(a.event_id)}</span>
                <button type="button" disabled={busy} onClick={() => void act(() => deadlinesUnassign(activeCase.case_id, rule.rule_id, a.event_id))}>Remove</button>
              </div>
            ))}
            <div className="row" style={{ gap: 6, marginTop: 6 }}>
              <select value={pick[rule.rule_id] ?? ""} onChange={(e) => setPick((p) => ({ ...p, [rule.rule_id]: e.target.value }))} aria-label={`Event that starts: ${rule.label}`} style={{ maxWidth: 380 }}>
                <option value="">Choose the event that starts the period…</option>
                {state.events.map((e) => (
                  <option key={e.event_id} value={e.event_id}>
                    {(e.date || "undated") + " — " + e.summary.slice(0, 80)}
                  </option>
                ))}
              </select>
              <button type="button" disabled={busy || !pick[rule.rule_id]} onClick={() => void act(() => deadlinesAssign(activeCase.case_id, rule.rule_id, pick[rule.rule_id]))}>Use this event</button>
            </div>
            {state.events.length === 0 && <div className="muted" style={{ fontSize: "0.8rem" }}>This matter's chronology has no events yet. Process the documents first.</div>}
          </div>
        );
      })}

      {exclusionTypes.length > 0 && (
        <div className="card" style={{ gridColumn: "1 / -1" }}>
          <h4>Time that the rules exclude</h4>
          <p className="muted" style={{ fontSize: "0.8rem" }}>
            Some rules exclude time (for example the days taken to obtain a copy of the order). Record it only if it happened; say where it is recorded. It counts only once you confirm it; until then the result is a range.
          </p>
          {exclusionTypes.map((ft) => {
            const saved = state?.facts.find((f) => f.fact_type === ft);
            const draft = fact[ft] ?? { days: saved ? String(saved.days) : "", doc: saved?.doc_id ?? "", excerpt: saved?.excerpt ?? "", confirmed: saved?.confirmed ?? false };
            const set = (patch: Partial<typeof draft>) => setFact((f) => ({ ...f, [ft]: { ...draft, ...patch } }));
            return (
              <div key={ft} className="row" style={{ gap: 6, alignItems: "center", marginBottom: 6, flexWrap: "wrap" }}>
                <span style={{ minWidth: 190 }}>{ft.toLowerCase().replace(/_/g, " ")}</span>
                <input type="number" min={0} max={3650} value={draft.days} onChange={(e) => set({ days: e.target.value })} placeholder="days" style={{ width: 80 }} aria-label="Days excluded" />
                <input value={draft.doc} onChange={(e) => set({ doc: e.target.value })} placeholder="where recorded (document, page)" style={{ width: 220 }} />
                <input value={draft.excerpt} onChange={(e) => set({ excerpt: e.target.value })} placeholder="what it says" style={{ width: 260 }} />
                <label><input type="checkbox" checked={draft.confirmed} onChange={(e) => set({ confirmed: e.target.checked })} /> confirmed</label>
                <button type="button" disabled={busy || draft.days === "" || !draft.doc.trim() || !draft.excerpt.trim()} onClick={() => void act(() => deadlinesFactSave(activeCase.case_id, ft, Number(draft.days), draft.confirmed, draft.doc, draft.excerpt))}>Save</button>
                {saved && <button type="button" disabled={busy} onClick={() => void act(() => deadlinesFactRemove(activeCase.case_id, ft))}>Remove</button>}
              </div>
            );
          })}
        </div>
      )}
      {state && <div className="muted mono" style={{ fontSize: "0.7rem", gridColumn: "1 / -1" }}>{state.engine_version} · computed on this computer from installed rules · no AI model</div>}
    </div>
  );
}
