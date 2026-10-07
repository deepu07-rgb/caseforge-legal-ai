/**
 * Mock Court format catalogue + offline coach (B-S2-02).
 * Five PRD §2 formats — sequential ship order.
 * Stage 2 adversary path uses host mock_court (case_graph + retrieval only).
 */

export type CourtRole = "counsel" | "bench_prep" | "adversary_drill";

/** PRD §2 Stage 2 rehearsal formats (sequential order 1–5). */
export type CourtFormat =
  | "Bail_Interim_Motion"
  | "Final_Arguments"
  | "Witness_Cross_Examination"
  | "Appellate"
  | "Motion_Generic";

export type ChatMessage = {
  id: string;
  role: "user" | "assistant" | "system";
  text: string;
  speaker?: string;
  meta?: string;
};

export const COURT_FORMATS: {
  id: CourtFormat;
  order: number;
  label: string;
  blurb: string;
}[] = [
  {
    id: "Bail_Interim_Motion",
    order: 1,
    label: "Bail / Interim-Motion",
    blurb: "Urgency, irreparable harm, balance of convenience.",
  },
  {
    id: "Final_Arguments",
    order: 2,
    label: "Final Arguments",
    blurb: "Burden, proof map, answer to defence, relief.",
  },
  {
    id: "Witness_Cross_Examination",
    order: 3,
    label: "Witness Cross-Examination",
    blurb: "One fact per question; pin documents on the graph.",
  },
  {
    id: "Appellate",
    order: 4,
    label: "Appellate",
    blurb: "Standard of review and error points from the record graph.",
  },
  {
    id: "Motion_Generic",
    order: 5,
    label: "Generic Motion",
    blurb: "Prayer, grounds, evidence list, draft order (prep only).",
  },
];

export const COURT_ROLES: {
  id: CourtRole;
  label: string;
  description: string;
}[] = [
  {
    id: "counsel",
    label: "Counsel (your side)",
    description: "You argue as counsel. The coach plays a rigorous prep partner.",
  },
  {
    id: "bench_prep",
    label: "Bench Prep Mode",
    description: "Face hard judicial questions. No named-judge profiling.",
  },
  {
    id: "adversary_drill",
    label: "Adversary drill",
    description: "Spar using only case_graph + retrieved law (AC-2.1).",
  },
];

function speakerFor(role: CourtRole): string {
  switch (role) {
    case "bench_prep":
      return "Bench (prep)";
    case "adversary_drill":
      return "Adversary (drill)";
    default:
      return "Prep coach";
  }
}

export function roleToMockCourtRole(role: CourtRole): string {
  switch (role) {
    case "bench_prep":
      return "bench";
    case "adversary_drill":
      return "adversary";
    default:
      return "counsel";
  }
}

export function welcomeMessage(
  role: CourtRole,
  format: CourtFormat,
  modelReady: boolean,
): ChatMessage {
  const formatLabel = COURT_FORMATS.find((f) => f.id === format)?.label ?? format;
  const modelLine = modelReady
    ? "Local infer-llm may assist counsel drafting; adversary path still constrains to case_graph + retrieval."
    : "Deterministic Mock Court engine online (text). Adversary uses case_graph + retrieval only.";

  return {
    id: `sys-${Date.now()}`,
    role: "system",
    text: `${speakerFor(role)} · ${formatLabel}\n${modelLine}\nPreparation only. CaseForge never predicts verdicts. No named judges.`,
  };
}

/** Offline scaffold when host IPC unavailable. */
export function coachReply(opts: {
  userText: string;
  role: CourtRole;
  format: CourtFormat;
  caseTitle?: string;
  jurisdiction?: string;
  modelReady: boolean;
}): ChatMessage {
  const { userText, role, format, caseTitle, jurisdiction } = opts;
  const speaker = speakerFor(role);
  const title = caseTitle?.trim() || "this matter";
  const jur = jurisdiction?.trim() || "the selected jurisdiction";
  const snippet = userText.trim().slice(0, 280);
  const formatLabel = COURT_FORMATS.find((f) => f.id === format)?.label ?? format;

  const body = [
    `${speaker} · ${formatLabel} (${jur})`,
    `Matter: ${title}`,
    "",
    "Stage 2 constraint: answers must rest on case_graph + retrieved law only — never raw PDF parse.",
    "",
    `You said: “${snippet || "…"}”`,
    "",
    "1. State the relief / order sought in ≤ 20 words.",
    "2. Name the graph anchor (chronology / evidence / statute) you rely on.",
    "3. Name the open gap you must still close.",
    "",
    "No outcome prediction. No named-judge profiling.",
    "",
    "—",
    "[Offline scaffold] Prefer host mock_court_turn_for_case when vault case is open.",
  ].join("\n");

  return {
    id: `a-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
    role: "assistant",
    speaker,
    text: body,
  };
}
