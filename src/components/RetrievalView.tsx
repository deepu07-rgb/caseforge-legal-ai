/**
 * B-S1-13 Retrieval + citation GREEN/AMBER/RED UI.
 * Pipeline: DECOMPOSE → RETRIEVE → GENERATE → VERIFY → SCORE → RENDER
 */

import { CurrencyStamp } from "./CurrencyStamp";
import { useState } from "react";
import type { CitationRecord, RetrievalResult } from "../lib/ipc";
import { citationProvenance, retrievalHitProvenance } from "../lib/provenance";
import ProvenanceInspector from "./ProvenanceInspector";

const JURIS = ["", "India", "United States", "United Kingdom", "Other"] as const;
const AUTHORITY_TYPES = ["", "CONSTITUTION", "STATUTE", "REGULATION", "RULE", "JUDGMENT", "TREATY", "OFFICIAL_GUIDANCE", "SECONDARY", "CUSTOM", "UNKNOWN"] as const;
const TRUST_LEVELS = ["", "OFFICIAL_VERIFIED", "OFFICIAL_UNVERIFIED", "CUSTOM", "DEVELOPMENT_ONLY"] as const;
const TEMPORAL = ["", "APPLICABLE", "NOT_YET_EFFECTIVE", "EXPIRED", "UNKNOWN"] as const;
const SUPERSESSION = ["", "CURRENT", "SUPERSEDED", "PARTIALLY_SUPERSEDED", "UNKNOWN"] as const;

function statusClass(s: string): string {
  if (s === "GREEN") return "cit-green";
  if (s === "AMBER") return "cit-amber";
  return "cit-red";
}

export default function RetrievalView({
  busy,
  result,
  policyNote,
  activeCaseId,
  onRun,
}: {
  busy: boolean;
  result: RetrievalResult | null;
  policyNote: string;
  activeCaseId: string | null;
  onRun: (args: {
    query: string;
    topK: number;
    jurisdiction: string | null;
    authorityType: string | null;
    trustLevel: string | null;
    temporalStatus: string | null;
    supersededStatus: string | null;
    forceAuthorities: string[];
    caseId: string | null;
    userConfirmedRed: boolean;
  }) => Promise<void>;
}) {
  const [query, setQuery] = useState(
    "consideration under contract neutral citation INSC DEMO",
  );
  const [jurisdiction, setJurisdiction] = useState("India");
  const [topK, setTopK] = useState(8);
  const [authorityType, setAuthorityType] = useState("");
  const [trustLevel, setTrustLevel] = useState("");
  const [temporalStatus, setTemporalStatus] = useState("");
  const [supersededStatus, setSupersededStatus] = useState("");
  const [forceAuth, setForceAuth] = useState(
    "PLANTED_FAKE v. Placeholder, (2099) 1 SYNTH 1 (Neverland HC)",
  );
  const [useCaseGraph, setUseCaseGraph] = useState(false);
  const [confirmRed, setConfirmRed] = useState(false);

  const run = () => {
    const forced = forceAuth
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean);
    void onRun({
      query: query.trim(),
      topK,
      jurisdiction: jurisdiction || null,
      authorityType: authorityType || null,
      trustLevel: trustLevel || null,
      temporalStatus: temporalStatus || null,
      supersededStatus: supersededStatus || null,
      forceAuthorities: forced,
      caseId: useCaseGraph ? activeCaseId : null,
      userConfirmedRed: confirmRed,
    });
  };

  return (
    <div className="retrieval-layout" data-testid="retrieval-panel">
      <div className="row" style={{ justifyContent: "space-between", marginTop: 0 }}>
        <span className="eyebrow">library · retrieval</span>
        <span className="chip muted">AC-4.3 · AC-7.2</span>
      </div>
      <h3>Authority retrieval</h3>
      <p className="muted" style={{ fontSize: "0.85rem" }}>
        Retrieval-first pipeline labels every authority GREEN / AMBER / RED. Deterministic filters run
        before BM25 and offline legal-concept semantic retrieval, followed by explainable reranking.
        Planted fakes and citations outside the RETRIEVE set remain RED.
      </p>
      <p className="mono muted" style={{ fontSize: "0.72rem" }}>
        {policyNote}
      </p>

      <div className="retrieval-form">
        <label>
          Query
          <textarea
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            rows={3}
            data-testid="retrieval-query"
          />
        </label>
        <div className="row" style={{ gap: "0.75rem", flexWrap: "wrap" }}>
          <label style={{ flex: "1 1 10rem" }}>
            Jurisdiction filter
            <select
              value={jurisdiction}
              onChange={(e) => setJurisdiction(e.target.value)}
              data-testid="retrieval-jurisdiction"
            >
              {JURIS.map((j) => (
                <option key={j || "any"} value={j}>
                  {j || "Any"}
                </option>
              ))}
            </select>
          </label>
          <label style={{ width: "6rem" }}>
            Top-k
            <input
              type="number"
              min={1}
              max={32}
              value={topK}
              onChange={(e) => setTopK(Number(e.target.value) || 8)}
              data-testid="retrieval-topk"
            />
          </label>
        </div>
        <div className="row" style={{ gap: "0.75rem", flexWrap: "wrap" }}>
          {[
            ["Authority type", authorityType, setAuthorityType, AUTHORITY_TYPES],
            ["Trust", trustLevel, setTrustLevel, TRUST_LEVELS],
            ["Temporal", temporalStatus, setTemporalStatus, TEMPORAL],
            ["Supersession", supersededStatus, setSupersededStatus, SUPERSESSION],
          ].map(([label, value, setter, values]) => (
            <label key={String(label)} style={{ flex: "1 1 10rem" }}>
              {String(label)} filter
              <select value={String(value)} onChange={(e) => (setter as (v: string) => void)(e.target.value)}>
                {(values as readonly string[]).map((v) => <option key={v || "any"} value={v}>{v || "Any"}</option>)}
              </select>
            </label>
          ))}
        </div>
        <label>
          Force authorities into VERIFY (one per line; use PLANTED_FAKE for AC-4.3)
          <textarea
            value={forceAuth}
            onChange={(e) => setForceAuth(e.target.value)}
            rows={2}
            className="mono"
            data-testid="retrieval-force-auth"
          />
        </label>
        <div className="row" style={{ gap: "1rem", marginTop: "0.5rem", flexWrap: "wrap" }}>
          <label className="row" style={{ gap: "0.4rem", alignItems: "center" }}>
            <input
              type="checkbox"
              checked={useCaseGraph}
              onChange={(e) => setUseCaseGraph(e.target.checked)}
              disabled={!activeCaseId}
            />
            Include open case_graph statutes
          </label>
          <label className="row" style={{ gap: "0.4rem", alignItems: "center" }}>
            <input
              type="checkbox"
              checked={confirmRed}
              onChange={(e) => setConfirmRed(e.target.checked)}
              data-testid="retrieval-confirm-red"
            />
            User confirms RED into downstream
          </label>
        </div>
        <div className="row" style={{ marginTop: "0.75rem", gap: "0.4rem" }}>
          <button
            type="button"
            className="primary"
            disabled={busy || !query.trim()}
            onClick={run}
            data-testid="retrieval-run"
          >
            Run pipeline
          </button>
        </div>
      </div>

      {result && (
        <div className="retrieval-result" data-testid="retrieval-result">
          <div className="stat-label" style={{ marginTop: "1.15rem" }}>
            Retrieved (top {result.retrieved.length}) · {result.chunk_count} chunks ·{" "}
            {result.index_id}
          </div>
          <p className="mono muted" style={{ fontSize: "0.72rem" }}>
            stages: {result.stages_completed.join(" → ")} · query_id={result.query_id} · mode=
            {result.retrieval_mode} · index={result.index_state} · checksum={result.index_checksum.slice(0, 23)}…
          </p>
          {result.lexical_fallback_notice && (
            <div className="notice warn" data-testid="lexical-fallback" style={{ marginBottom: "0.8rem" }}>
              <strong>Lexical-only fallback</strong>
              <div>{result.lexical_fallback_notice}</div>
            </div>
          )}
          {result.repository_gap && (
            <div className="notice warn" data-testid="repository-gap" style={{ marginBottom: "0.8rem" }}>
              <strong>Repository gap — do not answer from model memory</strong>
              <div style={{ marginTop: "0.25rem" }}>{result.repository_gap_reason}</div>
              {result.repository_gap_required_sources.length > 0 && (
                <div className="muted" style={{ marginTop: "0.25rem" }}>
                  Required: {result.repository_gap_required_sources.join("; ")}
                </div>
              )}
            </div>
          )}
          {result.retrieved.length === 0 ? (
            <p className="muted">No hits — install a knowledge pack or point at pack content.</p>
          ) : (
            <ul className="retrieval-hits">
              {result.retrieved.map((h, i) => (
                <li key={h.chunk_id} data-testid="retrieval-hit">
                  <span className="chip muted">#{i + 1}</span>{" "}
                  <strong>{h.authority_hint || h.path}</strong>
                  <div className="mono muted" style={{ fontSize: "0.72rem" }}>
                    hybrid={h.score.toFixed(3)} · bm25={h.lexical_score.toFixed(3)} · semantic=
                    {h.semantic_score.toFixed(3)} · rerank={h.rerank_score.toFixed(3)} · {h.corpus_kind}
                  </div>
                  <div className="mono muted" style={{ fontSize: "0.7rem" }}>
                    why: {h.match_reasons.join("; ")} · {h.pack_id} · {h.path}
                  </div>
                  <div className="mono muted" style={{ fontSize: "0.7rem" }}>
                    {h.trust_level} · {h.authority_type} · temporal={h.temporal_status} · supersession=
                    {h.superseded_status} · source={h.source_id}
                  </div>
                  <div className="muted" style={{ fontSize: "0.82rem" }}>
                    {h.snippet}
                  </div>
                  <ProvenanceInspector record={retrievalHitProvenance(h)} compact />
                  {h.temporal_warning && <div className="notice warn" style={{ marginTop: "0.4rem" }}>{h.temporal_warning}</div>}
                </li>
              ))}
            </ul>
          )}

          <div className="stat-label" style={{ marginTop: "1.15rem" }}>
            Citations (labeled)
          </div>
          <ul className="citation-list">
            {result.citations.map((c) => (
              <CitationRow key={c.citation_id} c={c} />
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

function CitationRow({ c }: { c: CitationRecord }) {
  return (
    <li className={`citation-row ${statusClass(c.verification_status)}`} data-testid="citation-row">
      <div className="row" style={{ justifyContent: "space-between", gap: "0.5rem" }}>
        <strong>{c.authority_string}</strong>
        <span className={`cit-badge ${statusClass(c.verification_status)}`} data-testid="citation-status">
          {c.verification_status}
        </span>
      </div>
      <div className="mono muted" style={{ fontSize: "0.72rem" }}>
        score={c.match_score.toFixed(3)} · retrieved={String(c.retrieved)} · planted=
        {String(c.planted_fake)} · exclude={String(c.exclude_from_downstream)}
        {c.pack_id ? ` · pack=${c.pack_id}` : ""}
      </div>
      <div className="mono muted" style={{ fontSize: "0.7rem" }}>
        {c.trust_level} · {c.authority_type} · temporal={c.temporal_status} · supersession=
        {c.superseded_status} · kind={c.content_kind}
      </div>
      {c.notes && (
        <div className="muted" style={{ fontSize: "0.8rem" }}>
          {c.notes}
        </div>
      )}
      <CurrencyStamp packId={c.pack_id} />
      <ProvenanceInspector record={citationProvenance(c)} compact />
    </li>
  );
}
