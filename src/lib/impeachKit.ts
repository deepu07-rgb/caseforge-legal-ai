/**
 * B-S4-08: Impeachment Kit presentation — cross-examination outline (commit → credit → confront) and the
 * DOCX/PDF export document. Every quote is the exact stored text; questions are preparation suggestions only.
 */
import type { CaseSummary, ImpeachFinding, ImpeachmentKit, MatrixWitness, MatterLocator } from "./ipc";
import {
  EXPORT_FOOTER_ID,
  EXPORT_FOOTER_LINES,
  EXPORT_FOOTER_TEXT,
  EXPORT_PIPELINE_ID,
  makeAiSpan,
  type DraftDocument,
} from "./exportDraft";

export const DIMENSION_LABEL: Record<string, string> = {
  PERSON: "Who did it",
  PRESENCE: "Who was present",
  MATERIAL_FACT: "Omission / improvement",
  DATE: "Date",
  TIME: "Time",
  PLACE: "Place",
  SEQUENCE: "Order of events",
  COUNT: "Number of persons",
  AMOUNT: "Amount",
  SEMANTIC: "Meaning (local AI, quotes verified)",
};

export const KIND_LABEL: Record<string, string> = {
  FIR: "FIR",
  POLICE_STATEMENT: "statement to police",
  MAGISTRATE_STATEMENT: "statement before the Magistrate",
  COMPLAINT: "complaint",
  EXAMINATION_IN_CHIEF: "examination-in-chief",
  CROSS_EXAMINATION: "cross-examination",
  RE_EXAMINATION: "re-examination",
  AFFIDAVIT: "affidavit",
  DECLARATION: "declaration",
  INTERROGATORY_ANSWERS: "interrogatory answers",
  DEPOSITION: "deposition",
  TRIAL_TESTIMONY: "trial testimony",
  OTHER: "statement",
};

export function oneLine(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

/** "PW1_Examination_in_chief.pdf p.1" or "Deposition.txt p.2, lines 5-6". */
export function locatorLabel(loc: MatterLocator | null, docName: (docId: string) => string): string {
  if (!loc) return "";
  const page = loc.page_index != null ? ` p.${loc.page_index + 1}` : "";
  const lines = loc.line_start != null ? `, line${loc.line_end !== loc.line_start ? "s" : ""} ${loc.line_start}${loc.line_end !== loc.line_start ? `-${loc.line_end}` : ""}` : "";
  return `${docName(loc.doc_id)}${page}${lines}`;
}

export function statementLabel(w: MatrixWitness, statementId: string): string {
  const s = w.statements.find((x) => x.statement_id === statementId);
  if (!s) return statementId;
  return `${KIND_LABEL[s.statement_kind] ?? s.statement_kind}${s.recorded_on ? ` of ${s.recorded_on}` : ""}`;
}

/** Commit → credit → confront. Suggestions for counsel; never what the witness should say. */
export function crossOutline(f: ImpeachFinding, w: MatrixWitness, docName: (docId: string) => string): string[] {
  const later = oneLine(f.later.quote ?? "");
  const earlierLabel = statementLabel(w, f.earlier.statement_id);
  if (f.finding_kind === "OMISSION") {
    return [
      `Commit: "Today you say: '${later}'."`,
      `Credit: "You gave your ${earlierLabel} soon after the event, when it was fresh in your mind?"`,
      `Confront: "In it you did not say this. This is the first time you say it?"`,
    ];
  }
  return [
    `Commit: "Today you say: '${later}'."`,
    `Credit: "You gave your ${earlierLabel}, and you told the truth then?"`,
    `Confront: "In it you said: '${oneLine(f.earlier.quote ?? "")}' (${locatorLabel(f.earlier.locator, docName)})."`,
  ];
}

export function findingLines(f: ImpeachFinding, w: MatrixWitness, docName: (docId: string) => string): string[] {
  const head = `${f.confrontation_order}. ${DIMENSION_LABEL[f.dimension] ?? f.dimension}${f.finding_kind === "OMISSION" ? " (counsel judgement)" : ""}${f.status === "COUNSEL_ACCEPTED" ? " — accepted" : ""}`;
  const earlier =
    f.earlier.quote != null
      ? `   Earlier — ${statementLabel(w, f.earlier.statement_id)} (${locatorLabel(f.earlier.locator, docName)}): "${oneLine(f.earlier.quote)}"`
      : `   Earlier — ${f.earlier.absence_note ?? "not stated"}`;
  const later = `   Later — ${statementLabel(w, f.later.statement_id)} (${locatorLabel(f.later.locator, docName)}): "${oneLine(f.later.quote ?? "")}"`;
  return [head, earlier, later, ...crossOutline(f, w, docName).map((l) => `   ${l}`), ""];
}

export function kitDocument(opts: {
  kit: ImpeachmentKit;
  witness: MatrixWitness;
  activeCase: CaseSummary | null;
  docName: (docId: string) => string;
}): DraftDocument {
  const { kit, witness, activeCase, docName } = opts;
  const findings = kit.findings.filter((f) => f.status !== "COUNSEL_REJECTED");
  const body = [
    `IMPEACHMENT KIT — ${witness.display_name}`,
    activeCase ? `${activeCase.cause_title || activeCase.label} · ${activeCase.jurisdiction}` : "",
    `Statements, earliest first: ${witness.statements.map((s) => statementLabel(witness, s.statement_id)).join("; ")}`,
    `Kit ${kit.kit_id} · matrix ${kit.matrix_id} · generated ${kit.generated_at}`,
    "Every quotation below is the exact stored text at the location shown. Suggested questions are preparation aids for counsel.",
    "Confirm the permitted use of each prior statement under the applicable law before confronting the witness.",
    "",
    ...findings.flatMap((f) => findingLines(f, witness, docName)),
  ];
  const semantic = findings.filter((f) => f.detection === "MODEL_VERIFIED");
  const aiSpans = semantic.map((f) =>
    makeAiSpan({
      text: `${oneLine(f.earlier.quote ?? "")} <> ${oneLine(f.later.quote ?? "")}`,
      modelHash: kit.model_fingerprint ?? "unknown-model",
      promptHash: "caseforge.impeachment.v1",
      retrievalIds: f.proposition_ids,
    }),
  );
  return {
    template_id: "impeachment_kit",
    title: `Impeachment kit ${witness.display_name}`,
    body_lines: body,
    provenance: {
      export_id: `exp_${kit.kit_id}_${Date.now().toString(36)}`,
      generated_at: new Date().toISOString(),
      template_id: "impeachment_kit",
      case_id: kit.case_id,
      jurisdiction: activeCase?.jurisdiction ?? "—",
      pipeline_id: EXPORT_PIPELINE_ID,
      footer_id: EXPORT_FOOTER_ID,
      footer_text: EXPORT_FOOTER_TEXT,
      g5_fields: ["model_hash", "prompt_hash", "retrieval_ids"],
      extractor_id: "caseforge.impeachment.v1",
      ai_spans: aiSpans,
      source_records: aiSpans.map((s) => s.provenance_record),
      llm_used: aiSpans.length > 0,
    },
    footer_lines: EXPORT_FOOTER_LINES,
  };
}
