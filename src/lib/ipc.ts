/**
 * Typed shell IPC for CaseForge (B-S1-01+).
 * Shell host methods: shell.ping/version/heavy_job/sidecar_status.
 * B-S1-05/06: docpipe triage + recognition host commands + stdio sidecar.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type SidecarName = "infer-llm" | "infer-speech" | "docpipe";

export type SidecarState = "idle" | "spawned_stub" | "error";

export interface PingResult {
  ok: true;
  pong: "pong";
  server_time_ms: number;
}

export interface VersionResult {
  product: "CaseForge";
  version: string;
  batch: string;
  tauri: string;
  /** Milliseconds from process start to this response (cold-start diagnostic). */
  uptime_ms: number;
  /** Wall-clock process start (unix ms). */
  process_start_unix_ms: number;
}

export interface HeavyJobStartResult {
  job_id: string;
  accepted: true;
}

export interface HeavyJobStatus {
  job_id: string;
  state: "running" | "completed" | "failed" | "unknown";
  progress_percent: number;
  /** Max observed main-thread gap is measured on UI side; backend reports work ms. */
  work_elapsed_ms: number;
  message: string;
}

export interface SidecarStatusEntry {
  name: SidecarName;
  state: SidecarState;
  detail: string;
}

export interface SidecarStatusResult {
  sidecars: SidecarStatusEntry[];
}

export interface HeavyJobProgressEvent {
  job_id: string;
  progress_percent: number;
  message: string;
}

/** True when running inside a Tauri webview. */
export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function shellPing(): Promise<PingResult> {
  return invoke<PingResult>("shell_ping");
}

export async function shellVersion(): Promise<VersionResult> {
  return invoke<VersionResult>("shell_version");
}

export async function shellHeavyJobStart(durationMs = 1500): Promise<HeavyJobStartResult> {
  return invoke<HeavyJobStartResult>("shell_heavy_job_start", { durationMs });
}

export async function shellHeavyJobStatus(jobId: string): Promise<HeavyJobStatus> {
  return invoke<HeavyJobStatus>("shell_heavy_job_status", { jobId });
}

export async function shellSidecarStatus(): Promise<SidecarStatusResult> {
  return invoke<SidecarStatusResult>("shell_sidecar_status");
}

// --- Local infer-llm (offline reasoner) ---

export interface LlmStatus {
  ready: boolean;
  controlled_beta_ready: boolean;
  backend: string;
  backend_version: string;
  model: string;
  model_digest: string;
  quantization: string;
  parameters: string;
  context_length: number;
  classification: string;
  license: string;
  legally_fine_tuned: boolean;
  checkpoint_evidence: boolean;
  prompt_version: string;
  system_prompt_hash: string;
  generation: Record<string, unknown> | null;
  detail: string;
  cloud: boolean;
  product_role: string;
  /** B-S4-00: active model tier ("default" base install | "addon" optional model). */
  tier?: string;
  tier_error?: string | null;
}

export interface LlmTierEntry {
  tier: "default" | "addon";
  model: string;
  parameters: string;
  quantization: string;
  license: string;
  install_policy: string;
}

export interface LlmTiers {
  active: string;
  tiers: LlmTierEntry[];
  auto_download: boolean;
}

export interface LlmMessage {
  role: string;
  content: string;
}

export interface LlmChatRequest {
  user_text: string;
  role?: string | null;
  format?: string | null;
  case_title?: string | null;
  jurisdiction?: string | null;
  user_side?: string | null;
  case_context?: string | null;
  authority_context?: string | null;
  authority_ids?: string[] | null;
  working_set_version?: string | null;
  structured_propositions?: boolean | null;
  output_kind?: PropositionOutputKind | null;
  support_sources?: PropositionSupportSource[] | null;
  permitted_authority_ids?: string[] | null;
  mock_court_schema_v2?: boolean | null;
  messages?: LlmMessage[] | null;
  max_tokens?: number | null;
}

export interface LlmChatResult {
  content: string;
  model: string;
  backend: string;
  elapsed_ms: number;
  role: string;
  format: string;
  disclaimer: string;
  source: string;
  controlled_beta_ready: boolean;
  provenance: Record<string, unknown> | null;
  eval_count?: number | null;
  tokens_per_second?: number | null;
  propositions: PropositionInput[];
  proposition_ledger?: PropositionLedger | null;
}

export async function llmStatus(): Promise<LlmStatus> {
  return invoke<LlmStatus>("llm_status");
}

export async function llmTiers(): Promise<LlmTiers> {
  return invoke<LlmTiers>("llm_tiers");
}

/** Switch model tier for this session; never downloads a model. */
export async function llmSetTier(tier: "default" | "addon"): Promise<LlmStatus> {
  return invoke<LlmStatus>("llm_set_tier", { tier });
}

export async function llmChat(request: LlmChatRequest): Promise<LlmChatResult> {
  return invoke<LlmChatResult>("llm_chat", { request });
}

// --- Host-owned proposition grounding (B-S3-14) ---

export type PropositionClass = "FACTUAL_BASIS" | "LEGAL_BASIS" | "INFERENCE" | "COUNSEL_INPUT" | "UNSUPPORTED_OR_GAP";
export type PropositionOutputKind = "DIGEST" | "DRAFTING" | "ARGUMENT_INTEGRITY" | "MOCK_COURT" | "REASONING";
export interface PropositionSupportClaim { support_id: string; exact_excerpt: string; }
export interface PropositionInput {
  proposition_id: string; class: PropositionClass; text: string; support_ids: string[];
  basis_ids: string[]; support_claims: PropositionSupportClaim[]; quotation?: PropositionSupportClaim | null;
}
export interface PropositionSupportSource {
  support_id: string; kind: "MATTER_SOURCE" | "LEGAL_AUTHORITY" | "COUNSEL_INPUT"; exact_text: string;
  source_id?: string | null; chunk_id?: string | null; authority_id?: string | null; doc_id?: string | null;
  page?: number | null; paragraph?: string | null; trust_level?: string | null; temporal_status?: string | null;
  superseded_status?: string | null; permitted: boolean;
}
export interface VerifiedProposition extends PropositionInput {
  verification_status: CitationStatus; render_action: "TRUSTED" | "LABELLED" | "REPOSITORY_GAP" | "BLOCKED";
  verified_support_ids: string[]; rejected_support_ids: string[];
  quote_offsets: { support_id: string; char_start: number; char_end: number } | null; reason: string;
}
export interface PropositionLedger {
  schema_version: "caseforge.proposition-ledger.v1"; batch: "B-S3-14"; output_kind: PropositionOutputKind;
  propositions: VerifiedProposition[]; rendered_text: string; trusted_count: number; gap_count: number;
  blocked_count: number; all_legal_grounded: boolean; all_quotes_exact: boolean;
}
export async function propositionPolicy(): Promise<Record<string, unknown>> { return invoke("proposition_policy"); }
export async function propositionVerify(request: { output_kind: PropositionOutputKind; propositions: PropositionInput[]; support_sources: PropositionSupportSource[]; permitted_authority_ids: string[] }): Promise<PropositionLedger> { return invoke("proposition_verify", { request }); }
export async function caseLlmSupportSources(caseId: string, maxChars = 24000): Promise<PropositionSupportSource[]> { return invoke("case_llm_support_sources", { caseId, maxChars }); }

export async function onHeavyJobProgress(
  handler: (event: HeavyJobProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<HeavyJobProgressEvent>("heavy_job_progress", (event) => {
    handler(event.payload);
  });
}

// --- Vault (B-S1-02) ---

export interface VaultStatus {
  root_dir: string;
  app_exists: boolean;
  app_unlocked: boolean;
  open_case_ids: string[];
  case_count_registered: number | null;
  crypto: {
    engine: string;
    argon2id_params_version: number;
    memory_kib: number;
    iterations: number;
    parallelism: number;
  };
}

export interface CaseWizardInput {
  cause_title: string;
  court: string;
  jurisdiction: string;
  matter_type: string;
  user_side: string;
  code_era?: string | null;
  language?: string | null;
}

export interface CaseWizardProfile {
  cause_title: string;
  court: string;
  jurisdiction: string;
  matter_type: string;
  user_side: string;
  code_era: string;
  language: string;
}

export interface CaseSummary {
  case_id: string;
  created_at_unix_ms: number;
  label: string;
  cause_title: string;
  court: string;
  jurisdiction: string;
  matter_type: string;
  user_side: string;
  code_era: string;
  language: string;
  is_open: boolean;
}

export interface CaseCreateResult {
  case_id: string;
  path: string;
  profile: CaseWizardProfile;
}

export async function vaultStatus(): Promise<VaultStatus> {
  return invoke<VaultStatus>("vault_status");
}

export async function vaultInitUnlock(passphrase: string): Promise<VaultStatus> {
  return invoke<VaultStatus>("vault_init_unlock", { passphrase });
}

export async function vaultUnlock(passphrase: string): Promise<VaultStatus> {
  return invoke<VaultStatus>("vault_unlock", { passphrase });
}

export async function vaultLock(): Promise<VaultStatus> {
  return invoke<VaultStatus>("vault_lock");
}

/** Wipe local app + case vaults (forgot-password path). Irreversible for local case data. */
export async function vaultResetLocal(): Promise<VaultStatus> {
  return invoke<VaultStatus>("vault_reset_local");
}

export async function caseCreate(
  passphrase: string,
  wizard: CaseWizardInput,
): Promise<CaseCreateResult> {
  return invoke<CaseCreateResult>("case_create", { passphrase, wizard });
}

export async function caseWizardMeta(): Promise<{
  jurisdictions: string[];
  default_language: string;
  fields: string[];
  batch: string;
  prd_ref: string;
}> {
  return invoke("case_wizard_meta");
}

export async function caseSuggestCodeEra(jurisdiction: string): Promise<string> {
  return invoke<string>("case_suggest_code_era", { jurisdiction });
}

export async function caseOpen(caseId: string, passphrase: string): Promise<CaseSummary> {
  return invoke<CaseSummary>("case_open", { caseId, passphrase });
}

export async function caseClose(caseId: string): Promise<void> {
  return invoke<void>("case_close", { caseId });
}

export async function caseList(): Promise<CaseSummary[]> {
  return invoke<CaseSummary[]>("case_list");
}

// --- Document intake (B-S1-04) ---

export interface DocumentMeta {
  doc_id: string;
  original_filename: string;
  type_tag: string;
  language_tag: string;
  source_format: string;
  byte_size: number;
  page_count: number | null;
  per_page_quality_grade: string | null;
  ingested_at_unix_ms: number;
}

export interface DocumentIngestRequest {
  original_filename: string;
  content_base64: string;
  type_tag?: string | null;
  language_tag?: string | null;
}

export async function docIntakeMeta(): Promise<{
  batch: string;
  allowed_extensions: string[];
  type_tag_suggestions: string[];
  default_language_tag: string;
  auto_type: string;
  out_of_scope: string[];
}> {
  return invoke("doc_intake_meta");
}

export async function docList(caseId: string): Promise<DocumentMeta[]> {
  return invoke<DocumentMeta[]>("doc_list", { caseId });
}

export async function docIngest(
  caseId: string,
  request: DocumentIngestRequest,
): Promise<DocumentMeta> {
  return invoke<DocumentMeta>("doc_ingest", { caseId, request });
}

export async function docUpdateTypeTag(
  caseId: string,
  docId: string,
  typeTag: string,
): Promise<DocumentMeta> {
  return invoke<DocumentMeta>("doc_update_type_tag", {
    caseId,
    docId,
    typeTag,
  });
}

/** Read browser File → base64 for doc_ingest (no path permissions needed). */
export function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result;
      if (typeof result !== "string") {
        reject(new Error("Failed to read file"));
        return;
      }
      const comma = result.indexOf(",");
      resolve(comma >= 0 ? result.slice(comma + 1) : result);
    };
    reader.onerror = () => reject(reader.error ?? new Error("FileReader error"));
    reader.readAsDataURL(file);
  });
}

export function isAllowedIntakeFile(name: string): boolean {
  const lower = name.toLowerCase();
  return (
    lower.endsWith(".pdf") ||
    lower.endsWith(".docx") ||
    lower.endsWith(".jpg") ||
    lower.endsWith(".jpeg") ||
    lower.endsWith(".png") ||
    lower.endsWith(".txt")
  );
}

export function suggestTypeTagFromName(name: string): string {
  const lower = name.toLowerCase();
  if (lower.endsWith(".pdf")) return "PDF";
  if (lower.endsWith(".docx")) return "DOCX";
  if (lower.endsWith(".jpg") || lower.endsWith(".jpeg")) return "JPG";
  if (lower.endsWith(".png")) return "PNG";
  if (lower.endsWith(".txt")) return "TXT";
  return "OTHER";
}

export async function docTextPreview(
  caseId: string,
  docId: string,
  maxChars?: number,
): Promise<{
  doc_id: string;
  original_filename: string;
  source_format: string;
  type_tag: string;
  is_binary_stub: boolean;
  text: string;
  char_count: number;
}> {
  return invoke("doc_text_preview", { caseId, docId, maxChars: maxChars ?? null });
}

export async function caseLlmContext(caseId: string, maxChars?: number): Promise<string> {
  return invoke<string>("case_llm_context", { caseId, maxChars: maxChars ?? null });
}

// --- docpipe triage (B-S1-05) ---

export type TriageClass =
  | "typed_english"
  | "other"
  | "mixed"
  | "clean_handwriting"
  | "messy_handwriting"
  | "non_text";

export interface PageTriageRow {
  doc_id: string;
  page_index: number;
  triage_class: TriageClass | string;
  confidence: number;
  classifier_id: string;
  classified_at_unix_ms: number;
  notes: string | null;
}

export interface TriageStartResult {
  job_id: string;
  accepted: boolean;
}

export interface TriageJobStatus {
  job_id: string;
  case_id: string;
  doc_id: string;
  state: "running" | "completed" | "failed" | "unknown";
  progress_percent: number;
  message: string;
  page_count: number | null;
  error: string | null;
}

export interface TriageProgressEvent {
  job_id: string;
  case_id: string;
  doc_id: string;
  progress_percent: number;
  message: string;
}

export async function docpipeMeta(): Promise<{
  batch: string;
  pdf_parser_id: string;
  classifier_id: string;
  recognizer_id?: string;
  triage_classes: string[];
  provisional: boolean;
  methods: string[];
  out_of_scope: string[];
}> {
  return invoke("docpipe_meta");
}

export async function docpipeTriageStart(
  caseId: string,
  docId: string,
): Promise<TriageStartResult> {
  return invoke<TriageStartResult>("docpipe_triage_start", { caseId, docId });
}

export async function docpipeTriageCase(caseId: string): Promise<TriageStartResult> {
  return invoke<TriageStartResult>("docpipe_triage_case", { caseId });
}

export async function docpipeTriageStatus(jobId: string): Promise<TriageJobStatus> {
  return invoke<TriageJobStatus>("docpipe_triage_status", { jobId });
}

export async function docpipePageList(
  caseId: string,
  docId?: string | null,
): Promise<PageTriageRow[]> {
  return invoke<PageTriageRow[]>("docpipe_page_list", {
    caseId,
    docId: docId ?? null,
  });
}

export interface PageSourceRow {
  doc_id: string;
  page_index: number;
  parser_id: string;
  source_kind: "native" | "scanned" | "mixed" | "empty" | "unsupported";
  state: "ready" | "ocr_required" | "cancelled" | "malformed" | "encrypted" | "oversized" | "unsupported";
  width_points: number;
  height_points: number;
  rotation_degrees: number;
  source_sha256: string;
  page_source_json: string;
  parsed_at_unix_ms: number;
}

export async function docpipePageSourceList(caseId: string, docId?: string | null): Promise<PageSourceRow[]> {
  return invoke<PageSourceRow[]>("docpipe_page_source_list", { caseId, docId: docId ?? null });
}

export async function onDocpipeTriageProgress(
  handler: (event: TriageProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<TriageProgressEvent>("docpipe_triage_progress", (event) => {
    handler(event.payload);
  });
}

// --- docpipe recognition (B-S1-06) ---

export interface PageWordRow {
  word_index: number;
  text: string;
  confidence: number;
  raw_engine_confidence: number | null;
  calibrated_confidence: number | null;
  confidence_state: "calibrated" | "raw_only" | "missing" | "counsel_confirmed";
  counsel_confirmed: boolean;
  char_start: number;
  char_end: number;
  low_confidence: boolean;
  force_review: boolean;
  suppress_from_auto: boolean;
  bbox_json: string | null;
}

export interface CriticalFieldRow {
  field_index: number;
  field_type: "name" | "date" | "money" | "citation" | "provision" | "case_number" | "exhibit_label";
  text: string;
  char_start: number;
  char_end: number;
  bbox: { x: number; y: number; w: number; h: number } | null;
  raw_engine_confidence: number | null;
  calibrated_confidence: number | null;
  field_confidence: number | null;
  confidence_state: "calibrated" | "raw_only" | "missing" | "counsel_confirmed";
  syntactically_valid: boolean;
  validation_reasons: string[];
  force_review: boolean;
  suppress_from_auto: boolean;
  counsel_confirmed: boolean;
  validator_id: string;
}

export interface PageTextLayerRow {
  doc_id: string;
  page_index: number;
  recognizer_id: string;
  recognized_at_unix_ms: number;
  full_text: string;
  mean_confidence: number;
  page_quality_warn: boolean;
  notes: string | null;
  calibration_id: string | null;
  critical_fields: CriticalFieldRow[];
  words: PageWordRow[];
}

export async function docpipeRecognizeStart(
  caseId: string,
  docId: string,
): Promise<TriageStartResult> {
  return invoke<TriageStartResult>("docpipe_recognize_start", { caseId, docId });
}

export async function docpipeRecognizeCase(caseId: string): Promise<TriageStartResult> {
  return invoke<TriageStartResult>("docpipe_recognize_case", { caseId });
}

export async function docpipeTextLayerList(
  caseId: string,
  docId?: string | null,
): Promise<PageTextLayerRow[]> {
  return invoke<PageTextLayerRow[]>("docpipe_text_layer_list", {
    caseId,
    docId: docId ?? null,
  });
}

export interface DocpipePageImage {
  page_index: number;
  media_type: string;
  image_base64: string;
  render_size?: { width: number; height: number; dpi?: number };
}

export async function docpipePageImage(
  caseId: string,
  docId: string,
  pageIndex: number,
): Promise<DocpipePageImage> {
  return invoke<DocpipePageImage>("docpipe_page_image", { caseId, docId, pageIndex });
}

export async function onDocpipeRecognizeProgress(
  handler: (event: TriageProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<TriageProgressEvent>("docpipe_recognize_progress", (event) => {
    handler(event.payload);
  });
}

// --- docpipe skeleton / case_graph (B-S1-07) ---

export interface SourcePointer {
  doc_id: string;
  page_index: number;
  kind?: string;
  char_start?: number | null;
  char_end?: number | null;
  quote?: string | null;
  content_hash?: string | null;
  bbox?: number[] | null;
}

export interface GraphProvenance {
  source?: SourcePointer | SourcePointer[];
  sources?: SourcePointer[];
  confidence?: number;
  origin?: "EXTRACTED" | "INFERRED" | "SOURCED_BY_COUNSEL" | "USER_EDIT" | "MIGRATED" | string;
  pipeline_version?: string;
  recorded_at?: string;
  counsel_confirmed?: boolean;
  correction_lineage?: string[];
  reviewed?: boolean;
  notes?: string | null;
}

export interface GraphDateExpression {
  raw: string;
  kind: "exact" | "partial" | "range" | "relative" | "unknown";
  precision: string;
  status: "resolved" | "ambiguous" | "unresolved" | "disputed";
  candidates: string[];
  resolved?: string | null;
  range_start?: string | null;
  range_end?: string | null;
  relative_to?: string | null;
}

export interface CaseGraphNode {
  node_id: string;
  node_type: string;
  label: string;
  status: string;
  attributes: {
    aliases?: string[];
    role?: string | null;
    side?: string | null;
    event_type?: string | null;
    date?: GraphDateExpression | null;
    jurisdiction?: string | null;
    citation?: string | null;
    document_id?: string | null;
    statement_kind?: string | null;
    alternative_group?: string | null;
  };
  provenance: GraphProvenance;
}

export interface CaseGraphEdge {
  edge_id: string;
  edge_type: string;
  from_node_id: string;
  to_node_id: string;
  status: string;
  provenance: GraphProvenance;
}

export interface CaseGraph {
  schema_version: string;
  case_id: string;
  jurisdiction: string;
  generated_at: string;
  pipeline?: {
    extractor_id?: string;
    recognizer_id?: string;
    classifier_id?: string;
    entity_resolver_id?: string;
    relationship_builder_id?: string;
    inference_policy?: string;
    notes?: string | null;
  };
  completeness?: {
    score_0_100?: number;
    open_gaps?: number;
    formula?: string;
  } | null;
  migration?: {
    strategy?: string;
    legacy_projections?: boolean;
    migrated_from?: string | null;
  };
  nodes?: CaseGraphNode[];
  edges?: CaseGraphEdge[];
  parties: Array<{
    party_id: string;
    display_name: string;
    role: string;
    side?: string | null;
    aliases?: string[];
    provenance?: GraphProvenance;
  }>;
  chronology: Array<{
    event_id: string;
    date: string;
    date_precision: string;
    summary: string;
    sort_key: string;
    event_type?: string;
    date_expression?: GraphDateExpression;
    provenance?: GraphProvenance;
  }>;
  statutes: Array<{
    statute_id: string;
    label: string;
    law_type?: string;
    provenance?: GraphProvenance;
  }>;
  evidence: Array<{
    evidence_id: string;
    label: string;
    kind?: string;
    provenance?: GraphProvenance;
  }>;
  procedural_posture: unknown;
  prayer_relief: Array<{
    relief_id: string;
    text: string;
    provenance?: GraphProvenance;
  }>;
  contradictions: Array<{
    contradiction_id: string;
    summary: string;
    left_ref: string;
    right_ref: string;
    left_source: SourcePointer;
    right_source: SourcePointer;
    basis: string;
    status: string;
    severity: string;
    confidence: number;
    provenance: GraphProvenance;
  }>;
  gaps: Array<{
    gap_id: string;
    topic: string;
    concept?: string;
    reason?: string;
    question_prompt?: string | null;
    status: string;
  }>;
  counsel_facts: unknown[];
}

export async function docpipeSkeletonStart(caseId: string): Promise<TriageStartResult> {
  return invoke<TriageStartResult>("docpipe_skeleton_start", { caseId });
}

export async function docpipeCaseGraphGet(caseId: string): Promise<CaseGraph | null> {
  return invoke<CaseGraph | null>("docpipe_case_graph_get", { caseId });
}

export async function onDocpipeSkeletonProgress(
  handler: (event: TriageProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<TriageProgressEvent>("docpipe_skeleton_progress", (event) => {
    handler(event.payload);
  });
}

// --- Review corrections (B-S1-08) ---

export interface ReviewCorrection {
  correction_id: string;
  created_at_unix_ms: number;
  target_kind: string;
  collection?: string | null;
  item_id?: string | null;
  field_name?: string | null;
  doc_id?: string | null;
  page_index?: number | null;
  word_index?: number | null;
  old_value?: string | null;
  new_value?: string | null;
  interaction_count: number;
}

export interface AuthorityConflict {
  conflict_id: string;
  overlay_event_id: string;
  target_key: string;
  target_kind: "graph_item" | "word_span";
  conflict_kind: "MATERIAL_DISAGREEMENT" | "TARGET_DELETED" | "AMBIGUOUS_MATCH";
  generated_value?: string | null;
  counsel_value?: string | null;
  anchor: Record<string, unknown>;
  status: "OPEN" | "KEEP_COUNSEL" | "ACCEPT_GENERATED";
  created_at_unix_ms: number;
  resolved_at_unix_ms?: number | null;
}

export interface CorrectionEvent {
  event_id: string;
  created_at_unix_ms: number;
  event_type: "CORRECT" | "REVERT" | "DELETE" | "RESOLVE_KEEP_COUNSEL" | "RESOLVE_ACCEPT_GENERATED";
  target_kind: "graph_item" | "word_span";
  target_key: string;
  base_value?: string | null;
  counsel_value?: string | null;
  reason: string;
  actor: "COUNSEL";
  supersedes_event_id?: string | null;
  anchor: Record<string, unknown>;
  immutable_hash: string;
  active_overlay: boolean;
}

/** AC-6.4: correction must complete in ≤ 2 interactions (select + save). */
export const REVIEW_MAX_INTERACTIONS = 2;

export async function reviewCorrectGraphItem(
  caseId: string,
  collection: string,
  itemId: string,
  fieldName: string,
  newValue: string,
  interactionCount = 2,
): Promise<CaseGraph> {
  return invoke<CaseGraph>("review_correct_graph_item", {
    caseId,
    collection,
    itemId,
    fieldName,
    newValue,
    interactionCount,
  });
}

export async function reviewCorrectWord(
  caseId: string,
  docId: string,
  pageIndex: number,
  wordIndex: number,
  newText: string,
  interactionCount = 2,
): Promise<PageTextLayerRow> {
  return invoke<PageTextLayerRow>("review_correct_word", {
    caseId,
    docId,
    pageIndex,
    wordIndex,
    newText,
    interactionCount,
  });
}

export async function reviewCorrectionsList(
  caseId: string,
  limit = 50,
): Promise<ReviewCorrection[]> {
  return invoke<ReviewCorrection[]>("review_corrections_list", { caseId, limit });
}

export async function authorityConflictsList(caseId: string, includeResolved = false): Promise<AuthorityConflict[]> {
  return invoke<AuthorityConflict[]>("authority_conflicts_list", { caseId, includeResolved });
}

export async function correctionEventsList(caseId: string, limit = 100): Promise<CorrectionEvent[]> {
  return invoke<CorrectionEvent[]>("correction_events_list", { caseId, limit });
}

export async function correctionRevert(caseId: string, eventId: string, reason: string): Promise<Record<string, unknown>> {
  return invoke<Record<string, unknown>>("correction_revert", { caseId, eventId, reason });
}

export async function authorityConflictResolve(caseId: string, conflictId: string, resolution: "KEEP_COUNSEL" | "ACCEPT_GENERATED", reason: string): Promise<Record<string, unknown>> {
  return invoke<Record<string, unknown>>("authority_conflict_resolve", { caseId, conflictId, resolution, reason });
}

// --- Guided Intake Interview (B-S1-09) ---

export interface InterviewQuestion {
  question_index: number;
  gap_id?: string | null;
  topic: string;
  prompt: string;
  status: "pending" | "answered" | "skipped" | string;
  answer_text?: string | null;
  answered_fact_id?: string | null;
  answered_at_unix_ms?: number | null;
}

export interface InterviewSession {
  session_id: string;
  case_id: string;
  status: "open" | "paused" | "completed" | string;
  created_at_unix_ms: number;
  updated_at_unix_ms: number;
  max_questions: number;
  cursor_index: number;
  completion_percent: number;
  min_completion_percent?: number;
  notes?: string | null;
  questions: InterviewQuestion[];
  batch?: string;
}

export async function interviewOpen(caseId: string): Promise<InterviewSession> {
  return invoke<InterviewSession>("interview_open", { caseId });
}

export async function interviewGet(
  caseId: string,
  sessionId?: string | null,
): Promise<InterviewSession | null> {
  return invoke<InterviewSession | null>("interview_get", {
    caseId,
    sessionId: sessionId ?? null,
  });
}

export async function interviewPause(
  caseId: string,
  sessionId: string,
): Promise<InterviewSession> {
  return invoke<InterviewSession>("interview_pause", { caseId, sessionId });
}

export async function interviewResume(
  caseId: string,
  sessionId: string,
): Promise<InterviewSession> {
  return invoke<InterviewSession>("interview_resume", { caseId, sessionId });
}

export async function interviewAnswer(
  caseId: string,
  sessionId: string,
  questionIndex: number,
  answerText: string,
): Promise<InterviewSession> {
  return invoke<InterviewSession>("interview_answer", {
    caseId,
    sessionId,
    questionIndex,
    answerText,
  });
}

export async function interviewSkip(
  caseId: string,
  sessionId: string,
  questionIndex: number,
): Promise<InterviewSession> {
  return invoke<InterviewSession>("interview_skip", {
    caseId,
    sessionId,
    questionIndex,
  });
}

// --- Overnight batch + resume (B-S1-10) ---

export interface BatchCheckpoint {
  unit_kind: string;
  unit_id: string;
  status: string;
  detail?: string | null;
  completed_at_unix_ms?: number | null;
}

export interface BatchJob {
  job_id: string;
  case_id: string;
  status: string;
  stage: string;
  progress_percent: number;
  message?: string | null;
  total_units: number;
  completed_units: number;
  pending_units?: number;
  page_count_estimate?: number | null;
  created_at_unix_ms: number;
  updated_at_unix_ms: number;
  last_checkpoint_at_unix_ms?: number | null;
  error?: string | null;
  checkpoints?: BatchCheckpoint[];
  ac_6_8?: string;
  ac_3_2_budget?: {
    max_pages: number;
    max_h: number;
    note: string;
  };
  batch?: string;
}

export async function batchStart(
  caseId: string,
  stopAfterUnits?: number | null,
): Promise<TriageStartResult> {
  return invoke<TriageStartResult>("batch_start", {
    caseId,
    stopAfterUnits: stopAfterUnits ?? null,
  });
}

export async function batchResume(caseId: string, jobId: string): Promise<TriageStartResult> {
  return invoke<TriageStartResult>("batch_resume", { caseId, jobId });
}

export async function batchCancel(caseId: string, jobId: string): Promise<BatchJob> {
  return invoke<BatchJob>("batch_cancel", { caseId, jobId });
}

export async function batchStatus(caseId: string, jobId: string): Promise<BatchJob> {
  return invoke<BatchJob>("batch_status", { caseId, jobId });
}

export async function batchLatest(caseId: string): Promise<BatchJob | null> {
  return invoke<BatchJob | null>("batch_latest", { caseId });
}

export async function onDocpipeBatchProgress(
  handler: (event: TriageProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<TriageProgressEvent>("docpipe_batch_progress", (event) => {
    handler(event.payload);
  });
}

// --- Phone Capture Companion (B-S1-11, LAN only) ---

export interface CompanionSession {
  session_id: string;
  case_id: string;
  token: string;
  bind_ip: string;
  port: number;
  pair_url: string;
  qr_text: string;
  status: string;
  uploads_accepted: number;
  lan_only: boolean;
  batch: string;
}

export interface CompanionPolicy {
  batch: string;
  lan_only: boolean;
  allowed_bind: string[];
  forbidden: string[];
  ac: string;
  packet_capture_doc: string;
}

export async function companionPolicy(): Promise<CompanionPolicy> {
  return invoke<CompanionPolicy>("companion_policy");
}

export async function companionStart(
  caseId: string,
  port?: number | null,
): Promise<{ session: CompanionSession; accepted: boolean }> {
  return invoke("companion_start", { caseId, port: port ?? null });
}

export async function companionStop(): Promise<CompanionSession | null> {
  return invoke<CompanionSession | null>("companion_stop");
}

export async function companionStatus(): Promise<CompanionSession | null> {
  return invoke<CompanionSession | null>("companion_status");
}

// --- Knowledge packs (B-S1-12) ---

export interface InstalledPack {
  pack_id: string;
  pack_kind: string;
  jurisdiction: string | null;
  version: string;
  display_name: string;
  install_path: string;
  content_sha256: string;
  headnotes_stripped: boolean;
  signature_ok: boolean;
  installed_at_unix_ms: number;
  isolated: boolean;
  schema_version: string;
  trust_level: "OFFICIAL_VERIFIED" | "CUSTOM" | "DEVELOPMENT_ONLY";
  controlled_beta_eligible: boolean;
  publisher: string | null;
  updated_at: string | null;
  source_status: "CURRENT" | "SUPERSEDED" | "PARTIALLY_SUPERSEDED" | "UNKNOWN" | null;
  supersedes_version: string | null;
  superseded_by_version: string | null;
  editorial_review_status: string | null;
  rights_basis: string | null;
}

export interface PackInstallResult {
  pack: InstalledPack;
  headnote_hits: string[];
  security_report: PackSecurityReport;
}

export interface PackSecurityReport {
  batch: string;
  pack_id: string;
  schema_version: string;
  files_validated: number;
  bytes_validated: number;
  checks: string[];
  transactional_upgrade: boolean;
  protection_verified: boolean;
  active_content_rejected: boolean;
  trust_domain: string;
}

export interface HeadnoteScanResult {
  pack_id: string;
  hits: string[];
  zero_matches: boolean;
  ac: string;
}

export async function packPolicy(): Promise<Record<string, unknown>> {
  return invoke("pack_policy");
}

export async function packList(): Promise<InstalledPack[]> {
  return invoke<InstalledPack[]>("pack_list");
}

export async function packInstall(sourceDir: string): Promise<PackInstallResult> {
  return invoke<PackInstallResult>("pack_install", { sourceDir });
}

export async function packUninstall(packId: string): Promise<void> {
  return invoke("pack_uninstall", { packId });
}

export async function packHeadnoteScan(packId: string): Promise<HeadnoteScanResult> {
  return invoke<HeadnoteScanResult>("pack_headnote_scan", { packId });
}

// --- Retrieval + citations (B-S1-13) ---

export type CitationStatus = "GREEN" | "AMBER" | "RED";

export interface CorpusLocus {
  path: string;
  char_start: number | null;
  char_end: number | null;
  snippet: string | null;
}

export interface CitationRecord {
  schema_version: string;
  citation_id: string;
  query_id: string | null;
  authority_string: string;
  verification_status: CitationStatus;
  match_score: number;
  retrieved: boolean;
  user_confirmed: boolean;
  pack_id: string | null;
  source_id: string | null;
  chunk_id: string | null;
  source_checksum: string | null;
  chunk_checksum: string | null;
  trust_level: string;
  authority_type: string;
  temporal_status: string;
  superseded_status: string;
  content_kind: string;
  corpus_locus: CorpusLocus | null;
  planted_fake: boolean;
  exclude_from_downstream: boolean;
  pipeline: {
    stages: string[];
    index_id: string;
    notes: string | null;
  };
  notes: string | null;
}

export interface RetrievedHit {
  chunk_id: string;
  pack_id: string;
  path: string;
  score: number;
  lexical_score: number;
  semantic_score: number;
  rerank_score: number;
  match_reasons: string[];
  retrieval_mode: "HYBRID" | "HYBRID_DENSE" | "LEXICAL_ONLY";
  corpus_kind: "LEGAL" | "MATTER" | "EXPERIENCE";
  temporal_warning: string | null;
  snippet: string;
  authority_hint: string;
  source_id: string;
  chunk_checksum: string;
  source_checksum: string;
  trust_level: string;
  authority_type: string;
  temporal_status: string;
  superseded_status: string;
  content_kind: string;
}

export interface RetrievalResult {
  query: string;
  query_id: string;
  jurisdiction_filter: string | null;
  stages_completed: string[];
  retrieved: RetrievedHit[];
  citations: CitationRecord[];
  index_id: string;
  index_checksum: string;
  index_state: "REUSED" | "REBUILT";
  retrieval_mode: "HYBRID" | "HYBRID_DENSE" | "LEXICAL_ONLY";
  semantic_engine_id: string;
  semantic_available: boolean;
  lexical_fallback_notice: string | null;
  chunk_count: number;
  batch: string;
  /** B-S1-17 Experience Store patterns injected into corpus. */
  experience_injected?: number;
  experience_top_k?: number;
  repository_gap: boolean;
  repository_gap_reason: string | null;
  repository_gap_required_sources: string[];
}

export async function retrievalPolicy(): Promise<Record<string, unknown>> {
  return invoke("retrieval_policy");
}

export async function retrievalQuery(args: {
  query: string;
  topK?: number | null;
  jurisdiction?: string | null;
  authorityType?: string | null;
  trustLevel?: string | null;
  temporalStatus?: string | null;
  supersededStatus?: string | null;
  extraPackDirs?: string[] | null;
  forceAuthorities?: string[] | null;
  caseId?: string | null;
  userConfirmedRed?: boolean | null;
}): Promise<RetrievalResult> {
  return invoke<RetrievalResult>("retrieval_query", {
    args: {
      query: args.query,
      top_k: args.topK ?? 8,
      jurisdiction: args.jurisdiction ?? null,
      authority_type: args.authorityType ?? null,
      trust_level: args.trustLevel ?? null,
      temporal_status: args.temporalStatus ?? null,
      superseded_status: args.supersededStatus ?? null,
      extra_pack_dirs: args.extraPackDirs ?? null,
      force_authorities: args.forceAuthorities ?? null,
      case_id: args.caseId ?? null,
      user_confirmed_red: args.userConfirmedRed ?? false,
    },
  });
}

export async function retrievalVerify(args: {
  authority: string;
  retrievedJson: string;
  chunksJson?: string | null;
  queryId?: string | null;
  userConfirmed?: boolean | null;
}): Promise<CitationRecord> {
  return invoke<CitationRecord>("retrieval_verify", {
    authority: args.authority,
    retrievedJson: args.retrievedJson,
    chunksJson: args.chunksJson ?? null,
    queryId: args.queryId ?? null,
    userConfirmed: args.userConfirmed ?? false,
  });
}

// --- Matter legal working set (B-S3-12) ---

export type WorkingSetDecision = "PROPOSED" | "INCLUDED" | "EXCLUDED" | "IRRELEVANT" | "REMOVED" | "SUPERSEDED";
export interface RegimeCandidate { regime_id: string; label: string; reason: string; confirmation_required: boolean; }
export interface WorkingSetItem {
  item_id: string; authority_id: string; source_id: string; chunk_id: string; pack_id: string;
  title: string; citation: string; section_label: string; jurisdiction: string; authority_type: string;
  trust_level: string; temporal_status: string; superseded_status: string; source_path: string;
  source_reference: string; source_checksum: string; chunk_checksum: string; exact_text: string;
  decision: WorkingSetDecision; pinned: boolean; controlling: boolean; counsel_rationale: string | null;
  first_added_at_unix_ms: number; updated_at_unix_ms: number; last_seen_index_checksum: string;
}
export interface LegalWorkingSet {
  schema_version: string; case_id: string; version: number; regime_status: "PENDING_CONFIRMATION" | "COUNSEL_CONFIRMED";
  confirmed_regime_id: string | null; regime_candidates: RegimeCandidate[]; items: WorkingSetItem[];
  usable_authority_ids: string[]; updated_at_unix_ms: number;
}
export interface WorkingSetSelection { output_id: string; output_kind: string; case_id: string; working_set_version: number; authority_ids: string[]; selected_at_unix_ms: number; }
export async function workingSetGet(caseId: string): Promise<LegalWorkingSet> { return invoke("working_set_get", { caseId }); }
export async function workingSetSuggest(caseId: string): Promise<LegalWorkingSet> { return invoke("working_set_suggest", { caseId }); }
export async function workingSetAddChunk(caseId: string, chunkId: string): Promise<LegalWorkingSet> { return invoke("working_set_add_chunk", { caseId, chunkId }); }
export async function workingSetUpdateItem(args: { caseId:string; itemId:string; decision:WorkingSetDecision; pinned:boolean; controlling:boolean; counselRationale?:string|null }): Promise<LegalWorkingSet> {
  return invoke("working_set_update_item", { caseId:args.caseId,itemId:args.itemId,decision:args.decision,pinned:args.pinned,controlling:args.controlling,counselRationale:args.counselRationale??null });
}
export async function workingSetConfirmRegime(caseId:string,regimeId:string):Promise<LegalWorkingSet>{ return invoke("working_set_confirm_regime",{caseId,regimeId}); }
export async function workingSetSelectForOutput(caseId:string,outputKind:string,outputId:string):Promise<WorkingSetSelection>{ return invoke("working_set_select_for_output",{caseId,outputKind,outputId}); }

// --- Voice dictation + lexicon (B-S1-16) ---

export interface SpeechStatus {
  ready: boolean;
  production_ready: boolean;
  backend: string;
  stt_backend: string;
  tts_backend: string;
  pipeline_id: string;
  whisper_size: string;
  whisper_status: string;
  primary_language: string;
  language_pinning: boolean;
  tts_roles: string[];
  cloud: boolean;
  network_required: boolean;
  real_audio_required: boolean;
  microphone_capture_required: boolean;
  developer_stub_enabled: boolean;
  retention_policy: string;
  batch: string;
  detail: string;
  product_role: string;
}

export interface SttResult {
  raw_transcript: string;
  staged_transcript: string;
  language: string;
  language_pinned: boolean;
  backend: string;
  pipeline_id: string;
  whisper_size: string;
  cloud: boolean;
  network_used: boolean;
  elapsed_ms: number;
  confidence: number | null;
  input_kind: "REAL_AUDIO" | "DEVELOPER_STUB" | "UNKNOWN";
  production_eligible: boolean;
  audio_sha256: string | null;
  audio_bytes: number;
  audio_retained: boolean;
  notes: string;
  disclaimer: string;
  lexicon_applied: boolean;
}

export interface TtsResult {
  role: string;
  text: string;
  spoken_text: string;
  audio_base64: string | null;
  audio_format: string | null;
  backend: string;
  pipeline_id: string;
  cloud: boolean;
  network_used: boolean;
  elapsed_ms: number;
  notes: string;
  disclaimer: string;
}

export interface LexiconEntry {
  schema_version: string;
  entry_id: string;
  entry_kind: "stt_correction" | "tts_pronunciation_override" | string;
  language: string;
  surface_form: string;
  corrected_form: string | null;
  pronunciation: string | null;
  pattern_type: string;
  source: string;
  active: boolean;
  scope: string;
  notes: string | null;
  created_at_unix_ms: number;
  updated_at_unix_ms: number;
}

export interface SpeechTranscriptCommit {
  commit_id: string;
  case_id: string | null;
  session_id: string | null;
  input_kind: "REAL_AUDIO" | "REAL_AUDIO_CONFIRMED";
  audio_sha256: string;
  raw_transcript: string;
  staged_transcript: string;
  correction_applied: boolean;
  counsel_confirmed: boolean;
  backend: string;
  pipeline_id: string;
  audio_retained: false;
  created_at_unix_ms: number;
}

export interface LexiconUpsertInput {
  entry_id?: string | null;
  entry_kind: string;
  surface_form: string;
  corrected_form?: string | null;
  pronunciation?: string | null;
  pattern_type?: string | null;
  source?: string | null;
  active?: boolean | null;
  notes?: string | null;
}

export interface LexiconExportEnvelope {
  schema_version: string;
  product: string;
  kind: string;
  language: string;
  exported_at_unix_ms: number;
  entry_count: number;
  entries: LexiconEntry[];
}

export async function speechStatus(): Promise<SpeechStatus> {
  return invoke<SpeechStatus>("speech_status");
}

export async function speechWarmup(): Promise<{ready:boolean;backend:string;elapsed_ms:number;network_used:boolean}> {
  return invoke("speech_warmup");
}

export async function speechStt(request: {
  transcript_stub?: string | null;
  audio_base64?: string | null;
  audio_format?: string | null;
  language?: string | null;
}): Promise<SttResult> {
  return invoke<SttResult>("speech_stt", { request });
}

export async function speechTts(request: {
  text: string;
  spoken_text?: string | null;
  role?: string | null;
  language?: string | null;
}): Promise<TtsResult> {
  return invoke<TtsResult>("speech_tts", { request });
}

export async function speechTranscriptCommit(input: {
  case_id?: string | null;
  session_id?: string | null;
  input_kind: "REAL_AUDIO" | "REAL_AUDIO_CONFIRMED";
  audio_sha256: string;
  raw_transcript: string;
  staged_transcript: string;
  correction_applied: boolean;
  counsel_confirmed: boolean;
  backend: string;
  pipeline_id: string;
}): Promise<SpeechTranscriptCommit> {
  return invoke("speech_transcript_commit", { input });
}

export async function lexiconList(kind?: string | null): Promise<LexiconEntry[]> {
  return invoke<LexiconEntry[]>("lexicon_list", { kind: kind ?? null });
}

export async function lexiconUpsert(entry: LexiconUpsertInput): Promise<LexiconEntry> {
  return invoke<LexiconEntry>("lexicon_upsert", { entry });
}

export async function lexiconDelete(entryId: string): Promise<boolean> {
  return invoke<boolean>("lexicon_delete", { entryId });
}

export async function lexiconExport(): Promise<LexiconExportEnvelope> {
  return invoke<LexiconExportEnvelope>("lexicon_export");
}

export async function lexiconImport(envelopeJson: string): Promise<number> {
  return invoke<number>("lexicon_import", { envelopeJson });
}

export async function lexiconApplyStt(text: string): Promise<string> {
  return invoke<string>("lexicon_apply_stt", { text });
}

export async function lexiconPolicy(): Promise<Record<string, unknown>> {
  return invoke("lexicon_policy");
}

// --- Experience Store (B-S1-17) ---

export interface ExperienceRecord {
  schema_version: string;
  record_id: string;
  record_category: string;
  stage: string;
  language: string;
  jurisdiction: string | null;
  case_id: string | null;
  summary: string;
  tags: string[];
  relevance_key: string;
  payload: Record<string, unknown>;
  source: string;
  fleet_status: string;
  active: boolean;
  notes: string | null;
  created_at_unix_ms: number;
  updated_at_unix_ms: number;
}

export interface ExperienceUpsertInput {
  record_id?: string | null;
  record_category: string;
  summary: string;
  tags?: string[] | null;
  payload?: Record<string, unknown> | null;
  case_id?: string | null;
  jurisdiction?: string | null;
  source?: string | null;
  notes?: string | null;
  stage?: string | null;
  fleet_status?: string | null;
  active?: boolean | null;
}

export interface ExperienceHit {
  record_id: string;
  record_category: string;
  summary: string;
  relevance_key: string;
  score: number;
  case_id: string | null;
  jurisdiction: string | null;
  tags: string[];
}

export interface ExperiencePolicy {
  batch: string;
  pipeline_id: string;
  top_k: number;
  fleet_enabled: boolean;
  fleet_network_upload: boolean;
  fleet_requires_explicit_review: boolean;
  stage1_writable: string[];
  stage2_writable?: string[];
  storage: string;
  acs: string[];
}

export async function experiencePolicy(): Promise<ExperiencePolicy> {
  return invoke<ExperiencePolicy>("experience_policy");
}

export async function experienceList(
  category?: string | null,
  limit?: number | null,
): Promise<ExperienceRecord[]> {
  return invoke<ExperienceRecord[]>("experience_list", {
    category: category ?? null,
    limit: limit ?? 100,
  });
}

export async function experienceUpsert(
  entry: ExperienceUpsertInput,
): Promise<ExperienceRecord> {
  return invoke<ExperienceRecord>("experience_upsert", { entry });
}

export async function experienceDelete(recordId: string): Promise<boolean> {
  return invoke<boolean>("experience_delete", { recordId });
}

export async function experienceTopK(args: {
  query: string;
  topK?: number | null;
  jurisdiction?: string | null;
}): Promise<ExperienceHit[]> {
  return invoke<ExperienceHit[]>("experience_top_k", {
    query: args.query,
    topK: args.topK ?? null,
    jurisdiction: args.jurisdiction ?? null,
  });
}

export async function experienceRequestFleetReview(
  recordId: string,
): Promise<ExperienceRecord> {
  return invoke<ExperienceRecord>("experience_request_fleet_review", { recordId });
}

// --- Licensing + air-gap (B-S1-18) ---

export interface FingerprintComponents {
  hostname: string;
  username: string;
  machine_guid: string;
  disk_root: string;
}

export interface LicenceStatus {
  state: string;
  licence_id: string | null;
  tier: string | null;
  expires_at_unix_ms: number | null;
  fingerprint_match_count: number;
  fingerprint_required: number;
  fingerprint_total: number;
  vault_writable: boolean;
  air_gap_enabled: boolean;
  air_gap_kills_networking: boolean;
  case_data_network_path_allowed: boolean;
  user_data_may_be_ransomed: boolean;
  pipeline_id: string;
  batch: string;
  detail: string;
}

export interface ActivateResult {
  status: LicenceStatus;
  fingerprint_match_count: number;
}

export interface NetworkAuditReport {
  ac: string;
  pass: boolean;
  case_data_network_path_allowed: boolean;
  air_gap_enabled: boolean;
  forbidden_modules_for_case_data: string[];
  allowed_non_case_paths: string[];
  findings: string[];
  notes: string;
}

export async function licencePolicy(): Promise<Record<string, unknown>> {
  return invoke("licence_policy");
}

export async function licenceStatus(): Promise<LicenceStatus> {
  return invoke<LicenceStatus>("licence_status");
}

export async function licenceFingerprint(): Promise<FingerprintComponents> {
  return invoke<FingerprintComponents>("licence_fingerprint");
}

export async function licenceActivate(payloadJson: string): Promise<ActivateResult> {
  return invoke<ActivateResult>("licence_activate", { payloadJson });
}

export async function licenceMarkExpired(): Promise<LicenceStatus> {
  return invoke<LicenceStatus>("licence_mark_expired");
}

export async function airGapSet(enabled: boolean): Promise<LicenceStatus> {
  return invoke<LicenceStatus>("air_gap_set", { enabled });
}

export async function networkAudit(): Promise<NetworkAuditReport> {
  return invoke<NetworkAuditReport>("network_audit");
}

// --- Protection (B-S1-19) ---

export interface ProtectionStatus {
  pipeline_id: string;
  batch: string;
  cipher: string;
  effort_budget_percent: number;
  packs_protected: number;
  packs_failed_integrity: number;
  models_dir_exists: boolean;
  models_protected: boolean;
  user_data_may_be_ransomed: boolean;
  key_fingerprint: string;
  foreign_machine_sim: boolean;
  detail: string;
}

export interface IntegrityResult {
  scope_id: string;
  ok: boolean;
  assets_checked: number;
  failures: string[];
  graceful_degrade: boolean;
  user_data_touched: boolean;
  detail: string;
}

export interface ProtectResult {
  scope_id: string;
  assets_protected: number;
  manifest_path: string;
  degraded: boolean;
  detail: string;
}

export interface CopiedFolderAudit {
  ac: string;
  pass: boolean;
  user_vault_opens_with_passphrase: boolean;
  protected_assets_usable_on_foreign_machine: boolean;
  protection_key_machine_bound: boolean;
  findings: string[];
  notes: string;
}

export interface NoRansomAudit {
  ac: string;
  pass: boolean;
  user_data_may_be_ransomed: boolean;
  protection_encrypts_user_case_db: boolean;
  expired_licence_deletes_data: boolean;
  online_kill_switch_for_user_files: boolean;
  findings: string[];
  notes: string;
}

export async function protectionPolicy(): Promise<Record<string, unknown>> {
  return invoke("protection_policy");
}

export async function protectionStatus(): Promise<ProtectionStatus> {
  return invoke<ProtectionStatus>("protection_status");
}

export async function protectionProtectPack(packId: string): Promise<ProtectResult> {
  return invoke<ProtectResult>("protection_protect_pack", { packId });
}

export async function protectionProtectModels(): Promise<ProtectResult> {
  return invoke<ProtectResult>("protection_protect_models");
}

export async function protectionVerifyAll(): Promise<IntegrityResult[]> {
  return invoke<IntegrityResult[]>("protection_verify_all");
}

export async function protectionCopiedFolderAudit(): Promise<CopiedFolderAudit> {
  return invoke<CopiedFolderAudit>("protection_copied_folder_audit");
}

export async function protectionNoRansomAudit(): Promise<NoRansomAudit> {
  return invoke<NoRansomAudit>("protection_no_ransom_audit");
}

// --- Model choreography (B-S1-20) ---

export interface ResidentSlot {
  role: string;
  large: boolean;
  acquired_at_unix_ms: number;
  last_used_unix_ms: number;
  detail: string;
}

export interface BudgetSnapshot {
  cold_start_max_s: number;
  process_uptime_ms: number;
  cold_start_ok: boolean | null;
  idle_ram_max_mb: number;
  process_rss_mb: number | null;
  idle_ram_ok: boolean | null;
  total_rss_max_gb: number;
  process_rss_gb: number | null;
  rss_ok: boolean | null;
  ui_freeze_max_ms: number;
  digest_max_min: number;
  measurement_notes: string;
}

export interface ChoreographyStatus {
  pipeline_id: string;
  batch: string;
  tier: string;
  live_simulation_allowed: boolean;
  resident: ResidentSlot[];
  large_resident_count: number;
  max_large_resident: number;
  primary_and_vision_concurrent: boolean;
  hard_fail_primary_vision: boolean;
  chip_label: string;
  chip_tone: string;
  idle_unload_after_s: number;
  budgets: BudgetSnapshot;
  detail: string;
}

export async function choreographyPolicy(): Promise<Record<string, unknown>> {
  return invoke("choreography_policy");
}

export async function choreographyStatus(): Promise<ChoreographyStatus> {
  return invoke<ChoreographyStatus>("choreography_status");
}

export async function choreographyAcquire(
  role: string,
  detail?: string | null,
): Promise<{ ok: boolean; role: string; status: ChoreographyStatus; error: string | null }> {
  return invoke("choreography_acquire", { role, detail: detail ?? null });
}

export async function choreographyRelease(role: string): Promise<ChoreographyStatus> {
  return invoke<ChoreographyStatus>("choreography_release", { role });
}

export async function choreographyUnloadAll(): Promise<ChoreographyStatus> {
  return invoke<ChoreographyStatus>("choreography_unload_all");
}

export async function choreographyMetrics(): Promise<Record<string, unknown>> {
  return invoke("choreography_metrics");
}

// --- Guardrails G1–G6 (B-S1-21) ---

export interface GuardViolation {
  guardrail: string;
  error_code: string;
  matched: string;
  phase: string;
}

export interface GuardResult {
  allowed: boolean;
  content: string;
  violations: GuardViolation[];
  refused: boolean;
  provenance: {
    model_hash: string;
    prompt_hash: string;
    retrieval_ids: string[];
    guardrail_pipeline: string;
  };
  pipeline_id: string;
  batch: string;
}

export async function guardrailsPolicy(): Promise<Record<string, unknown>> {
  return invoke("guardrails_policy");
}

export async function guardrailsCheckInput(text: string): Promise<GuardResult> {
  return invoke<GuardResult>("guardrails_check_input", { text });
}

export async function guardrailsCheckOutput(args: {
  userText: string;
  modelText: string;
  modelId?: string | null;
}): Promise<GuardResult> {
  return invoke<GuardResult>("guardrails_check_output", {
    userText: args.userText,
    modelText: args.modelText,
    modelId: args.modelId ?? null,
  });
}

export async function guardrailsRedteamStage1(
  promptsJson: string,
): Promise<Record<string, unknown>> {
  return invoke("guardrails_redteam_stage1", { promptsJson });
}

/** Full Stage-2 AC-15.1 red-team suite (>=200 prompts). */
export async function guardrailsRedteamFull(
  promptsJson: string,
): Promise<Record<string, unknown>> {
  return invoke("guardrails_redteam_full", { promptsJson });
}

// --- Ship / hardware gate (B-S1-22) ---

export interface HardwareGate {
  allowed: boolean;
  tier: string;
  bits: number;
  total_ram_gb: number | null;
  is_32bit: boolean;
  refuse_reason: string | null;
  messaging: string;
  batch: string;
  lite_note: string;
  full_note: string;
}

export async function hardwareGate(): Promise<HardwareGate> {
  return invoke<HardwareGate>("hardware_gate");
}

export async function hardwarePolicy(): Promise<Record<string, unknown>> {
  return invoke("hardware_policy");
}

export async function shipStage1Meta(): Promise<Record<string, unknown>> {
  return invoke("ship_stage1_meta");
}

export async function shipStage2Meta(): Promise<Record<string, unknown>> {
  return invoke("ship_stage2_meta");
}

// --- B-S2-01 Argument Integrity Engine (deterministic; no LLM) ---

export interface IntegrityLinks {
  Fact: number;
  Evidence: number;
  Law: number;
  Exposure: number;
}

export interface MissingLinkRecord {
  limb_id: string;
  link: string;
  score: number;
  threshold: number;
  rank: number;
  inputs_cited: string[];
  planted_gap_id?: string | null;
  description?: string | null;
}

export interface ScoredLimbReport {
  node_id: string;
  label: string;
  links: IntegrityLinks;
  min_link: string;
  min_score: number;
  VI: number;
  expected_VI?: number | null;
  expected_VI_match?: boolean | null;
}

export interface IntegrityScoreReport {
  scorer_id: string;
  formula: string;
  schema_version: string;
  case_id: string;
  threshold: number;
  limbs: ScoredLimbReport[];
  missing_links_panel: MissingLinkRecord[];
  weakest_limb?: string | null;
  weakest_VI?: number | null;
  planted_gaps_in_top_n: string[];
  recompute_ms: number;
  llm_used: boolean;
}

export interface IntegrityBuildResult {
  tree: Record<string, unknown>;
  score: IntegrityScoreReport;
}

export async function integrityPolicy(): Promise<Record<string, unknown>> {
  return invoke("integrity_policy");
}

export async function integrityScoreTree(
  tree: Record<string, unknown>,
): Promise<IntegrityScoreReport> {
  return invoke<IntegrityScoreReport>("integrity_score_tree", { tree });
}

export async function integrityScoreCase(caseId: string): Promise<IntegrityScoreReport> {
  return invoke<IntegrityScoreReport>("integrity_score_case", { caseId });
}

export async function integrityBuildFromGraph(
  caseId: string,
  persist = true,
): Promise<IntegrityBuildResult> {
  return invoke<IntegrityBuildResult>("integrity_build_from_graph", {
    caseId,
    persist,
  });
}

export async function argumentTreeGet(
  caseId: string,
): Promise<Record<string, unknown> | null> {
  return invoke<Record<string, unknown> | null>("argument_tree_get", { caseId });
}

export async function argumentTreePut(
  caseId: string,
  tree: Record<string, unknown>,
): Promise<IntegrityScoreReport> {
  return invoke<IntegrityScoreReport>("argument_tree_put", { caseId, tree });
}

// --- B-S2-02 Mock Court text formats ---

export interface CourtFormatMeta {
  order: number;
  id: string;
  label: string;
  unlocked_by_default_through: number;
}

export interface SessionTurn {
  schema_version: string;
  turn_id: string;
  session_id: string;
  case_id: string;
  format: string;
  role: string;
  stage: string;
  user_text?: string | null;
  assistant_text?: string | null;
  pipeline_stages_completed: string[];
  retrieval_ids: string[];
  model_hash: string;
  prompt_hash: string;
  elapsed_ms: number;
  sources: string[];
  disclaimer: string;
  format_order?: number | null;
  script_id?: string | null;
  token_estimate?: number | null;
}

export interface ScriptedSessionReport {
  pipeline_id: string;
  format: string;
  format_order: number;
  script_id: string;
  case_id: string;
  turns: SessionTurn[];
  pass: boolean;
  reasons: string[];
  max_turn_ms: number;
  turn_budget_s: number;
  used_raw_document: boolean;
  llm_required: boolean;
}

export interface MockCourtAuthority {
  authority_id: string;
  source_id: string;
  chunk_id: string;
  pack_id: string;
  title: string;
  citation: string;
  section_label: string;
  jurisdiction: string;
  authority_type: string;
  trust_level: string;
  temporal_status: string;
  superseded_status: string;
  source_reference: string;
  source_checksum: string;
  chunk_checksum: string;
  exact_text: string;
}

export interface MockCourtReadiness {
  schema_version: string;
  case_id: string;
  ready: boolean;
  graph_available: boolean;
  graph_version: string;
  graph_counsel_reviewed: boolean;
  working_set_version: number;
  regime_status: string;
  confirmed_regime_id?: string | null;
  usable_authority_count: number;
  authorities: MockCourtAuthority[];
  blocking_reasons: string[];
  warnings: string[];
}

/** B-S4-00b: deterministic feedback on counsel's answer to the previous challenge (contracts/answer_feedback.schema.json). */
export interface AnswerFeedbackItem {
  kind: "DATE" | "AMOUNT" | "AUTHORITY" | "OPEN_CONTRADICTION" | "UNUSED_AUTHORITY";
  status:
    | "SUPPORTED_BY_RECORD"
    | "CONFLICTS_WITH_RECORD"
    | "NOT_FOUND_IN_RECORD"
    | "VERIFIED_IN_WORKING_SET"
    | "NOT_IN_WORKING_SET"
    | "AVAILABLE_NOT_USED";
  value: string | null;
  message: string;
  record_ref: string | null;
  locator: { doc_id?: string; page?: number } | null;
}

export interface AnswerFeedback {
  schema_version: "1.0.0";
  engine_version: string;
  session_id: string;
  engagement: "ADDRESSED" | "PARTIAL" | "NOT_ADDRESSED" | "NO_PRIOR_CHALLENGE";
  engagement_terms_matched: number;
  engagement_terms_total: number;
  items: AnswerFeedbackItem[];
  generated_by: "deterministic";
  model_used: false;
  outcome_prediction: false;
}

export interface GroundedMockCourtTurn {
  schema_version: string;
  turn: SessionTurn;
  answer_feedback?: AnswerFeedback | null;
  graph_version: string;
  working_set_version: number;
  retrieval_query: string;
  retrieval_cache_key: string;
  cache_hit: boolean;
  authorities: MockCourtAuthority[];
  generation_mode: "LOCAL_LLM_VERIFIED" | "DEGRADED_DETERMINISTIC";
  degraded_reason?: string | null;
  model: string;
  backend: string;
  model_digest: string;
  prompt_version: string;
  prompt_hash: string;
  system_prompt_hash: string;
  model_elapsed_ms: number;
  tokens_per_second?: number | null;
  schema_repair_used: boolean;
  structured_turn?: Record<string, unknown> | null;
  proposition_ledger?: PropositionLedger | null;
}

export type DebriefEpistemicStatus = "OBSERVED" | "INFERENCE" | "EXPLICIT_GAP";
export type PreparationTaskStatus = "PROPOSED" | "ACCEPTED" | "DISMISSED" | "COMPLETED";
export interface DebriefSourceRef { source_kind:string; stable_id:string; locator:string; title:string; excerpt?:string|null; verification_status:"GREEN"|"AMBER"|"RED"|"UNSUPPORTED"; }
export interface DebriefFinding { finding_id:string; category:string; title:string; detail:string; turn_ids:string[]; source_refs:DebriefSourceRef[]; epistemic_status:DebriefEpistemicStatus; severity:"LOW"|"MEDIUM"|"HIGH"; }
export interface PreparationTask { task_id:string; debrief_id:string; title:string; detail:string; status:PreparationTaskStatus; turn_ids:string[]; finding_ids:string[]; source_refs:DebriefSourceRef[]; epistemic_status:DebriefEpistemicStatus; usefulness_score:number; counsel_note?:string|null; updated_at_unix_ms:number; }
export interface MockCourtDebriefV2 { schema_version:"caseforge.mock-court-debrief.v2"; debrief_id:string; case_id:string; session_id:string; format:string; created_at_unix_ms:number; turns_completed:number; findings:DebriefFinding[]; tasks:PreparationTask[]; comparison:{previous_debrief_id?:string|null;observed_change:string;supported_rate_previous?:number|null;supported_rate_current:number;outcome_prediction:false};source_fingerprint:string;rubric_version:string;generic_tasks_rejected:number;disclaimer:string; }
export interface DebriefTurnInput { turn_id:string;counsel_text:string;assistant_text:string;proposition_ledger?:PropositionLedger|null;authorities:MockCourtAuthority[]; }
export async function mockCourtDebriefV2Generate(request:{case_id:string;session_id:string;format:string;turns:DebriefTurnInput[]}):Promise<MockCourtDebriefV2>{return invoke("mock_court_debrief_v2_generate",{request});}
export async function preparationTasksList(caseId:string):Promise<PreparationTask[]>{return invoke("preparation_tasks_list",{caseId});}
export async function preparationTaskUpdate(opts:{caseId:string;taskId:string;status:PreparationTaskStatus;title?:string|null;counselNote?:string|null}):Promise<PreparationTask[]>{return invoke("preparation_task_update",{caseId:opts.caseId,taskId:opts.taskId,status:opts.status,title:opts.title??null,counselNote:opts.counselNote??null});}

export async function mockCourtPolicy(): Promise<Record<string, unknown>> {
  return invoke("mock_court_policy");
}

export async function mockCourtFormats(): Promise<CourtFormatMeta[]> {
  return invoke<CourtFormatMeta[]>("mock_court_formats");
}

export async function mockCourtTurn(request: {
  session_id: string;
  case_id: string;
  format: string;
  role?: string | null;
  user_text: string;
  retrieval_ids?: string[] | null;
  retrieval_snippets?: string[] | null;
  case_graph?: Record<string, unknown> | null;
  script_id?: string | null;
  deterministic_only?: boolean | null;
}): Promise<SessionTurn> {
  return invoke<SessionTurn>("mock_court_turn", { request });
}

export async function mockCourtRunScript(args: {
  format: string;
  case_id?: string | null;
  case_graph?: Record<string, unknown> | null;
  script_id?: string | null;
  user_lines?: string[] | null;
  retrieval_ids?: string[] | null;
  retrieval_snippets?: string[] | null;
}): Promise<ScriptedSessionReport> {
  return invoke<ScriptedSessionReport>("mock_court_run_script", { args });
}

export async function mockCourtTurnForCase(opts: {
  sessionId: string;
  caseId: string;
  format: string;
  role?: string | null;
  userText: string;
  expectedGraphVersion?: string | null;
  expectedWorkingSetVersion?: number | null;
  acceptVersionChange?: boolean | null;
  benchArchetypeId?: string | null;
  benchSliders?: BenchSliderSet | null;
  judgeName?: string | null;
  namedJudgeId?: string | null;
}): Promise<GroundedMockCourtTurn> {
  return invoke<GroundedMockCourtTurn>("mock_court_turn_for_case", {
    request: {
      session_id: opts.sessionId,
      case_id: opts.caseId,
      format: opts.format,
      role: opts.role ?? null,
      user_text: opts.userText,
      expected_graph_version: opts.expectedGraphVersion ?? null,
      expected_working_set_version: opts.expectedWorkingSetVersion ?? null,
      accept_version_change: opts.acceptVersionChange ?? false,
      bench_archetype_id: opts.benchArchetypeId ?? null,
      bench_sliders: opts.benchSliders ?? null,
      judge_name: opts.judgeName ?? null,
      named_judge_id: opts.namedJudgeId ?? null,
    },
  });
}

export async function mockCourtRetrievalReadiness(caseId: string): Promise<MockCourtReadiness> {
  return invoke<MockCourtReadiness>("mock_court_retrieval_readiness", { caseId });
}

// --- B-S2-03 Per-Case Adaptive Bench ---

export interface BenchSliderSet {
  tempo: number;
  formality: number;
  opposition_aggression: number;
  technicality: number;
  interruption: number;
}

export interface ArchetypeMeta {
  id: string;
  label: string;
  baseline: BenchSliderSet;
}

export interface CaseOrderSignals {
  jurisdiction: string;
  relief_count: number;
  open_gap_count: number;
  evidence_count: number;
  statute_count: number;
  chronology_density: number;
  has_procedural_posture: boolean;
  urgency_hint: boolean;
  suggested_archetype: string;
  fingerprint: string;
}

export interface BenchProfile {
  pipeline_id: string;
  schema_version: string;
  case_id: string;
  archetype_id: string;
  archetype_label: string;
  sliders: BenchSliderSet;
  signals: CaseOrderSignals;
  behaviour_brief: string;
  named_judge_profiles: boolean;
  sources: string[];
}

export async function adaptiveBenchPolicy(): Promise<Record<string, unknown>> {
  return invoke("adaptive_bench_policy");
}

export async function adaptiveBenchArchetypes(): Promise<ArchetypeMeta[]> {
  return invoke<ArchetypeMeta[]>("adaptive_bench_archetypes");
}

export async function adaptiveBenchResolve(request: {
  case_id: string;
  case_graph?: Record<string, unknown> | null;
  archetype_id?: string | null;
  sliders?: BenchSliderSet | null;
  judge_name?: string | null;
  named_judge_id?: string | null;
}): Promise<BenchProfile> {
  return invoke<BenchProfile>("adaptive_bench_resolve", { request });
}

export async function adaptiveBenchResolveCase(opts: {
  caseId: string;
  archetypeId?: string | null;
  sliders?: BenchSliderSet | null;
  judgeName?: string | null;
  namedJudgeId?: string | null;
}): Promise<BenchProfile> {
  return invoke<BenchProfile>("adaptive_bench_resolve_case", {
    caseId: opts.caseId,
    archetypeId: opts.archetypeId ?? null,
    sliders: opts.sliders ?? null,
    judgeName: opts.judgeName ?? null,
    namedJudgeId: opts.namedJudgeId ?? null,
  });
}

// --- B-S2-04 Voice-in-court + structured debrief ---

export interface VoiceGate {
  allowed: boolean;
  tier: string;
  live_simulation_allowed: boolean;
  honesty_notice?: string | null;
  refuse_code?: string | null;
  refuse_reason?: string | null;
  messaging: string;
}

export interface VoiceTurnResult {
  pipeline_id: string;
  session_id: string;
  case_id: string;
  format: string;
  turn_index: number;
  counsel_raw: string;
  counsel_staged: string;
  lexicon_applied: boolean;
  court_turn: SessionTurn;
  tts_role: string;
  tts_spoken_text: string;
  stt_network_used: boolean;
  tts_network_used: boolean;
  network_used: boolean;
  cloud: boolean;
  elapsed_ms: number;
  within_ac33: boolean;
  language: string;
  disclaimer: string;
}

export interface DebriefReport {
  pipeline_id: string;
  session_id: string;
  case_id: string;
  format: string;
  turns_completed: number;
  strengths: string[];
  weaknesses: string[];
  missing_links_focus: string[];
  recommended_actions: string[];
  lexicon_hits: number;
  network_used_any: boolean;
  max_turn_ms: number;
  within_ac33: boolean;
  tier: string;
  live_simulation_allowed: boolean;
  fingerprint: string;
  disclaimer: string;
}

export async function voiceCourtPolicy(): Promise<Record<string, unknown>> {
  return invoke("voice_court_policy");
}

export async function voiceCourtGate(forceTier?: string | null): Promise<VoiceGate> {
  return invoke<VoiceGate>("voice_court_gate", { forceTier: forceTier ?? null });
}

export async function voiceCourtTurn(request: Record<string, unknown>): Promise<VoiceTurnResult> {
  return invoke<VoiceTurnResult>("voice_court_turn", { request });
}

export async function voiceCourtRunScript(args: {
  format: string;
  case_id?: string | null;
  case_graph?: Record<string, unknown> | null;
  script_id?: string | null;
  user_lines?: string[] | null;
  lexicon_pairs?: [string, string][] | null;
  force_tier?: string | null;
}): Promise<Record<string, unknown>> {
  return invoke("voice_court_run_script", { args });
}

export async function voiceCourtTurnForCase(opts: {
  sessionId: string;
  caseId: string;
  format: string;
  transcriptStub?: string | null;
  counselText?: string | null;
  confirmedRawTranscript?: string | null;
  confirmedAudioSha256?: string | null;
  productionEligible?: boolean | null;
  courtRole?: string | null;
  ttsRole?: string | null;
  turnIndex?: number | null;
  benchArchetypeId?: string | null;
  benchSliders?: BenchSliderSet | null;
  forceTier?: string | null;
  language?: string | null;
}): Promise<{ gate: VoiceGate; turn: VoiceTurnResult; stt: SttResult; tts: TtsResult; generation: GroundedMockCourtTurn; retrieval:{authorities:MockCourtAuthority[]} }> {
  return invoke("voice_court_turn_for_case", {
    sessionId: opts.sessionId,
    caseId: opts.caseId,
    format: opts.format,
    transcriptStub: opts.transcriptStub ?? null,
    counselText: opts.counselText ?? null,
    confirmedRawTranscript: opts.confirmedRawTranscript ?? null,
    confirmedAudioSha256: opts.confirmedAudioSha256 ?? null,
    productionEligible: opts.productionEligible ?? null,
    courtRole: opts.courtRole ?? null,
    ttsRole: opts.ttsRole ?? null,
    turnIndex: opts.turnIndex ?? null,
    benchArchetypeId: opts.benchArchetypeId ?? null,
    benchSliders: opts.benchSliders ?? null,
    forceTier: opts.forceTier ?? null,
    language: opts.language ?? null,
  });
}

export async function voiceCourtDebrief(opts: {
  sessionId: string;
  caseId: string;
  format: string;
  turnsJson: string;
  forceTier?: string | null;
}): Promise<DebriefReport> {
  return invoke<DebriefReport>("voice_court_debrief", {
    sessionId: opts.sessionId,
    caseId: opts.caseId,
    format: opts.format,
    turnsJson: opts.turnsJson,
    forceTier: opts.forceTier ?? null,
  });
}

// --- B-S2-05 Bench Preparation Mode ---

export interface PrepTurnResult {
  pipeline_id: string;
  session_id: string;
  case_id: string;
  side_id: string;
  side_label: string;
  text: string;
  non_decision_maker: boolean;
  notice: string;
  g1_clean: boolean;
  refused: boolean;
  violations: string[];
  elapsed_ms: number;
  model_hash: string;
  prompt_hash: string;
  retrieval_ids: string[];
  sources: string[];
  disclaimer: string;
}

export interface PrepSessionReport {
  pipeline_id: string;
  session_id: string;
  case_id: string;
  format: string;
  non_decision_maker: boolean;
  notice: string;
  sides: { side_id: string; label: string; stance: string }[];
  exchanges: {
    turn_index: number;
    side_id: string;
    side_label: string;
    text: string;
    g1_clean: boolean;
    violations: string[];
    refused: boolean;
    elapsed_ms: number;
    sources: string[];
  }[];
  g1_violations_count: number;
  pass: boolean;
  reasons: string[];
  redteam_hits: number;
  disclaimer: string;
}

export async function benchPrepPolicy(): Promise<Record<string, unknown>> {
  return invoke("bench_prep_policy");
}

export async function benchPrepTurn(request: {
  session_id: string;
  case_id: string;
  format?: string | null;
  case_graph?: Record<string, unknown> | null;
  side?: string | null;
  user_cue?: string | null;
  retrieval_ids?: string[] | null;
  redteam?: boolean | null;
}): Promise<PrepTurnResult> {
  return invoke<PrepTurnResult>("bench_prep_turn", { request });
}

export async function benchPrepTurnForCase(opts: {
  sessionId: string;
  caseId: string;
  format?: string | null;
  side?: string | null;
  userCue?: string | null;
  redteam?: boolean | null;
}): Promise<PrepTurnResult> {
  return invoke<PrepTurnResult>("bench_prep_turn_for_case", {
    sessionId: opts.sessionId,
    caseId: opts.caseId,
    format: opts.format ?? null,
    side: opts.side ?? null,
    userCue: opts.userCue ?? null,
    redteam: opts.redteam ?? null,
  });
}

export async function benchPrepRunScript(args: {
  case_id?: string | null;
  case_graph?: Record<string, unknown> | null;
  format?: string | null;
  cues?: string[] | null;
  redteam?: boolean | null;
}): Promise<PrepSessionReport> {
  return invoke<PrepSessionReport>("bench_prep_run_script", { args });
}

export async function benchPrepRedteam(opts?: {
  caseId?: string | null;
  caseGraph?: Record<string, unknown> | null;
}): Promise<PrepSessionReport> {
  return invoke<PrepSessionReport>("bench_prep_redteam", {
    caseId: opts?.caseId ?? null,
    caseGraph: opts?.caseGraph ?? null,
  });
}

// --- B-S4-04 Citation & Quote Audit (contracts/audit_report.schema.json) ---

export type AuditResult =
  | "VERIFIED"
  | "VERIFIED_WITH_NOTE"
  | "MISQUOTED"
  | "NOT_IN_CORPUS"
  | "SUPERSEDED"
  | "NOT_APPLICABLE"
  | "CUSTOM_SOURCE";

export interface AuditLegalLocator {
  pack_id: string;
  source_id: string;
  chunk_id: string;
  char_start: number;
  char_end: number;
  page_index?: number | null;
  paragraph?: string | null;
  excerpt: string;
}

export interface AuditItem {
  item_id: string;
  kind: "CITATION" | "QUOTATION";
  citation_kind?: string | null;
  extraction_rule_id?: string | null;
  draft_char_start: number;
  draft_char_end: number;
  draft_text: string;
  attributed_citation_item_id?: string | null;
  result: AuditResult;
  resolved: AuditLegalLocator | null;
  pinpoint: { claimed: string | null; status: "MATCH" | "MISMATCH" | "NOT_CHECKED" | "NO_PINPOINT" };
  notes: { note_kind: string; text: string; ref_id?: string | null }[];
  misquote: { nearest: AuditLegalLocator; diff_ops: { op: "EQUAL" | "INSERT" | "DELETE"; text: string }[] } | null;
  proposition_ids: string[];
}

export interface AuditReport {
  schema_version: "1.0.0";
  audit_id: string;
  pipeline_id: "caseforge.audit.v1";
  generated_at: string;
  case_id?: string | null;
  subject: { subject_kind: string; sha256: string; filename: string; format: string };
  pack_snapshot: { pack_id: string; version: string; currency_date: string | null; trust_level: string }[];
  model_fingerprint: string | null;
  items: AuditItem[];
  counts: Record<AuditResult, number>;
}

export async function auditDraft(text: string, filename?: string | null, caseId?: string | null): Promise<AuditReport> {
  return invoke<AuditReport>("audit_draft", { text, filename: filename ?? null, caseId: caseId ?? null });
}

// --- B-S4-06 Opposing Filing Scan ---
export type FilingKind = "PLEADING" | "BRIEF" | "REPLY" | "WRITTEN_SUBMISSION" | "AFFIDAVIT" | "OTHER";
export interface FilingLocator { doc_id: string; page_index: number | null; char_start: number; char_end: number; excerpt: string }
export interface RecordConflict {
  conflict_id: string;
  assertion_id: string;
  record_fact_node_id: string;
  filing_locator: FilingLocator;
  record_locator: FilingLocator;
  record_origin: "SOURCED_BY_COUNSEL" | "EXTRACTED" | "USER_EDIT";
  basis: "DATE" | "AMOUNT" | "NAME" | "PRESENCE" | "OTHER";
  proposition_ids: string[];
}
export interface ExternalFiling {
  schema_version: "1.0.0";
  doc_id: string;
  type_tag: "EXTERNAL_FILING";
  filing_party_id: string;
  filing_kind: FilingKind;
  filed_on: string | null;
  trust: "UNTRUSTED_ADVERSE";
  assertions: { assertion_id: string; polarity: "ASSERTS" | "DENIES"; text: string; locator: FilingLocator; record_support_cited: boolean; graph_edge_id: string | null }[];
  scan: {
    audit_report_id: string;
    record_conflicts: RecordConflict[];
    unsupported_assertion_ids: string[];
    new_arguments: { argument_id: string; summary: string; locator: FilingLocator; matched_tree_node_id: string | null }[];
  };
}
export interface OppositionScanResult { pipeline_id: string; stale_graph: boolean; filing: ExternalFiling; audit: AuditReport }

export async function oppositionScan(args: { caseId: string; filingPartyId: string; filingKind: FilingKind; docId?: string | null; text?: string | null }): Promise<OppositionScanResult> {
  return invoke<OppositionScanResult>("opposition_scan", {
    caseId: args.caseId, filingPartyId: args.filingPartyId, filingKind: args.filingKind, docId: args.docId ?? null, text: args.text ?? null,
  });
}

// --- B-S4-05 Audit Certificate ---
export interface CertificateVerifyOutcome {
  valid: boolean;
  key_id: string | null;
  checks: { name: string; passed: boolean; detail: string }[];
  note: string;
}

export async function auditCertificateIssue(opts: {
  report: AuditReport;
  draftText: string;
  disclosureText?: string | null;
  disclosureConfirmed?: boolean;
}): Promise<Record<string, any>> {
  return invoke("audit_certificate_issue", {
    report: opts.report,
    draftText: opts.draftText,
    disclosureText: opts.disclosureText ?? null,
    disclosureConfirmed: opts.disclosureConfirmed ?? false,
  });
}

export async function auditCertificateVerify(certificateJson: string, draftText?: string | null): Promise<CertificateVerifyOutcome> {
  return invoke<CertificateVerifyOutcome>("audit_certificate_verify", { certificateJson, draftText: draftText ?? null });
}

// --- B-S4-07/08 Witness Statement Matrix and Impeachment Kit ---
export type LocatorKind = "PAGE_SPAN" | "PAGE_LINE";
export interface WitnessRegisterEntry {
  doc_id: string;
  witness_name: string;
  relation?: string | null;
  statement_kind: string;
  recorded_on: string | null;
  locator_kind: LocatorKind;
  sequence_override?: number | null;
  origin: "PROPOSED" | "COUNSEL_CONFIRMED";
}
export type WitnessDecision =
  | { action: "MERGE"; witness_id: string; into_witness_id: string }
  | { action: "SPLIT"; statement_id: string; new_display_name: string };
export interface WitnessRegisterState {
  register: WitnessRegisterEntry[];
  decisions: { key: string; decision: WitnessDecision; active: boolean }[];
  statement_kinds: { kind: string; label: string; jurisdiction: string; applies: boolean }[];
  vocabulary: { pack_id: string; table_id: string };
}
export interface MatterLocator {
  doc_id: string;
  page_index: number | null;
  char_start: number;
  char_end: number;
  line_start?: number | null;
  line_end?: number | null;
  excerpt: string;
}
export interface MatrixWitness {
  witness_id: string;
  display_name: string;
  alias_node_ids: string[];
  resolution: "PROPOSED" | "COUNSEL_CONFIRMED";
  statements: { statement_id: string; statement_kind: string; recorded_on: string | null; doc_id: string; locator_kind: LocatorKind; sequence_index: number }[];
  topics: { topic_id: string; topic_kind: string; label: string }[];
  cells: { topic_id: string; statement_id: string; assertions: { assertion_id: string; text: string; locator: MatterLocator }[] }[];
}
export interface WitnessMatrix {
  schema_version: "1.0.0";
  matrix_id: string;
  case_id: string;
  built_at: string;
  graph_version: string;
  statement_kind_vocabulary: { pack_id: string | null; table_id: string | null };
  witnesses: MatrixWitness[];
}
export interface ImpeachSide { statement_id: string; quote: string | null; locator: MatterLocator | null; absence_note?: string | null }
export interface ImpeachFinding {
  finding_id: string;
  finding_kind: "CONTRADICTION" | "OMISSION";
  detection: "DETERMINISTIC" | "MODEL_VERIFIED";
  dimension: string;
  topic_id: string;
  earlier: ImpeachSide;
  later: ImpeachSide;
  proposition_ids: string[];
  counsel_judgement_required: boolean;
  confrontation_order: number;
  status: "PROPOSED" | "COUNSEL_ACCEPTED" | "COUNSEL_REJECTED";
}
export interface ImpeachmentKit {
  schema_version: "1.0.0";
  kit_id: string;
  case_id: string;
  witness_id: string;
  matrix_id: string;
  generated_at: string;
  model_fingerprint: string | null;
  findings: ImpeachFinding[];
  export_footer_id: "caseforge.export_footer.v1";
}
export interface WitnessBuildResult {
  pipeline_id: string;
  matrix_pipeline_id: string;
  matrix: WitnessMatrix;
  kits: ImpeachmentKit[];
  stale_decisions: string[];
  unreadable: { doc_id: string; error: string }[];
}

export async function witnessPropose(caseId: string, docIds?: string[] | null): Promise<WitnessRegisterEntry[]> {
  return invoke<WitnessRegisterEntry[]>("witness_propose", { caseId, docIds: docIds ?? null });
}
export async function witnessRegisterList(caseId: string): Promise<WitnessRegisterState> {
  return invoke<WitnessRegisterState>("witness_register_list", { caseId });
}
export async function witnessRegisterSave(caseId: string, entry: WitnessRegisterEntry): Promise<WitnessRegisterEntry> {
  return invoke<WitnessRegisterEntry>("witness_register_save", { caseId, entry });
}
export async function witnessRegisterRemove(caseId: string, docId: string): Promise<void> {
  return invoke<void>("witness_register_remove", { caseId, docId });
}
export async function witnessDecisionAdd(caseId: string, decision: WitnessDecision): Promise<string> {
  return invoke<string>("witness_decision_add", { caseId, decision });
}
export async function witnessDecisionRevoke(caseId: string, key: string): Promise<void> {
  return invoke<void>("witness_decision_revoke", { caseId, key });
}
export async function witnessMatrixBuild(caseId: string): Promise<WitnessBuildResult> {
  return invoke<WitnessBuildResult>("witness_matrix_build", { caseId });
}
export async function impeachFindingStatus(caseId: string, findingId: string, status: ImpeachFinding["status"]): Promise<void> {
  return invoke<void>("impeach_finding_status", { caseId, findingId, status });
}
export async function impeachModelScan(caseId: string, witnessId: string): Promise<{ rendered: number; discarded: { candidate: string; reason: string }[]; model_fingerprint: string }> {
  return invoke("impeach_model_scan", { caseId, witnessId });
}

// --- B-S4-03 rule tables / B-S4-09 Code Bridge ---
export type DateKind = "OFFENCE_DATE" | "INSTITUTION_DATE" | "FILING_DATE" | "CAUSE_OF_ACTION_DATE" | "EVENT_DATE";
export interface KeyDate { kind: DateKind; date: string; locator: Record<string, unknown> }
export interface StatuteMapping {
  schema_version: "1.0.0";
  reference_id: string;
  context: "CASE_GRAPH" | "DRAFT" | "RETRIEVED_AUTHORITY" | "AUDIT_ITEM" | "EXTERNAL_FILING";
  context_ref_id: string | null;
  ref_text: string;
  code: string;
  provision: string;
  equivalence: "IDENTICAL" | "RENUMBERED" | "MODIFIED" | "NEW" | "OMITTED" | "UNKNOWN";
  counterpart: { direction: "SUCCESSOR" | "PREDECESSOR"; code: string; provision: string } | null;
  rule_ids: string[];
  governing: {
    status: "DETERMINED" | "NEEDS_COUNSEL_INPUT" | "UNKNOWN";
    governing_code: string | null;
    basis_date_kind: DateKind | null;
    basis_date: string | null;
    basis_locator: Record<string, unknown> | null;
    transition_rule_id: string | null;
    interview_question_id?: string | null;
  };
  side_by_side: { old: { pack_id: string; source_id: string; chunk_id: string; excerpt: string }; new: { pack_id: string; source_id: string; chunk_id: string; excerpt: string } } | null;
}
export interface CodeBridgeResult {
  pipeline_id: string;
  annotations: { annotation: StatuteMapping; trust: "SIGNED_OFF" | "DEVELOPMENT_ONLY" | "NONE"; char_start: number }[];
  succession_rows: number;
  signed_rows: number;
  rejected_tables: string[];
  key_dates: KeyDate[];
}
export async function codeBridgeAnnotate(caseId: string, text?: string | null, context?: "DRAFT" | "EXTERNAL_FILING"): Promise<CodeBridgeResult> {
  return invoke<CodeBridgeResult>("code_bridge_annotate", { caseId, text: text ?? null, context: context ?? null });
}
export async function codeBridgeSetKeyDate(caseId: string, kind: DateKind, date: string | null, note?: string | null): Promise<void> {
  return invoke<void>("code_bridge_set_key_date", { caseId, kind, date, note: note ?? null });
}
export async function ruleTablesStatus(): Promise<{ tables: { table_id: string; table_kind: string; jurisdiction: string; pack_id: string; rows: number; signed_rows: number }[]; rejected: string[] }> {
  return invoke("rule_tables_status");
}

// --- B-S4-28 step-by-step reasoning with best-of-n verification ---
export type ReasoningDepth = "QUICK" | "CAREFUL" | "THOROUGH";
export interface ReasoningCandidate { variant: number; points: number; trusted: number; labelled: number; red: number; conclusion_ok: boolean; agreement: number; repair: boolean; }
export interface ReasoningResult {
  schema_version: "caseforge.reasoning-run.v1"; depth: ReasoningDepth; candidates_requested: number; candidates_run: number;
  chosen_variant: number | null; status: "VERIFIED" | "PARTIAL" | "NO_VERIFIED_ANSWER"; verified_answer: boolean; agreement: number;
  stopped_early: boolean; stopped_for_time: boolean; repair_attempted: boolean; repair_accepted: boolean;
  candidates: ReasoningCandidate[]; discarded: { variant: number; reason: string }[];
  ledger: PropositionLedger | null; rendered_text: string; question: string;
  model: { model: string; backend: string; digest: string }; elapsed_ms: number; disclaimer: string;
}
export async function reasoningRun(caseId: string, question: string, depth: ReasoningDepth): Promise<ReasoningResult> {
  return invoke<ReasoningResult>("reasoning_run", { caseId, question, depth });
}

// --- B-S4-11 Deadline & Limitation Watch ---
export interface DeadlineRecord {
  schema_version: "1.0.0"; deadline_id: string; case_id: string; label: string; rule_id: string;
  authority: { pack_id: string; source_id: string; chunk_id: string; provision_label?: string | null; rule_trust_state?: string };
  trigger: { event_node_id: string; event_type: string; date: string | null; date_precision: string; date_range: { from: string; to: string } | null; locator: { doc_id: string; excerpt: string }; origin: string };
  computation: {
    period: { value: number; unit: "DAYS" | "MONTHS" | "YEARS" }; exclude_trigger_day: boolean;
    exclusions_applied: { exclusion_id: string; rule_id: string; days_excluded: number; triggering_fact_node_id: string; locator: { doc_id: string; excerpt: string } }[];
    calendar_rule_ids: string[]; rolled_forward_from: string | null;
  };
  result: { kind: "DATE" | "RANGE"; date: string | null; range_from: string | null; range_to: string | null; uncertainty_reasons: string[] };
  verify_label: "VERIFY"; guardrail: "G9"; computed_at: string; engine_version: "caseforge.deadlines.v1";
}
export interface DeadlineRuleInfo {
  rule_id: string; label: string; trigger_event_type: string; period: { value: number; unit: "DAYS" | "MONTHS" | "YEARS" };
  exclude_trigger_day: boolean; roll_forward: boolean; calendar_table_id: string | null; trust_state: string; provision: string | null;
  exclusions: { exclusion_id: string; fact_type: string }[];
}
export interface DeadlinesState {
  engine_version: string; jurisdiction: string; rules: DeadlineRuleInfo[];
  events: { event_id: string; summary: string; date: string; precision: string; date_range: [string, string] | null; other_dates: string[]; doc_id: string; excerpt: string }[];
  assignments: { rule_id: string; event_id: string }[];
  facts: { fact_type: string; days: number; confirmed: boolean; doc_id: string; excerpt: string }[];
  records: DeadlineRecord[]; errors: { rule_id: string; event_id: string; error: string }[];
  rule_problems: string[]; rejected_tables: string[]; calendar_tables: string[]; unsigned_rules: boolean;
}
export async function deadlinesCompute(caseId: string): Promise<DeadlinesState> { return invoke<DeadlinesState>("deadlines_compute", { caseId }); }
export async function deadlinesAssign(caseId: string, ruleId: string, eventId: string): Promise<void> { return invoke<void>("deadlines_assign", { caseId, ruleId, eventId }); }
export async function deadlinesUnassign(caseId: string, ruleId: string, eventId: string): Promise<void> { return invoke<void>("deadlines_unassign", { caseId, ruleId, eventId }); }
export async function deadlinesFactSave(caseId: string, factType: string, days: number, confirmed: boolean, docId: string, excerpt: string): Promise<void> { return invoke<void>("deadlines_fact_save", { caseId, factType, days, confirmed, docId, excerpt }); }
export async function deadlinesFactRemove(caseId: string, factType: string): Promise<void> { return invoke<void>("deadlines_fact_remove", { caseId, factType }); }
export async function deadlinesExportIcs(caseId: string): Promise<string> { return invoke<string>("deadlines_export_ics", { caseId }); }

// --- B-S4-10 Admissibility & Foundation Gaps ---
export interface FoundationElement { element_id: string; label: string; status: "PRESENT" | "ABSENT" | "UNKNOWN"; locators: { doc_id: string; excerpt: string }[]; authority: { pack_id: string; source_id: string; chunk_id: string; provision_label?: string | null } }
export interface FoundationCheck {
  schema_version: "1.0.0"; check_id: string; evidence_node_id: string; evidence_type: string; evidence_type_status: "PROPOSED" | "COUNSEL_CONFIRMED";
  rule_id: string; elements: FoundationElement[]; scorer_version: "caseforge.integrity.v1" | "caseforge.integrity.v2";
}
export interface FoundationResult {
  checks: FoundationCheck[]; gaps: Record<string, unknown>[]; panel: Record<string, unknown>[];
  evidence: { evidence_id: string; label: string; evidence_type: string | null; type_status: "PROPOSED" | "COUNSEL_CONFIRMED" | null }[];
  types: { evidence_type: string; label: string }[]; docs: { doc_id: string; filename: string; type_tag: string }[];
  as_of: string[]; rules_installed: number; unsigned_rules: boolean; rule_problems: string[]; rejected_tables: string[]; note: string;
}
export async function foundationCheck(caseId: string, v1Panel: Record<string, unknown>[]): Promise<FoundationResult> { return invoke<FoundationResult>("foundation_check", { caseId, v1Panel }); }
export async function foundationSetType(caseId: string, evidenceId: string, evidenceType: string | null): Promise<void> { return invoke<void>("foundation_set_type", { caseId, evidenceId, evidenceType }); }
export async function foundationLink(caseId: string, evidenceId: string, elementId: string, docId: string | null): Promise<void> { return invoke<void>("foundation_link", { caseId, evidenceId, elementId, docId }); }
