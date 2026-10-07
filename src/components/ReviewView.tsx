/**
 * B-S1-08 side-by-side verification review.
 * AC-6.3: click fact → source page + span highlighted.
 * AC-6.4: ≤ 2 interactions to correct (select + save).
 */

import { useEffect, useMemo, useState } from "react";
import type { AuthorityConflict, CaseGraph, CorrectionEvent, PageTextLayerRow } from "../lib/ipc";
import { authorityConflictResolve, authorityConflictsList, correctionEventsList, correctionRevert, REVIEW_MAX_INTERACTIONS } from "../lib/ipc";
import {
  collectGraphReviewItems,
  collectWordReviewItems,
  highlightSpan,
  layerForSource,
  type ReviewItem,
} from "../lib/review";
import { graphProvenance } from "../lib/provenance";
import ProvenanceInspector from "./ProvenanceInspector";

export default function ReviewView({
  caseId,
  caseGraph,
  textLayers,
  busy,
  onCorrectGraph,
  onCorrectWord,
}: {
  caseId: string | null;
  caseGraph: CaseGraph | null;
  textLayers: PageTextLayerRow[];
  busy: boolean;
  onCorrectGraph: (
    collection: string,
    itemId: string,
    fieldName: string,
    newValue: string,
  ) => Promise<void>;
  onCorrectWord: (
    docId: string,
    pageIndex: number,
    wordIndex: number,
    newText: string,
  ) => Promise<void>;
}) {
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [onlyLow, setOnlyLow] = useState(true);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** Interaction counter for AC-6.4: select=1, save=2 */
  const [interactions, setInteractions] = useState(0);
  const [conflicts, setConflicts] = useState<AuthorityConflict[]>([]);
  const [history, setHistory] = useState<CorrectionEvent[]>([]);
  const [authorityBusy, setAuthorityBusy] = useState(false);

  const refreshAuthority = async () => {
    if (!caseId) { setConflicts([]); setHistory([]); return; }
    const [nextConflicts, nextHistory] = await Promise.all([
      authorityConflictsList(caseId, true), correctionEventsList(caseId, 100),
    ]);
    setConflicts(nextConflicts); setHistory(nextHistory);
  };

  useEffect(() => { void refreshAuthority().catch((e) => setError(String(e))); }, [caseId, caseGraph, textLayers]);

  const resolveConflict = async (conflictId: string, resolution: "KEEP_COUNSEL" | "ACCEPT_GENERATED") => {
    if (!caseId) return;
    setAuthorityBusy(true); setError(null);
    try { await authorityConflictResolve(caseId, conflictId, resolution, `Counsel selected ${resolution.toLowerCase().replaceAll("_", " ")} in Review`); await refreshAuthority(); setStatus(`Conflict resolved: ${resolution}.`); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setAuthorityBusy(false); }
  };

  const revertEvent = async (eventId: string) => {
    if (!caseId) return;
    setAuthorityBusy(true); setError(null);
    try { await correctionRevert(caseId, eventId, "Counsel reverted correction from immutable history"); await refreshAuthority(); setStatus("Correction reverted; history retained."); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setAuthorityBusy(false); }
  };

  const items = useMemo(() => {
    const graphItems = collectGraphReviewItems(caseGraph);
    const wordItems = collectWordReviewItems(textLayers);
    const all = [...graphItems, ...wordItems];
    return onlyLow ? all.filter((i) => i.lowConfidence) : all;
  }, [caseGraph, textLayers, onlyLow]);

  const selected: ReviewItem | null =
    items.find((i) => i.key === selectedKey) ?? null;

  const sourceLayer = layerForSource(textLayers, selected?.source ?? null);
  const span = highlightSpan(
    sourceLayer?.full_text ?? selected?.source?.quote ?? "",
    selected?.source?.char_start,
    selected?.source?.char_end,
  );

  const selectItem = (item: ReviewItem) => {
    setSelectedKey(item.key);
    setDraft(item.value);
    setInteractions(1); // interaction 1: select
    setStatus(null);
    setError(null);
  };

  const saveCorrection = async () => {
    if (!selected) return;
    if (!draft.trim()) {
      setError("Replacement text required.");
      return;
    }
    const nextInteractions = Math.min(interactions + 1, REVIEW_MAX_INTERACTIONS);
    if (nextInteractions > REVIEW_MAX_INTERACTIONS) {
      setError(`AC-6.4: max ${REVIEW_MAX_INTERACTIONS} interactions`);
      return;
    }
    setError(null);
    try {
      if (selected.kind === "graph" && selected.collection && selected.itemId && selected.fieldName) {
        await onCorrectGraph(
          selected.collection,
          selected.itemId,
          selected.fieldName,
          draft.trim(),
        );
      } else if (
        selected.kind === "word" &&
        selected.docId != null &&
        selected.pageIndex != null &&
        selected.wordIndex != null
      ) {
        await onCorrectWord(
          selected.docId,
          selected.pageIndex,
          selected.wordIndex,
          draft.trim(),
        );
      } else {
        setError("Missing target for correction.");
        return;
      }
      setInteractions(nextInteractions);
      setStatus(
        `Saved in ${nextInteractions} interaction(s) (AC-6.4 ≤ ${REVIEW_MAX_INTERACTIONS}).`,
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  if (!caseGraph && textLayers.length === 0) {
    return (
      <div className="card empty">
        <h3>No extractions to review</h3>
        <p>
          Run skeleton extraction (and optionally recognition) first, then verify low-confidence
          items against the source page.
        </p>
      </div>
    );
  }

  return (
    <div className="review-split">
      <section className="review-list">
        <div className="authority-summary" data-testid="authority-summary">
          <strong>Counsel authority</strong> · {history.filter((e) => e.active_overlay).length} active overlays · {conflicts.filter((c) => c.status === "OPEN").length} open rerun conflicts
          <div className="mono muted">precedence: counsel overlay → generated extraction · history stored in encrypted vault</div>
        </div>
        {conflicts.filter((c) => c.status === "OPEN").map((conflict) => (
          <div className="authority-conflict" data-testid="authority-conflict" key={conflict.conflict_id}>
            <strong>{conflict.conflict_kind.replaceAll("_", " ")}</strong>
            <div>Generated: {conflict.generated_value ?? "target missing"}</div>
            <div>Counsel: {conflict.counsel_value ?? "explicit deletion"}</div>
            <div className="row">
              <button disabled={authorityBusy} onClick={() => void resolveConflict(conflict.conflict_id, "KEEP_COUNSEL")}>Keep counsel</button>
              <button disabled={authorityBusy} onClick={() => void resolveConflict(conflict.conflict_id, "ACCEPT_GENERATED")}>Accept generated</button>
            </div>
          </div>
        ))}
        <div className="row" style={{ justifyContent: "space-between", marginTop: 0 }}>
          <span className="eyebrow">CASEBOOK · review</span>
          <label className="row" style={{ gap: "0.35rem", fontSize: "0.8rem" }}>
            <input
              type="checkbox"
              checked={onlyLow}
              onChange={(e) => setOnlyLow(e.target.checked)}
            />
            Low confidence only
          </label>
        </div>
        <h3>Verify extractions</h3>
        <p className="muted" style={{ fontSize: "0.85rem" }}>
          Click a fact to open its source span (AC-6.3). Correct with select + save (≤ 2 steps,
          AC-6.4).
        </p>
        <div className="notice warn" style={{ fontSize: "0.82rem" }}>
          Low model confidence — verify against source page. Not a finding of fact.
        </div>
        {items.length === 0 ? (
          <p className="muted">No items match the current filter.</p>
        ) : (
          <ul className="review-items">
            {items.map((item) => (
              <li key={item.key}>
                <button
                  type="button"
                  className={`review-item ${selectedKey === item.key ? "active" : ""} ${
                    item.lowConfidence ? "low" : ""
                  }`}
                  onClick={() => selectItem(item)}
                  data-testid="review-fact"
                  data-review-key={item.key}
                >
                  <span className="review-item-label">{item.label}</span>
                  <span className="review-item-value">{item.value}</span>
                  <span className="mono review-item-conf">
                    {item.confidenceState === "missing" ? "confidence missing" : `cal ${item.confidence.toFixed(2)}`}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="review-source">
        <details className="authority-history" data-testid="authority-history">
          <summary>Immutable correction history ({history.length})</summary>
          {history.length === 0 ? <p className="muted">No counsel correction events yet.</p> : (
            <ul>
              {history.map((event) => (
                <li key={event.event_id}>
                  <strong>{event.event_type}</strong> · {event.target_kind} · {new Date(event.created_at_unix_ms).toLocaleString()}
                  <div>{event.base_value ?? "∅"} → {event.counsel_value ?? "∅"}</div>
                  <div className="mono muted">{event.reason} · {event.immutable_hash.slice(0, 24)}…</div>
                  {event.event_type === "CORRECT" && event.active_overlay && <button disabled={authorityBusy} onClick={() => void revertEvent(event.event_id)}>Revert</button>}
                </li>
              ))}
            </ul>
          )}
        </details>
        <div className="row" style={{ justifyContent: "space-between", marginTop: 0 }}>
          <span className="eyebrow">Source page</span>
          {selected?.source && (
            <span className="chip muted mono">
              {selected.source.doc_id?.slice(0, 8)}… · p.
              {selected.source.page_index}
            </span>
          )}
        </div>
        <h3>{selected ? "Linked source" : "Select a fact"}</h3>
        {!selected ? (
          <p className="muted">Click any fact on the left to highlight its source span.</p>
        ) : (
          <>
            <div
              className="source-pane"
              data-testid="review-source-pane"
              data-doc-id={selected.source?.doc_id ?? ""}
              data-page-index={selected.source?.page_index ?? ""}
              data-char-start={selected.source?.char_start ?? ""}
              data-char-end={selected.source?.char_end ?? ""}
            >
              <div className="confidence-audit" data-testid="confidence-audit">
                <span className="chip muted">
                  raw OCR: {selected.rawConfidence == null ? "missing" : selected.rawConfidence.toFixed(3)}
                </span>
                <span className="chip muted">
                  calibrated: {selected.calibratedConfidence == null ? "missing" : selected.calibratedConfidence.toFixed(3)}
                </span>
                <span className={`chip ${selected.validationReasons?.length ? "warn" : "muted"}`}>
                  validation: {selected.validationReasons?.join("; ") || "not applicable"}
                </span>
                <span className={`chip ${selected.counselConfirmed ? "ok" : "muted"}`}>
                  counsel: {selected.counselConfirmed ? "confirmed" : "not confirmed"}
                </span>
              </div>
              <ProvenanceInspector
                record={graphProvenance(selected.key, selected.label, {
                  source: selected.source ?? undefined,
                  confidence: selected.confidence,
                  counsel_confirmed: selected.counselConfirmed,
                  origin: selected.counselConfirmed ? "USER_EDIT" : "EXTRACTED",
                })}
              />
              {selected.bbox ? (
                <p className="mono muted" style={{ fontSize: "0.72rem" }}>
                  source region x={selected.bbox.x.toFixed(4)} y={selected.bbox.y.toFixed(4)} w={selected.bbox.w.toFixed(4)} h={selected.bbox.h.toFixed(4)}
                </p>
              ) : null}
              {span.mid ? (
                <p className="source-text">
                  {span.before}
                  <mark className="source-span" data-testid="review-source-span">
                    {span.mid}
                  </mark>
                  {span.after}
                </p>
              ) : (
                <p className="source-text">
                  {sourceLayer?.full_text ||
                    selected.source?.quote ||
                    "No page text layer for this span. Run recognition, or use the quote below."}
                </p>
              )}
              {selected.source?.quote && (
                <p className="mono muted" style={{ fontSize: "0.75rem" }}>
                  quote: {selected.source.quote}
                </p>
              )}
            </div>

            <div className="review-correct" data-testid="review-correct">
              <label>
                Correct value
                <input
                  value={draft}
                  onChange={(e) => setDraft(e.target.value)}
                  disabled={busy || selected.kind === "critical_field"}
                  data-testid="review-correct-input"
                />
              </label>
              <div className="row" style={{ marginTop: "0.55rem" }}>
                <button
                  type="button"
                  className="primary"
                  disabled={busy || !draft.trim() || selected.kind === "critical_field"}
                  onClick={() => void saveCorrection()}
                  data-testid="review-correct-save"
                >
                  Save correction
                </button>
                <span className="chip muted mono">
                  interactions: {interactions}/{REVIEW_MAX_INTERACTIONS}
                </span>
              </div>
              {selected.kind === "critical_field" ? (
                <p className="notice warn" style={{ marginTop: "0.65rem", fontSize: "0.82rem" }}>
                  Critical field is blocked from trusted extraction. Correct its flagged OCR words first; counsel confirmation remains separate and auditable.
                </p>
              ) : null}
              {status && (
                <p className="notice ok" style={{ marginTop: "0.65rem", fontSize: "0.82rem" }}>
                  {status}
                </p>
              )}
              {error && (
                <p className="notice warn" style={{ marginTop: "0.65rem", fontSize: "0.82rem" }}>
                  {error}
                </p>
              )}
            </div>
          </>
        )}
      </section>
    </div>
  );
}
