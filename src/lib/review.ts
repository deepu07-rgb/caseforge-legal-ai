/**
 * B-S1-08 review helpers — low-confidence items + source resolution.
 * Thresholds must match contracts/constants.json (loaded at runtime on host;
 * UI uses frozen defaults aligned with Q-CONST-01).
 */

import type { CaseGraph, PageTextLayerRow, PageWordRow, SourcePointer } from "./ipc";

/** From constants confidence_thresholds.values.item_highlight_below */
export const ITEM_HIGHLIGHT_BELOW = 0.8;
/** From constants confidence_thresholds.values.word_highlight_below */
export const WORD_HIGHLIGHT_BELOW = 0.75;

export type ReviewItemKind = "graph" | "word" | "critical_field";

export interface ReviewItem {
  key: string;
  kind: ReviewItemKind;
  collection?: string;
  itemId?: string;
  fieldName?: string;
  label: string;
  value: string;
  confidence: number;
  rawConfidence?: number | null;
  calibratedConfidence?: number | null;
  confidenceState?: string;
  validationReasons?: string[];
  counselConfirmed?: boolean;
  bbox?: { x: number; y: number; w: number; h: number } | null;
  lowConfidence: boolean;
  source: SourcePointer | null;
  /** word-only */
  docId?: string;
  pageIndex?: number;
  wordIndex?: number;
}

function firstSource(
  src: SourcePointer | SourcePointer[] | undefined | null,
): SourcePointer | null {
  if (!src) return null;
  if (Array.isArray(src)) return src[0] ?? null;
  return src;
}

export function collectGraphReviewItems(
  graph: CaseGraph | null,
  itemThreshold = ITEM_HIGHLIGHT_BELOW,
): ReviewItem[] {
  if (!graph) return [];
  const out: ReviewItem[] = [];

  for (const e of graph.chronology || []) {
    const conf = e.provenance?.confidence ?? 0;
    out.push({
      key: `chronology:${e.event_id}:summary`,
      kind: "graph",
      collection: "chronology",
      itemId: e.event_id,
      fieldName: "summary",
      label: `Event ${e.date}`,
      value: e.summary,
      confidence: conf,
      lowConfidence: conf < itemThreshold,
      source: firstSource(e.provenance?.source as SourcePointer | SourcePointer[] | undefined),
    });
  }
  for (const p of graph.parties || []) {
    const conf = p.provenance?.confidence ?? 0;
    out.push({
      key: `parties:${p.party_id}:display_name`,
      kind: "graph",
      collection: "parties",
      itemId: p.party_id,
      fieldName: "display_name",
      label: `Party · ${p.role}`,
      value: p.display_name,
      confidence: conf,
      lowConfidence: conf < itemThreshold,
      source: firstSource(p.provenance?.source as SourcePointer | SourcePointer[] | undefined),
    });
  }
  for (const s of graph.statutes || []) {
    const conf = s.provenance?.confidence ?? 0;
    out.push({
      key: `statutes:${s.statute_id}:label`,
      kind: "graph",
      collection: "statutes",
      itemId: s.statute_id,
      fieldName: "label",
      label: "Statute",
      value: s.label,
      confidence: conf,
      lowConfidence: conf < itemThreshold,
      source: firstSource(s.provenance?.source as SourcePointer | SourcePointer[] | undefined),
    });
  }
  for (const r of graph.prayer_relief || []) {
    const conf = r.provenance?.confidence ?? 0;
    out.push({
      key: `prayer_relief:${r.relief_id}:text`,
      kind: "graph",
      collection: "prayer_relief",
      itemId: r.relief_id,
      fieldName: "text",
      label: "Relief",
      value: r.text,
      confidence: conf,
      lowConfidence: conf < itemThreshold,
      source: firstSource(r.provenance?.source as SourcePointer | SourcePointer[] | undefined),
    });
  }
  // Low-confidence first
  return out.sort((a, b) => Number(b.lowConfidence) - Number(a.lowConfidence) || a.confidence - b.confidence);
}

export function collectWordReviewItems(
  layers: PageTextLayerRow[],
  wordThreshold = WORD_HIGHLIGHT_BELOW,
): ReviewItem[] {
  const out: ReviewItem[] = [];
  for (const layer of layers) {
    for (const w of layer.words || []) {
      if (!w.low_confidence && w.confidence >= wordThreshold) continue;
      out.push({
        key: `word:${layer.doc_id}:${layer.page_index}:${w.word_index}`,
        kind: "word",
        label: `Word p.${layer.page_index}`,
        value: w.text,
        confidence: w.confidence,
        rawConfidence: w.raw_engine_confidence,
        calibratedConfidence: w.calibrated_confidence,
        confidenceState: w.confidence_state,
        counselConfirmed: w.counsel_confirmed,
        lowConfidence: true,
        source: {
          doc_id: layer.doc_id,
          page_index: layer.page_index,
          kind: "pdf_span",
          char_start: w.char_start,
          char_end: w.char_end,
          quote: w.text,
        },
        docId: layer.doc_id,
        pageIndex: layer.page_index,
        wordIndex: w.word_index,
      });
    }
  }
  for (const layer of layers) {
    for (const field of layer.critical_fields || []) {
      out.push({
        key: `critical:${layer.doc_id}:${layer.page_index}:${field.field_index}`,
        kind: "critical_field",
        label: `${field.field_type.replace("_", " ")} · p.${layer.page_index}`,
        value: field.text,
        confidence: field.field_confidence ?? 0,
        rawConfidence: field.raw_engine_confidence,
        calibratedConfidence: field.calibrated_confidence,
        confidenceState: field.confidence_state,
        validationReasons: field.validation_reasons,
        counselConfirmed: field.counsel_confirmed,
        bbox: field.bbox,
        lowConfidence: field.force_review,
        source: {
          doc_id: layer.doc_id, page_index: layer.page_index, kind: "pdf_span",
          char_start: field.char_start, char_end: field.char_end, quote: field.text,
        },
      });
    }
  }
  return out;
}

/** Build HTML-safe highlight of full_text for char_start..char_end (codepoints). */
export function highlightSpan(
  fullText: string,
  charStart: number | null | undefined,
  charEnd: number | null | undefined,
): { before: string; mid: string; after: string } {
  const chars = [...fullText];
  const start = Math.max(0, charStart ?? 0);
  const end = Math.min(chars.length, charEnd ?? chars.length);
  if (charStart == null || charEnd == null || start >= end) {
    return { before: fullText, mid: "", after: "" };
  }
  return {
    before: chars.slice(0, start).join(""),
    mid: chars.slice(start, end).join(""),
    after: chars.slice(end).join(""),
  };
}

export function layerForSource(
  layers: PageTextLayerRow[],
  source: SourcePointer | null,
): PageTextLayerRow | null {
  if (!source) return null;
  return (
    layers.find((l) => l.doc_id === source.doc_id && l.page_index === source.page_index) ?? null
  );
}

export function wordAt(
  layer: PageTextLayerRow | null,
  wordIndex: number | undefined,
): PageWordRow | null {
  if (!layer || wordIndex == null) return null;
  return layer.words.find((w) => w.word_index === wordIndex) ?? null;
}
