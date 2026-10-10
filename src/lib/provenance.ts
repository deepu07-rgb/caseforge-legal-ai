import type {
  CitationRecord,
  GraphProvenance,
  PropositionClass,
  RetrievedHit,
  SourcePointer,
  VerifiedProposition,
} from "./ipc";

export type ProvenanceState = "GREEN" | "AMBER" | "RED" | "UNSUPPORTED";
export type ProvenanceOrigin =
  | "MATTER_EVIDENCE"
  | "LEGAL_AUTHORITY"
  | "COUNSEL_INPUT"
  | "MODEL_INFERENCE"
  | "UNSUPPORTED_GAP";

export interface ProvenanceRecord {
  stableId: string;
  propositionClass: PropositionClass;
  origin: ProvenanceOrigin;
  supportState: ProvenanceState;
  title: string;
  verification: string;
  version?: string | null;
  docId?: string | null;
  pageIndex?: number | null;
  charStart?: number | null;
  charEnd?: number | null;
  bbox?: number[] | null;
  packId?: string | null;
  sourceId?: string | null;
  chunkId?: string | null;
  section?: string | null;
  paragraph?: string | null;
  excerpt?: string | null;
  supportIds?: string[];
  basisIds?: string[];
}

const firstSource = (p?: GraphProvenance | null): SourcePointer | null => {
  const source = p?.source;
  if (Array.isArray(source)) return source[0] ?? null;
  return source ?? p?.sources?.[0] ?? null;
};

export function graphProvenance(
  stableId: string,
  title: string,
  provenance?: GraphProvenance | null,
  /** Why this item is held for review (low confidence, a failed check). A held item has a source but is not verified, so it is never GREEN. */
  held?: string | null,
): ProvenanceRecord {
  const source = firstSource(provenance);
  const counsel = Boolean(provenance?.counsel_confirmed) || ["SOURCED_BY_COUNSEL", "USER_EDIT"].includes(provenance?.origin || "");
  const inferred = provenance?.origin === "INFERRED";
  const hasPointer = Boolean(source?.doc_id) && source?.page_index != null;
  const translated = (provenance?.origin as string | undefined) === "TRANSLATED";
  const heldBack = !counsel && (translated || Boolean(held));
  return {
    stableId,
    propositionClass: counsel ? "COUNSEL_INPUT" : inferred ? "INFERENCE" : hasPointer ? "FACTUAL_BASIS" : "UNSUPPORTED_OR_GAP",
    origin: counsel ? "COUNSEL_INPUT" : inferred ? "MODEL_INFERENCE" : hasPointer ? "MATTER_EVIDENCE" : "UNSUPPORTED_GAP",
    supportState: counsel || inferred || (heldBack && hasPointer) ? "AMBER" : hasPointer ? "GREEN" : "UNSUPPORTED",
    title,
    verification: heldBack && hasPointer
      ? translated
        ? "Read from a machine translation - wording not verified. A page pointer is recorded; check the original."
        : `Held for review: ${held}. A page pointer is recorded, but this value is not verified.`
      : counsel
      ? "Counsel-authored or counsel-confirmed; distinct from documentary evidence."
      : inferred
        ? "Model/extractor inference; inspect its stated bases before use."
        : hasPointer
          ? "Stable matter document and page pointer recorded."
          : "No stable source pointer is attached; do not treat as verified evidence.",
    version: provenance?.pipeline_version ?? null,
    docId: source?.doc_id ?? null,
    pageIndex: source?.page_index ?? null,
    charStart: source?.char_start ?? null,
    charEnd: source?.char_end ?? null,
    bbox: source?.bbox ?? null,
    excerpt: source?.quote ?? null,
  };
}

export function verifiedPropositionProvenance(
  proposition: VerifiedProposition,
  source?: Partial<ProvenanceRecord>,
): ProvenanceRecord {
  const cls = proposition.class;
  const origin: ProvenanceOrigin = cls === "LEGAL_BASIS" ? "LEGAL_AUTHORITY"
    : cls === "FACTUAL_BASIS" ? "MATTER_EVIDENCE"
      : cls === "COUNSEL_INPUT" ? "COUNSEL_INPUT"
        : cls === "INFERENCE" ? "MODEL_INFERENCE" : "UNSUPPORTED_GAP";
  const state: ProvenanceState = proposition.verification_status === "GREEN" ? "GREEN"
    : proposition.verification_status === "AMBER" ? "AMBER"
      : proposition.render_action === "REPOSITORY_GAP" ? "UNSUPPORTED" : "RED";
  return {
    stableId: proposition.proposition_id,
    propositionClass: cls,
    origin,
    supportState: state,
    title: proposition.text,
    verification: proposition.reason,
    supportIds: proposition.verified_support_ids,
    basisIds: proposition.basis_ids,
    ...source,
  };
}

export function retrievalHitProvenance(hit: RetrievedHit): ProvenanceRecord {
  const green = hit.trust_level === "OFFICIAL_VERIFIED" && hit.temporal_status === "APPLICABLE" && hit.superseded_status === "CURRENT";
  return {
    stableId: hit.chunk_id,
    propositionClass: "LEGAL_BASIS",
    origin: "LEGAL_AUTHORITY",
    supportState: green ? "GREEN" : "AMBER",
    title: hit.authority_hint || hit.path,
    verification: green ? "Retrieved official, applicable, current legal passage." : "Retrieved passage needs trust, temporal, or supersession review.",
    packId: hit.pack_id,
    sourceId: hit.source_id,
    chunkId: hit.chunk_id,
    section: hit.path,
    excerpt: hit.snippet,
    version: hit.chunk_checksum,
  };
}

export function citationProvenance(citation: CitationRecord): ProvenanceRecord {
  const state: ProvenanceState = citation.verification_status === "GREEN" ? "GREEN" : citation.verification_status === "AMBER" ? "AMBER" : "RED";
  return {
    stableId: citation.citation_id,
    propositionClass: "LEGAL_BASIS",
    origin: state === "RED" ? "UNSUPPORTED_GAP" : "LEGAL_AUTHORITY",
    supportState: state,
    title: citation.authority_string,
    verification: citation.notes || (state === "GREEN" ? "Citation verified against the retrieved legal repository." : "Not verified authority; excluded unless explicitly confirmed."),
    packId: citation.pack_id,
    sourceId: citation.source_id,
    chunkId: citation.chunk_id,
    version: citation.chunk_checksum,
  };
}

export function provenanceLocator(p: ProvenanceRecord): string {
  if (p.docId && p.pageIndex != null) {
    const span = p.charStart != null && p.charEnd != null ? `&chars=${p.charStart}-${p.charEnd}` : "";
    return `matter://${encodeURIComponent(p.docId)}?page=${p.pageIndex + 1}${span}`;
  }
  if (p.packId || p.sourceId || p.chunkId) {
    return `legal://${encodeURIComponent(p.packId || "repository")}/${encodeURIComponent(p.sourceId || "source")}/${encodeURIComponent(p.chunkId || "passage")}`;
  }
  return `gap://${encodeURIComponent(p.stableId)}`;
}

export function provenanceExportLine(p: ProvenanceRecord): string {
  return `[PROVENANCE ${p.supportState} | ${p.propositionClass} | ${p.origin}] id=${p.stableId} locator=${provenanceLocator(p)} version=${p.version || "unversioned"} support=${(p.supportIds || []).join(",") || "none"}`;
}
