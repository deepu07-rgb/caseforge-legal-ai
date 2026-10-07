/**
 * B-S1-15 DRAFTING surface — templates + DOCX/PDF export with G4 footer and G5 provenance.
 */

import { useMemo, useState } from "react";
import {
  buildDraftDocument,
  downloadBytes,
  EXPORT_FOOTER_ID,
  EXPORT_FOOTER_LINES,
  EXPORT_PIPELINE_ID,
  renderDocx,
  renderPdf,
  renderPlainText,
  safeFilename,
  TEMPLATES,
  type TemplateId,
} from "../lib/exportDraft";
import type { CaseGraph, CaseSummary } from "../lib/ipc";
import ProvenanceInspector from "./ProvenanceInspector";
import { AuditPanel } from "./AuditPanel";

export default function DraftingView({
  activeCase,
  caseGraph,
  onRunSkeleton,
  busy,
}: {
  activeCase: CaseSummary | null;
  caseGraph: CaseGraph | null;
  onRunSkeleton: () => Promise<void>;
  busy: boolean;
}) {
  const [templateId, setTemplateId] = useState<TemplateId>("court_formatted_digest");
  const [editedBody, setEditedBody] = useState<string | null>(null);
  const [lastExport, setLastExport] = useState<string | null>(null);

  const draft = useMemo(
    () =>
      buildDraftDocument({
        templateId,
        activeCase,
        caseGraph,
        bodyOverride: editedBody,
      }),
    [templateId, activeCase, caseGraph, editedBody],
  );

  const preview = useMemo(() => renderPlainText(draft), [draft]);
  const hasMatter = Boolean(activeCase);

  const onTemplateChange = (id: TemplateId) => {
    setTemplateId(id);
    setEditedBody(null);
    setLastExport(null);
  };

  const doExport = (fmt: "docx" | "pdf") => {
    const live = buildDraftDocument({
      templateId,
      activeCase,
      caseGraph,
      bodyOverride: editedBody,
    });
    const name = safeFilename(live.title, live.template_id, fmt);
    if (fmt === "docx") {
      downloadBytes(
        name,
        renderDocx(live),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
      );
    } else {
      downloadBytes(name, renderPdf(live), "application/pdf");
    }
    setLastExport(`${name} · G4 ${EXPORT_FOOTER_ID} · spans ${live.provenance.ai_spans.length}`);
  };

  return (
    <div className="digest-layout drafting-root" data-testid="panel-drafting">
      <div className="card">
        <span className="eyebrow">DRAFTING · export</span>
        <h3 className="digest-section-title">Templates & court-format export</h3>
        <p className="muted" style={{ fontSize: "0.9rem" }}>
          Build preparation drafts from the case graph only. Every export carries the G4 footer and a
          G5 provenance log (model_hash, prompt_hash, retrieval_ids). Not a court filing portal.
        </p>

        {!hasMatter && (
          <p className="muted">Open a matter to draft and export.</p>
        )}

        <label className="field-label" htmlFor="draft-template">
          Template
        </label>
        <select
          id="draft-template"
          data-testid="draft-template"
          value={templateId}
          disabled={!hasMatter}
          onChange={(e) => onTemplateChange(e.target.value as TemplateId)}
        >
          {TEMPLATES.map((t) => (
            <option key={t.id} value={t.id}>
              {t.label}
            </option>
          ))}
        </select>
        <p className="muted" style={{ fontSize: "0.82rem", marginTop: "0.35rem" }}>
          {TEMPLATES.find((t) => t.id === templateId)?.description}
        </p>

        <div className="row" style={{ marginTop: "0.85rem", gap: "0.4rem", flexWrap: "wrap" }}>
          <button
            type="button"
            className="primary"
            data-testid="export-docx"
            disabled={!hasMatter}
            onClick={() => doExport("docx")}
          >
            Export DOCX
          </button>
          <button
            type="button"
            className="primary"
            data-testid="export-pdf"
            disabled={!hasMatter}
            onClick={() => doExport("pdf")}
          >
            Export PDF
          </button>
          <button
            type="button"
            disabled={!hasMatter || busy}
            onClick={() => void onRunSkeleton()}
          >
            {caseGraph ? "Re-run skeleton" : "Build case graph"}
          </button>
          <button
            type="button"
            disabled={!hasMatter}
            onClick={() => {
              setEditedBody(null);
              setLastExport(null);
            }}
          >
            Reset body
          </button>
        </div>

        {lastExport && (
          <p className="mono muted" style={{ fontSize: "0.75rem", marginTop: "0.65rem" }} data-testid="export-notice">
            Exported: {lastExport}
          </p>
        )}
      </div>

      <div className="card">
        <h4>G4 footer (required on every export)</h4>
        <ul className="footer-lines" data-testid="g4-footer-preview">
          {EXPORT_FOOTER_LINES.map((ln) => (
            <li key={ln}>{ln}</li>
          ))}
        </ul>
        <p className="mono muted" style={{ fontSize: "0.7rem" }}>
          {EXPORT_FOOTER_ID} · {EXPORT_PIPELINE_ID}
        </p>

        <h4 style={{ marginTop: "1rem" }}>G5 provenance</h4>
        <p className="mono muted" style={{ fontSize: "0.75rem" }} data-testid="g5-summary">
          export_id={draft.provenance.export_id} · llm_used={String(draft.provenance.llm_used)} ·
          ai_spans={draft.provenance.ai_spans.length} · fields=
          {draft.provenance.g5_fields.join(",")}
        </p>
        {draft.provenance.ai_spans.length > 0 ? (
          <ul className="gap-list provenance-list" data-testid="g5-spans">
            {draft.provenance.ai_spans.slice(0, 12).map((s) => (
              <li key={s.span_id}>
                <span className="chip muted">{s.origin}</span>{" "}
                <span className="mono" style={{ fontSize: "0.72rem" }}>
                  model_hash={s.model_hash} · prompt_hash={s.prompt_hash} · retrieval_ids=
                  {JSON.stringify(s.retrieval_ids)}
                </span>
                <div className="muted" style={{ fontSize: "0.82rem" }}>
                  {s.text}
                </div>
                <ProvenanceInspector record={s.provenance_record} compact />
              </li>
            ))}
          </ul>
        ) : (
          <p className="muted">No graph spans yet — build skeleton or pick a template with content.</p>
        )}
      </div>

      <div className="card drafting-preview-card" style={{ gridColumn: "1 / -1" }}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline" }}>
          <h4>Editable draft body</h4>
          <span className="mono muted" style={{ fontSize: "0.7rem" }}>
            Edits apply to export · source of truth remains case_graph until you re-generate
          </span>
        </div>
        <textarea
          className="draft-editor"
          data-testid="draft-body"
          value={editedBody ?? draft.body_lines.join("\n")}
          disabled={!hasMatter}
          rows={22}
          onChange={(e) => setEditedBody(e.target.value)}
          spellCheck={false}
        />
        <details style={{ marginTop: "0.75rem" }}>
          <summary className="muted" style={{ cursor: "pointer", fontSize: "0.85rem" }}>
            Full plain-text preview (includes G4 + G5 appendix)
          </summary>
          <pre className="draft-preview" data-testid="draft-preview">
            {preview}
          </pre>
        </details>
      </div>

      {hasMatter && (
        <AuditPanel
          text={editedBody ?? draft.body_lines.join("\n")}
          caseId={activeCase?.case_id ?? null}
          filename={`${templateId}.txt`}
        />
      )}
    </div>
  );
}
