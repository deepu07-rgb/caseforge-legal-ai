/**
 * Preparation Completeness Score — caseforge.prep_completeness.v1 (B-S1-14).
 * Deterministic from case_graph only. No LLM. G6 preparation language only.
 */

import type { CaseGraph } from "./ipc";

export const PREP_SCORE_ID = "caseforge.prep_completeness.v1";

export const PREP_FORMULA =
  "20 points for each non-empty array among: parties, chronology, statutes, evidence, prayer_relief (max 100).";

export const PREP_G6 =
  "Preparation completeness only — never win prediction, outcome odds, or verdict language.";

/** Frozen component weights (mirrors contracts/constants.json preparation_completeness). */
export const PREP_COMPONENTS = [
  { id: "parties", graphField: "parties" as const, points: 20, label: "Parties" },
  { id: "chronology", graphField: "chronology" as const, points: 20, label: "Chronology" },
  { id: "statutes", graphField: "statutes" as const, points: 20, label: "Statutes" },
  { id: "evidence", graphField: "evidence" as const, points: 20, label: "Evidence" },
  { id: "prayer_relief", graphField: "prayer_relief" as const, points: 20, label: "Relief" },
] as const;

export type PrepComponentId = (typeof PREP_COMPONENTS)[number]["id"];

export interface PrepComponentResult {
  id: PrepComponentId;
  label: string;
  points: number;
  earned: number;
  filled: boolean;
  count: number;
}

export interface PrepCompleteness {
  score_id: typeof PREP_SCORE_ID;
  score_0_100: number;
  open_gaps: number;
  formula: typeof PREP_FORMULA;
  components: PrepComponentResult[];
  language_g6: typeof PREP_G6;
  source: "case_graph";
  llm_used: false;
}

function arrayLen(v: unknown): number {
  return Array.isArray(v) ? v.length : 0;
}

/**
 * Recompute preparation completeness from the current case_graph.
 * Forbidden: any model/LLM judgment path.
 */
export function computePrepCompleteness(graph: CaseGraph | null | undefined): PrepCompleteness {
  const components: PrepComponentResult[] = PREP_COMPONENTS.map((c) => {
    const count = graph
      ? arrayLen((graph as unknown as Record<string, unknown>)[c.graphField])
      : 0;
    const filled = count > 0;
    return {
      id: c.id,
      label: c.label,
      points: c.points,
      earned: filled ? c.points : 0,
      filled,
      count,
    };
  });
  const score = components.reduce((n, c) => n + c.earned, 0);
  const openGaps = graph
    ? (graph.gaps || []).filter((g) => (g.status || "open") === "open").length
    : 0;
  return {
    score_id: PREP_SCORE_ID,
    score_0_100: Math.max(0, Math.min(100, score)),
    open_gaps: openGaps,
    formula: PREP_FORMULA,
    components,
    language_g6: PREP_G6,
    source: "case_graph",
    llm_used: false,
  };
}

/** Item confidence heat band for Digest UI (thresholds from Q-CONST-01 defaults). */
export type ConfidenceBand = "ok" | "low" | "force" | "missing";

export function confidenceBand(
  conf: number | null | undefined,
  itemHighlightBelow = 0.8,
  itemBlockBelow = 0.55,
): ConfidenceBand {
  if (conf == null || Number.isNaN(conf)) return "missing";
  if (conf < itemBlockBelow) return "force";
  if (conf < itemHighlightBelow) return "low";
  return "ok";
}

export function formatSourceChip(
  source:
    | { doc_id?: string; page_index?: number; kind?: string; quote?: string | null }
    | Array<{ doc_id?: string; page_index?: number }>
    | null
    | undefined,
): string {
  if (!source) return "no source";
  const s = Array.isArray(source) ? source[0] : source;
  if (!s) return "no source";
  const page = s.page_index != null ? `p.${s.page_index + 1}` : "page?";
  return `${s.doc_id ?? "doc"} · ${page}`;
}
