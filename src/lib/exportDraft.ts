/**
 * B-S1-15 — Drafting surface export (DOCX / PDF).
 * G4 footer + G5 provenance. Deterministic from case_graph unless AI spans are attached.
 * Footer text frozen in contracts/constants.json → export_footer_g4 (reuse existing honesty + G6).
 */

import type { CaseGraph, CaseSummary, GraphProvenance } from "./ipc";
import { graphProvenance, provenanceExportLine, type ProvenanceRecord } from "./provenance";

export const EXPORT_FOOTER_ID = "caseforge.export_footer.v1";
export const EXPORT_PIPELINE_ID = "caseforge.draft_export.v1";

/** Frozen G4 footer lines (must match contracts/constants.json export_footer_g4.lines). */
export const EXPORT_FOOTER_LINES = [
  "Local model output for preparation only. Not legal advice. Citations require pack verification.",
  "Preparation completeness only — never win prediction, outcome odds, or verdict language.",
  "Export footer (G4) · caseforge.export_footer.v1 · verify every fact against its source pointer before filing.",
] as const;

export const EXPORT_FOOTER_TEXT = EXPORT_FOOTER_LINES.join(" | ");

export type TemplateId =
  | "court_formatted_digest"
  | "list_of_dates"
  | "synopsis"
  | "counsel_memo"
  | "impeachment_kit";

export const TEMPLATES: {
  id: TemplateId;
  label: string;
  description: string;
}[] = [
  {
    id: "court_formatted_digest",
    label: "Court-formatted digest",
    description: "Full digest: parties, chronology, statutes, evidence, posture, relief, contradictions, gaps.",
  },
  {
    id: "list_of_dates",
    label: "List of dates",
    description: "Chronology-only dated events list with source chips.",
  },
  {
    id: "synopsis",
    label: "Synopsis",
    description: "Short matter synopsis for preparation (not a prediction).",
  },
  {
    id: "counsel_memo",
    label: "Counsel memo template",
    description: "Editable preparation memo skeleton with graph pins.",
  },
];

export type SpanOrigin = "deterministic_graph" | "ai_assisted" | "user_edited";

export interface AiSpanProvenance {
  span_id: string;
  text: string;
  origin: SpanOrigin;
  model_hash: string;
  prompt_hash: string;
  retrieval_ids: string[];
  provenance_record: ProvenanceRecord;
}

export interface ExportProvenance {
  export_id: string;
  generated_at: string;
  template_id: TemplateId;
  case_id: string;
  jurisdiction: string;
  pipeline_id: typeof EXPORT_PIPELINE_ID;
  footer_id: typeof EXPORT_FOOTER_ID;
  footer_text: string;
  g5_fields: ["model_hash", "prompt_hash", "retrieval_ids"];
  extractor_id: string | null;
  ai_spans: AiSpanProvenance[];
  source_records: ProvenanceRecord[];
  llm_used: boolean;
}

export interface DraftDocument {
  template_id: TemplateId;
  title: string;
  body_lines: string[];
  provenance: ExportProvenance;
  footer_lines: readonly string[];
}

function nowIso(): string {
  return new Date().toISOString();
}

function uid(prefix: string): string {
  return `${prefix}_${Math.random().toString(36).slice(2, 10)}_${Date.now().toString(36)}`;
}

function sourceChip(
  source:
    | { doc_id?: string; page_index?: number }
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

function detSpan(text: string, extractorId: string | null, stableId?: string, provenance?: GraphProvenance | null): AiSpanProvenance {
  const record = graphProvenance(stableId || uid("graph"), text.slice(0, 100), provenance);
  return {
    span_id: uid("span"),
    text: text.slice(0, 200),
    origin: "deterministic_graph",
    model_hash: extractorId ? `extractor:${extractorId}` : "deterministic:case_graph",
    prompt_hash: "deterministic:template",
    retrieval_ids: [],
    provenance_record: record,
  };
}

function counselEditSpan(text: string): AiSpanProvenance {
  return {
    span_id: uid("counsel"),
    text: text.slice(0, 500),
    origin: "user_edited",
    model_hash: "none:counsel_edit",
    prompt_hash: "none:counsel_edit",
    retrieval_ids: [],
    provenance_record: {
      stableId: uid("counsel-edit"),
      propositionClass: "COUNSEL_INPUT",
      origin: "COUNSEL_INPUT",
      supportState: "AMBER",
      title: "Counsel-edited export body",
      verification: "User-edited drafting text. It is not documentary evidence or verified legal authority.",
      excerpt: text.slice(0, 500),
      version: "caseforge.draft_export.v1",
    },
  };
}

function matterHeader(activeCase: CaseSummary | null, graph: CaseGraph | null): string[] {
  const court = activeCase?.court || "—";
  const title = activeCase?.cause_title || graph?.case_id || "Untitled matter";
  const jurisdiction = activeCase?.jurisdiction || graph?.jurisdiction || "—";
  const matter = activeCase?.matter_type || "—";
  const side = activeCase?.user_side || "—";
  return [
    `IN THE ${court}`,
    title,
    "",
    `Jurisdiction: ${jurisdiction}`,
    `Matter type: ${matter}`,
    `Counsel side: ${side}`,
    `Language: ${activeCase?.language || "English"}`,
    "",
  ];
}

function buildCourtDigest(
  activeCase: CaseSummary | null,
  graph: CaseGraph | null,
  spans: AiSpanProvenance[],
  extractorId: string | null,
): string[] {
  const lines = [...matterHeader(activeCase, graph)];
  if (!graph) {
    lines.push("(No case_graph bound — run skeleton extraction before exporting a court digest.)");
    return lines;
  }

  lines.push("I. PARTIES");
  for (const p of graph.parties || []) {
    const row = `${p.display_name} — ${p.role}  [${sourceChip(p.provenance?.source)}]`;
    lines.push(row);
    spans.push(detSpan(row, extractorId, `party:${p.party_id}`, p.provenance));
  }
  if (!(graph.parties || []).length) lines.push("(none extracted)");
  lines.push("");

  lines.push("II. CHRONOLOGY");
  for (const e of graph.chronology || []) {
    const row = `${e.date} (${e.date_precision}): ${e.summary}  [${sourceChip(e.provenance?.source)}]`;
    lines.push(row);
    spans.push(detSpan(row, extractorId, `chronology:${e.event_id}`, e.provenance));
  }
  if (!(graph.chronology || []).length) lines.push("(none extracted)");
  lines.push("");

  lines.push("III. STATUTES / RULES");
  for (const s of graph.statutes || []) {
    const row = `${s.label}  [${sourceChip(s.provenance?.source)}]`;
    lines.push(row);
    spans.push(detSpan(row, extractorId, `statute:${s.statute_id}`, s.provenance));
  }
  if (!(graph.statutes || []).length) lines.push("(none extracted)");
  lines.push("");

  lines.push("IV. EVIDENCE INVENTORY");
  for (const e of graph.evidence || []) {
    const row = `${e.label}  [${sourceChip(e.provenance?.source)}]`;
    lines.push(row);
    spans.push(detSpan(row, extractorId, `evidence:${e.evidence_id}`, e.provenance));
  }
  if (!(graph.evidence || []).length) lines.push("(none extracted)");
  lines.push("");

  lines.push("V. PROCEDURAL POSTURE");
  if (graph.procedural_posture && typeof graph.procedural_posture === "object") {
    const pp = graph.procedural_posture as { summary?: string; text?: string };
    const t = pp.summary || pp.text || JSON.stringify(graph.procedural_posture);
    lines.push(String(t));
    spans.push(detSpan(String(t), extractorId));
  } else {
    lines.push("(not extracted)");
  }
  lines.push("");

  lines.push("VI. PRAYER / RELIEF");
  for (const r of graph.prayer_relief || []) {
    const row = `${r.text}  [${sourceChip(r.provenance?.source)}]`;
    lines.push(row);
    spans.push(detSpan(row, extractorId, `relief:${r.relief_id}`, r.provenance));
  }
  if (!(graph.prayer_relief || []).length) lines.push("(none extracted)");
  lines.push("");

  lines.push("VII. CONTRADICTIONS");
  const cons = (graph.contradictions || []) as Array<{ summary?: string; text?: string }>;
  if (cons.length) {
    for (const c of cons) {
      const row = c.summary || c.text || JSON.stringify(c);
      lines.push(String(row));
      spans.push(detSpan(String(row), extractorId));
    }
  } else {
    lines.push("(none flagged)");
  }
  lines.push("");

  lines.push("VIII. OPEN GAPS");
  const gaps = (graph.gaps || []).filter((g) => (g.status || "open") === "open");
  if (gaps.length) {
    for (const g of gaps) {
      lines.push(`- ${g.topic}${g.question_prompt ? `: ${g.question_prompt}` : ""}`);
    }
  } else {
    lines.push("(none open)");
  }

  return lines;
}

function buildListOfDates(
  activeCase: CaseSummary | null,
  graph: CaseGraph | null,
  spans: AiSpanProvenance[],
  extractorId: string | null,
): string[] {
  const lines = [
    ...matterHeader(activeCase, graph),
    "LIST OF DATES",
    "(Preparation chronology — verify each date against source before use.)",
    "",
  ];
  const events = [...(graph?.chronology || [])].sort((a, b) =>
    (a.sort_key || a.date || "").localeCompare(b.sort_key || b.date || ""),
  );
  if (!events.length) {
    lines.push("(No dated events in case_graph.)");
    return lines;
  }
  let i = 1;
  for (const e of events) {
    const row = `${i}. ${e.date} — ${e.summary}  [${sourceChip(e.provenance?.source)}]`;
    lines.push(row);
    spans.push(detSpan(row, extractorId, `chronology:${e.event_id}`, e.provenance));
    i += 1;
  }
  return lines;
}

function buildSynopsis(
  activeCase: CaseSummary | null,
  graph: CaseGraph | null,
  spans: AiSpanProvenance[],
  extractorId: string | null,
): string[] {
  const lines = [
    ...matterHeader(activeCase, graph),
    "SYNOPSIS",
    "(Preparation synopsis only — not a prediction of outcome.)",
    "",
  ];
  if (!graph) {
    lines.push("(No case_graph bound.)");
    return lines;
  }
  const parties = (graph.parties || [])
    .map((p) => `${p.display_name} (${p.role})`)
    .join("; ");
  const relief = (graph.prayer_relief || []).map((r) => r.text).join("; ");
  const first = graph.chronology?.[0];
  const last = graph.chronology?.[graph.chronology.length - 1];
  const body = [
    parties ? `Parties: ${parties}.` : "Parties: not yet extracted.",
    first
      ? `Matter window: ${first.date}${last && last !== first ? ` to ${last.date}` : ""}.`
      : "Matter window: chronology not yet extracted.",
    relief ? `Relief sought: ${relief}.` : "Relief sought: not yet extracted.",
    `Statutes flagged: ${(graph.statutes || []).length}. Evidence items: ${(graph.evidence || []).length}.`,
    `Open gaps: ${(graph.gaps || []).filter((g) => (g.status || "open") === "open").length}.`,
  ].join(" ");
  lines.push(body);
  spans.push(detSpan(body, extractorId));
  return lines;
}

function buildCounselMemo(
  activeCase: CaseSummary | null,
  graph: CaseGraph | null,
  spans: AiSpanProvenance[],
  extractorId: string | null,
): string[] {
  const lines = [
    ...matterHeader(activeCase, graph),
    "COUNSEL PREPARATION MEMO",
    "",
    "1. Facts to lock",
    ...(graph?.chronology || []).slice(0, 8).map((e) => {
      const row = `   - [${e.date}] ${e.summary}`;
      spans.push(detSpan(row, extractorId, `chronology:${e.event_id}`, e.provenance));
      return row;
    }),
    !(graph?.chronology || []).length ? "   - (add from review / interview)" : "",
    "",
    "2. Authorities to verify (pack retrieval)",
    ...(graph?.statutes || []).slice(0, 8).map((s) => {
      const row = `   - ${s.label}`;
      spans.push(detSpan(row, extractorId, `statute:${s.statute_id}`, s.provenance));
      return row;
    }),
    !(graph?.statutes || []).length ? "   - (run Library retrieval; mark GREEN/AMBER/RED)" : "",
    "",
    "3. Evidence checklist",
    ...(graph?.evidence || []).slice(0, 8).map((e) => {
      const row = `   - ${e.label}`;
      spans.push(detSpan(row, extractorId, `evidence:${e.evidence_id}`, e.provenance));
      return row;
    }),
    !(graph?.evidence || []).length ? "   - (inventory from case file)" : "",
    "",
    "4. Open questions for client / co-counsel",
    ...(graph?.gaps || [])
      .filter((g) => (g.status || "open") === "open")
      .slice(0, 8)
      .map((g) => `   - ${g.topic}`),
    "",
    "5. Notes (user-editable before export)",
    "   - ",
  ].filter((l) => l !== undefined);
  return lines;
}

/**
 * Build a draft document from case_graph + optional AI-assisted spans.
 * Graph-derived content is deterministic (no LLM). AI spans must carry G5 fields.
 */
export function buildDraftDocument(opts: {
  templateId: TemplateId;
  activeCase: CaseSummary | null;
  caseGraph: CaseGraph | null;
  aiSpans?: AiSpanProvenance[];
  bodyOverride?: string | null;
}): DraftDocument {
  const spans: AiSpanProvenance[] = [...(opts.aiSpans || [])];
  const extractorId = opts.caseGraph?.pipeline?.extractor_id ?? null;

  let body: string[];
  switch (opts.templateId) {
    case "list_of_dates":
      body = buildListOfDates(opts.activeCase, opts.caseGraph, spans, extractorId);
      break;
    case "synopsis":
      body = buildSynopsis(opts.activeCase, opts.caseGraph, spans, extractorId);
      break;
    case "counsel_memo":
      body = buildCounselMemo(opts.activeCase, opts.caseGraph, spans, extractorId);
      break;
    case "court_formatted_digest":
    default:
      body = buildCourtDigest(opts.activeCase, opts.caseGraph, spans, extractorId);
      break;
  }

  if (opts.bodyOverride != null) {
    body = opts.bodyOverride.split(/\r?\n/);
    spans.push(counselEditSpan(opts.bodyOverride));
  }

  const title =
    opts.activeCase?.cause_title ||
    opts.caseGraph?.case_id ||
    TEMPLATES.find((t) => t.id === opts.templateId)?.label ||
    "Draft";

  const llmUsed = spans.some((s) => s.origin === "ai_assisted");
  const provenance: ExportProvenance = {
    export_id: uid("exp"),
    generated_at: nowIso(),
    template_id: opts.templateId,
    case_id: opts.activeCase?.case_id || opts.caseGraph?.case_id || "unknown",
    jurisdiction: opts.activeCase?.jurisdiction || opts.caseGraph?.jurisdiction || "—",
    pipeline_id: EXPORT_PIPELINE_ID,
    footer_id: EXPORT_FOOTER_ID,
    footer_text: EXPORT_FOOTER_TEXT,
    g5_fields: ["model_hash", "prompt_hash", "retrieval_ids"],
    extractor_id: extractorId,
    ai_spans: spans,
    source_records: spans.map((span) => span.provenance_record),
    llm_used: llmUsed,
  };

  return {
    template_id: opts.templateId,
    title,
    body_lines: body,
    provenance,
    footer_lines: EXPORT_FOOTER_LINES,
  };
}

/** Full plain-text export including G4 footer and G5 provenance appendix. */
export function renderPlainText(draft: DraftDocument): string {
  const lines = [
    ...draft.body_lines,
    "",
    "────────────────────────────────────────",
    "EXPORT FOOTER (G4)",
    ...draft.footer_lines,
    "",
    "PROVENANCE LOG (G5)",
    `export_id: ${draft.provenance.export_id}`,
    `generated_at: ${draft.provenance.generated_at}`,
    `template_id: ${draft.provenance.template_id}`,
    `case_id: ${draft.provenance.case_id}`,
    `pipeline_id: ${draft.provenance.pipeline_id}`,
    `footer_id: ${draft.provenance.footer_id}`,
    `llm_used: ${draft.provenance.llm_used}`,
    `extractor_id: ${draft.provenance.extractor_id ?? "—"}`,
    `ai_spans: ${draft.provenance.ai_spans.length}`,
  ];
  for (const s of draft.provenance.ai_spans) {
    lines.push(
      `  - [${s.span_id}] origin=${s.origin} model_hash=${s.model_hash} prompt_hash=${s.prompt_hash} retrieval_ids=${JSON.stringify(s.retrieval_ids)}`,
    );
    lines.push(`    ${provenanceExportLine(s.provenance_record)}`);
  }
  lines.push("");
  lines.push(EXPORT_FOOTER_TEXT);
  return lines.join("\n");
}

// ——— Minimal PDF writer (text-only, Helvetica) ———

/** Encode visible PDF text as WinAnsi octal escapes so the content stream stays ASCII.
 * Exact Unicode is preserved separately in /ActualText (UTF-16BE) for copy/extraction.
 */
function pdfVisibleText(s: string): string {
  const cp1252 = new Map<number, number>([
    [0x20ac, 0x80], [0x201a, 0x82], [0x0192, 0x83], [0x201e, 0x84],
    [0x2026, 0x85], [0x2020, 0x86], [0x2021, 0x87], [0x02c6, 0x88],
    [0x2030, 0x89], [0x0160, 0x8a], [0x2039, 0x8b], [0x0152, 0x8c],
    [0x017d, 0x8e], [0x2018, 0x91], [0x2019, 0x92], [0x201c, 0x93],
    [0x201d, 0x94], [0x2022, 0x95], [0x2013, 0x96], [0x2014, 0x97],
    [0x02dc, 0x98], [0x2122, 0x99], [0x0161, 0x9a], [0x203a, 0x9b],
    [0x0153, 0x9c], [0x017e, 0x9e], [0x0178, 0x9f],
  ]);
  const fallback = new Map<number, string>([
    [0x2264, "<="],
    [0x2265, ">="],
    [0x2192, "->"],
    [0x2190, "<-"],
    [0x20b9, "INR"],
  ]);
  let out = "";
  for (const ch of s) {
    const code = ch.codePointAt(0) ?? 0x3f;
    const replacement = fallback.get(code);
    if (replacement) {
      out += replacement;
      continue;
    }
    const byte = code <= 0xff ? code : cp1252.get(code);
    if (byte == null) {
      out += "?";
    } else if (byte === 0x28 || byte === 0x29 || byte === 0x5c || byte < 0x20 || byte > 0x7e) {
      out += `\\${byte.toString(8).padStart(3, "0")}`;
    } else {
      out += String.fromCharCode(byte);
    }
  }
  return out;
}

function pdfActualTextHex(s: string): string {
  const units = [0xfeff];
  for (let i = 0; i < s.length; i++) units.push(s.charCodeAt(i));
  return units.map((n) => n.toString(16).padStart(4, "0").toUpperCase()).join("");
}

function pdfTextRun(s: string): string {
  return `/Span << /ActualText <${pdfActualTextHex(s)}> >> BDC (${pdfVisibleText(s)}) Tj EMC`;
}

function wrapLine(line: string, maxChars: number): string[] {
  if (line.length <= maxChars) return [line];
  const out: string[] = [];
  let rest = line;
  while (rest.length > maxChars) {
    let cut = rest.lastIndexOf(" ", maxChars);
    if (cut < 20) cut = maxChars;
    out.push(rest.slice(0, cut));
    rest = rest.slice(cut).trimStart();
  }
  if (rest) out.push(rest);
  return out;
}

export function renderPdf(draft: DraftDocument): Uint8Array {
  const text = renderPlainText(draft);
  const rawLines = text.split("\n");
  const wrapped: string[] = [];
  for (const ln of rawLines) {
    wrapped.push(...wrapLine(ln.length ? ln : " ", 90));
  }

  const linesPerPage = 58;
  const pages: string[][] = [];
  for (let i = 0; i < wrapped.length; i += linesPerPage) {
    pages.push(wrapped.slice(i, i + linesPerPage));
  }
  if (!pages.length) pages.push([" "]);
  return buildPdfDocument(pages);
}

function buildPdfDocument(pages: string[][]): Uint8Array {
  const encoder = new TextEncoder();
  // obj 1 = catalog, 2 = pages, 3 = font, then content+page pairs
  const parts: { id: number; bytes: Uint8Array }[] = [];

  const fontId = 3;
  parts.push({
    id: fontId,
    bytes: encoder.encode("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
  });

  let nextId = 4;
  const pageIds: number[] = [];
  for (const pageLines of pages) {
    const contentParts = ["BT", "/F1 10 Tf", "50 780 Td", "12 TL"];
    pageLines.forEach((ln, idx) => {
      if (idx === 0) contentParts.push(pdfTextRun(ln));
      else {
        contentParts.push("T*");
        contentParts.push(pdfTextRun(ln));
      }
    });
    contentParts.push("ET");
    const stream = contentParts.join("\n");
    const contentId = nextId++;
    parts.push({
      id: contentId,
      bytes: encoder.encode(`<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`),
    });
    const pageId = nextId++;
    pageIds.push(pageId);
    parts.push({
      id: pageId,
      bytes: encoder.encode(
        `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents ${contentId} 0 R /Resources << /Font << /F1 ${fontId} 0 R >> >> >>`,
      ),
    });
  }

  const kids = pageIds.map((id) => `${id} 0 R`).join(" ");
  parts.unshift({
    id: 2,
    bytes: encoder.encode(`<< /Type /Pages /Kids [${kids}] /Count ${pageIds.length} >>`),
  });
  parts.unshift({
    id: 1,
    bytes: encoder.encode("<< /Type /Catalog /Pages 2 0 R >>"),
  });

  parts.sort((a, b) => a.id - b.id);

  let pdf = "%PDF-1.4\n";
  const offsets: number[] = [0];
  for (const p of parts) {
    offsets[p.id] = pdf.length;
    pdf += `${p.id} 0 obj\n`;
    pdf += new TextDecoder().decode(p.bytes);
    pdf += "\nendobj\n";
  }
  const xrefPos = pdf.length;
  const maxId = parts[parts.length - 1].id;
  pdf += `xref\n0 ${maxId + 1}\n`;
  pdf += "0000000000 65535 f \n";
  for (let i = 1; i <= maxId; i++) {
    const off = offsets[i] ?? 0;
    pdf += `${String(off).padStart(10, "0")} 00000 n \n`;
  }
  pdf += `trailer\n<< /Size ${maxId + 1} /Root 1 0 R >>\nstartxref\n${xrefPos}\n%%EOF\n`;
  return encoder.encode(pdf);
}

// ——— Minimal DOCX (OOXML ZIP, store method) ———

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(data: Uint8Array): number {
  let c = 0xffffffff;
  for (let i = 0; i < data.length; i++) c = CRC_TABLE[(c ^ data[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function u16(n: number): Uint8Array {
  return new Uint8Array([n & 0xff, (n >>> 8) & 0xff]);
}
function u32(n: number): Uint8Array {
  return new Uint8Array([n & 0xff, (n >>> 8) & 0xff, (n >>> 16) & 0xff, (n >>> 24) & 0xff]);
}

function concatBytes(chunks: Uint8Array[]): Uint8Array {
  const total = chunks.reduce((n, c) => n + c.length, 0);
  const out = new Uint8Array(total);
  let o = 0;
  for (const c of chunks) {
    out.set(c, o);
    o += c.length;
  }
  return out;
}

function zipStore(files: { name: string; data: Uint8Array }[]): Uint8Array {
  const encoder = new TextEncoder();
  const locals: Uint8Array[] = [];
  const centrals: Uint8Array[] = [];
  let offset = 0;

  for (const f of files) {
    const nameBytes = encoder.encode(f.name);
    const crc = crc32(f.data);
    const local = concatBytes([
      u32(0x04034b50),
      u16(20),
      u16(0),
      u16(0), // store
      u16(0),
      u16(0),
      u32(crc),
      u32(f.data.length),
      u32(f.data.length),
      u16(nameBytes.length),
      u16(0),
      nameBytes,
      f.data,
    ]);
    locals.push(local);

    const central = concatBytes([
      u32(0x02014b50),
      u16(20),
      u16(20),
      u16(0),
      u16(0),
      u16(0),
      u16(0),
      u32(crc),
      u32(f.data.length),
      u32(f.data.length),
      u16(nameBytes.length),
      u16(0),
      u16(0),
      u16(0),
      u16(0),
      u32(0),
      u32(offset),
      nameBytes,
    ]);
    centrals.push(central);
    offset += local.length;
  }

  const centralDir = concatBytes(centrals);
  const end = concatBytes([
    u32(0x06054b50),
    u16(0),
    u16(0),
    u16(files.length),
    u16(files.length),
    u32(centralDir.length),
    u32(offset),
    u16(0),
  ]);
  return concatBytes([...locals, centralDir, end]);
}

function xmlEscape(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function renderDocx(draft: DraftDocument): Uint8Array {
  const text = renderPlainText(draft);
  const paras = text.split("\n").map((line) => {
    const t = xmlEscape(line.length ? line : " ");
    return `<w:p><w:r><w:t xml:space="preserve">${t}</w:t></w:r></w:p>`;
  });

  const documentXml = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    ${paras.join("\n    ")}
    <w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr>
  </w:body>
</w:document>`;

  const contentTypes = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>`;

  const rels = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>`;

  const encoder = new TextEncoder();
  return zipStore([
    { name: "[Content_Types].xml", data: encoder.encode(contentTypes) },
    { name: "_rels/.rels", data: encoder.encode(rels) },
    { name: "word/document.xml", data: encoder.encode(documentXml) },
  ]);
}

export function downloadBytes(filename: string, data: Uint8Array, mime: string): void {
  // Copy into a plain ArrayBuffer-backed view for Blob typing across TS DOM libs.
  const copy = new Uint8Array(data.byteLength);
  copy.set(data);
  const blob = new Blob([copy.buffer], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

export function safeFilename(title: string, templateId: string, ext: string): string {
  const base = (title || "draft")
    .replace(/[^\w\s-]+/g, "")
    .trim()
    .replace(/\s+/g, "_")
    .slice(0, 48);
  return `${base || "draft"}_${templateId}.${ext}`;
}

/** Attach an AI-assisted span with required G5 fields (for future LLM insert paths). */
export function makeAiSpan(opts: {
  text: string;
  modelHash: string;
  promptHash: string;
  retrievalIds?: string[];
}): AiSpanProvenance {
  return {
    span_id: uid("ai"),
    text: opts.text.slice(0, 500),
    origin: "ai_assisted",
    model_hash: opts.modelHash,
    prompt_hash: opts.promptHash,
    retrieval_ids: opts.retrievalIds || [],
    provenance_record: {
      stableId: uid("inference"),
      propositionClass: "INFERENCE",
      origin: "MODEL_INFERENCE",
      supportState: opts.retrievalIds?.length ? "AMBER" : "UNSUPPORTED",
      title: opts.text.slice(0, 100),
      verification: opts.retrievalIds?.length
        ? "AI-assisted span linked to retrieval identifiers; counsel must inspect and confirm."
        : "AI-assisted span has no retrieval support and must not be treated as verified.",
      supportIds: opts.retrievalIds || [],
      version: `${opts.modelHash}:${opts.promptHash}`,
    },
  };
}

/** B-S4-05: printable PDF from plain lines (Helvetica, WinAnsi-safe: typographic characters are mapped to ASCII). */
export function renderLinesPdf(lines: string[]): Uint8Array {
  const ascii = (s: string) =>
    s
      .replace(/[‘’]/g, "'")
      .replace(/[“”]/g, '"')
      .replace(/[–—]/g, "-")
      .replace(/…/g, "...")
      .replace(/[^\x20-\x7E]/g, "?");
  const wrapped: string[] = [];
  for (const ln of lines) wrapped.push(...wrapLine(ln.length ? ascii(ln) : " ", 90));
  const pages: string[][] = [];
  for (let i = 0; i < wrapped.length; i += 58) pages.push(wrapped.slice(i, i + 58));
  if (!pages.length) pages.push([" "]);
  return buildPdfDocument(pages);
}
