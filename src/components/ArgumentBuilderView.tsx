import { useMemo, useState } from "react";
import {
  caseLlmContext,
  caseLlmSupportSources,
  llmChat,
  workingSetGet,
  workingSetSelectForOutput,
  type CaseSummary,
  type LlmStatus,
} from "../lib/ipc";

type Props = {
  activeCase: CaseSummary | null;
  llm: LlmStatus | null;
  onOpenCases: () => void;
  onOpenCourt: () => void;
  onRefreshLlm: () => void;
};

const LIMBS = [
  { id: 1, label: "Maintainability / jurisdiction limb" },
  { id: 2, label: "Subsisting patent right (pleaded)" },
  { id: 3, label: "Alleged infringement theory" },
  { id: 4, label: "Interim relief factors" },
  { id: 5, label: "Evidentiary gaps (counsel to close)" },
];

export default function ArgumentBuilderView({
  activeCase,
  llm,
  onOpenCases,
  onOpenCourt,
  onRefreshLlm,
}: Props) {
  const [busy, setBusy] = useState(false);
  const [output, setOutput] = useState<string | null>(null);
  const [meta, setMeta] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [contextPreview, setContextPreview] = useState<string | null>(null);
  const [includeContext, setIncludeContext] = useState(true);
  const [activeLimb, setActiveLimb] = useState(3);

  const ready = Boolean(llm?.controlled_beta_ready);

  const loadContext = async () => {
    if (!activeCase) return;
    setError(null);
    try {
      const ctx = await caseLlmContext(activeCase.case_id, 20000);
      setContextPreview(ctx);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const build = async () => {
    if (!activeCase) {
      setError("Open a case first (Cases → Unlock & open).");
      return;
    }
    if (!ready) {
      setError("Local LLM is not controlled-beta ready. Open System to inspect the exact model, digest, runtime, and prompt gate.");
      return;
    }
    setBusy(true);
    setError(null);
    setOutput(null);
    try {
      let case_context: string | null = null;
      let authority_context: string | null = null;
      let authority_ids: string[] = [];
      let working_set_version: string | null = null;
      let support_sources = [] as Awaited<ReturnType<typeof caseLlmSupportSources>>;
      if (includeContext) {
        case_context = await caseLlmContext(activeCase.case_id, 24000);
        setContextPreview(case_context);
        support_sources = await caseLlmSupportSources(activeCase.case_id, 24000);
        const workingSet = await workingSetGet(activeCase.case_id);
        const selection = await workingSetSelectForOutput(
          activeCase.case_id,
          "ARGUMENT_INTEGRITY",
          `argument-map-${Date.now()}`,
        );
        authority_ids = selection.authority_ids;
        working_set_version = String(selection.working_set_version);
        const selected = workingSet.items.filter((item) => authority_ids.includes(item.authority_id));
        authority_context = selected
          .map((item) => `[${item.authority_id}] ${item.title} · ${item.citation} · ${item.section_label}\n${item.exact_text}`)
          .join("\n\n");
        if (authority_context.length > 16000) {
          throw new Error(
            `The selected authority text is ${authority_context.length} characters. Narrow the matter working set below 16,000 characters; no source was silently removed.`,
          );
        }
        support_sources = support_sources.concat(selected.map((item) => ({
          support_id: item.authority_id,
          kind: "LEGAL_AUTHORITY" as const,
          exact_text: item.exact_text,
          source_id: item.source_id,
          chunk_id: item.chunk_id,
          authority_id: item.authority_id,
          doc_id: null,
          page: null,
          paragraph: item.section_label,
          trust_level: item.trust_level,
          temporal_status: item.temporal_status,
          superseded_status: item.superseded_status,
          permitted: true,
        })));
      }
      const result = await llmChat({
        user_text:
          "Build a complete argument map for this matter. " +
          "Include: (1) relief root in one sentence; (2) 1–3 argument limbs with theory; " +
          "(3) Fact/Evidence/Law/Exposure scores 0–100 for each limb; " +
          "(4) VI = 100 − min(links) per limb; (5) Missing Links under 60; " +
          "(6) ranked 'where to strengthen' actions. " +
          "Use only the CASE FILE EXCERPT. Mark unknown facts as gaps. Do not invent authorities. " +
          "Use hedges such as 'the plaint alleges' and 'counsel may develop'.",
        role: "argument_builder",
        format: "argument_tree",
        case_title: activeCase.cause_title || activeCase.label,
        jurisdiction: activeCase.jurisdiction,
        user_side: activeCase.user_side,
        case_context: null,
        authority_context: null,
        authority_ids,
        working_set_version,
        structured_propositions: true,
        output_kind: "ARGUMENT_INTEGRITY",
        support_sources,
        permitted_authority_ids: authority_ids,
        max_tokens: 900,
      });
      setOutput(result.content);
      setMeta(
        `${result.backend} · ${result.model} · ${result.tokens_per_second ?? "—"} tok/s · ledger ${result.proposition_ledger?.trusted_count ?? 0} verified / ${result.proposition_ledger?.gap_count ?? 0} gaps / ${result.proposition_ledger?.blocked_count ?? 0} blocked`,
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const placeholderDraft = useMemo(
    () =>
      activeCase
        ? `The plaint alleges theories that counsel may develop from the sealed case packet for “${activeCase.cause_title || activeCase.label}”.\n\nGenerate an argument map with the local model to replace this limb draft. Outputs are drafts for counsel — no automatic filing, no outcome prediction.`
        : "Open a case to bind the argument map to a vault matter.",
    [activeCase],
  );

  return (
    <div className="page-grid">
      {activeCase && (
        <div className="matter-chips">
          <span className="chip matter">{activeCase.cause_title || activeCase.label}</span>
          <span className="chip matter">{activeCase.jurisdiction}</span>
          <span className="chip matter">{activeCase.court || "Court"}</span>
          <span className="chip matter">{activeCase.user_side || "Side"}</span>
          <span className="chip ok">Encrypted</span>
        </div>
      )}

      <div className="context-strip">
        <span className="chip matter">
          {activeCase ? "Vault-sealed context" : "No case open"}
        </span>
        <span className={`chip ${ready ? "ok" : "warn"}`}>
          <span className={`status-dot ${ready ? "on" : "off"}`} />
          {ready ? `${llm?.model || "local"} · ready` : "LLM offline"}
        </span>
        <label
          style={{
            flexDirection: "row",
            alignItems: "center",
            gap: "0.4rem",
            textTransform: "none",
            letterSpacing: 0,
            fontSize: "0.82rem",
            fontWeight: 500,
          }}
        >
          <input
            type="checkbox"
            checked={includeContext}
            onChange={(e) => setIncludeContext(e.target.checked)}
            style={{ width: "auto" }}
          />
          Include vault document text
        </label>
        <div className="grow" />
        <button type="button" disabled={!activeCase || busy} onClick={() => void loadContext()}>
          Refresh from case file
        </button>
        <button
          type="button"
          className="primary"
          disabled={busy || !activeCase}
          onClick={() => void build()}
        >
          {busy ? "Generating…" : "Generate argument map"}
        </button>
      </div>

      <p className="muted" style={{ margin: "-0.35rem 0 0", fontSize: "0.82rem" }}>
        Outputs are drafts for counsel. No automatic filing. No outcome prediction.
      </p>

      {error && (
        <div className="notice error" role="alert">
          {error}
        </div>
      )}

      {!activeCase && (
        <div className="card empty">
          <h3>No open case</h3>
          <p>Unlock a matter so the local LLM can read the sealed packet.</p>
          <button type="button" className="primary" style={{ marginTop: "0.75rem" }} onClick={onOpenCases}>
            Go to Cases
          </button>
        </div>
      )}

      <div className="builder-layout">
        <div className="builder-col">
          <h3>Argument map</h3>
          {LIMBS.map((limb) => (
            <button
              key={limb.id}
              type="button"
              className={`limb-node ${activeLimb === limb.id ? "active" : ""}`}
              onClick={() => setActiveLimb(limb.id)}
            >
              {limb.id}. {activeLimb === limb.id ? "Active · " : ""}
              {limb.label}
            </button>
          ))}
          <p className="muted" style={{ marginTop: "0.85rem", fontSize: "0.78rem" }}>
            Tree is editable · counsel owns structure
          </p>
        </div>

        <div className="builder-col">
          <div className="row" style={{ justifyContent: "space-between", marginTop: 0 }}>
            <h3 style={{ margin: 0 }}>Limb draft</h3>
            {meta && <span className="chip muted mono">{meta}</span>}
          </div>
          <div className="limb-draft">
            <h4>{LIMBS.find((l) => l.id === activeLimb)?.label}</h4>
            {output ? output : placeholderDraft}
          </div>
          <div className="row">
            <button type="button" disabled={busy || !activeCase || !ready} onClick={() => void build()}>
              Regenerate limb
            </button>
            <button type="button" onClick={onOpenCourt}>
              Open in Mock Court
            </button>
            <button type="button" onClick={onRefreshLlm}>
              Probe LLM
            </button>
          </div>
        </div>

        <div className="builder-col">
          <h3>Support & integrity</h3>
          <div className="stat-label" style={{ marginBottom: "0.55rem" }}>
            From case file
          </div>
          {contextPreview ? (
            <pre
              className="mono"
              style={{
                whiteSpace: "pre-wrap",
                maxHeight: 140,
                overflow: "auto",
                margin: "0 0 1rem",
                fontSize: "0.75rem",
                color: "var(--ink-muted)",
                border: "1px solid var(--line)",
                padding: "0.65rem",
              }}
            >
              {contextPreview.slice(0, 900)}
              {contextPreview.length > 900 ? "…" : ""}
            </pre>
          ) : (
            <p className="muted" style={{ fontSize: "0.85rem", marginBottom: "1rem" }}>
              Preview case context to pin source excerpts to this limb.
            </p>
          )}

          <div className="stat-label" style={{ marginBottom: "0.55rem" }}>
            Authority slots
          </div>
          <div className="chip-row" style={{ marginBottom: "1rem" }}>
            <span className="chip ok">GREEN · confirmed</span>
            <span className="chip warn">AMBER · verify</span>
            <span className="chip">RED · contradiction</span>
          </div>
          <div className="notice warn" style={{ fontSize: "0.82rem", marginBottom: "1rem" }}>
            No pack verification claimed in this mock. Citations stay AMBER until knowledge packs
            ship.
          </div>

          <div className="stat-label" style={{ marginBottom: "0.55rem" }}>
            Integrity nudges
          </div>
          <ul style={{ margin: 0, paddingLeft: "1.1rem", color: "var(--ink-muted)", fontFamily: "var(--font-reading)", fontSize: "0.9rem" }}>
            <li>Missing counter-argument frame</li>
            <li>Missing prayer link on active limb</li>
            <li>Weak source on date conflict</li>
          </ul>

          <button type="button" style={{ marginTop: "1rem", width: "100%" }} onClick={onOpenCourt}>
            Open in Mock Court
          </button>
        </div>
      </div>

      <div className="system-pulse-bar">
        <span>Heavy generation runs in sidecar · UI stays responsive · nothing leaves this machine</span>
        <button type="button" className="linkish" onClick={onRefreshLlm}>
          Probe LLM
        </button>
      </div>
    </div>
  );
}
