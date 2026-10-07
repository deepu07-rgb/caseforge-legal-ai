/**
 * B-S1-10 Overnight batch panel — durable checkpoints, kill/resume (AC-6.8).
 * AC-3.2 full-tier 300 pages / 8h is hardware-gated (shown as budget note).
 */

import type { BatchJob } from "../lib/ipc";

export default function BatchView({
  job,
  busy,
  onStart,
  onResume,
  onCancel,
  onSimulateKill,
  onRefresh,
}: {
  job: BatchJob | null;
  busy: boolean;
  onStart: () => Promise<void>;
  onResume: () => Promise<void>;
  onCancel: () => Promise<void>;
  onSimulateKill: () => Promise<void>;
  onRefresh: () => Promise<void>;
}) {
  const pct = job?.progress_percent ?? 0;
  const running = job?.status === "running";
  const interrupted =
    job?.status === "interrupted" || job?.status === "failed" || job?.status === "cancelled";
  const done = job?.status === "completed";

  return (
    <div className="batch-layout" data-testid="batch-panel">
      <div className="row" style={{ justifyContent: "space-between", marginTop: 0 }}>
        <span className="eyebrow">docpipe · overnight batch</span>
        <span className={`chip ${running ? "ok" : done ? "ok" : "muted"}`}>
          {job?.status ?? "idle"}
        </span>
      </div>
      <h3>Batch queue + resume</h3>
      <p className="muted" style={{ fontSize: "0.85rem" }}>
        Triage → recognize → skeleton with durable checkpoints. Kill mid-batch and resume from the
        next pending unit (AC-6.8). Local only — no cloud queue.
      </p>

      <div className="meter" aria-hidden>
        <span style={{ width: `${running || done || interrupted ? pct : 0}%` }} />
      </div>
      <p className="mono muted" style={{ fontSize: "0.75rem" }} data-testid="batch-progress">
        {job
          ? `${pct}% · ${job.completed_units}/${job.total_units} units · stage ${job.stage} · ${
              job.message || "—"
            }`
          : "No batch job yet"}
      </p>

      {job?.ac_3_2_budget && (
        <div className="notice warn" style={{ fontSize: "0.82rem", marginTop: "0.75rem" }}>
          AC-3.2 budget: ≤ {job.ac_3_2_budget.max_pages} pages in ≤ {job.ac_3_2_budget.max_h} h on
          Full tier. {job.ac_3_2_budget.note}
        </div>
      )}

      <div className="row" style={{ marginTop: "0.85rem", flexWrap: "wrap", gap: "0.4rem" }}>
        <button
          type="button"
          className="primary"
          disabled={busy || running}
          onClick={() => void onStart()}
          data-testid="batch-start"
        >
          Start overnight batch
        </button>
        <button
          type="button"
          disabled={busy || !job || running || done}
          onClick={() => void onResume()}
          data-testid="batch-resume"
        >
          Resume
        </button>
        <button
          type="button"
          disabled={busy || !job || !running}
          onClick={() => void onCancel()}
          data-testid="batch-cancel"
        >
          Cancel / kill
        </button>
        <button
          type="button"
          disabled={busy || running}
          onClick={() => void onSimulateKill()}
          data-testid="batch-simulate-kill"
          title="Starts batch and stops after 1 unit to exercise AC-6.8 resume"
        >
          Simulate kill (1 unit)
        </button>
        <button type="button" disabled={busy} onClick={() => void onRefresh()}>
          Refresh
        </button>
      </div>

      {job?.error && (
        <p className="notice warn" style={{ marginTop: "0.75rem", fontSize: "0.82rem" }}>
          {job.error}
        </p>
      )}

      {job?.checkpoints && job.checkpoints.length > 0 && (
        <>
          <div className="stat-label" style={{ marginTop: "1.1rem" }}>
            Checkpoints
          </div>
          <ul className="batch-checkpoints" data-testid="batch-checkpoints">
            {job.checkpoints.map((c) => (
              <li
                key={`${c.unit_kind}:${c.unit_id}`}
                className={`batch-cp ${c.status}`}
              >
                <span className="mono">
                  {c.unit_kind}/{c.unit_id.slice(0, 12)}
                </span>
                <span className="mono">{c.status}</span>
                <span className="muted">{c.detail || ""}</span>
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}
