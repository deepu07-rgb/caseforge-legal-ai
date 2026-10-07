/**
 * B-S1-14 Digest surface — CASEBOOK · CHAMBER · DIGEST (PRD §12).
 * B-S1-15 adds DRAFTING (templates + DOCX/PDF export, G4/G5).
 * Preparation Completeness from case_graph only (no LLM). G6 language.
 */

import { useMemo, useState } from "react";
import {
  computePrepCompleteness,
  confidenceBand,
  formatSourceChip,
  PREP_SCORE_ID,
  type ConfidenceBand,
} from "../lib/completeness";
import type { CaseGraph, CaseSummary, InterviewSession } from "../lib/ipc";
import { graphProvenance, type ProvenanceRecord } from "../lib/provenance";
import DraftingView from "./DraftingView";
import ProvenanceInspector from "./ProvenanceInspector";

type Surface = "casebook" | "chamber" | "digest" | "drafting";

export default function DigestView({
  activeCase,
  docCount,
  caseGraph,
  interview,
  onRefreshGraph,
  onOpenWorkspace,
  onOpenReview,
  onOpenInterview,
  onRunSkeleton,
  busy,
}: {
  activeCase: CaseSummary | null;
  docCount: number;
  caseGraph: CaseGraph | null;
  interview: InterviewSession | null;
  onRefreshGraph: () => Promise<void>;
  onOpenWorkspace: () => void;
  onOpenReview: () => void;
  onOpenInterview: () => void;
  onRunSkeleton: () => Promise<void>;
  busy: boolean;
}) {
  const [surface, setSurface] = useState<Surface>("digest");
  const prep = useMemo(() => computePrepCompleteness(caseGraph), [caseGraph]);
  const title = activeCase?.cause_title || activeCase?.label || "No matter open";
  const hasMatter = Boolean(activeCase);
  const hasGraph = Boolean(caseGraph);

  return (
    <div className="page-grid digest-root" data-testid="digest-panel">
      <div className="matter-chips">
        <span className="chip matter">{title}</span>
        {activeCase && (
          <>
            <span className="chip matter">{activeCase.jurisdiction}</span>
            <span className="chip matter">{activeCase.court || "Court"}</span>
            <span className="chip matter">{activeCase.user_side || "Side"}</span>
            <span className="chip ok">Encrypted</span>
          </>
        )}
        <span className="chip muted">Stage 1 · Digest</span>
      </div>

      {/* §12 surface switcher */}
      <div className="surface-tabs" data-testid="digest-surfaces">
        {(
          [
            ["casebook", "CASEBOOK"],
            ["chamber", "CHAMBER"],
            ["digest", "DIGEST"],
            ["drafting", "DRAFTING"],
          ] as [Surface, string][]
        ).map(([id, label]) => (
          <button
            key={id}
            type="button"
            className={`surface-tab ${surface === id ? "active" : ""}`}
            onClick={() => setSurface(id)}
            data-testid={
              id === "casebook"
                ? "surface-casebook"
                : id === "chamber"
                  ? "surface-chamber"
                  : id === "digest"
                    ? "surface-digest"
                    : "surface-drafting"
            }
          >
            {label}
          </button>
        ))}
      </div>

      <p className="muted g6-banner" data-testid="g6-banner">
        Preparation completeness only — never win prediction, outcome odds, or verdict language.
        Score recomputes from case_graph only (no LLM). Not a prediction of case outcome.
      </p>

      <PrepScorePanel prep={prep} hasGraph={hasGraph} hasMatter={hasMatter} docCount={docCount} />

      <div className="row" style={{ gap: "0.4rem", flexWrap: "wrap" }}>
        <button
          type="button"
          className="primary"
          disabled={!hasMatter || busy}
          onClick={() => void onRunSkeleton()}
          data-testid="digest-run-skeleton"
        >
          {hasGraph ? "Re-run skeleton" : "Build case graph"}
        </button>
        <button type="button" disabled={!hasMatter || busy} onClick={() => void onRefreshGraph()}>
          Refresh graph
        </button>
        <button type="button" disabled={!hasMatter} onClick={onOpenWorkspace}>
          Case file
        </button>
      </div>

      {surface === "casebook" && (
        <CasebookPanel
          caseGraph={caseGraph}
          onOpenReview={onOpenReview}
          onOpenWorkspace={onOpenWorkspace}
        />
      )}
      {surface === "chamber" && (
        <ChamberPanel
          caseGraph={caseGraph}
          interview={interview}
          prep={prep}
          onOpenInterview={onOpenInterview}
          onOpenWorkspace={onOpenWorkspace}
        />
      )}
      {surface === "digest" && <DigestPanels caseGraph={caseGraph} prep={prep} />}
      {surface === "drafting" && (
        <DraftingView
          activeCase={activeCase}
          caseGraph={caseGraph}
          onRunSkeleton={onRunSkeleton}
          busy={busy}
        />
      )}

      <div className="matter-strip">
        <span>
          {PREP_SCORE_ID} · {prep.score_0_100}% prep · open gaps {prep.open_gaps} ·{" "}
          {hasGraph ? "graph bound" : "no graph"} · {docCount} docs
        </span>
        <span>llm_used={String(prep.llm_used)} · source={prep.source}</span>
      </div>
    </div>
  );
}

function PrepScorePanel({
  prep,
  hasGraph,
  hasMatter,
  docCount,
}: {
  prep: ReturnType<typeof computePrepCompleteness>;
  hasGraph: boolean;
  hasMatter: boolean;
  docCount: number;
}) {
  return (
    <div className="completeness" data-testid="prep-completeness">
      <div className="completeness-head">
        <div>
          <div className="stat-label">Preparation completeness</div>
          <div className="prep-score-value" data-testid="prep-score">
            {hasGraph ? `${prep.score_0_100}%` : hasMatter ? "—" : "—"}
          </div>
        </div>
        <div className="muted" style={{ fontSize: "0.85rem", fontFamily: "var(--font-reading)", maxWidth: 360 }}>
          Deterministic from case_graph only ({prep.score_id}). Never win probability.
          {!hasGraph && hasMatter && " Run skeleton extraction to populate the graph."}
        </div>
      </div>
      <div className="meter" style={{ height: 4 }}>
        <span style={{ width: hasGraph ? `${prep.score_0_100}%` : hasMatter ? "8%" : "3%" }} />
      </div>
      <div className="completeness-stats">
        {prep.components.map((c) => (
          <div key={c.id} data-testid={`prep-bucket-${c.id}`}>
            <strong className={c.filled ? "filled" : ""}>{c.count}</strong>
            <span>
              {c.label} ({c.earned}/{c.points})
            </span>
          </div>
        ))}
      </div>
      <p className="mono muted" style={{ fontSize: "0.7rem", marginTop: "0.65rem" }}>
        formula: {prep.formula}
        {!hasGraph ? ` · docs in vault: ${docCount}` : ` · open_gaps: ${prep.open_gaps}`}
      </p>
    </div>
  );
}

function CasebookPanel({
  caseGraph,
  onOpenReview,
  onOpenWorkspace,
}: {
  caseGraph: CaseGraph | null;
  onOpenReview: () => void;
  onOpenWorkspace: () => void;
}) {
  const lowItems = useMemo(() => collectLowConfidence(caseGraph), [caseGraph]);
  return (
    <div className="digest-layout" data-testid="panel-casebook">
      <div className="card">
        <span className="eyebrow">CASEBOOK · verification</span>
        <h3 className="digest-section-title">Source-linked review</h3>
        <p className="muted" style={{ fontSize: "0.9rem" }}>
          CASEBOOK is the verification desk: click a fact to jump to its page span (AC-6.3), correct
          in ≤2 steps (AC-6.4). Low-confidence items are highlighted only.
        </p>
        {lowItems.length === 0 ? (
          <p className="muted">
            {caseGraph
              ? "No low-confidence extracted items in the current graph heat map."
              : "No case_graph yet — build the skeleton first."}
          </p>
        ) : (
          <ul className="heat-list">
            {lowItems.slice(0, 12).map((it) => (
              <li key={it.key} className={`heat-${it.band}`}>
                <strong>{it.kind}</strong> · {it.label}
                <div className="mono muted" style={{ fontSize: "0.72rem" }}>
                  conf={it.conf ?? "missing"} · {it.source}
                </div>
                <ProvenanceInspector record={it.provenance} compact />
              </li>
            ))}
          </ul>
        )}
        <div className="row" style={{ marginTop: "0.85rem", gap: "0.4rem" }}>
          <button type="button" className="primary" onClick={onOpenReview} data-testid="goto-review">
            Open Review
          </button>
          <button type="button" onClick={onOpenWorkspace}>
            Case file
          </button>
        </div>
      </div>
      <div className="card">
        <h4>Confidence heat legend</h4>
        <div className="heat-legend">
          <span className="heat-ok">OK (≥ threshold)</span>
          <span className="heat-low">Low — verify source</span>
          <span className="heat-force">Force review</span>
          <span className="heat-missing">Missing confidence</span>
        </div>
        <p className="muted" style={{ fontSize: "0.82rem", marginTop: "0.75rem" }}>
          Thresholds from contracts confidence_thresholds (item_highlight_below / item_block_downstream_below).
        </p>
      </div>
    </div>
  );
}

function ChamberPanel({
  caseGraph,
  interview,
  prep,
  onOpenInterview,
  onOpenWorkspace,
}: {
  caseGraph: CaseGraph | null;
  interview: InterviewSession | null;
  prep: ReturnType<typeof computePrepCompleteness>;
  onOpenInterview: () => void;
  onOpenWorkspace: () => void;
}) {
  const openGaps = (caseGraph?.gaps || []).filter((g) => (g.status || "open") === "open");
  const counselN = caseGraph?.counsel_facts?.length ?? 0;
  return (
    <div className="digest-layout" data-testid="panel-chamber">
      <div className="card">
        <span className="eyebrow">CHAMBER · working desk</span>
        <h3 className="digest-section-title">Guided intake & gaps</h3>
        <p className="muted" style={{ fontSize: "0.9rem" }}>
          CHAMBER is counsel&apos;s working surface: fill skeleton gaps via guided interview
          (SOURCED_BY_COUNSEL). Questions never invent legal strategy.
        </p>
        <div className="chamber-stats">
          <div>
            <strong>{prep.score_0_100}%</strong>
            <span>prep complete</span>
          </div>
          <div>
            <strong>{openGaps.length}</strong>
            <span>open gaps</span>
          </div>
          <div>
            <strong>{counselN}</strong>
            <span>counsel facts</span>
          </div>
          <div>
            <strong>{interview ? `${Math.round(interview.completion_percent)}%` : "—"}</strong>
            <span>interview</span>
          </div>
        </div>
        {openGaps.length > 0 ? (
          <ul className="gap-list">
            {openGaps.slice(0, 12).map((g) => (
              <li key={g.gap_id}>
                <strong>{g.topic}</strong>
                {g.question_prompt && (
                  <div className="muted" style={{ fontSize: "0.85rem" }}>
                    {g.question_prompt}
                  </div>
                )}
              </li>
            ))}
          </ul>
        ) : (
          <p className="muted">
            {caseGraph ? "No open gaps in graph." : "No case_graph — extract skeleton first."}
          </p>
        )}
        <div className="row" style={{ marginTop: "0.85rem", gap: "0.4rem" }}>
          <button
            type="button"
            className="primary"
            onClick={onOpenInterview}
            data-testid="goto-interview"
          >
            Open guided interview
          </button>
          <button type="button" onClick={onOpenWorkspace}>
            Case file
          </button>
        </div>
      </div>
      <div className="card">
        <h4>Interview session</h4>
        {interview ? (
          <>
            <p className="mono muted" style={{ fontSize: "0.75rem" }}>
              status={interview.status} · cursor={interview.cursor_index}/
              {interview.max_questions} · completion={Math.round(interview.completion_percent)}%
            </p>
            <ul className="gap-list">
              {interview.questions.slice(0, 8).map((q) => (
                <li key={q.question_index}>
                  <span className={`chip ${q.status === "answered" ? "ok" : "muted"}`}>
                    {q.status}
                  </span>{" "}
                  {q.prompt || q.topic || `Q${q.question_index}`}
                </li>
              ))}
            </ul>
          </>
        ) : (
          <p className="muted">No interview session yet. Open guided interview to start (AC-6.5/6.6).</p>
        )}
      </div>
    </div>
  );
}

function DigestPanels({
  caseGraph,
  prep,
}: {
  caseGraph: CaseGraph | null;
  prep: ReturnType<typeof computePrepCompleteness>;
}) {
  if (!caseGraph) {
    return (
      <div className="card" data-testid="panel-digest">
        <span className="eyebrow">DIGEST · synthesis</span>
        <h3 className="digest-section-title">No case graph yet</h3>
        <p className="muted">
          Chronology, parties, statutes, contradictions, and preparation completeness appear here
          after skeleton extraction writes case_graph.
        </p>
      </div>
    );
  }

  const contradictions = caseGraph.contradictions || [];
  const nodes = caseGraph.nodes || [];
  const edges = caseGraph.edges || [];
  const nodeCounts = nodes.reduce<Record<string, number>>((acc, node) => {
    acc[node.node_type] = (acc[node.node_type] || 0) + 1;
    return acc;
  }, {});

  return (
    <div className="digest-layout" data-testid="panel-digest">
      <div className="card">
        <span className="eyebrow">DIGEST · chronology</span>
        <h3 className="digest-section-title">Chronology</h3>
        {caseGraph.schema_version === "2.0.0" && (
          <div className="graph-v2-audit" data-testid="graph-v2-audit">
            <strong>Graph v2</strong> · {nodes.length} nodes · {edges.length} traceable links ·{" "}
            {caseGraph.gaps.filter((g) => g.status === "open").length} explicit gaps
            <div className="mono muted">
              extractor={caseGraph.pipeline?.extractor_id} · resolver={caseGraph.pipeline?.entity_resolver_id}
            </div>
          </div>
        )}
        <div className="timeline">
          {(caseGraph.chronology || []).length === 0 ? (
            <div className="timeline-item low">
              <div className="date">Empty</div>
              <h4>No dated events extracted</h4>
              <span className="source">Fill via documents or guided interview</span>
            </div>
          ) : (
            (caseGraph.chronology || []).map((e) => {
              const conf = e.provenance?.confidence;
              const band = confidenceBand(conf);
              return (
                <div key={e.event_id} className={`timeline-item heat-${band}`} data-testid="chrono-item">
                  <div className="date">{e.date || e.sort_key || "—"}</div>
                  <h4>{e.summary}</h4>
                  {e.date_expression && e.date_expression.status !== "resolved" && (
                    <div className="graph-ambiguity" data-testid="ambiguous-date">
                      {e.date_expression.status}: “{e.date_expression.raw}”
                      {e.date_expression.candidates.length > 0 &&
                        ` · candidates ${e.date_expression.candidates.join(" / ")}`}
                      {" · unresolved until evidence or counsel confirms"}
                    </div>
                  )}
                  <span className="source">
                    {formatSourceChip(e.provenance?.source as never)} · conf=
                    {conf != null ? conf.toFixed(2) : "missing"} · {bandLabel(band)}
                  </span>
                  <ProvenanceInspector record={graphProvenance(`chronology:${e.event_id}`, e.summary, e.provenance)} compact />
                </div>
              );
            })
          )}
        </div>
      </div>

      <div className="side-panel page-grid" style={{ gap: "1rem" }}>
        <div className="card" data-testid="graph-concept-coverage">
          <h4>Litigation concept coverage</h4>
          {nodes.length === 0 ? (
            <p className="muted">Legacy v1 graph. Re-run extraction to build normalized v2 nodes and links.</p>
          ) : (
            <div className="graph-concept-grid">
              {Object.entries(nodeCounts).map(([kind, count]) => (
                <span className="chip" key={kind}>{kind.replaceAll("_", " ")} · {count}</span>
              ))}
            </div>
          )}
          <p className="mono muted">INFERRED items are hypotheses, never extracted document facts.</p>
        </div>
        <div className="card">
          <h4>Parties</h4>
          {(caseGraph.parties || []).length === 0 ? (
            <p className="muted">None extracted.</p>
          ) : (
            <ul className="party-list">
              {(caseGraph.parties || []).map((p) => {
                const conf = p.provenance?.confidence;
                const band = confidenceBand(conf);
                return (
                  <li key={p.party_id} className={`party-row heat-${band}`}>
                    <div>
                      <strong>{p.display_name}</strong>
                      <div className="muted" style={{ fontSize: "0.78rem" }}>
                        {p.role} · {formatSourceChip(p.provenance?.source as never)}
                      </div>
                      <ProvenanceInspector record={graphProvenance(`party:${p.party_id}`, p.display_name, p.provenance)} compact />
                    </div>
                    <span className={`heat-dot heat-${band}`} title={bandLabel(band)} />
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        <div className="card">
          <h4>Statutes / instruments</h4>
          {(caseGraph.statutes || []).length === 0 ? (
            <span className="chip muted">Empty</span>
          ) : (
            <ul className="pin-list">
              {(caseGraph.statutes || []).map((s) => (
                <li key={s.statute_id} className={`heat-${confidenceBand(s.provenance?.confidence)}`}>
                  {s.label}
                  <div><ProvenanceInspector record={graphProvenance(`statute:${s.statute_id}`, s.label, s.provenance)} compact /></div>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="card">
          <h4>Evidence</h4>
          {(caseGraph.evidence || []).length === 0 ? (
            <span className="chip muted">Empty</span>
          ) : (
            <ul className="pin-list">
              {(caseGraph.evidence || []).map((ev) => (
                <li
                  key={ev.evidence_id}
                  className={`heat-${confidenceBand(ev.provenance?.confidence)}`}
                >
                  {ev.label}
                  <div><ProvenanceInspector record={graphProvenance(`evidence:${ev.evidence_id}`, ev.label, ev.provenance)} compact /></div>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="card">
          <h4>Prayer / relief</h4>
          {(caseGraph.prayer_relief || []).length === 0 ? (
            <span className="chip muted">Empty</span>
          ) : (
            <ul className="pin-list">
              {(caseGraph.prayer_relief || []).map((r) => (
                <li key={r.relief_id}>{r.text}<div><ProvenanceInspector record={graphProvenance(`relief:${r.relief_id}`, r.text, r.provenance)} compact /></div></li>
              ))}
            </ul>
          )}
        </div>

        <div className="card">
          <h4>Contradiction candidates</h4>
          {contradictions.length === 0 ? (
            <div className="contradiction-card">
              <div className="tag">None flagged</div>
              No contradiction seeds in graph. Detection only surfaces source-linked candidates —
              counsel verifies.
            </div>
          ) : (
            contradictions.map((c, i) => (
              <div key={c.contradiction_id || i} className="contradiction-card" data-testid="contradiction-candidate">
                <div className="tag">{c.status} · {c.basis} · conf {c.confidence.toFixed(2)}</div>
                {c.summary || "Contradiction candidate"}
                <div className="mono muted">
                  left {formatSourceChip(c.left_source as never)} · right {formatSourceChip(c.right_source as never)}
                </div>
              </div>
            ))
          )}
        </div>

        <div className="card">
          <h4>Open gaps</h4>
          {(caseGraph.gaps || []).filter((g) => g.status === "open").length === 0 ? (
            <p className="muted">No open gaps ({prep.open_gaps}).</p>
          ) : (
            (caseGraph.gaps || [])
              .filter((g) => g.status === "open")
              .map((g) => (
                <div key={g.gap_id} className="gap-row">
                  <span>{g.concept || g.topic} · {g.reason || "missing"}<br/><ProvenanceInspector compact record={{ stableId: `gap:${g.gap_id}`, propositionClass: "UNSUPPORTED_OR_GAP", origin: "UNSUPPORTED_GAP", supportState: "UNSUPPORTED", title: g.concept || g.topic, verification: g.reason || "Open preparation gap has no verified support.", version: caseGraph.schema_version }} /></span>
                  <span className="status">open</span>
                </div>
              ))
          )}
        </div>
      </div>
    </div>
  );
}

function bandLabel(b: ConfidenceBand): string {
  if (b === "ok") return "ok";
  if (b === "low") return "low conf";
  if (b === "force") return "force review";
  return "missing conf";
}

function collectLowConfidence(graph: CaseGraph | null): Array<{
  key: string;
  kind: string;
  label: string;
  conf: number | null;
  band: ConfidenceBand;
  source: string;
  provenance: ProvenanceRecord;
}> {
  if (!graph) return [];
  const out: Array<{
    key: string;
    kind: string;
    label: string;
    conf: number | null;
    band: ConfidenceBand;
    source: string;
    provenance: ProvenanceRecord;
  }> = [];
  const push = (
    kind: string,
    id: string,
    label: string,
    conf: number | null | undefined,
    source: unknown,
  ) => {
    const band = confidenceBand(conf ?? null);
    if (band === "ok") return;
    out.push({
      key: `${kind}:${id}`,
      kind,
      label,
      conf: conf ?? null,
      band,
      source: formatSourceChip(source as never),
      provenance: graphProvenance(`${kind}:${id}`, label, { source: source as never, confidence: conf ?? undefined }),
    });
  };
  for (const e of graph.chronology || []) {
    push("event", e.event_id, e.summary, e.provenance?.confidence, e.provenance?.source);
  }
  for (const p of graph.parties || []) {
    push("party", p.party_id, p.display_name, p.provenance?.confidence, p.provenance?.source);
  }
  for (const s of graph.statutes || []) {
    push("statute", s.statute_id, s.label, s.provenance?.confidence, s.provenance?.source);
  }
  for (const ev of graph.evidence || []) {
    push("evidence", ev.evidence_id, ev.label, ev.provenance?.confidence, ev.provenance?.source);
  }
  return out;
}
