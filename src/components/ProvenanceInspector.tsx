import { useEffect, useRef, useState } from "react";
import { provenanceExportLine, provenanceLocator, type ProvenanceRecord } from "../lib/provenance";

const ICON: Record<ProvenanceRecord["origin"], string> = {
  MATTER_EVIDENCE: "▣",
  LEGAL_AUTHORITY: "§",
  COUNSEL_INPUT: "✎",
  MODEL_INFERENCE: "◇",
  UNSUPPORTED_GAP: "!",
};

export default function ProvenanceInspector({ record, compact = false }: { record: ProvenanceRecord; compact?: boolean }) {
  const [open, setOpen] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const drawer = useRef<HTMLElement>(null);
  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") { setOpen(false); trigger.current?.focus(); }
      if (event.key === "Tab" && drawer.current) {
        const focusable = [...drawer.current.querySelectorAll<HTMLElement>('button,[href],input,select,textarea,[tabindex]:not([tabindex="-1"])')].filter((el) => !el.hasAttribute("disabled"));
        if (!focusable.length) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open]);
  const label = `${record.supportState}: ${record.origin.replaceAll("_", " ")}. Inspect provenance for ${record.title}`;
  return (
    <span className="provenance-control" data-testid="provenance-control">
      <button
        ref={trigger}
        type="button"
        className={`provenance-badge provenance-${record.supportState.toLowerCase()} ${compact ? "compact" : ""}`}
        aria-label={label}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen(true)}
        data-provenance-class={record.propositionClass}
        data-provenance-state={record.supportState}
      >
        <span aria-hidden="true">{ICON[record.origin]}</span> {record.supportState} · {record.origin.replaceAll("_", " ")}
      </button>
      {open && (
        <div className="provenance-backdrop" onMouseDown={(e) => { if (e.target === e.currentTarget) setOpen(false); }}>
          <aside ref={drawer} className="provenance-drawer" role="dialog" aria-modal="true" aria-labelledby={`prov-title-${record.stableId}`} data-testid="provenance-drawer">
            <div className="provenance-drawer-head">
              <div>
                <div className="eyebrow">Where did CaseForge get that?</div>
                <h3 id={`prov-title-${record.stableId}`}>{record.title}</h3>
              </div>
              <button type="button" autoFocus aria-label="Close provenance inspector" onClick={() => { setOpen(false); trigger.current?.focus(); }}>Close</button>
            </div>
            <div className={`provenance-state provenance-${record.supportState.toLowerCase()}`}>
              <strong>{record.supportState}</strong> — {record.verification}
            </div>
            <dl className="provenance-details">
              <dt>Proposition class</dt><dd>{record.propositionClass.replaceAll("_", " ")}</dd>
              <dt>Origin</dt><dd>{record.origin.replaceAll("_", " ")}</dd>
              <dt>Stable ID</dt><dd className="mono">{record.stableId}</dd>
              <dt>Source locator</dt><dd className="mono" data-testid="provenance-locator">{provenanceLocator(record)}</dd>
              <dt>Version/checksum</dt><dd className="mono">{record.version || "not recorded"}</dd>
              {record.docId && <><dt>Matter source</dt><dd className="mono">document {record.docId} · page {(record.pageIndex ?? 0) + 1}{record.charStart != null ? ` · characters ${record.charStart}–${record.charEnd}` : ""}</dd></>}
              {record.bbox?.length ? <><dt>Page region</dt><dd className="mono">{record.bbox.join(", ")}</dd></> : null}
              {(record.packId || record.sourceId || record.chunkId) && <><dt>Legal source</dt><dd className="mono">pack {record.packId || "—"} · source {record.sourceId || "—"} · passage {record.chunkId || "—"}{record.section ? ` · ${record.section}` : ""}</dd></>}
              <dt>Verified support</dt><dd className="mono">{record.supportIds?.join(" · ") || "none"}</dd>
              {record.basisIds?.length ? <><dt>Inference bases</dt><dd className="mono">{record.basisIds.join(" · ")}</dd></> : null}
            </dl>
            {record.excerpt && <blockquote className="provenance-excerpt">{record.excerpt}</blockquote>}
            <p className="notice warn">Preparation aid only. Verify the cited source before filing or relying on this proposition.</p>
            <button type="button" onClick={() => void navigator.clipboard?.writeText(`${record.title}\n${provenanceExportLine(record)}\nPreparation aid only — verify before filing.`)}>
              Copy with provenance
            </button>
          </aside>
        </div>
      )}
    </span>
  );
}
