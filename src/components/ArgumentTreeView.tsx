/**
 * Argument Tree + Integrity Engine UI (B-S2-01 / PRD §8).
 * Scoring is host-side deterministic (caseforge.integrity.v1) — never LLM.
 */

import { useCallback, useEffect, useState } from "react";
import {
  argumentTreeGet,
  integrityBuildFromGraph,
  integrityPolicy,
  integrityScoreTree,
  type CaseSummary,
  type IntegrityScoreReport,
  type MissingLinkRecord,
  type ScoredLimbReport,
} from "../lib/ipc";
import type { ProvenanceRecord } from "../lib/provenance";
import ProvenanceInspector from "./ProvenanceInspector";

type Props = {
  activeCase: CaseSummary | null;
  onOpenCases: () => void;
  onError: (msg: string | null) => void;
  onNotice: (msg: string | null) => void;
};

const LINK_ORDER = ["Fact", "Evidence", "Law", "Exposure"] as const;

function viChipClass(vi: number): string {
  if (vi <= 25) return "chip ok";
  if (vi <= 50) return "chip";
  return "chip warn";
}

function LimbCard({
  limb,
  threshold,
}: {
  limb: ScoredLimbReport;
  threshold: number;
}) {
  const links = limb.links;
  return (
    <article className="limb-card" data-testid={`limb-${limb.node_id}`}>
      <div className="limb-head">
        <div>
          <div className="limb-id">{limb.node_id}</div>
          <h4 className="limb-label">{limb.label}</h4>
        </div>
        <span
          className={viChipClass(limb.VI)}
          title="Vulnerability Index — higher means weaker preparation on the min link (not a win prediction)"
        >
          VI <strong>{limb.VI}</strong>
          <span className="muted"> · min {limb.min_link}:{limb.min_score}</span>
        </span>
      </div>

      <div className="table-wrap">
        <table className="table mono">
          <thead>
            <tr>
              <th>Link</th>
              <th>Score</th>
              <th>Bar</th>
              <th>Missing</th>
            </tr>
          </thead>
          <tbody>
            {LINK_ORDER.map((k) => {
              const score = links[k];
              const isMin = k === limb.min_link;
              const missing = score < threshold;
              return (
                <tr key={k}>
                  <td>
                    {k}
                    {isMin ? " ← min" : ""}
                  </td>
                  <td>{score}</td>
                  <td style={{ minWidth: 120 }}>
                    <div className="vi-meter-wrap">
                      <div className="meter" aria-hidden>
                        <span
                          style={{
                            width: `${score}%`,
                            background: missing ? "var(--amber)" : undefined,
                          }}
                        />
                      </div>
                    </div>
                  </td>
                  <td>
                    {missing ? (
                      <span className="chip warn">&lt;{threshold}</span>
                    ) : (
                      "—"
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </article>
  );
}

function MissingPanel({
  rows,
  topN = 2,
}: {
  rows: MissingLinkRecord[];
  topN?: number;
}) {
  if (rows.length === 0) {
    return (
      <div className="notice ok" data-testid="missing-links-empty">
        No Missing Links under threshold — all four links on every limb score ≥ threshold.
      </div>
    );
  }
  return (
    <div className="table-wrap" data-testid="missing-links-panel">
      <table className="table mono">
        <thead>
          <tr>
            <th>Rank</th>
            <th>Limb</th>
            <th>Link</th>
            <th>Score</th>
            <th>Planted</th>
            <th>Inputs cited</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((m) => (
            <tr
              key={`${m.limb_id}-${m.link}-${m.rank}`}
              className={m.rank <= topN ? "row-emphasis" : undefined}
            >
              <td>{m.rank}</td>
              <td>{m.limb_id}</td>
              <td>{m.link}</td>
              <td>{m.score}</td>
              <td>
                {m.planted_gap_id ? (
                  <span className="chip warn">{m.planted_gap_id}</span>
                ) : (
                  "—"
                )}
              </td>
              <td className="muted" style={{ maxWidth: 360, whiteSpace: "normal" }}>
                {(m.inputs_cited || []).join(" · ") || "—"}
                {m.description ? (
                  <div className="theory" style={{ marginTop: 4 }}>
                    {m.description}
                  </div>
                ) : null}
                <div>
                  <ProvenanceInspector compact record={{
                    stableId: `missing-link:${m.limb_id}:${m.link}:${m.rank}`,
                    propositionClass: "UNSUPPORTED_OR_GAP",
                    origin: "UNSUPPORTED_GAP",
                    supportState: "UNSUPPORTED",
                    title: `${m.limb_id} ${m.link} missing link`,
                    verification: "Deterministic Integrity Engine gap. This is not verified evidence or legal authority.",
                    version: "caseforge.integrity.v1",
                    supportIds: m.inputs_cited || [],
                  } satisfies ProvenanceRecord} />
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export default function ArgumentTreeView({
  activeCase,
  onOpenCases,
  onError,
  onNotice,
}: Props) {
  const [busy, setBusy] = useState(false);
  const [policy, setPolicy] = useState<Record<string, unknown> | null>(null);
  const [tree, setTree] = useState<Record<string, unknown> | null>(null);
  const [score, setScore] = useState<IntegrityScoreReport | null>(null);

  useEffect(() => {
    void integrityPolicy()
      .then(setPolicy)
      .catch((e) => onError(e instanceof Error ? e.message : String(e)));
  }, [onError]);

  const load = useCallback(async () => {
    if (!activeCase) {
      setTree(null);
      setScore(null);
      return;
    }
    setBusy(true);
    onError(null);
    try {
      const t = await argumentTreeGet(activeCase.case_id);
      setTree(t);
      if (t) {
        const r = await integrityScoreTree(t);
        setScore(r);
      } else {
        setScore(null);
      }
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }, [activeCase, onError]);

  useEffect(() => {
    void load();
  }, [load]);

  const buildFromGraph = async () => {
    if (!activeCase) {
      onError("Open a case first (Cases → Unlock & open).");
      return;
    }
    setBusy(true);
    onError(null);
    try {
      const res = await integrityBuildFromGraph(activeCase.case_id, true);
      setTree(res.tree);
      setScore(res.score);
      onNotice(
        `Argument Tree built from case_graph · scorer ${res.score.scorer_id} · ${res.score.recompute_ms} ms · no LLM`,
      );
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const rescore = async () => {
    if (!tree) {
      onError("No argument tree loaded.");
      return;
    }
    setBusy(true);
    onError(null);
    try {
      const r = await integrityScoreTree(tree);
      setScore(r);
      onNotice(
        `VI recomputed in ${r.recompute_ms} ms (budget ≤ 5 s) · llm_used=${r.llm_used}`,
      );
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const threshold =
    score?.threshold ??
    (typeof policy?.missing_link_threshold === "number"
      ? policy.missing_link_threshold
      : 60);
  const relief = tree?.relief_root as
    | { label?: string; node_id?: string; jurisdiction?: string }
    | undefined;

  return (
    <div className="page-grid" data-testid="argument-tree-view">
      {activeCase && (
        <div className="matter-chips">
          <span className="chip matter">
            {activeCase.cause_title || activeCase.label}
          </span>
          <span className="chip matter">{activeCase.jurisdiction}</span>
          <span className="chip ok">Integrity · deterministic</span>
          {score && !score.llm_used && (
            <span className="chip ok">AC-8.3 no LLM</span>
          )}
        </div>
      )}

      <div className="card">
        <div className="card-head">
          <div>
            <div className="eyebrow">B-S2-01 · PRD §8</div>
            <h3>Argument Tree &amp; Integrity Engine</h3>
            <p className="muted">
              Root = relief · limbs = arguments · leaves = anchors. Vulnerability
              Index = 100 − min(Fact, Evidence, Law, Exposure). Missing Links always
              cite inputs. Scores are preparation signals — not outcome prediction
              (G6).
            </p>
          </div>
        </div>

        <div className="btn-row" style={{ marginTop: 12 }}>
          {!activeCase ? (
            <button type="button" className="btn primary" onClick={onOpenCases}>
              Open a case
            </button>
          ) : (
            <>
              <button
                type="button"
                className="btn primary"
                disabled={busy}
                onClick={() => void buildFromGraph()}
              >
                {busy ? "Working…" : "Build from case_graph"}
              </button>
              <button
                type="button"
                className="btn"
                disabled={busy || !tree}
                onClick={() => void rescore()}
              >
                Rescore VI
              </button>
              <button
                type="button"
                className="btn ghost"
                disabled={busy}
                onClick={() => void load()}
              >
                Reload
              </button>
            </>
          )}
        </div>

        {policy && (
          <p className="muted mono" style={{ marginTop: 10, fontSize: 12 }}>
            {String(policy.pipeline_id)} · threshold{" "}
            {String(policy.missing_link_threshold)} · recompute ≤{" "}
            {String(policy.recompute_max_s)}s · scoring_may_use_llm=
            {String(policy.scoring_may_use_llm)}
          </p>
        )}
      </div>

      {relief && (
        <div className="card">
          <div className="eyebrow">Relief root</div>
          <h4>{relief.label}</h4>
          <p className="muted mono">
            {relief.node_id}
            {relief.jurisdiction ? ` · ${relief.jurisdiction}` : ""}
          </p>
        </div>
      )}

      {score && (
        <>
          <div className="card">
            <div className="card-head">
              <div>
                <div className="eyebrow">Missing Links panel</div>
                <h3>Where the tree needs strength</h3>
                <p className="muted">
                  Ranked weakest-first. Planted gaps must appear in top-2 (AC-8.1).
                  Each row cites its inputs (anchors / case_graph / planted ids).
                </p>
              </div>
              <div className="stat-stack">
                <div className="stat">
                  <div className="stat-label">Weakest limb</div>
                  <div className="stat-value">
                    {score.weakest_limb ?? "—"}{" "}
                    <span className="muted">VI {score.weakest_VI ?? "—"}</span>
                  </div>
                </div>
                <div className="stat">
                  <div className="stat-label">Recompute</div>
                  <div className="stat-value">{score.recompute_ms} ms</div>
                </div>
              </div>
            </div>
            <MissingPanel rows={score.missing_links_panel} topN={2} />
          </div>

          <div className="card">
            <div className="eyebrow">Limbs</div>
            <h3>Link scores &amp; VI</h3>
            <div className="stack" style={{ marginTop: 12, gap: 12 }}>
              {score.limbs.map((limb) => (
                <LimbCard key={limb.node_id} limb={limb} threshold={threshold} />
              ))}
            </div>
          </div>
        </>
      )}

      {activeCase && !tree && !busy && (
        <div className="notice warn">
          No argument tree stored for this case. Run skeleton extract if needed,
          then <strong>Build from case_graph</strong>. Stage 2 never re-parses
          documents (AC-2.1).
        </div>
      )}
    </div>
  );
}
