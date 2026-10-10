import { useCallback, useEffect, useRef, useState } from "react";
import type { RehearsalPolicy, RehearsalRecord, RehearsalStart } from "../lib/ipc";
import { rehearsalEnd, rehearsalPolicy, rehearsalStart, rehearsalState } from "../lib/ipc";

/**
 * B-S4-12: Timed Oral Rehearsal. The host keeps the clock; this panel only shows it (it asks the host for the time left a few times a
 * second and counts down between answers). At the limit the session ends on its own, the unsent text stays in the box, and the
 * timeline below shows which points of the argument were reached, when, for how long, and which never were.
 */
const RATE_LABEL: Record<string, string> = { NONE: "No interruptions", LOW: "Occasional", MEDIUM: "Regular", HIGH: "Frequent" };

export const mmss = (ms: number) => {
  const s = Math.max(0, Math.round(ms / 1000));
  return `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
};

function plain(e: unknown): string {
  const s = e instanceof Error ? e.message : String(e);
  if (s.includes("CF_REHEARSAL_NO_TREE")) return "Build the argument tree first (Argument Integrity), so the bench has points and Missing Links to press on.";
  if (s.includes("CF_EDITION_NOT_ENTITLED")) return "Timed rehearsal is not part of this licence.";
  return s.replace(/^CF_[A-Z_]+:\s*/, "");
}

export default function TimedRehearsalPanel({
  caseId,
  sessionId,
  archetypeId,
  interruptionSlider,
  onActive,
  onRecord,
  onNotice,
}: {
  caseId: string | null;
  sessionId: string;
  archetypeId: string;
  interruptionSlider: number;
  onActive: (active: boolean) => void;
  onRecord: (record: RehearsalRecord | null) => void;
  onNotice: (text: string) => void;
}) {
  const [policy, setPolicy] = useState<RehearsalPolicy | null>(null);
  const [slot, setSlot] = useState<number>(300);
  const [custom, setCustom] = useState("");
  const [rate, setRate] = useState<string>("AUTO");
  const [run, setRun] = useState<RehearsalStart | null>(null);
  const [remaining, setRemaining] = useState<number>(0);
  const [error, setError] = useState<string | null>(null);
  const base = useRef<{ remaining: number; at: number } | null>(null);
  const ending = useRef(false);

  useEffect(() => {
    void rehearsalPolicy().then(setPolicy).catch(() => setPolicy(null));
  }, []);

  const finish = useCallback(async () => {
    if (!caseId || ending.current) return;
    ending.current = true;
    try {
      const rec = await rehearsalEnd(caseId, sessionId);
      onRecord(rec);
      onNotice("The slot has ended. Your timeline is in the Debrief tab.");
    } catch (e) {
      setError(plain(e));
    } finally {
      setRun(null);
      base.current = null;
      onActive(false);
      ending.current = false;
    }
  }, [caseId, sessionId, onActive, onRecord, onNotice]);

  // ask the host for the time left; count down locally between answers
  useEffect(() => {
    if (!run) return;
    let alive = true;
    const poll = async () => {
      try {
        const st = await rehearsalState(sessionId);
        if (!alive) return;
        base.current = { remaining: st.remaining_ms, at: performance.now() };
        setRemaining(st.remaining_ms);
        for (const e of st.new_events) {
          if (e.kind === "TIME_WARNING") onNotice(`${mmss(st.remaining_ms)} left.`);
        }
        if (st.ended_by) void finish();
      } catch (e) {
        if (alive) setError(plain(e));
      }
    };
    void poll();
    const ask = window.setInterval(() => void poll(), 500);
    const show = window.setInterval(() => {
      if (base.current) setRemaining(Math.max(0, base.current.remaining - (performance.now() - base.current.at)));
    }, 200);
    return () => {
      alive = false;
      window.clearInterval(ask);
      window.clearInterval(show);
    };
  }, [run, sessionId, finish, onNotice]);

  const start = async () => {
    if (!caseId) return;
    setError(null);
    onRecord(null);
    const seconds = custom.trim() ? Math.round(Number(custom) * 60) : slot;
    try {
      const r = await rehearsalStart({ caseId, sessionId, slotSeconds: seconds, rate: rate === "AUTO" ? undefined : rate, interruptionSlider, archetypeId });
      setRun(r);
      setRemaining(r.slot_ms);
      base.current = { remaining: r.slot_ms, at: performance.now() };
      onActive(true);
      onNotice(`Timed session started: ${mmss(r.slot_ms)}. The bench ${r.interval_ms ? `may press you on a Missing Link about every ${Math.round(r.interval_ms / 1000)} s` : "will not interrupt"}.`);
    } catch (e) {
      setError(plain(e));
    }
  };

  if (!caseId) return null;
  return (
    <div className="card" data-testid="timed-rehearsal" style={{ marginBottom: "0.75rem" }}>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline", flexWrap: "wrap", gap: 8 }}>
        <strong>Timed rehearsal</strong>
        {run && (
          <span className={`chip ${remaining < 30_000 ? "warn" : "ok"}`} data-testid="rehearsal-clock" style={{ fontSize: "1.1rem" }}>
            {mmss(remaining)}
          </span>
        )}
      </div>
      {!run && (
        <div className="row" style={{ gap: 8, flexWrap: "wrap", alignItems: "center", marginTop: 6 }}>
          <label>
            Slot{" "}
            <select value={slot} disabled={!!custom.trim()} onChange={(e) => setSlot(Number(e.target.value))}>
              {(policy?.presets_seconds ?? [120, 300, 600, 900, 1800]).map((s) => (
                <option key={s} value={s}>
                  {s / 60} min
                </option>
              ))}
            </select>
          </label>
          <label>
            or minutes <input type="number" min={1} max={120} step={1} value={custom} onChange={(e) => setCustom(e.target.value)} style={{ width: 70 }} />
          </label>
          <label>
            Bench interruptions{" "}
            <select value={rate} onChange={(e) => setRate(e.target.value)}>
              <option value="AUTO">From the bench style</option>
              {(policy?.rates ?? ["NONE", "LOW", "MEDIUM", "HIGH"]).map((r) => (
                <option key={r} value={r}>
                  {RATE_LABEL[r] ?? r}
                </option>
              ))}
            </select>
          </label>
          <button type="button" onClick={() => void start()}>
            Start the clock
          </button>
        </div>
      )}
      {run && (
        <div className="row" style={{ gap: 8, marginTop: 6, flexWrap: "wrap", alignItems: "center" }}>
          <span className="muted" style={{ fontSize: "0.8rem" }}>
            {RATE_LABEL[run.rate] ?? run.rate} · {run.limbs.length} points to reach · the clock runs in the app, not in this window
          </span>
          <button type="button" onClick={() => void finish()}>
            End now
          </button>
        </div>
      )}
      {error && <div className="notice warn">{error}</div>}
      <p className="muted" style={{ fontSize: "0.75rem", marginTop: 6 }}>
        When the clock runs out the session ends and anything you had not yet sent stays in the box. The bench’s interruptions come from your own Missing Links, not from a model. A point counts as reached when you use enough of its own words; a point made in other words can be missed, and time per point is estimated from when each answer was sent.
      </p>
    </div>
  );
}

export function RehearsalTimelineView({ record }: { record: RehearsalRecord }) {
  const tl = record.timeline;
  const labels = record.limb_labels;
  const slotMs = tl.slot_seconds * 1000;
  const label = (id: string | null) => (id ? labels?.[id] ?? id : "");
  const pct = (ms: number) => `${Math.min(100, (100 * ms) / slotMs)}%`;
  return (
    <div data-testid="rehearsal-timeline" style={{ marginBottom: "0.75rem" }}>
      <div className="stat-label">Timed rehearsal · {mmss(slotMs)} slot · ended {tl.ended_by === "TIME_EXPIRED" ? "when time ran out" : "by you"}</div>
      <div style={{ position: "relative", height: 18, background: "var(--panel-2, #eee)", borderRadius: 4, margin: "6px 0 10px" }} aria-label="Slot timeline">
        {tl.events
          .filter((e) => ["INTERRUPTION", "TIME_WARNING", "LIMB_REACHED"].includes(e.kind))
          .map((e, i) => (
            <span
              key={i}
              title={`${mmss(e.t_ms)} ${e.kind === "INTERRUPTION" ? "bench interrupts" : e.kind === "LIMB_REACHED" ? `reached: ${label(e.limb_node_id)}` : "time warning"}`}
              style={{ position: "absolute", left: pct(e.t_ms), top: 0, width: 3, height: 18, background: e.kind === "INTERRUPTION" ? "#b45309" : e.kind === "LIMB_REACHED" ? "#15803d" : "#6b7280" }}
            />
          ))}
      </div>
      <table style={{ width: "100%", fontSize: "0.82rem", borderCollapse: "collapse" }}>
        <thead>
          <tr>
            <th align="left">Point of the argument</th>
            <th align="left">Reached</th>
            <th align="left">Time on it</th>
            <th align="left">Interrupted</th>
          </tr>
        </thead>
        <tbody>
          {tl.limbs.map((l) => (
            <tr key={l.limb_node_id} data-testid={l.reached ? "limb-reached" : "limb-not-reached"}>
              <td>{label(l.limb_node_id)}</td>
              <td>{l.reached ? `at ${mmss(l.first_reached_ms ?? 0)}` : <strong>never reached</strong>}</td>
              <td>
                {l.reached ? (
                  <span style={{ display: "inline-block", minWidth: 80 }}>
                    <span style={{ display: "inline-block", height: 8, width: pct(l.time_spent_ms), background: "#2563eb", borderRadius: 2, verticalAlign: "middle" }} /> {Math.round(l.time_spent_ms / 1000)} s
                  </span>
                ) : (
                  "—"
                )}
              </td>
              <td>{l.interruptions || "—"}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <ul style={{ paddingLeft: "1.1rem", fontSize: "0.82rem" }}>
        {record.findings.map((f, i) => (
          <li key={i} data-testid="rehearsal-finding">
            {f.text} <span className="muted" style={{ fontSize: "0.7rem" }}>(from the timeline{f.event_index !== null ? `, event ${f.event_index + 1}` : ""})</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
