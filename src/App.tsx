import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import ArgumentBuilderView from "./components/ArgumentBuilderView";
import ArgumentTreeView from "./components/ArgumentTreeView";
import DemoArgumentPanel from "./components/DemoArgumentPanel";
import MockCourtView from "./components/MockCourtView";
import BatchView from "./components/BatchView";
import CompanionView from "./components/CompanionView";
import BundlePanel from "./components/BundlePanel";
import InterviewView from "./components/InterviewView";
import DigestView from "./components/DigestView";
import PacksView from "./components/PacksView";
import RetrievalView from "./components/RetrievalView";
import WorkingLibraryView from "./components/WorkingLibraryView";
import ReviewView from "./components/ReviewView";
import VoiceView from "./components/VoiceView";
import ExperienceView from "./components/ExperienceView";
import LicenceView from "./components/LicenceView";
import ProtectionView from "./components/ProtectionView";
import GuardrailsView from "./components/GuardrailsView";
import WorkflowJourney from "./components/WorkflowJourney";
import OppositionView from "./components/OppositionView";
import WitnessView from "./components/WitnessView";
import DeadlinesView from "./components/DeadlinesView";
import IssueMatrixView from "./components/IssueMatrixView";
import ChambersView from "./components/ChambersView";
import CauseListView from "./components/CauseListView";
import ScaffoldView from "./components/ScaffoldView";
import { editionStatus, type EditionStatus } from "./lib/ipc";
import { ModelTierSelector } from "./components/ModelTierSelector";
import {
  deriveLawyerWorkflow,
  type WorkflowStage,
  type WorkflowStageId,
} from "./lib/lawyerWorkflow";
import { DEMO_SEEDS, textToBase64, type DemoSeedId } from "./lib/demoSeeds";
import {
  emptyWizard,
  JURISDICTIONS,
  suggestCodeEraLocal,
  validateWizard,
  type CaseWizardForm,
} from "./lib/caseWizard";
import {
  caseClose,
  caseCreate,
  caseList,
  caseOpen,
  docIngest,
  docList,
  docUpdateTypeTag,
  docpipeCaseGraphGet,
  docpipePageList,
  docpipePageImage,
  docpipePageSourceList,
  docpipeRecognizeCase,
  docpipeRecognizeStart,
  docpipeSkeletonStart,
  docpipeTextLayerList,
  docpipeTriageCase,
  docpipeTriageStart,
  docpipeTriageStatus,
  fileToBase64,
  isAllowedIntakeFile,
  isTauriRuntime,
  onDocpipeRecognizeProgress,
  onDocpipeSkeletonProgress,
  onDocpipeTriageProgress,
  batchCancel,
  batchLatest,
  batchResume,
  batchStart,
  batchStatus,
  companionPolicy,
  companionStart,
  companionStatus,
  companionStop,
  packHeadnoteScan,
  packInstall,
  packList,
  packPolicy,
  packUninstall,
  retrievalPolicy,
  retrievalQuery,
  interviewAnswer,
  interviewGet,
  interviewOpen,
  interviewPause,
  interviewResume,
  interviewSkip,
  onDocpipeBatchProgress,
  reviewCorrectGraphItem,
  reviewCorrectWord,
  llmStatus,
  choreographyStatus,
  choreographyUnloadAll,
  shellHeavyJobStart,
  shellPing,
  shellSidecarStatus,
  shellVersion,
  suggestTypeTagFromName,
  vaultInitUnlock,
  vaultLock,
  vaultResetLocal,
  vaultStatus,
  vaultUnlock,
  hardwareGate,
  argumentTreeGet,
  mockCourtRetrievalReadiness,
  preparationTasksList,
  type BatchJob,
  type CaseGraph,
  type CaseSummary,
  type ChoreographyStatus,
  type HardwareGate,
  type CompanionPolicy,
  type CompanionSession,
  type DocumentMeta,
  type HeadnoteScanResult,
  type InstalledPack,
  type PackSecurityReport,
  type InterviewSession,
  type LlmStatus,
  type PageTextLayerRow,
  type PageTriageRow,
  type PageSourceRow,
  type RetrievalResult,
  type SidecarStatusResult,
  type TriageJobStatus,
  type VaultStatus,
  type VersionResult,
  type MockCourtReadiness,
  type PreparationTask,
} from "./lib/ipc";

/** AC-3.5 budget (ms) — heavy work must stay off the UI thread. */
const UI_FREEZE_BUDGET_MS = 200;

const TYPE_TAG_OPTIONS = ["PDF", "DOCX", "JPG", "PNG", "TXT", "OTHER"] as const;
const TRIAGE_CLASSES = [
  "typed_english",
  "clean_handwriting",
  "messy_handwriting",
  "mixed",
  "other",
  "non_text",
] as const;

type NavId =
  | "home"
  | "cases"
  | "new-case"
  | "workspace"
  | "builder"
  | "integrity"
  | "digest"
  | "voice"
  | "experience"
  | "licence"
  | "protection"
  | "guardrails"
  | "court"
  | "opposition"
  | "witnesses"
  | "deadlines"
  | "issues"
  | "chambers"
  | "causelist"
  | "scaffold"
  | "library"
  | "practice"
  | "settings";

type CaseTab =
  | "overview"
  | "documents"
  | "triage"
  | "text"
  | "skeleton"
  | "review"
  | "interview"
  | "batch"
  | "capture"
  | "share";

const NAV: { id: NavId; label: string; section?: string; badge?: string }[] = [
  { id: "home", label: "Preparation Journey", section: "Prepare" },
  { id: "cases", label: "Matters" },
  { id: "new-case", label: "New case" },
  { id: "workspace", label: "Case Material & Review" },
  { id: "builder", label: "Argument Builder" },
  { id: "integrity", label: "Argument Analysis" },
  { id: "digest", label: "Case Digest" },
  { id: "voice", label: "Voice" },
  { id: "experience", label: "Experience" },
  { id: "court", label: "Mock Court" },
  { id: "opposition", label: "Opponent's Filing" },
  { id: "witnesses", label: "Witnesses" },
  { id: "deadlines", label: "Deadlines" },
  { id: "chambers", label: "Chambers" },
  { id: "causelist", label: "Cause List" },
  { id: "scaffold", label: "Judgment" },
  { id: "issues", label: "Issue Matrix" },
  { id: "practice", label: "Integrity lab" },
  { id: "library", label: "Legal Authorities", section: "Library" },
  { id: "licence", label: "Licence", section: "App" },
  { id: "protection", label: "Protection" },
  { id: "guardrails", label: "Guardrails" },
  { id: "settings", label: "System" },
];
const PRIMARY_NAV_IDS:NavId[]=["home","cases","workspace","digest","library","integrity","court","opposition","witnesses","deadlines","chambers","causelist","scaffold","issues"];
const ADVANCED_NAV_IDS:NavId[]=["builder","voice","experience","practice","licence","protection","guardrails","settings"];
const PRIMARY_NAV=NAV.filter(item=>PRIMARY_NAV_IDS.includes(item.id));
const ADVANCED_NAV=NAV.filter(item=>ADVANCED_NAV_IDS.includes(item.id));

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function titleFor(nav: NavId): { title: string; sub: string } {
  switch (nav) {
    case "home":
      return { title: "Preparation Journey", sub: "The next safe step for the active matter" };
    case "cases":
      return {
        title: "Matters",
        sub: "Open or create an encrypted legal matter",
      };
    case "new-case":
      return {
        title: "Cases",
        sub: "Encrypted vault matters · jurisdiction is required and immutable",
      };
    case "workspace":
      return { title: "Case Material & Review", sub: "Add records, check readability, and verify what CaseForge understood" };
    case "builder":
      return {
        title: "Argument Builder",
        sub: "Local LLM drafts argument map from case file text",
      };
    case "integrity":
      return {
        title: "Argument Analysis",
        sub: "Review vulnerabilities, contradictions, and missing support",
      };
    case "digest":
      return {
        title: "Case Digest",
        sub: "CASEBOOK · CHAMBER · DIGEST · DRAFTING · preparation completeness (G6)",
      };
    case "voice":
      return {
        title: "Voice & lexicon",
        sub: "STT staging · TTS roles · self-correcting lexicon (English primary)",
      };
    case "experience":
      return {
        title: "Experience Store",
        sub: "Local encrypted patterns · top-K retrieval injection · fleet off by default",
      };
    case "licence":
      return {
        title: "Licence & air-gap",
        sub: "Offline Ed25519 activation · EXPIRED read-only · AC-13.1 network audit",
      };
    case "protection":
      return {
        title: "Asset protection",
        sub: "AES-256-GCM packs/models · integrity · never ransom user data (AC-14)",
      };
    case "guardrails":
      return {
        title: "Guardrails G1–G6",
        sub: "No verdicts · no citation bypass · refuse fabrication · footer · provenance · prep language",
      };
    case "court":
      return { title: "Mock Court", sub: "Argue · bench prep · adversary drill" };
    case "opposition":
      return { title: "Opponent's Filing", sub: "Check their citations, record conflicts and new arguments · never added to your facts" };
    case "witnesses":
      return { title: "Witnesses", sub: "Statements lined up by topic · contradictions and omissions with exact quotes · impeachment kit" };
    case "deadlines":
      return { title: "Deadlines", sub: "Limitation and procedural dates computed from the rule, the start event and the court calendar · always to verify" };
    case "scaffold":
      return { title: "Judgment", sub: "Filled from the record where it can be · analysis, findings and the operative order are written only by the judge" };
    case "causelist":
      return { title: "Cause List", sub: "One page per listed matter · every line cites an order or document, or says it was not found in the record" };
    case "chambers":
      return { title: "Chambers", sub: "Record only · every statement tied to the record or installed law · no analysis, findings or orders" };
    case "issues":
      return { title: "Issue Matrix", sub: "Each accepted issue side by side · what is agreed, disputed and unanswered · no ranking, no scoring, no suggested answer" };
    case "library":
      return { title: "Legal Authorities", sub: "Confirm the applicable law and source-backed working set" };
    case "practice":
      return { title: "Argument Integrity lab", sub: "Synthetic easy / medium / hard drills" };
    case "settings":
      return { title: "System", sub: "Vault, sidecars, and runtime" };
    default:
      return { title: "CaseForge", sub: "" };
  }
}

function formatWhen(iso?: string | null): string {
  if (!iso) return "—";
  try {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return "—";
    const diff = Date.now() - d.getTime();
    const mins = Math.floor(diff / 60000);
    if (mins < 60) return `${Math.max(mins, 0)}m ago`;
    const hours = Math.floor(mins / 60);
    if (hours < 48) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
  } catch {
    return "—";
  }
}

export default function App() {
  const tauri = isTauriRuntime();
  const [nav, setNav] = useState<NavId>("home");
  const [edition, setEdition] = useState<EditionStatus | null>(null);
  const [caseTab, setCaseTab] = useState<CaseTab>("overview");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [passphrase, setPassphrase] = useState("");
  const [passphraseConfirm, setPassphraseConfirm] = useState("");
  const [showPass, setShowPass] = useState(false);
  const [vault, setVault] = useState<VaultStatus | null>(null);
  const [version, setVersion] = useState<VersionResult | null>(null);
  const [sidecars, setSidecars] = useState<SidecarStatusResult | null>(null);
  const [llm, setLlm] = useState<LlmStatus | null>(null);
  const [cases, setCases] = useState<CaseSummary[]>([]);
  const [vaultBusy, setVaultBusy] = useState(false);
  const [wizard, setWizard] = useState<CaseWizardForm>(() => emptyWizard());
  const [activeCaseId, setActiveCaseId] = useState<string | null>(null);
  const [documents, setDocuments] = useState<DocumentMeta[]>([]);
  const [docBusy, setDocBusy] = useState(false);
  const [triageRows, setTriageRows] = useState<PageTriageRow[]>([]);
  const [pageSources, setPageSources] = useState<PageSourceRow[]>([]);
  const [triageJob, setTriageJob] = useState<TriageJobStatus | null>(null);
  const [triageBusy, setTriageBusy] = useState(false);
  const [textLayers, setTextLayers] = useState<PageTextLayerRow[]>([]);
  const [recognizeJob, setRecognizeJob] = useState<TriageJobStatus | null>(null);
  const [recognizeBusy, setRecognizeBusy] = useState(false);
  const [caseGraph, setCaseGraph] = useState<CaseGraph | null>(null);
  const [skeletonJob, setSkeletonJob] = useState<TriageJobStatus | null>(null);
  const [skeletonBusy, setSkeletonBusy] = useState(false);
  const [reviewBusy, setReviewBusy] = useState(false);
  const [interview, setInterview] = useState<InterviewSession | null>(null);
  const [interviewBusy, setInterviewBusy] = useState(false);
  const [batchJob, setBatchJob] = useState<BatchJob | null>(null);
  const [batchBusy, setBatchBusy] = useState(false);
  const [companion, setCompanion] = useState<CompanionSession | null>(null);
  const [companionPol, setCompanionPol] = useState<CompanionPolicy | null>(null);
  const [companionBusy, setCompanionBusy] = useState(false);
  const [packs, setPacks] = useState<InstalledPack[]>([]);
  const [packsBusy, setPacksBusy] = useState(false);
  const [packScan, setPackScan] = useState<HeadnoteScanResult | null>(null);
  const [packSecurityReport, setPackSecurityReport] = useState<PackSecurityReport | null>(null);
  const [packPolicyNote, setPackPolicyNote] = useState("Ed25519 official · isolated custom");
  const [retrievalBusy, setRetrievalBusy] = useState(false);
  const [retrievalResult, setRetrievalResult] = useState<RetrievalResult | null>(null);
  const [retrievalPolicyNote, setRetrievalPolicyNote] = useState(
    "DECOMPOSE→RETRIEVE→GENERATE→VERIFY→SCORE→RENDER · GREEN/AMBER/RED",
  );
  const [dragOver, setDragOver] = useState(false);
  const [caseFilter, setCaseFilter] = useState<"All" | "India" | "United States" | "Other">("All");
  const [caseQuery, setCaseQuery] = useState("");
  const [selectedCaseId, setSelectedCaseId] = useState<string | null>(null);
  const [maxFrameGapMs, setMaxFrameGapMs] = useState(0);
  const [heavyProbeBusy, setHeavyProbeBusy] = useState(false);
  const [choreo, setChoreo] = useState<ChoreographyStatus | null>(null);
  const [hwGate, setHwGate] = useState<HardwareGate | null>(null);
  const [workflowReadiness,setWorkflowReadiness]=useState<MockCourtReadiness|null>(null);
  const [workflowTasks,setWorkflowTasks]=useState<PreparationTask[]>([]);
  const [workflowArgumentTree,setWorkflowArgumentTree]=useState<Record<string,unknown>|null>(null);
  const [workflowLoading,setWorkflowLoading]=useState(false);
  const [workflowError,setWorkflowError]=useState<string|null>(null);
  const [courtInitialPanel,setCourtInitialPanel]=useState<"bench"|"debrief">("bench");
  const [courtSeed,setCourtSeed]=useState<{format:"Witness_Cross_Examination";draft:string;nonce:number}|null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const folderInputRef = useRef<HTMLInputElement | null>(null);
  const lastFrameTs = useRef<number | null>(null);

  const activeCase = useMemo(
    () => cases.find((c) => c.case_id === activeCaseId) ?? null,
    [cases, activeCaseId],
  );

  const modelReady = Boolean(llm?.controlled_beta_ready);

  const refreshWorkflowState=useCallback(async(caseId?:string|null)=>{
    const id=caseId??activeCaseId;
    if(!tauri||!id){setWorkflowReadiness(null);setWorkflowTasks([]);setWorkflowArgumentTree(null);setWorkflowError(null);return;}
    setWorkflowLoading(true);setWorkflowError(null);
    const [ready,tasks,tree]=await Promise.allSettled([mockCourtRetrievalReadiness(id),preparationTasksList(id),argumentTreeGet(id)]);
    if(ready.status==="fulfilled")setWorkflowReadiness(ready.value);else{setWorkflowReadiness(null);setWorkflowError(ready.reason instanceof Error?ready.reason.message:String(ready.reason));}
    if(tasks.status==="fulfilled")setWorkflowTasks(tasks.value);else setWorkflowTasks([]);
    if(tree.status==="fulfilled")setWorkflowArgumentTree(tree.value);else setWorkflowArgumentTree(null);
    setWorkflowLoading(false);
  },[tauri,activeCaseId]);

  useEffect(()=>{void refreshWorkflowState();},[refreshWorkflowState,nav,caseGraph?.generated_at,documents.length]);

  const workflowStages=useMemo(()=>deriveLawyerWorkflow({hasCase:Boolean(activeCase),documents,pageSources,triageJob,triageBusy,graph:caseGraph,skeletonJob,skeletonBusy,interview,interviewBusy,readiness:workflowReadiness,readinessLoading:workflowLoading,readinessError:workflowError,argumentTree:workflowArgumentTree,argumentLoading:workflowLoading,tasks:workflowTasks,tasksLoading:workflowLoading,batchJob}),[activeCase,documents,pageSources,triageJob,triageBusy,caseGraph,skeletonJob,skeletonBusy,interview,interviewBusy,workflowReadiness,workflowLoading,workflowError,workflowArgumentTree,workflowTasks,batchJob]);

  const refreshLlm = useCallback(async () => {
    if (!tauri) {
      setLlm({
        ready: false,
        controlled_beta_ready: false,
        backend: "none",
        backend_version: "unknown",
        model: "",
        model_digest: "unverified",
        quantization: "unknown",
        parameters: "unknown",
        context_length: 0,
        classification: "unavailable",
        license: "unknown",
        legally_fine_tuned: false,
        checkpoint_evidence: false,
        prompt_version: "unknown",
        system_prompt_hash: "unverified",
        generation: null,
        detail: "Tauri required for local infer-llm",
        cloud: false,
        product_role: "Primary local reasoner",
      });
      return;
    }
    try {
      const st = await llmStatus();
      setLlm(st);
    } catch (e) {
      setLlm({
        ready: false,
        controlled_beta_ready: false,
        backend: "none",
        backend_version: "unknown",
        model: "",
        model_digest: "unverified",
        quantization: "unknown",
        parameters: "unknown",
        context_length: 0,
        classification: "unavailable",
        license: "unknown",
        legally_fine_tuned: false,
        checkpoint_evidence: false,
        prompt_version: "unknown",
        system_prompt_hash: "unverified",
        generation: null,
        detail: e instanceof Error ? e.message : String(e),
        cloud: false,
        product_role: "Primary local reasoner",
      });
    }
  }, [tauri]);

  const refreshChoreo = useCallback(async () => {
    if (!tauri) {
      setChoreo(null);
      return;
    }
    try {
      setChoreo(await choreographyStatus());
    } catch {
      setChoreo(null);
    }
  }, [tauri]);

  const refreshHealth = useCallback(async () => {
    if (!tauri) {
      setError("Run with `npm run tauri:dev` for full product features (vault, cases, intake, LLM).");
      return;
    }
    setError(null);
    try {
      const [v, s, vs] = await Promise.all([shellVersion(), shellSidecarStatus(), vaultStatus()]);
      setVersion(v);
      setSidecars(s);
      setVault(vs);
      try {
        setLlm(await llmStatus());
      } catch (e) {
        setLlm({
          ready: false,
          controlled_beta_ready: false,
          backend: "none",
          backend_version: "unknown",
          model: "",
          model_digest: "unverified",
          quantization: "unknown",
          parameters: "unknown",
          context_length: 0,
          classification: "unavailable",
          license: "unknown",
          legally_fine_tuned: false,
          checkpoint_evidence: false,
          prompt_version: "unknown",
          system_prompt_hash: "unverified",
          generation: null,
          detail: e instanceof Error ? e.message : String(e),
          cloud: false,
          product_role: "Primary local reasoner",
        });
      }
      if (vs.app_unlocked) {
        const list = await caseList();
        setCases(list);
        const openIds = new Set(list.filter((c) => c.is_open).map((c) => c.case_id));
        if (activeCaseId && !openIds.has(activeCaseId)) {
          setActiveCaseId(null);
          setDocuments([]);
          setTriageRows([]);
        }
      } else {
        setCases([]);
        setActiveCaseId(null);
        setDocuments([]);
        setTriageRows([]);
      }
      await shellPing();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [tauri, activeCaseId]);

  useEffect(() => {
    void refreshHealth();
  }, [refreshHealth]);

  useEffect(() => {
    if (!tauri) return;
    void hardwareGate()
      .then(setHwGate)
      .catch(() => setHwGate(null));
  }, [tauri]);

  useEffect(() => {
    void refreshChoreo();
    if (!tauri) return;
    const id = window.setInterval(() => void refreshChoreo(), 4000);
    return () => window.clearInterval(id);
  }, [tauri, refreshChoreo]);

  useEffect(() => {
    if (nav === "library" && tauri) {
      void refreshPacks();
    }
  }, [nav, tauri]);

  // AC-3.5: measure frame gaps via requestAnimationFrame (probe only).
  useEffect(() => {
    let raf = 0;
    let stopped = false;
    const tick = (ts: number) => {
      if (stopped) return;
      if (lastFrameTs.current != null) {
        const gap = ts - lastFrameTs.current;
        setMaxFrameGapMs((prev) => (gap > prev ? Math.round(gap) : prev));
      }
      lastFrameTs.current = ts;
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => {
      stopped = true;
      cancelAnimationFrame(raf);
    };
  }, []);

  const runHeavyProbe = async () => {
    if (!tauri) {
      setNotice(
        `UI freeze budget ${UI_FREEZE_BUDGET_MS} ms · browser preview cannot run shell heavy job`,
      );
      return;
    }
    setHeavyProbeBusy(true);
    setMaxFrameGapMs(0);
    lastFrameTs.current = null;
    try {
      await shellHeavyJobStart(1500);
      // Allow a couple of frames to settle, then read the measured peak.
      await sleep(100);
      setMaxFrameGapMs((gap) => {
        setNotice(
          `Heavy job probe finished · max_frame_gap ${gap} ms (budget ${UI_FREEZE_BUDGET_MS} ms)`,
        );
        return gap;
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setHeavyProbeBusy(false);
    }
  };

  const runVault = async (op: () => Promise<void>) => {
    if (!tauri) return;
    setVaultBusy(true);
    setError(null);
    try {
      await op();
      await refreshHealth();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setVaultBusy(false);
    }
  };

  const refreshDocs = async (caseId: string) => {
    const docs = await docList(caseId);
    setDocuments(docs);
    try {
      const pages = await docpipePageList(caseId);
      setTriageRows(pages);
    } catch {
      setTriageRows([]);
    }
    try {
      setPageSources(await docpipePageSourceList(caseId));
    } catch {
      setPageSources([]);
    }
    try {
      const layers = await docpipeTextLayerList(caseId);
      setTextLayers(layers);
    } catch {
      setTextLayers([]);
    }
    try {
      const g = await docpipeCaseGraphGet(caseId);
      setCaseGraph(g);
    } catch {
      setCaseGraph(null);
    }
    try {
      const sess = await interviewGet(caseId);
      setInterview(sess);
    } catch {
      setInterview(null);
    }
    try {
      const b = await batchLatest(caseId);
      setBatchJob(b);
    } catch {
      setBatchJob(null);
    }
  };

  const openCase = async (caseId: string) => {
    if (!passphrase) {
      setError("Enter your vault passphrase to open a case.");
      return;
    }
    await runVault(async () => {
      const summary = cases.find((c) => c.case_id === caseId);
      if (!summary?.is_open) {
        await caseOpen(caseId, passphrase);
      }
      setActiveCaseId(caseId);
      setSelectedCaseId(caseId);
      await refreshDocs(caseId);
      setNav("workspace");
      setCaseTab("documents");
      setNotice(null);
    });
  };

  const pollTriageJob = async (jobId: string) => {
    let status = await docpipeTriageStatus(jobId);
    setTriageJob(status);
    const unlisten = await onDocpipeTriageProgress((ev) => {
      if (ev.job_id === jobId) {
        setTriageJob((prev) =>
          prev
            ? {
                ...prev,
                progress_percent: ev.progress_percent,
                message: ev.message,
                doc_id: ev.doc_id,
              }
            : prev,
        );
      }
    });
    for (let i = 0; i < 600; i++) {
      status = await docpipeTriageStatus(jobId);
      setTriageJob(status);
      if (status.state === "completed" || status.state === "failed") break;
      await sleep(50);
    }
    unlisten();
    return status;
  };

  const runTriage = async (docId: string) => {
    if (!activeCaseId) return;
    setTriageBusy(true);
    setError(null);
    try {
      const start = await docpipeTriageStart(activeCaseId, docId);
      const status = await pollTriageJob(start.job_id);
      if (status.state === "failed") setError(status.error ?? "Triage failed");
      await refreshDocs(activeCaseId);
      setCaseTab("triage");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setTriageBusy(false);
    }
  };

  const runTriageCase = async () => {
    if (!activeCaseId) return;
    setTriageBusy(true);
    setError(null);
    try {
      const start = await docpipeTriageCase(activeCaseId);
      const status = await pollTriageJob(start.job_id);
      if (status.state === "failed") setError(status.error ?? "Triage failed");
      await refreshDocs(activeCaseId);
      setCaseTab("triage");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setTriageBusy(false);
    }
  };

  const pollRecognizeJob = async (jobId: string) => {
    let status = await docpipeTriageStatus(jobId);
    setRecognizeJob(status);
    const unlisten = await onDocpipeRecognizeProgress((ev) => {
      if (ev.job_id === jobId) {
        setRecognizeJob((prev) =>
          prev
            ? {
                ...prev,
                progress_percent: ev.progress_percent,
                message: ev.message,
                doc_id: ev.doc_id,
              }
            : prev,
        );
      }
    });
    for (let i = 0; i < 600; i++) {
      status = await docpipeTriageStatus(jobId);
      setRecognizeJob(status);
      if (status.state === "completed" || status.state === "failed") break;
      await sleep(50);
    }
    unlisten();
    return status;
  };

  const runRecognize = async (docId: string) => {
    if (!activeCaseId) return;
    setRecognizeBusy(true);
    setError(null);
    try {
      const start = await docpipeRecognizeStart(activeCaseId, docId);
      const status = await pollRecognizeJob(start.job_id);
      if (status.state === "failed") setError(status.error ?? "Recognition failed");
      await refreshDocs(activeCaseId);
      setCaseTab("text");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRecognizeBusy(false);
    }
  };

  const runRecognizeCase = async () => {
    if (!activeCaseId) return;
    setRecognizeBusy(true);
    setError(null);
    try {
      const start = await docpipeRecognizeCase(activeCaseId);
      const status = await pollRecognizeJob(start.job_id);
      if (status.state === "failed") setError(status.error ?? "Recognition failed");
      await refreshDocs(activeCaseId);
      setCaseTab("text");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRecognizeBusy(false);
    }
  };

  const pollSkeletonJob = async (jobId: string) => {
    let status = await docpipeTriageStatus(jobId);
    setSkeletonJob(status);
    const unlisten = await onDocpipeSkeletonProgress((ev) => {
      if (ev.job_id === jobId) {
        setSkeletonJob((prev) =>
          prev
            ? {
                ...prev,
                progress_percent: ev.progress_percent,
                message: ev.message,
              }
            : prev,
        );
      }
    });
    for (let i = 0; i < 600; i++) {
      status = await docpipeTriageStatus(jobId);
      setSkeletonJob(status);
      if (status.state === "completed" || status.state === "failed") break;
      await sleep(50);
    }
    unlisten();
    return status;
  };

  const runSkeleton = async () => {
    if (!activeCaseId) return;
    setSkeletonBusy(true);
    setError(null);
    try {
      const start = await docpipeSkeletonStart(activeCaseId);
      const status = await pollSkeletonJob(start.job_id);
      if (status.state === "failed") setError(status.error ?? "Skeleton extraction failed");
      await refreshDocs(activeCaseId);
      setCaseTab("skeleton");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSkeletonBusy(false);
    }
  };

  const correctGraphItem = async (
    collection: string,
    itemId: string,
    fieldName: string,
    newValue: string,
  ) => {
    if (!activeCaseId) return;
    setReviewBusy(true);
    setError(null);
    try {
      const g = await reviewCorrectGraphItem(
        activeCaseId,
        collection,
        itemId,
        fieldName,
        newValue,
        2,
      );
      setCaseGraph(g);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      throw e;
    } finally {
      setReviewBusy(false);
    }
  };

  const correctWord = async (
    docId: string,
    pageIndex: number,
    wordIndex: number,
    newText: string,
  ) => {
    if (!activeCaseId) return;
    setReviewBusy(true);
    setError(null);
    try {
      await reviewCorrectWord(activeCaseId, docId, pageIndex, wordIndex, newText, 2);
      const layers = await docpipeTextLayerList(activeCaseId);
      setTextLayers(layers);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      throw e;
    } finally {
      setReviewBusy(false);
    }
  };

  const openInterview = async () => {
    if (!activeCaseId) return;
    setInterviewBusy(true);
    setError(null);
    try {
      const sess = await interviewOpen(activeCaseId);
      setInterview(sess);
      const g = await docpipeCaseGraphGet(activeCaseId);
      setCaseGraph(g);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      throw e;
    } finally {
      setInterviewBusy(false);
    }
  };

  const pauseInterview = async () => {
    if (!activeCaseId || !interview) return;
    setInterviewBusy(true);
    try {
      setInterview(await interviewPause(activeCaseId, interview.session_id));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      throw e;
    } finally {
      setInterviewBusy(false);
    }
  };

  const resumeInterview = async () => {
    if (!activeCaseId || !interview) return;
    setInterviewBusy(true);
    try {
      setInterview(await interviewResume(activeCaseId, interview.session_id));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      throw e;
    } finally {
      setInterviewBusy(false);
    }
  };

  const answerInterview = async (questionIndex: number, text: string) => {
    if (!activeCaseId || !interview) return;
    setInterviewBusy(true);
    try {
      setInterview(
        await interviewAnswer(activeCaseId, interview.session_id, questionIndex, text),
      );
      const g = await docpipeCaseGraphGet(activeCaseId);
      setCaseGraph(g);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      throw e;
    } finally {
      setInterviewBusy(false);
    }
  };

  const skipInterview = async (questionIndex: number) => {
    if (!activeCaseId || !interview) return;
    setInterviewBusy(true);
    try {
      setInterview(
        await interviewSkip(activeCaseId, interview.session_id, questionIndex),
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      throw e;
    } finally {
      setInterviewBusy(false);
    }
  };

  const pollBatchJob = async (jobId: string) => {
    if (!activeCaseId) return null;
    let job = await batchStatus(activeCaseId, jobId);
    setBatchJob(job);
    const unlisten = await onDocpipeBatchProgress((ev) => {
      if (ev.job_id === jobId) {
        setBatchJob((prev) =>
          prev
            ? {
                ...prev,
                progress_percent: ev.progress_percent,
                message: ev.message,
              }
            : prev,
        );
      }
    });
    for (let i = 0; i < 1200; i++) {
      job = await batchStatus(activeCaseId, jobId);
      setBatchJob(job);
      if (
        job.status === "completed" ||
        job.status === "failed" ||
        job.status === "interrupted" ||
        job.status === "cancelled"
      ) {
        break;
      }
      await sleep(80);
    }
    unlisten();
    await refreshDocs(activeCaseId);
    return job;
  };

  const startBatch = async (stopAfter?: number) => {
    if (!activeCaseId) return;
    setBatchBusy(true);
    setError(null);
    try {
      const start = await batchStart(activeCaseId, stopAfter ?? null);
      await pollBatchJob(start.job_id);
      setCaseTab("batch");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBatchBusy(false);
    }
  };

  const resumeBatch = async () => {
    if (!activeCaseId || !batchJob) return;
    setBatchBusy(true);
    setError(null);
    try {
      const start = await batchResume(activeCaseId, batchJob.job_id);
      await pollBatchJob(start.job_id);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBatchBusy(false);
    }
  };

  const cancelBatch = async () => {
    if (!activeCaseId || !batchJob) return;
    setBatchBusy(true);
    try {
      setBatchJob(await batchCancel(activeCaseId, batchJob.job_id));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBatchBusy(false);
    }
  };

  const refreshBatch = async () => {
    if (!activeCaseId) return;
    try {
      if (batchJob) {
        setBatchJob(await batchStatus(activeCaseId, batchJob.job_id));
      } else {
        setBatchJob(await batchLatest(activeCaseId));
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const startCompanion = async () => {
    if (!activeCaseId) {
      setError("Open a case before starting phone capture.");
      return;
    }
    setCompanionBusy(true);
    setError(null);
    try {
      if (!companionPol) {
        setCompanionPol(await companionPolicy());
      }
      const res = await companionStart(activeCaseId);
      setCompanion(res.session);
      setCaseTab("capture");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setCompanionBusy(false);
    }
  };

  const stopCompanionSession = async () => {
    setCompanionBusy(true);
    try {
      const s = await companionStop();
      setCompanion(s);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setCompanionBusy(false);
    }
  };

  const refreshCompanion = async () => {
    try {
      setCompanion(await companionStatus());
      if (!companionPol) setCompanionPol(await companionPolicy());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const refreshPacks = async () => {
    if (!tauri) return;
    setPacksBusy(true);
    try {
      setPacks(await packList());
      const pol = await packPolicy();
      setPackPolicyNote(
        `batch=${String(pol.batch)} · ${String(pol.signature)} · AC-7.1 official headnote scan`,
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setPacksBusy(false);
    }
  };

  const installPackFromPath = async (sourceDir: string) => {
    setPacksBusy(true);
    setError(null);
    try {
      const res = await packInstall(sourceDir);
      setPackSecurityReport(res.security_report);
      setPacks(await packList());
      setNotice(`Installed ${res.pack.display_name} (${res.pack.pack_kind})`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setPacksBusy(false);
    }
  };

  const uninstallPack = async (packId: string) => {
    const pack = packs.find((item) => item.pack_id === packId);
    const confirmed = window.confirm(
      `Remove knowledge pack "${pack?.display_name || packId}"?\n\n` +
        "This removes the local installed copy. Matter files are not changed, and the pack can be installed again from its source.",
    );
    if (!confirmed) return;
    setPacksBusy(true);
    try {
      await packUninstall(packId);
      setPacks(await packList());
      setPackScan(null);
      setNotice("Knowledge pack removed. Matter files were not changed; reinstall it from its source if needed.");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setPacksBusy(false);
    }
  };

  const scanPackHeadnotes = async (packId: string) => {
    setPacksBusy(true);
    try {
      setPackScan(await packHeadnoteScan(packId));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setPacksBusy(false);
    }
  };

  const runRetrieval = async (args: {
    query: string;
    topK: number;
    jurisdiction: string | null;
    authorityType: string | null;
    trustLevel: string | null;
    temporalStatus: string | null;
    supersededStatus: string | null;
    forceAuthorities: string[];
    caseId: string | null;
    userConfirmedRed: boolean;
  }) => {
    setRetrievalBusy(true);
    setError(null);
    try {
      try {
        const pol = await retrievalPolicy();
        setRetrievalPolicyNote(
          `batch=${String(pol.batch)} · index=${String(pol.index_id)} · AC-4.3 planted RED · AC-7.2 top-8 >=85%`,
        );
      } catch {
        /* policy optional in degraded mode */
      }
      const res = await retrievalQuery({
        query: args.query,
        topK: args.topK,
        jurisdiction: args.jurisdiction,
        authorityType: args.authorityType,
        trustLevel: args.trustLevel,
        temporalStatus: args.temporalStatus,
        supersededStatus: args.supersededStatus,
        forceAuthorities: args.forceAuthorities,
        caseId: args.caseId,
        userConfirmedRed: args.userConfirmedRed,
      });
      setRetrievalResult(res);
      const reds = res.citations.filter((c) => c.verification_status === "RED").length;
      setNotice(
        `Retrieval: ${res.retrieved.length} hits · ${res.citations.length} citations (${reds} RED)`,
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRetrievalBusy(false);
    }
  };

  const ingestFiles = async (files: FileList | File[]) => {
    if (!activeCaseId) {
      setNotice("Open a case first, then upload documents.");
      setNav("cases");
      return;
    }
    const list = Array.from(files);
    if (list.length === 0) return;
    setDocBusy(true);
    setNotice(null);
    setError(null);
    let ok = 0;
    let skipped = 0;
    try {
      for (const file of list) {
        if (!isAllowedIntakeFile(file.name)) {
          skipped += 1;
          continue;
        }
        const content_base64 = await fileToBase64(file);
        await docIngest(activeCaseId, {
          original_filename: file.name,
          content_base64,
          type_tag: suggestTypeTagFromName(file.name),
          language_tag: "en",
        });
        ok += 1;
      }
      await refreshDocs(activeCaseId);
      setNotice(
        `Ingested ${ok} file(s)` +
          (skipped ? `; skipped ${skipped} unsupported (PDF/DOCX/JPG/PNG/TXT only)` : "") +
          ".",
      );
      setCaseTab("documents");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setDocBusy(false);
    }
  };

  const seedDemo = async (seedId: DemoSeedId) => {
    if (!tauri || !passphrase) {
      setError("Unlock vault and enter passphrase, then load a demo pack.");
      return;
    }
    const seed = DEMO_SEEDS.find((s) => s.id === seedId);
    if (!seed) return;
    setNotice(null);
    setError(null);
    await runVault(async () => {
      const res = await caseCreate(passphrase, seed.wizard);
      await docIngest(res.case_id, {
        original_filename: seed.packetFilename,
        content_base64: textToBase64(seed.packetText),
        type_tag: "TXT",
        language_tag: "en",
      });
      setActiveCaseId(res.case_id);
      await refreshDocs(res.case_id);
      setNotice(
        `Loaded ${seed.label} · packet ingested as TXT. Open Argument Builder or Mock Court.`,
      );
      setNav("workspace");
      setCaseTab("documents");
    });
  };

  const createCase = async () => {
    const err = validateWizard(wizard);
    if (err) {
      setError(err);
      return;
    }
    if (!passphrase) {
      setError("Passphrase required to create an encrypted case vault.");
      return;
    }
    setNotice(null);
    await runVault(async () => {
      const res = await caseCreate(passphrase, {
        cause_title: wizard.cause_title,
        court: wizard.court,
        jurisdiction: wizard.jurisdiction,
        matter_type: wizard.matter_type,
        user_side: wizard.user_side,
        code_era: wizard.code_era || null,
        language: wizard.language,
      });
      setWizard(emptyWizard());
      setActiveCaseId(res.case_id);
      await refreshDocs(res.case_id);
      setNotice(`Case created · ${res.profile.jurisdiction} locked for this matter.`);
      setNav("workspace");
      setCaseTab("documents");
    });
  };

  const headings = titleFor(nav);
  const unlocked = Boolean(vault?.app_unlocked);

  // B-S4-17: what the licence entitles decides which screens exist. The host refuses what is hidden; this only tidies the menu.
  const showNav = (item: { id: NavId }) => (edition ? edition.visible_surfaces.includes(item.id) : !["chambers", "causelist", "scaffold", "issues"].includes(item.id));
  useEffect(() => {
    if (!tauri) return;
    void editionStatus().then(setEdition).catch(() => setEdition(null));
  }, [tauri, unlocked, nav]);
  useEffect(() => {
    if (edition && nav !== "new-case" && !edition.visible_surfaces.includes(nav)) setNav("home");
  }, [edition, nav]);

  const openWorkflowStage = (stageId:WorkflowStageId) => {
    const stage=workflowStages.find(s=>s.id===stageId);
    setError(null);
    if(stageId==="material"){if(!activeCase){setNav("cases");return;}setNav("workspace");setCaseTab("documents");return;}
    if(stageId==="quality"){if(documents.length===0){setNav("workspace");setCaseTab("documents");return;}setNav("workspace");setCaseTab("overview");if(pageSources.length===0){void runTriageCase();}return;}
    if(stageId==="understanding"){setNav("workspace");if(!caseGraph||stage?.state==="STALE"){setCaseTab("overview");void runSkeleton();return;}setCaseTab("review");return;}
    if(stageId==="missing_facts"){setNav("workspace");setCaseTab("interview");return;}
    if(stageId==="digest"){setNav("digest");return;}
    if(stageId==="authorities"){setNav("library");return;}
    if(stageId==="argument"){setNav("integrity");return;}
    if(stageId==="mock_court"&&!workflowReadiness?.ready){setNotice(`Mock Court is blocked: ${(workflowReadiness?.blocking_reasons||["confirm the matter understanding and legal working set"]).join(" · ")}`);setNav("library");return;}
    if(stageId==="tasks"){setCourtInitialPanel("debrief");setNav("court");return;}
    setCourtInitialPanel("bench");setNav("court");
  };

  const navigatePrimary=(id:NavId)=>{
    if(id==="court"){openWorkflowStage("mock_court");return;}
    setNav(id);setError(null);
  };

  const doUnlock = () =>
    void runVault(async () => {
      if (!vault) return;
      if (!passphrase.trim()) {
        setError("Enter a password to continue.");
        return;
      }
      if (!vault.app_exists) {
        if (passphrase !== passphraseConfirm) {
          setError("Passwords do not match. Re-enter both fields.");
          return;
        }
        if (passphrase.trim().length < 4) {
          setError("Choose a password of at least 4 characters.");
          return;
        }
        setVault(await vaultInitUnlock(passphrase));
        setPassphraseConfirm("");
        return;
      }
      setVault(await vaultUnlock(passphrase));
    });

  const doResetVault = () =>
    void runVault(async () => {
      const ok = window.confirm(
        "Reset local vault?\n\n" +
          "This permanently deletes the encrypted vault on this computer " +
          "(cases and documents stored here). There is no cloud recovery.\n\n" +
          "You will then create a new password and start fresh.",
      );
      if (!ok) return;
      const ok2 = window.confirm(
        "Final confirmation: delete all local CaseForge vault data on this machine?",
      );
      if (!ok2) return;
      setVault(await vaultResetLocal());
      setPassphrase("");
      setPassphraseConfirm("");
      setError(null);
      setNotice("Local vault wiped. Create a new password to continue.");
    });

  // ——— Hardware refuse (B-S1-22 / PRD §3) ———
  if (tauri && hwGate && !hwGate.allowed) {
    return (
      <div className="unlock-screen">
        <div className="unlock-brand">
          <h1 className="wordmark">CaseForge</h1>
          <div className="tags">Hardware not supported</div>
          <p className="manifesto">Installer refuses this machine.</p>
        </div>
        <div className="unlock-panel">
          <div className="unlock-card" data-testid="hardware-refuse">
            <h1>Cannot run</h1>
            <p className="lede">{hwGate.refuse_reason || hwGate.messaging}</p>
            <p className="helpers">
              Requires 64-bit OS and at least 4 GB RAM (PRD §3). Lite tier is 4–6 GB; Full tier ≥8 GB.
            </p>
          </div>
        </div>
      </div>
    );
  }

  // ——— Unlock gate ———
  if (tauri && vault && !vault.app_unlocked) {
    return (
      <div className="unlock-screen">
        <div className="unlock-brand">
          <div className="rules" />
          <h1 className="wordmark">CaseForge</h1>
          <div className="tags">Offline · Encrypted · Local</div>
          <p className="manifesto">
            Perfect memory of the file.
            <br />
            Rigorous practice.
            <br />
            Nothing leaves the machine.
          </p>
          <div className="foot">
            Stage 1 Digest · Stage 2 Mock Court
            <div className="airgap">Air-gapped ready</div>
          </div>
        </div>
        <div className="unlock-panel">
          <div className="unlock-card">
            <div className="lock-mark">🔒</div>
            <h1>{vault.app_exists ? "Sign in" : "Create vault password"}</h1>
            <p className="lede">
              {vault.app_exists
                ? "Enter the password you chose for this machine"
                : "Choose a password — it encrypts your cases on this device only"}
            </p>
            {hwGate && (
              <p className="helpers" data-testid="tier-messaging" style={{ marginBottom: "0.75rem" }}>
                {hwGate.messaging}
              </p>
            )}
            {notice && (
              <div className="notice" style={{ marginBottom: "1rem" }}>
                {notice}
              </div>
            )}
            {error && (
              <div className="notice warn" style={{ marginBottom: "1rem" }}>
                {error.includes("CF_VAULT_PASSPHRASE_INVALID") ||
                error.toLowerCase().includes("wrong password") ||
                error.toLowerCase().includes("passphrase")
                  ? "Wrong password. This vault was sealed earlier on this PC. There is no recovery without the original password — use Reset below to start over."
                  : error}
              </div>
            )}
            <label className="field-wrap">
              Password
              <div style={{ position: "relative" }}>
                <input
                  type={showPass ? "text" : "password"}
                  autoComplete={vault.app_exists ? "current-password" : "new-password"}
                  value={passphrase}
                  onChange={(e) => setPassphrase(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && passphrase) doUnlock();
                  }}
                  placeholder={vault.app_exists ? "Enter password" : "Create a password"}
                />
                <button
                  type="button"
                  className="ghost"
                  style={{
                    position: "absolute",
                    right: 0,
                    top: "50%",
                    transform: "translateY(-50%)",
                    padding: "0.25rem 0.4rem",
                    letterSpacing: 0,
                    textTransform: "none",
                    fontSize: "0.8rem",
                  }}
                  onClick={() => setShowPass((v) => !v)}
                >
                  {showPass ? "Hide" : "Show"}
                </button>
              </div>
            </label>
            {!vault.app_exists && (
              <label className="field-wrap" style={{ marginTop: "0.75rem" }}>
                Confirm password
                <input
                  type={showPass ? "text" : "password"}
                  autoComplete="new-password"
                  value={passphraseConfirm}
                  onChange={(e) => setPassphraseConfirm(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && passphrase) doUnlock();
                  }}
                  placeholder="Re-enter password"
                />
              </label>
            )}
            <p className="helpers">
              Offline app — no account / email / cloud ID.
              <br />
              Your password is the only key to encrypted case data on this computer.
            </p>
            <button
              type="button"
              className="primary"
              disabled={
                vaultBusy ||
                !passphrase ||
                (!vault.app_exists && !passphraseConfirm)
              }
              onClick={doUnlock}
            >
              {vault.app_exists ? "Unlock" : "Create password & open"}
            </button>
            {vault.app_exists && (
              <button
                type="button"
                className="secondary-link"
                disabled={vaultBusy}
                onClick={doResetVault}
                data-testid="vault-reset-local"
                style={{ color: "#a33", marginTop: "0.35rem" }}
              >
                Forgot password? Reset local vault…
              </button>
            )}
            <button
              type="button"
              className="secondary-link"
              disabled={vaultBusy}
              onClick={() => void refreshHealth()}
            >
              Refresh status
            </button>
            {vault.app_exists ? (
              <p className="helpers" style={{ textAlign: "center", marginTop: "0.35rem" }}>
                A vault already exists on this machine. If you did not set it, use Reset to wipe and
                create a new password.
              </p>
            ) : (
              <p className="helpers" style={{ textAlign: "center", marginTop: "0.35rem" }}>
                First time on this machine — set a password, confirm it, then open the app.
              </p>
            )}
            <div className="foot-meta">
              <span className="chip ok">
                <span className="status-dot on" /> No network required
              </span>
              <div className="mono-line">Shell · Vault · Docpipe</div>
              <div className="version">v{version?.version ?? "1.0"} · Desktop</div>
            </div>
          </div>
        </div>
      </div>
    );
  }

  const showShell = !tauri || unlocked || !vault;

  if (!showShell) {
    return (
      <div className="unlock-screen">
        <div className="unlock-brand">
          <h1 className="wordmark">CaseForge</h1>
          <p className="manifesto">Connecting to vault…</p>
        </div>
        <div className="unlock-panel">
          <div className="unlock-card">
            <h1>Please wait</h1>
            <p className="lede">Opening encrypted vault status</p>
            <button type="button" className="primary" onClick={() => void refreshHealth()}>
              Retry
            </button>
          </div>
        </div>
      </div>
    );
  }

  const llmLabel = modelReady
    ? `${llm?.model || "qwen2.5"} · ready`
    : "LLM offline";

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="sidebar-brand">
          <h1>CaseForge</h1>
          <p>{unlocked ? "Vault unlocked" : tauri ? "Vault locked" : "Browser preview"}</p>
        </div>

        <div className="nav-scroll">
          {PRIMARY_NAV.filter(showNav).map((item) => {
            // Cases stays active while the split wizard is open (Screen 3).
            const active =
              nav === item.id || (item.id === "cases" && nav === "new-case");
            return (
              <div key={item.id}>
                <button
                  type="button"
                  className={`nav-btn ${active ? "active" : ""}`}
                  onClick={() => {
                    navigatePrimary(item.id);
                  }}
                >
                  {item.label}
                  {item.badge && <span className="badge soon">{item.badge}</span>}
                </button>
              </div>
            );
          })}
          <details className="advanced-nav" data-testid="advanced-navigation">
            <summary>Advanced details</summary>
            {ADVANCED_NAV.filter(showNav).map(item=><button key={item.id} type="button" className={`nav-btn ${nav===item.id?"active":""}`} onClick={()=>{setNav(item.id);setError(null);}}>{item.label}</button>)}
          </details>
        </div>

        <div className="sidebar-foot">
          <div className="sidebar-pulse">
            <span className="dot" />
            <span>shell · vault · docpipe · llm</span>
          </div>
          {tauri && unlocked ? (
            <button
              type="button"
              className="lock-btn"
              disabled={vaultBusy}
              onClick={() =>
                void runVault(async () => {
                  setVault(await vaultLock());
                  setCases([]);
                  setPassphrase("");
                })
              }
            >
              Lock vault →
            </button>
          ) : (
            <div style={{ marginTop: "0.5rem" }}>
              {version ? `v${version.version} · ${version.batch}` : "CaseForge desktop"}
            </div>
          )}
        </div>
      </aside>

      <div className="main">
        <div className="mobile-nav">
          {PRIMARY_NAV.filter(showNav).map((item) => (
            <button
              key={item.id}
              type="button"
              className={nav === item.id ? "primary" : undefined}
              onClick={() => navigatePrimary(item.id)}
            >
              {item.label}
            </button>
          ))}
        </div>

        <header className="topbar">
          <div>
            <h2>{headings.title}</h2>
            <p className="sub">{headings.sub}</p>
          </div>
          <div className="topbar-actions">
            {activeCase && nav !== "home" && (
              <span className="chip matter" title={activeCase.case_id}>
                {activeCase.cause_title || activeCase.label || "Active case"}
              </span>
            )}
            {edition?.chambers && <span className="chip ok" title={edition.banner}>Chambers Edition · record only · offline</span>}
            <span className={`chip ${unlocked || !tauri ? "ok" : "warn"}`}>
              <span className={`status-dot ${unlocked || !tauri ? "on" : "off"}`} />
              {unlocked ? "Vault unlocked" : tauri ? "Vault locked" : "Preview"}
            </span>
            <details className="topbar-diagnostics" data-testid="advanced-system-details">
              <summary>System details</summary>
              <div className="topbar-diagnostics-popover">
            <span className={`chip ${modelReady ? "ok" : "warn"}`}>
              <span className={`status-dot ${modelReady ? "on" : "off"}`} />
              {llmLabel}
            </span>
            {/* B-S1-20: required visible model choreography status chip (PRD §4.2) */}
            <span
              className={`chip model-chip ${
                choreo?.hard_fail_primary_vision
                  ? "warn"
                  : choreo?.chip_tone === "ok"
                    ? "ok"
                    : "muted"
              }`}
              data-testid="model-status-chip"
              title={
                choreo
                  ? `${choreo.detail} · tier=${choreo.tier} · RSS ${
                      choreo.budgets.process_rss_mb ?? "—"
                    } MB / max ${choreo.budgets.total_rss_max_gb} GB`
                  : "Model choreography status"
              }
            >
              <span
                className={`status-dot ${
                  choreo && choreo.large_resident_count > 0 ? "on" : "off"
                }`}
              />
              {choreo?.chip_label ?? "MODEL · —"}
            </span>
            {choreo && (
              <span className="chip muted" data-testid="tier-chip" title="Hardware tier">
                {choreo.tier}
                {!choreo.live_simulation_allowed ? " · no live sim" : ""}
              </span>
            )}
            <span className="chip muted">{cases.length} matters</span>
            <span
              className={`chip ${maxFrameGapMs > UI_FREEZE_BUDGET_MS ? "warn" : "muted"}`}
              title={`AC-3.5 frame gap probe · budget ${UI_FREEZE_BUDGET_MS} ms`}
              data-testid="ui-freeze-chip"
            >
              max_frame_gap {maxFrameGapMs} ms
            </span>
            <span className="chip muted">Desktop · v{version?.version ?? "1.0"}</span>
              </div>
            </details>
          </div>
        </header>

        <div className="content">
          <div className="banner-stack">
            {error && (
              <div className="notice error" role="alert">
                {error}
              </div>
            )}
            {notice && <div className="notice">{notice}</div>}
            {!tauri && (
              <div className="notice warn">
                Browser preview only. Start <span className="mono">npm run tauri:dev</span> for
                encrypted vault, case files, and document upload.
              </div>
            )}
          </div>

          {nav === "home" && (
            <div className="content-rail">
              <WorkflowJourney stages={workflowStages} onOpen={openWorkflowStage} />
              <HomeView
                cases={cases}
                documents={documents}
                activeCase={activeCase}
                modelReady={modelReady}
                llm={llm}
                seedBusy={vaultBusy}
                onNavigate={setNav}
                onOpenCase={(id) => void openCase(id)}
                onSeedDemo={(id) => void seedDemo(id)}
              />
            </div>
          )}

          {(nav === "cases" || nav === "new-case") && (
            <CasesWizardView
              cases={cases}
              activeCaseId={activeCaseId}
              selectedCaseId={selectedCaseId}
              setSelectedCaseId={setSelectedCaseId}
              busy={vaultBusy}
              wizard={wizard}
              setWizard={setWizard}
              caseFilter={caseFilter}
              setCaseFilter={setCaseFilter}
              caseQuery={caseQuery}
              setCaseQuery={setCaseQuery}
              wizardFocus={nav === "new-case"}
              onNew={() => setNav("new-case")}
              onCreate={() => void createCase()}
              onOpen={(id) => void openCase(id)}
              onClose={(id) =>
                void runVault(async () => {
                  await caseClose(id);
                  if (activeCaseId === id) {
                    setActiveCaseId(null);
                    setDocuments([]);
                    setTriageRows([]);
                    setPageSources([]);
                    setTextLayers([]);
                    setCaseGraph(null);
                    setInterview(null);
                    setBatchJob(null);
                    void companionStop().then(() => setCompanion(null)).catch(() => undefined);
                  }
                })
              }
              onFocus={(id) => {
                setActiveCaseId(id);
                setSelectedCaseId(id);
                void refreshDocs(id).catch((e) =>
                  setError(e instanceof Error ? e.message : String(e)),
                );
                setNav("workspace");
              }}
              createDisabled={vaultBusy || !tauri}
            />
          )}

          {nav === "workspace" && (
            <WorkspaceView
              activeCase={activeCase}
              cases={cases}
              caseTab={caseTab}
              setCaseTab={setCaseTab}
              documents={documents}
              triageRows={triageRows}
              pageSources={pageSources}
              triageJob={triageJob}
              triageBusy={triageBusy}
              textLayers={textLayers}
              recognizeJob={recognizeJob}
              recognizeBusy={recognizeBusy}
              caseGraph={caseGraph}
              skeletonJob={skeletonJob}
              skeletonBusy={skeletonBusy}
              reviewBusy={reviewBusy}
              interview={interview}
              interviewBusy={interviewBusy}
              batchJob={batchJob}
              batchBusy={batchBusy}
              companion={companion}
              companionPol={companionPol}
              companionBusy={companionBusy}
              docBusy={docBusy}
              dragOver={dragOver}
              setDragOver={setDragOver}
              fileInputRef={fileInputRef}
              folderInputRef={folderInputRef}
              onOpenCase={(id) => void openCase(id)}
              onIngest={(files) => void ingestFiles(files)}
              onTriageDoc={(id) => void runTriage(id)}
              onTriageAll={() => void runTriageCase()}
              onRecognizeDoc={(id) => void runRecognize(id)}
              onRecognizeAll={() => void runRecognizeCase()}
              onSkeleton={() => void runSkeleton()}
              onCorrectGraph={correctGraphItem}
              onCorrectWord={correctWord}
              onInterviewOpen={openInterview}
              onInterviewPause={pauseInterview}
              onInterviewResume={resumeInterview}
              onInterviewAnswer={answerInterview}
              onInterviewSkip={skipInterview}
              onBatchStart={() => startBatch()}
              onBatchResume={resumeBatch}
              onBatchCancel={cancelBatch}
              onBatchSimulateKill={() => startBatch(1)}
              onBatchRefresh={refreshBatch}
              onCompanionStart={startCompanion}
              onCompanionStop={stopCompanionSession}
              onCompanionRefresh={refreshCompanion}
              onTypeTag={(docId, type_tag) => {
                if (!activeCaseId) return;
                setDocBusy(true);
                void docUpdateTypeTag(activeCaseId, docId, type_tag)
                  .then(() => refreshDocs(activeCaseId))
                  .catch((err) => setError(err instanceof Error ? err.message : String(err)))
                  .finally(() => setDocBusy(false));
              }}
              onGoNew={() => setNav("new-case")}
              workflowStages={workflowStages}
              onWorkflowOpen={openWorkflowStage}
            />
          )}

          {nav === "digest" && (
            <DigestView
              activeCase={activeCase}
              docCount={documents.length}
              caseGraph={caseGraph}
              interview={interview}
              busy={skeletonBusy}
              onRefreshGraph={async () => {
                if (!activeCaseId) return;
                try {
                  setCaseGraph(await docpipeCaseGraphGet(activeCaseId));
                } catch (e) {
                  setError(e instanceof Error ? e.message : String(e));
                }
              }}
              onOpenWorkspace={() => {
                setNav("workspace");
                setCaseTab("overview");
              }}
              onOpenReview={() => {
                setNav("workspace");
                setCaseTab("review");
              }}
              onOpenInterview={() => {
                setNav("workspace");
                setCaseTab("interview");
              }}
              onRunSkeleton={async () => {
                if (!activeCaseId) {
                  setNotice("Open a case first.");
                  setNav("cases");
                  return;
                }
                setSkeletonBusy(true);
                setError(null);
                try {
                  await docpipeSkeletonStart(activeCaseId);
                  setCaseGraph(await docpipeCaseGraphGet(activeCaseId));
                  setNotice("Case graph refreshed — preparation completeness recomputed from graph.");
                } catch (e) {
                  setError(e instanceof Error ? e.message : String(e));
                } finally {
                  setSkeletonBusy(false);
                }
              }}
            />
          )}

          {nav === "builder" && (
            <ArgumentBuilderView
              activeCase={activeCase}
              llm={llm}
              onOpenCases={() => setNav("cases")}
              onOpenCourt={() => setNav("court")}
              onRefreshLlm={() => void refreshLlm()}
            />
          )}

          {nav === "integrity" && (
            <ArgumentTreeView
              activeCase={activeCase}
              onOpenCases={() => setNav("cases")}
              onError={setError}
              onNotice={setNotice}
            />
          )}

          {nav === "voice" && (
            <VoiceView
              vaultUnlocked={Boolean(vault?.app_unlocked)}
              onError={setError}
              onNotice={setNotice}
            />
          )}

          {nav === "experience" && (
            <ExperienceView
              vaultUnlocked={Boolean(vault?.app_unlocked)}
              onError={setError}
              onNotice={setNotice}
            />
          )}

          {nav === "licence" && (
            <LicenceView
              vaultUnlocked={Boolean(vault?.app_unlocked)}
              onError={setError}
              onNotice={setNotice}
            />
          )}

          {nav === "protection" && (
            <ProtectionView
              vaultUnlocked={Boolean(vault?.app_unlocked)}
              onError={setError}
              onNotice={setNotice}
            />
          )}

          {nav === "guardrails" && (
            <GuardrailsView onError={setError} onNotice={setNotice} />
          )}

          {nav === "court" && (
            <MockCourtView
              cases={cases}
              activeCase={activeCase}
              llm={llm}
              onOpenCase={(id) => void openCase(id)}
              onRefreshLlm={() => void refreshLlm()}
              initialInsightTab={courtInitialPanel}
              seed={courtSeed}
            />
          )}

          {nav === "opposition" && (
            <OppositionView
              activeCase={activeCase}
              documents={documents}
              onDocumentsChanged={() => { if (activeCase) void refreshDocs(activeCase.case_id); }}
              onNavigate={(target) => setNav(target)}
            />
          )}

          {nav === "witnesses" && (
            <WitnessView
              activeCase={activeCase}
              documents={documents}
              modelReady={modelReady}
              onRehearse={(draft) => { setCourtSeed({ format: "Witness_Cross_Examination", draft, nonce: Date.now() }); setNav("court"); }}
            />
          )}

          {nav === "deadlines" && <DeadlinesView activeCase={activeCase} />}

          {nav === "issues" && <IssueMatrixView activeCase={activeCase} />}

          {nav === "scaffold" && <ScaffoldView activeCase={activeCase} />}

          {nav === "causelist" && <CauseListView activeCase={activeCase} />}

          {nav === "chambers" && <ChambersView activeCase={activeCase} edition={edition} onOpenIssues={() => setNav("issues")} />}

          {nav === "library" && (
            <>
              <PacksView
                packs={packs}
                busy={packsBusy}
                lastScan={packScan}
                securityReport={packSecurityReport}
                policyNote={packPolicyNote}
                onRefresh={() => refreshPacks()}
                onInstallPath={(p) => installPackFromPath(p)}
                onUninstall={(id) => uninstallPack(id)}
                onHeadnoteScan={(id) => scanPackHeadnotes(id)}
              />
              <RetrievalView
                busy={retrievalBusy}
                result={retrievalResult}
                policyNote={retrievalPolicyNote}
                activeCaseId={activeCaseId}
                onRun={runRetrieval}
              />
              <WorkingLibraryView
                caseId={activeCaseId}
                retrievalResult={retrievalResult}
                onError={setError}
                onNotice={(message)=>{setNotice(message);void refreshWorkflowState();}}
              />
            </>
          )}

          {nav === "practice" && (
            <div className="page-grid">
              <div className="notice warn">
                Synthetic teaching packs only — not GOLDEN, not law, not your client files. Use{" "}
                <strong>Case file</strong> for real uploads.
              </div>
              <DemoArgumentPanel />
            </div>
          )}

          {nav === "settings" && (
            <SettingsView
              tauri={tauri}
              vault={vault}
              version={version}
              sidecars={sidecars}
              llm={llm}
              maxFrameGapMs={maxFrameGapMs}
              heavyProbeBusy={heavyProbeBusy}
              onRefresh={() => void refreshHealth()}
              onRefreshLlm={() => void refreshLlm()}
              onHeavyProbe={() => void runHeavyProbe()}
              choreo={choreo}
              onRefreshChoreo={() => void refreshChoreo()}
              onUnloadModels={() =>
                void choreographyUnloadAll()
                  .then((s) => {
                    setChoreo(s);
                    setNotice("All model slots unloaded");
                  })
                  .catch((e) => setError(e instanceof Error ? e.message : String(e)))
              }
            />
          )}
        </div>
      </div>
    </div>
  );
}

/* ——— Views ——— */

function HomeView({
  cases,
  documents,
  activeCase,
  modelReady,
  llm,
  seedBusy,
  onNavigate,
  onOpenCase,
  onSeedDemo,
}: {
  cases: CaseSummary[];
  documents: DocumentMeta[];
  activeCase: CaseSummary | null;
  modelReady: boolean;
  llm: LlmStatus | null;
  seedBusy: boolean;
  onNavigate: (n: NavId) => void;
  onOpenCase: (id: string) => void;
  onSeedDemo: (id: DemoSeedId) => void;
}) {
  return (
    <div className="page-grid">
      <div className="card hero-card">
        <div>
          <div className="eyebrow">Command</div>
          <h3>Continue preparation.</h3>
          <p>
            Your vault is sealed on this machine. All intelligence models and document pipelines are
            standing by for offline processing.
          </p>
          <div className="row">
            <button type="button" className="primary" onClick={() => onNavigate("cases")}>
              Open cases
            </button>
            <button type="button" onClick={() => onNavigate("new-case")}>
              New case
            </button>
          </div>
        </div>
        <div className="hero-metrics">
          <div>
            <div className="big">0</div>
            <div className="label">Golden pages</div>
          </div>
          <div className="label">Triage gate open</div>
          <div className="label">AC-6.1 pending</div>
        </div>
      </div>

      <div className="page-grid cols-3">
        <div className="card pillar">
          <div className="eyebrow">Digest Engine</div>
          <h3>Perfect memory of the file</h3>
          <p>
            Intake, page triage, chronology and parties when Stage 1 is complete. Source-linked facts
            only.
          </p>
          <button type="button" className="linkish pillar-link" onClick={() => onNavigate("workspace")}>
            Enter case file →
          </button>
        </div>
        <div className="card pillar">
          <div className="eyebrow">Argument Builder</div>
          <h3>Map the argument</h3>
          <p>
            Local LLM drafts an argument map from the case packet on this machine. No cloud.
          </p>
          <button type="button" className="linkish pillar-link" onClick={() => onNavigate("builder")}>
            Open builder →
          </button>
        </div>
        <div className="card pillar">
          <div className="eyebrow">Mock Court</div>
          <h3>Rigorous practice</h3>
          <p>
            Argue, bench prep, and adversary drill. Practice completeness — never win prediction.
          </p>
          <button type="button" className="linkish pillar-link" onClick={() => onNavigate("court")}>
            Enter chamber →
          </button>
        </div>
      </div>

      <div className="card">
        <div className="stat-label" style={{ marginBottom: "0.85rem" }}>
          Recent matters
        </div>
        {cases.length === 0 ? (
          <div className="empty" style={{ padding: "1.5rem 0" }}>
            <p>No cases yet. Create a matter to unlock document intake.</p>
            <button type="button" className="primary" onClick={() => onNavigate("new-case")}>
              New case
            </button>
          </div>
        ) : (
          <div className="table-wrap" style={{ border: "none" }}>
            <table className="table">
              <thead>
                <tr>
                  <th>Matter</th>
                  <th>Court</th>
                  <th>Jurisdiction</th>
                  <th>Side</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {cases.slice(0, 6).map((c) => (
                  <tr key={c.case_id} style={{ cursor: "pointer" }} onClick={() => onOpenCase(c.case_id)}>
                    <td className="matter-title">{c.cause_title || c.label || "Untitled"}</td>
                    <td>{c.court || "—"}</td>
                    <td>
                      <span className="chip matter">{c.jurisdiction}</span>
                    </td>
                    <td>{c.user_side || "—"}</td>
                    <td>{c.is_open ? "Active" : "Idle"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      <div className="page-grid cols-2">
        <div className="card">
          <h3>Demo packs</h3>
          <p style={{ marginBottom: "0.75rem" }}>
            Synthetic teaching packets only — not GOLDEN, not law. Loads an encrypted case + TXT
            packet for the local LLM.
          </p>
          <div className="row">
            {DEMO_SEEDS.map((s) => (
              <button
                key={s.id}
                type="button"
                className={s.difficulty === "hard" ? "primary" : undefined}
                disabled={seedBusy}
                onClick={() => onSeedDemo(s.id)}
              >
                Load {s.difficulty}
              </button>
            ))}
          </div>
        </div>
        <div className="card">
          <h3>At a glance</h3>
          <div className="row" style={{ marginTop: "0.5rem" }}>
            <span className="chip">{cases.length} matters</span>
            <span className="chip">{activeCase ? documents.length : 0} docs open</span>
            <span className={`chip ${modelReady ? "ok" : "warn"}`}>
              {modelReady ? `${llm?.backend} · ready` : "LLM offline"}
            </span>
          </div>
          <p style={{ marginTop: "0.85rem" }}>
            Heavy work never freezes the UI &gt; 200 ms. Case content has zero network path.
          </p>
        </div>
      </div>

      <div className="system-pulse-bar">
        <span>Shell · ok · Vault · {activeCase ? "open" : "ok"} · Docpipe · idle · Llm · {modelReady ? "ready" : "offline"}</span>
        <span>Heavy work never freezes the UI &gt; 200 ms.</span>
      </div>
    </div>
  );
}

function CasesWizardView({
  cases,
  activeCaseId,
  selectedCaseId,
  setSelectedCaseId,
  busy,
  wizard,
  setWizard,
  caseFilter,
  setCaseFilter,
  caseQuery,
  setCaseQuery,
  wizardFocus: _wizardFocus,
  onNew,
  onCreate,
  onOpen,
  onClose,
  onFocus,
  createDisabled,
}: {
  cases: CaseSummary[];
  activeCaseId: string | null;
  selectedCaseId: string | null;
  setSelectedCaseId: (id: string | null) => void;
  busy: boolean;
  wizard: CaseWizardForm;
  setWizard: React.Dispatch<React.SetStateAction<CaseWizardForm>>;
  caseFilter: "All" | "India" | "United States" | "Other";
  setCaseFilter: (f: "All" | "India" | "United States" | "Other") => void;
  caseQuery: string;
  setCaseQuery: (q: string) => void;
  wizardFocus: boolean;
  onNew: () => void;
  onCreate: () => void;
  onOpen: (id: string) => void;
  onClose: (id: string) => void;
  onFocus: (id: string) => void;
  createDisabled: boolean;
}) {
  const filtered = cases.filter((c) => {
    if (caseFilter !== "All" && c.jurisdiction !== caseFilter) return false;
    if (!caseQuery.trim()) return true;
    const q = caseQuery.toLowerCase();
    return (
      (c.cause_title || "").toLowerCase().includes(q) ||
      (c.court || "").toLowerCase().includes(q) ||
      (c.label || "").toLowerCase().includes(q)
    );
  });

  const selected = selectedCaseId || activeCaseId;

  return (
    <div className="cases-split">
      <div className="cases-list-pane">
        <div className="cases-toolbar">
          <div className="search">
            <span aria-hidden>⌕</span>
            <input
              value={caseQuery}
              onChange={(e) => setCaseQuery(e.target.value)}
              placeholder="Search matters"
            />
          </div>
          <div className="filter-chips">
            {(["All", "India", "United States", "Other"] as const).map((f) => (
              <button
                key={f}
                type="button"
                className={caseFilter === f ? "active" : undefined}
                onClick={() => setCaseFilter(f)}
              >
                {f === "United States" ? "United States" : f}
              </button>
            ))}
          </div>
          <button type="button" className="primary" onClick={onNew}>
            New case
          </button>
        </div>

        {filtered.length === 0 ? (
          <div className="empty">
            <h3>No matters yet</h3>
            <p>Create a case to start uploading the paperbook.</p>
            <button type="button" className="primary" style={{ marginTop: "0.75rem" }} onClick={onNew}>
              Create first case
            </button>
          </div>
        ) : (
          <div className="table-wrap" style={{ border: "none" }}>
            <table className="table">
              <thead>
                <tr>
                  <th>Matter</th>
                  <th>Court / Jrd</th>
                  <th>Side / Code</th>
                  <th>Updated</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {filtered.map((c) => (
                  <tr
                    key={c.case_id}
                    className={selected === c.case_id ? "selected" : undefined}
                    onClick={() => setSelectedCaseId(c.case_id)}
                    style={{ cursor: "pointer" }}
                  >
                    <td>
                      <div className="matter-title">{c.cause_title || c.label || "Untitled"}</div>
                    </td>
                    <td>
                      <div>{c.court || "—"}</div>
                      <span className="chip matter" style={{ marginTop: "0.25rem" }}>
                        {c.jurisdiction}
                      </span>
                    </td>
                    <td>
                      <div>{c.user_side || "—"}</div>
                      <div className="muted" style={{ fontSize: "0.82rem" }}>
                        {c.matter_type || c.code_era || "—"}
                      </div>
                    </td>
                    <td className="muted">{formatWhen((c as { updated_at?: string }).updated_at)}</td>
                    <td>
                      {c.is_open ? (
                        <div className="row" style={{ marginTop: 0 }}>
                          <button
                            type="button"
                            className="linkish"
                            disabled={busy}
                            onClick={(e) => {
                              e.stopPropagation();
                              onFocus(c.case_id);
                            }}
                          >
                            Open →
                          </button>
                          <button
                            type="button"
                            className="ghost"
                            disabled={busy}
                            onClick={(e) => {
                              e.stopPropagation();
                              onClose(c.case_id);
                            }}
                          >
                            Close
                          </button>
                        </div>
                      ) : (
                        <button
                          type="button"
                          className="linkish"
                          disabled={busy}
                          onClick={(e) => {
                            e.stopPropagation();
                            onOpen(c.case_id);
                          }}
                        >
                          Open →
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        <p className="muted" style={{ marginTop: "1.25rem", fontSize: "0.82rem" }}>
          🛡 All case data stays in the encrypted vault on this machine.
        </p>
      </div>

      <div className="cases-wizard-pane">
        <div className="wizard-title">
          <div className="eyebrow">New Case wizard</div>
          <h3>Jurisdiction first</h3>
          <p>Jurisdiction is captured once and cannot be changed for this matter.</p>
        </div>
        <div className="form-stack">
          <label>
            Cause title <span className="req">*</span>
            <input
              value={wizard.cause_title}
              onChange={(e) => setWizard((w) => ({ ...w, cause_title: e.target.value }))}
              placeholder="e.g. State v. … / Smith v. …"
            />
          </label>
          <label>
            Court
            <input
              value={wizard.court}
              onChange={(e) => setWizard((w) => ({ ...w, court: e.target.value }))}
              placeholder="Court name"
            />
          </label>
          <label>
            Jurisdiction <span className="req">*</span>
            <span className="hint">Select jurisdiction (required · immutable after create)</span>
            <div className="jurisdiction-seg" role="group" aria-label="Select jurisdiction">
              {JURISDICTIONS.map((j) => (
                <button
                  key={j}
                  type="button"
                  className={wizard.jurisdiction === j ? "active" : undefined}
                  onClick={() =>
                    setWizard((w) => ({
                      ...w,
                      jurisdiction: j,
                      code_era: suggestCodeEraLocal(j),
                    }))
                  }
                >
                  {j}
                </button>
              ))}
            </div>
          </label>
          <div className="pair">
            <label>
              Matter type <span className="req">*</span>
              <input
                value={wizard.matter_type}
                onChange={(e) => setWizard((w) => ({ ...w, matter_type: e.target.value }))}
                placeholder="Civil, Criminal, Commercial…"
              />
            </label>
            <label>
              Your side <span className="req">*</span>
              <input
                value={wizard.user_side}
                onChange={(e) => setWizard((w) => ({ ...w, user_side: e.target.value }))}
                placeholder="Plaintiff, Defence…"
              />
            </label>
          </div>
          <label>
            Code / era
            <input
              value={wizard.code_era}
              onChange={(e) => setWizard((w) => ({ ...w, code_era: e.target.value }))}
            />
            <span className="hint">Auto-suggest is local stub only — counsel confirms</span>
          </label>
          <label>
            Language
            <input value={wizard.language} readOnly disabled />
          </label>
          <div className="honesty-box">
            <span>ⓘ</span>
            <span>No AI suggestions invent legal strategy in this step.</span>
          </div>
          <div className="row" style={{ justifyContent: "flex-end" }}>
            <button type="button" className="ghost" onClick={() => setWizard(emptyWizard())}>
              Cancel
            </button>
            <button type="button" className="primary" disabled={createDisabled} onClick={onCreate}>
              Create case
            </button>
          </div>
          <p className="mono muted" style={{ fontSize: "0.72rem", margin: 0 }}>
            Creates encrypted case vault · empty until intake
          </p>
        </div>
      </div>
    </div>
  );
}

function WorkspaceView({
  activeCase,
  cases,
  caseTab,
  setCaseTab,
  documents,
  triageRows,
  pageSources,
  triageJob,
  triageBusy,
  textLayers,
  recognizeJob,
  recognizeBusy,
  caseGraph,
  skeletonJob,
  skeletonBusy,
  reviewBusy,
  interview,
  interviewBusy,
  batchJob,
  batchBusy,
  companion,
  companionPol,
  companionBusy,
  docBusy,
  dragOver,
  setDragOver,
  fileInputRef,
  folderInputRef,
  onOpenCase,
  onIngest,
  onTriageDoc,
  onTriageAll,
  onRecognizeDoc,
  onRecognizeAll,
  onSkeleton,
  onCorrectGraph,
  onCorrectWord,
  onInterviewOpen,
  onInterviewPause,
  onInterviewResume,
  onInterviewAnswer,
  onInterviewSkip,
  onBatchStart,
  onBatchResume,
  onBatchCancel,
  onBatchSimulateKill,
  onBatchRefresh,
  onCompanionStart,
  onCompanionStop,
  onCompanionRefresh,
  onTypeTag,
  onGoNew,
  workflowStages,
  onWorkflowOpen,
}: {
  activeCase: CaseSummary | null;
  cases: CaseSummary[];
  caseTab: CaseTab;
  setCaseTab: (t: CaseTab) => void;
  documents: DocumentMeta[];
  triageRows: PageTriageRow[];
  pageSources: PageSourceRow[];
  triageJob: TriageJobStatus | null;
  triageBusy: boolean;
  textLayers: PageTextLayerRow[];
  recognizeJob: TriageJobStatus | null;
  recognizeBusy: boolean;
  caseGraph: CaseGraph | null;
  skeletonJob: TriageJobStatus | null;
  skeletonBusy: boolean;
  reviewBusy: boolean;
  interview: InterviewSession | null;
  interviewBusy: boolean;
  batchJob: BatchJob | null;
  batchBusy: boolean;
  companion: CompanionSession | null;
  companionPol: CompanionPolicy | null;
  companionBusy: boolean;
  docBusy: boolean;
  dragOver: boolean;
  setDragOver: (v: boolean) => void;
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  folderInputRef: React.RefObject<HTMLInputElement | null>;
  onOpenCase: (id: string) => void;
  onIngest: (files: FileList | File[]) => void;
  onTriageDoc: (docId: string) => void;
  onTriageAll: () => void;
  onRecognizeDoc: (docId: string) => void;
  onRecognizeAll: () => void;
  onSkeleton: () => void;
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
  onInterviewOpen: () => Promise<void>;
  onInterviewPause: () => Promise<void>;
  onInterviewResume: () => Promise<void>;
  onInterviewAnswer: (questionIndex: number, text: string) => Promise<void>;
  onInterviewSkip: (questionIndex: number) => Promise<void>;
  onBatchStart: () => Promise<void>;
  onBatchResume: () => Promise<void>;
  onBatchCancel: () => Promise<void>;
  onBatchSimulateKill: () => Promise<void>;
  onBatchRefresh: () => Promise<void>;
  onCompanionStart: () => Promise<void>;
  onCompanionStop: () => Promise<void>;
  onCompanionRefresh: () => Promise<void>;
  onTypeTag: (docId: string, tag: string) => void;
  onGoNew: () => void;
  workflowStages: WorkflowStage[];
  onWorkflowOpen: (id: WorkflowStageId) => void;
}) {
  const [selectedDocId, setSelectedDocId] = useState<string | null>(null);
  const [ocrPageImages, setOcrPageImages] = useState<Record<string, string>>({});
  const [ocrPreviewError, setOcrPreviewError] = useState<string | null>(null);

  useEffect(() => {
    if (!selectedDocId && documents[0]) setSelectedDocId(documents[0].doc_id);
  }, [documents, selectedDocId]);

  if (!activeCase) {
    return (
      <div className="card empty">
        <h3>No case file open</h3>
        <p>Create or unlock a case to upload PDFs, DOCX, and page images.</p>
        <div className="row" style={{ marginTop: "0.85rem", justifyContent: "center" }}>
          <button type="button" className="primary" onClick={onGoNew}>
            New case
          </button>
          {cases
            .filter((c) => c.is_open)
            .slice(0, 3)
            .map((c) => (
              <button key={c.case_id} type="button" onClick={() => onOpenCase(c.case_id)}>
                {c.cause_title || "Open matter"}
              </button>
            ))}
        </div>
      </div>
    );
  }

  const selectedPages = triageRows
    .filter((r) => !selectedDocId || r.doc_id === selectedDocId)
    .slice(0, 8);
  const selectedLayers = textLayers
    .filter((r) => !selectedDocId || r.doc_id === selectedDocId)
    .slice(0, 4);
  const lowWordCount = textLayers.reduce(
    (n, layer) => n + layer.words.filter((w) => w.low_confidence).length,
    0,
  );
  const pageTotal = documents.reduce((n, d) => n + (d.page_count ?? 0), 0);
  const progress = triageJob?.progress_percent ?? 0;
  const running = triageBusy || triageJob?.state === "running";
  const recogProgress = recognizeJob?.progress_percent ?? 0;
  const recogRunning = recognizeBusy || recognizeJob?.state === "running";

  return (
    <div>
      <div className="matter-chips">
        <span className="chip matter">{activeCase.cause_title || activeCase.label}</span>
        <span className="chip matter">{activeCase.jurisdiction}</span>
        <span className="chip matter">{activeCase.court || "Court"}</span>
        <span className="chip matter">{activeCase.user_side || "Side"}</span>
        <span className="chip ok">Encrypted</span>
      </div>

      <WorkflowJourney stages={workflowStages} onOpen={onWorkflowOpen} />

      <div className="tabs" aria-label="Case preparation workspace">
        {(
          [
            ["overview", "Case progress"],
            ["documents", "Add material"],
            ["review", "Review understanding"],
            ["interview", "Missing facts"],
            ["share", "Share with colleagues"],
          ] as [CaseTab, string][]
        ).map(([id, label]) => (
          <button
            key={id}
            type="button"
            className={`tab ${caseTab === id ? "active" : ""}`}
            onClick={() => setCaseTab(id)}
          >
            {label}
          </button>
        ))}
      </div>

      <details className="advanced-processing" data-testid="advanced-processing-details">
        <summary>Advanced processing details</summary>
        <p className="muted">
          Inspect page classification, OCR, case-graph jobs, batch recovery, and phone capture.
        </p>
        <div className="tabs" aria-label="Advanced processing tools">
          {(
            [
              ["triage", "Page quality"],
              ["text", "Text and OCR"],
              ["skeleton", "Case-graph build"],
              ["batch", "Batch jobs"],
              ["capture", "Phone capture"],
            ] as [CaseTab, string][]
          ).map(([id, label]) => (
            <button
              key={id}
              type="button"
              className={`tab ${caseTab === id ? "active" : ""}`}
              onClick={() => setCaseTab(id)}
            >
              {label}
            </button>
          ))}
        </div>
      </details>

      {caseTab === "overview" && (
        <div className="page-grid cols-2">
          <div className="card">
            <div className="stat-label">Documents</div>
            <div className="card-stat">{documents.length}</div>
            <p>Stored as encrypted originals in this case vault.</p>
            <button
              type="button"
              className="primary"
              style={{ marginTop: "0.75rem" }}
              onClick={() => setCaseTab("documents")}
            >
              Upload files
            </button>
          </div>
          <div className="card">
            <div className="stat-label">Document quality</div>
            <div className="card-stat">{triageRows.length}</div>
            <p>
              {triageRows.length
                ? `${triageRows.length} pages checked; ${lowWordCount} low-confidence words need review.`
                : "Run the quality check before relying on extracted material."}
            </p>
            <button
              type="button"
              style={{ marginTop: "0.75rem" }}
              onClick={() => setCaseTab("triage")}
            >
              Review document quality
            </button>
          </div>
        </div>
      )}

      {caseTab === "review" && (
        <ReviewView
          caseId={activeCase?.case_id ?? null}
          caseGraph={caseGraph}
          textLayers={textLayers}
          busy={reviewBusy}
          onCorrectGraph={onCorrectGraph}
          onCorrectWord={onCorrectWord}
        />
      )}

      {caseTab === "interview" && (
        <InterviewView
          session={interview}
          busy={interviewBusy}
          onOpen={onInterviewOpen}
          onPause={onInterviewPause}
          onResume={onInterviewResume}
          onAnswer={onInterviewAnswer}
          onSkip={onInterviewSkip}
        />
      )}

      {caseTab === "batch" && (
        <BatchView
          job={batchJob}
          busy={batchBusy}
          onStart={onBatchStart}
          onResume={onBatchResume}
          onCancel={onBatchCancel}
          onSimulateKill={onBatchSimulateKill}
          onRefresh={onBatchRefresh}
        />
      )}

      {caseTab === "share" && activeCase && <BundlePanel caseId={activeCase.case_id} caseTitle={activeCase.cause_title || activeCase.label} />}

      {caseTab === "capture" && (
        <CompanionView
          session={companion}
          policy={companionPol}
          busy={companionBusy}
          onStart={onCompanionStart}
          onStop={onCompanionStop}
          onRefresh={onCompanionRefresh}
        />
      )}

      {(caseTab === "documents" || caseTab === "triage" || caseTab === "text" || caseTab === "skeleton") && (
        <div className="workspace-grid">
          <div>
            {caseTab === "documents" && (
              <div
                className={`dropzone ${dragOver ? "dropzone-active" : ""}`}
                onDragEnter={(e) => {
                  e.preventDefault();
                  setDragOver(true);
                }}
                onDragOver={(e) => {
                  e.preventDefault();
                  setDragOver(true);
                }}
                onDragLeave={() => setDragOver(false)}
                onDrop={(e) => {
                  e.preventDefault();
                  setDragOver(false);
                  if (e.dataTransfer.files?.length) onIngest(e.dataTransfer.files);
                }}
              >
                <strong>Document intake — add to case file</strong>
                <p>
                  PDF, DOCX, JPG, PNG, TXT · stored encrypted in the case vault · Drag & drop
                  supported
                </p>
                <div className="row" style={{ marginTop: "0.85rem" }}>
                  <button
                    type="button"
                    className="primary"
                    disabled={docBusy}
                    onClick={() => fileInputRef.current?.click()}
                  >
                    {docBusy ? "Uploading…" : "Choose files"}
                  </button>
                  <button
                    type="button"
                    disabled={docBusy}
                    onClick={() => folderInputRef.current?.click()}
                  >
                    Add folder
                  </button>
                  <span className="muted" style={{ fontSize: "0.82rem" }}>
                    Originals never leave this machine. Type tag is editable after upload.
                  </span>
                </div>
                <input
                  ref={(el) => {
                    (fileInputRef as React.MutableRefObject<HTMLInputElement | null>).current = el;
                  }}
                  type="file"
                  multiple
                  accept=".pdf,.docx,.jpg,.jpeg,.png,.txt,application/pdf,text/plain,image/*"
                  hidden
                  onChange={(e) => {
                    if (e.target.files) onIngest(e.target.files);
                    e.target.value = "";
                  }}
                />
                <input
                  ref={(el) => {
                    (folderInputRef as React.MutableRefObject<HTMLInputElement | null>).current = el;
                    if (el) {
                      el.setAttribute("webkitdirectory", "");
                      el.setAttribute("directory", "");
                    }
                  }}
                  type="file"
                  multiple
                  hidden
                  onChange={(e) => {
                    if (e.target.files) onIngest(e.target.files);
                    e.target.value = "";
                  }}
                />
              </div>
            )}

            {caseTab === "triage" && (
              <div className="row" style={{ marginBottom: "0.85rem" }}>
                <button
                  type="button"
                  className="primary"
                  disabled={triageBusy || documents.length === 0}
                  onClick={onTriageAll}
                >
                  {triageBusy ? "Running triage…" : "Run triage on case"}
                </button>
                <span className="chip muted">classifier: caseforge.triage.heuristic.v1</span>
              </div>
            )}

            {caseTab === "text" && (
              <div className="row" style={{ marginBottom: "0.85rem" }}>
                <button
                  type="button"
                  className="primary"
                  disabled={recognizeBusy || documents.length === 0}
                  onClick={onRecognizeAll}
                >
                  {recognizeBusy ? "Recognizing…" : "Run recognition on case"}
                </button>
                <span className="chip muted">recognizer: caseforge.recognize.provisional.v1</span>
              </div>
            )}

            {caseTab === "skeleton" && (
              <div className="row" style={{ marginBottom: "0.85rem" }}>
                <button
                  type="button"
                  className="primary"
                  disabled={skeletonBusy || documents.length === 0}
                  onClick={onSkeleton}
                >
                  {skeletonBusy ? "Extracting skeleton…" : "Run skeleton extraction"}
                </button>
                <span className="chip muted">extractor: caseforge.graph-extractor.deterministic.v2</span>
              </div>
            )}


            <div className="stat-label" style={{ margin: "1.15rem 0 0.65rem" }}>
              Documents · {documents.length}
            </div>

            {documents.length === 0 ? (
              <div className="card empty">
                <h3>No documents yet</h3>
                <p>Drop pleadings, scans, and exhibits into the encrypted case vault.</p>
              </div>
            ) : (
              <div className="table-wrap">
                <table className="table">
                  <thead>
                    <tr>
                      <th>Name</th>
                      <th>Type</th>
                      <th>Pages</th>
                      <th>PDF quality</th>
                      <th>Language</th>
                      <th>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {documents.map((d) => (
                      <tr
                        key={d.doc_id}
                        className={selectedDocId === d.doc_id ? "selected" : undefined}
                        onClick={() => setSelectedDocId(d.doc_id)}
                        style={{ cursor: "pointer" }}
                      >
                        <td className="matter-title">{d.original_filename}</td>
                        <td onClick={(e) => e.stopPropagation()}>
                          <select
                            value={
                              TYPE_TAG_OPTIONS.includes(
                                d.type_tag as (typeof TYPE_TAG_OPTIONS)[number],
                              )
                                ? d.type_tag
                                : "OTHER"
                            }
                            disabled={docBusy}
                            onChange={(e) => onTypeTag(d.doc_id, e.target.value)}
                            style={{
                              fontFamily: "var(--font-ui)",
                              fontSize: "0.78rem",
                              letterSpacing: "0.06em",
                              textTransform: "uppercase",
                              padding: "0.35rem 0",
                              minWidth: 72,
                            }}
                          >
                            {TYPE_TAG_OPTIONS.map((t) => (
                              <option key={t} value={t}>
                                {t}
                              </option>
                            ))}
                          </select>
                        </td>
                        <td className="mono">{d.page_count ?? "—"}</td>
                        <td className="mono">
                          {d.source_format !== "pdf"
                            ? "n/a"
                            : (() => {
                                const states = pageSources.filter((p) => p.doc_id === d.doc_id);
                                if (!states.length) return "not parsed";
                                const kinds = Array.from(new Set(states.map((p) => p.source_kind)));
                                const ocr = states.filter((p) => p.state === "ocr_required").length;
                                return `${kinds.join("+")}${ocr ? ` · OCR ${ocr}` : " · native text"}`;
                              })()}
                        </td>
                        <td className="mono">{d.language_tag || "EN"}</td>
                        <td onClick={(e) => e.stopPropagation()}>
                          <div className="row" style={{ gap: "0.35rem", flexWrap: "wrap" }}>
                            <button
                              type="button"
                              disabled={triageBusy}
                              onClick={() => onTriageDoc(d.doc_id)}
                            >
                              Triage
                            </button>
                            <button
                              type="button"
                              disabled={recognizeBusy}
                              onClick={() => onRecognizeDoc(d.doc_id)}
                            >
                              Recognize
                            </button>
                          </div>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          
            {caseTab === "skeleton" && (
              <div className="card" style={{ marginTop: "1rem" }}>
                <div className="stat-label">Case graph · chronology</div>
                {!caseGraph ? (
                  <p className="muted">No case_graph yet. Run recognition (optional) then skeleton extraction.</p>
                ) : (
                  <>
                    <p className="mono muted" style={{ fontSize: "0.75rem" }}>
                      schema {caseGraph.schema_version} · parties {caseGraph.parties?.length ?? 0} ·
                      events {caseGraph.chronology?.length ?? 0} · statutes {caseGraph.statutes?.length ?? 0} ·
                      prep {caseGraph.completeness?.score_0_100 ?? "—"}%
                    </p>
                    <div className="table-wrap" style={{ marginTop: "0.65rem" }}>
                      <table className="table">
                        <thead>
                          <tr>
                            <th>Date</th>
                            <th>Summary</th>
                            <th>Conf</th>
                            <th>Source</th>
                          </tr>
                        </thead>
                        <tbody>
                          {(caseGraph.chronology || []).slice(0, 40).map((e) => {
                            const src = e.provenance?.source;
                            const page =
                              src && !Array.isArray(src)
                                ? src.page_index
                                : Array.isArray(src)
                                  ? src[0]?.page_index
                                  : undefined;
                            return (
                            <tr key={e.event_id}>
                              <td className="mono">{e.date}</td>
                              <td>{e.summary}</td>
                              <td className="mono">
                                {(e.provenance?.confidence ?? 0).toFixed(2)}
                              </td>
                              <td className="mono">
                                p.{page ?? "—"}
                              </td>
                            </tr>
                            );
                          })}
                        </tbody>
                      </table>
                    </div>
                    {(caseGraph.gaps || []).length > 0 && (
                      <div className="notice warn" style={{ marginTop: "0.85rem" }}>
                        Open gaps: {(caseGraph.gaps || []).map((g) => g.topic).join(", ")}
                      </div>
                    )}
                  </>
                )}
              </div>
            )}

          <aside className="triage-rail">
            {caseTab !== "text" ? (
              <>
                <div className="row" style={{ justifyContent: "space-between", marginTop: 0 }}>
                  <span className="eyebrow">docpipe · triage</span>
                  <span className={`chip ${running ? "ok" : "muted"}`}>
                    <span className={`status-dot ${running ? "on" : "off"}`} />
                    {running ? "Running" : "Idle"}
                  </span>
                </div>
                <h3>Document triage</h3>
                <p className="muted" style={{ fontSize: "0.85rem", marginBottom: "0.35rem" }}>
                  Page classes · offline · B-S1-05
                </p>
                <div className="meter" aria-hidden>
                  <span
                    style={{ width: `${running ? progress : triageRows.length ? 100 : 0}%` }}
                  />
                </div>
                <p className="mono muted" style={{ fontSize: "0.72rem" }}>
                  {running
                    ? `${progress}% · ${triageJob?.message || "working"}`
                    : triageRows.length
                      ? `${triageRows.length} page labels`
                      : "No results yet"}
                </p>
                <button
                  type="button"
                  className="primary"
                  style={{ width: "100%", marginTop: "0.85rem" }}
                  disabled={triageBusy || documents.length === 0}
                  onClick={onTriageAll}
                >
                  Run triage on case
                </button>
                <button
                  type="button"
                  style={{ width: "100%", marginTop: "0.45rem" }}
                  disabled={triageBusy || !selectedDocId}
                  onClick={() => selectedDocId && onTriageDoc(selectedDocId)}
                >
                  Re-run selected document
                </button>
                <div className="class-chips">
                  {TRIAGE_CLASSES.map((c) => (
                    <span key={c}>{c}</span>
                  ))}
                </div>
                {selectedPages.length === 0 ? (
                  <p className="muted" style={{ fontSize: "0.85rem" }}>
                    Run triage to classify pages for the selected document.
                  </p>
                ) : (
                  selectedPages.map((r) => (
                    <div
                      key={`${r.doc_id}-${r.page_index}`}
                      className={`page-row ${r.confidence < 0.75 ? "low" : ""}`}
                    >
                      <span>p.{String(r.page_index).padStart(2, "0")}</span>
                      <span>{r.triage_class}</span>
                      <span>{r.confidence.toFixed(2)}</span>
                    </div>
                  ))
                )}
              </>
            ) : (
              <>
                <div className="row" style={{ justifyContent: "space-between", marginTop: 0 }}>
                  <span className="eyebrow">docpipe · recognize</span>
                  <span className={`chip ${recogRunning ? "ok" : "muted"}`}>
                    <span className={`status-dot ${recogRunning ? "on" : "off"}`} />
                    {recogRunning ? "Running" : "Idle"}
                  </span>
                </div>
                <h3>Text layer</h3>
                <p className="muted" style={{ fontSize: "0.85rem", marginBottom: "0.35rem" }}>
                  Offline English OCR · engine confidence · positioned words · B-S3-04
                </p>
                <div className="meter" aria-hidden>
                  <span
                    style={{
                      width: `${recogRunning ? recogProgress : textLayers.length ? 100 : 0}%`,
                    }}
                  />
                </div>
                <p className="mono muted" style={{ fontSize: "0.72rem" }}>
                  {recogRunning
                    ? `${recogProgress}% · ${recognizeJob?.message || "working"}`
                    : textLayers.length
                      ? `${textLayers.length} page(s) · ${lowWordCount} low-conf words`
                      : "No text layer yet"}
                </p>
                <div className="notice warn" style={{ margin: "0.85rem 0", fontSize: "0.82rem" }}>
                  Warned or empty pages require human review and are blocked from automatic extraction.
                </div>
                <button
                  type="button"
                  className="primary"
                  style={{ width: "100%" }}
                  disabled={recognizeBusy || documents.length === 0}
                  onClick={onRecognizeAll}
                >
                  Run recognition on case
                </button>
                <button
                  type="button"
                  style={{ width: "100%", marginTop: "0.45rem" }}
                  disabled={recognizeBusy || !selectedDocId}
                  onClick={() => selectedDocId && onRecognizeDoc(selectedDocId)}
                >
                  Re-run selected document
                </button>
                {selectedLayers.length === 0 ? (
                  <p className="muted" style={{ fontSize: "0.85rem", marginTop: "0.85rem" }}>
                    Run recognition to build a word-level text layer for highlights.
                  </p>
                ) : (
                  selectedLayers.map((layer) => (
                    <div key={`${layer.doc_id}-${layer.page_index}`} className="text-layer-card">
                      <div className="page-row">
                        <span>p.{String(layer.page_index).padStart(2, "0")}</span>
                        <span>
                          mean {layer.mean_confidence.toFixed(2)}
                          {layer.page_quality_warn ? " · warn" : ""}
                        </span>
                        <span>{layer.words.length}w</span>
                      </div>
                      {layer.recognizer_id === "caseforge.ocr.tesseract-eng-fast.v1" &&
                      layer.words.some((word) => word.bbox_json) ? (
                        <button
                          type="button"
                          className="ocr-preview-button"
                          onClick={async () => {
                            setOcrPreviewError(null);
                            try {
                              const result = await docpipePageImage(
                                activeCase.case_id,
                                layer.doc_id,
                                layer.page_index,
                              );
                              const key = `${layer.doc_id}-${layer.page_index}`;
                              setOcrPageImages((current) => ({
                                ...current,
                                [key]: `data:${result.media_type};base64,${result.image_base64}`,
                              }));
                            } catch (error) {
                              setOcrPreviewError(String(error));
                            }
                          }}
                        >
                          {ocrPageImages[`${layer.doc_id}-${layer.page_index}`]
                            ? "Refresh source overlay"
                            : "Inspect words on source"}
                        </button>
                      ) : null}
                      {ocrPageImages[`${layer.doc_id}-${layer.page_index}`] ? (
                        <div
                          className="ocr-page-overlay"
                          aria-label={`OCR source overlay page ${layer.page_index + 1}`}
                        >
                          <img
                            src={ocrPageImages[`${layer.doc_id}-${layer.page_index}`]}
                            alt={`Source page ${layer.page_index + 1}`}
                          />
                          {layer.words.map((word) => {
                            let box: { x: number; y: number; w: number; h: number } | null = null;
                            try {
                              box = JSON.parse(word.bbox_json || "null");
                            } catch {
                              box = null;
                            }
                            return box ? (
                              <span
                                key={word.word_index}
                                className={`ocr-word-box${
                                  word.force_review ? " force" : word.low_confidence ? " low" : ""
                                }`}
                                style={{
                                  left: `${box.x * 100}%`,
                                  top: `${box.y * 100}%`,
                                  width: `${box.w * 100}%`,
                                  height: `${box.h * 100}%`,
                                }}
                                title={`${word.text} · ${word.confidence.toFixed(2)}`}
                              />
                            ) : null;
                          })}
                        </div>
                      ) : null}
                      <div className="word-flow">
                        {layer.words.slice(0, 80).map((w) => (
                          <span
                            key={w.word_index}
                            className={
                              w.force_review
                                ? "word force"
                                : w.low_confidence
                                  ? "word low"
                                  : "word"
                            }
                            title={`conf=${w.confidence.toFixed(2)}${
                              w.force_review ? " · force review" : ""
                            }`}
                          >
                            {w.text}
                          </span>
                        ))}
                        {layer.words.length > 80 ? (
                          <span className="word muted">…</span>
                        ) : null}
                      </div>
                    </div>
                  ))
                )}
                {ocrPreviewError ? <p className="notice warn">{ocrPreviewError}</p> : null}
              </>
            )}
          </aside>
        </div>
      )}

      <div className="matter-strip">
        <span>
          Case vault · open · {documents.length} documents · {pageTotal || triageRows.length} pages
          {triageJob ? ` · triage ${triageJob.state}` : ""}
          {recognizeJob ? ` · recognize ${recognizeJob.state}` : ""}
          {skeletonJob ? ` · skeleton ${skeletonJob.state}` : ""}
          {caseGraph?.completeness?.score_0_100 != null
            ? ` · prep ${caseGraph.completeness.score_0_100}%`
            : ""}
        </span>
        <span>Source pointers required for all extracted facts (Stage 1)</span>
      </div>
    </div>
  );
}

function SettingsView({
  tauri,
  vault,
  version,
  sidecars,
  llm,
  maxFrameGapMs,
  heavyProbeBusy,
  onRefresh,
  onRefreshLlm,
  onHeavyProbe,
  choreo,
  onRefreshChoreo,
  onUnloadModels,
}: {
  tauri: boolean;
  vault: VaultStatus | null;
  version: VersionResult | null;
  sidecars: SidecarStatusResult | null;
  llm: LlmStatus | null;
  maxFrameGapMs: number;
  heavyProbeBusy: boolean;
  onRefresh: () => void;
  onRefreshLlm: () => void;
  onHeavyProbe: () => void;
  choreo: ChoreographyStatus | null;
  onRefreshChoreo: () => void;
  onUnloadModels: () => void;
}) {
  const workers =
    sidecars?.sidecars ??
    ([
      { name: "infer-llm", state: "idle", detail: "stub" },
      { name: "infer-speech", state: "idle", detail: "stub" },
      { name: "docpipe", state: "idle", detail: "heuristic triage" },
    ] as const);

  const llmReady = Boolean(llm?.controlled_beta_ready);
  const vaultOpen = Boolean(vault?.app_unlocked);

  return (
    <div className="page-grid">
      <div className="card hero-card">
        <div>
          <div className="eyebrow">System</div>
          <h3>Vault, sidecars, and runtime</h3>
          <p>
            CaseForge runs offline on this machine. Heavy work stays in Rust / Python sidecars so
            the UI never freezes longer than 200 ms. Case content has no cloud path.
          </p>
          <div className="row">
            <button type="button" className="primary" onClick={onRefresh}>
              Refresh status
            </button>
            <button type="button" onClick={onRefreshLlm}>
              Probe LLM
            </button>
            <button type="button" disabled={heavyProbeBusy || !tauri} onClick={onHeavyProbe}>
              {heavyProbeBusy ? "Heavy probe…" : "Probe UI freeze (AC-3.5)"}
            </button>
          </div>
        </div>
        <div className="hero-metrics">
          <div>
            <div className="big" style={{ fontSize: "1.35rem" }}>
              {tauri ? "Desktop" : "Preview"}
            </div>
            <div className="label">Mode</div>
          </div>
          <div className="label">v{version?.version ?? "—"}</div>
          <div className="label">
            max_frame_gap {maxFrameGapMs} ms / {UI_FREEZE_BUDGET_MS}
          </div>
        </div>
      </div>

      <div className="card" data-testid="choreography-panel">
        <span className="eyebrow">B-S1-20 · model choreography</span>
        <h3>Sequential load · one large model</h3>
        <p className="muted" style={{ fontSize: "0.88rem" }}>
          Primary reasoner and vision never concurrent on 8 GB (hard fail). Idle unload after{" "}
          {choreo?.idle_unload_after_s ?? 120}s. Status chip required in the top bar.
        </p>
        {choreo ? (
          <ul className="gap-list">
            <li>
              chip: <strong data-testid="choreo-chip-label">{choreo.chip_label}</strong>
            </li>
            <li>
              tier: {choreo.tier} · live_sim={String(choreo.live_simulation_allowed)}
            </li>
            <li>
              large resident: {choreo.large_resident_count}/{choreo.max_large_resident}
            </li>
            <li>
              primary+vision concurrent:{" "}
              <strong>{String(choreo.primary_and_vision_concurrent)}</strong>
            </li>
            <li>
              RSS: {choreo.budgets.process_rss_mb ?? "—"} MB · budget{" "}
              {choreo.budgets.total_rss_max_gb} GB · ok={String(choreo.budgets.rss_ok)}
            </li>
            <li>
              idle RAM ok (models unloaded): {String(choreo.budgets.idle_ram_ok)} · budget{" "}
              {choreo.budgets.idle_ram_max_mb} MB
            </li>
            <li className="mono muted" style={{ fontSize: "0.72rem" }}>
              {choreo.budgets.measurement_notes}
            </li>
          </ul>
        ) : (
          <p className="muted">Choreography status unavailable outside Tauri shell.</p>
        )}
        <div className="row" style={{ gap: "0.4rem", flexWrap: "wrap" }}>
          <button type="button" onClick={onRefreshChoreo} disabled={!tauri}>
            Refresh choreography
          </button>
          <button type="button" className="primary" onClick={onUnloadModels} disabled={!tauri}>
            Unload all models
          </button>
        </div>
      </div>

      <div className="stat-label">Runtime pulse</div>
      <div className="system-stamps">
        <div className={`system-stamp ${tauri ? "ok" : "warn"}`}>
          <span className="stamp-name">Shell</span>
          <span className="stamp-state">{tauri ? "ok" : "browser"}</span>
          <span className="stamp-detail">{version?.product ?? "CaseForge"}</span>
        </div>
        <div className={`system-stamp ${vaultOpen ? "ok" : "warn"}`}>
          <span className="stamp-name">Vault</span>
          <span className="stamp-state">{vaultOpen ? "unlocked" : "locked"}</span>
          <span className="stamp-detail">{vault?.case_count_registered ?? 0} matters</span>
        </div>
        <div className="system-stamp ok">
          <span className="stamp-name">Docpipe</span>
          <span className="stamp-state">
            {workers.find((w) => w.name.includes("docpipe"))?.state ?? "idle"}
          </span>
          <span className="stamp-detail">triage heuristic</span>
        </div>
        <div className={`system-stamp ${llmReady ? "ok" : "warn"}`}>
          <span className="stamp-name">LLM</span>
          <span className="stamp-state">{llmReady ? "beta-ready" : llm?.ready ? "unsupported" : "offline"}</span>
          <span className="stamp-detail">{llm?.model || llm?.backend || "none"}</span>
        </div>
      </div>

      <div className="page-grid cols-2">
        <div className="card">
          <h3>Local LLM brain</h3>
          <p style={{ marginBottom: "0.85rem" }}>
            Offline argument builder and Mock Court reasoner. Controlled beta requires the exact
            allowlisted model, runtime, prompt, and digest shown below.
          </p>
          <table className="table">
            <tbody>
              <tr>
                <th>Controlled beta</th>
                <td>{llm ? (llm.controlled_beta_ready ? "ready" : "not ready") : "—"}</td>
              </tr>
              <tr>
                <th>Backend</th>
                <td className="mono">{llm?.backend ?? "—"}</td>
              </tr>
              <tr><th>Runtime</th><td className="mono">{llm?.backend_version ?? "—"}</td></tr>
              <tr>
                <th>Model</th>
                <td className="mono">{llm?.model || "—"}</td>
              </tr>
              <tr><th>Digest</th><td className="mono" title={llm?.model_digest}>{llm?.model_digest ? `${llm.model_digest.slice(0, 22)}…` : "—"}</td></tr>
              <tr><th>Build</th><td>{llm ? `${llm.parameters} · ${llm.quantization} · ctx ${llm.context_length}` : "—"}</td></tr>
              <tr><th>Classification</th><td>{llm?.classification ?? "—"}</td></tr>
              <tr><th>Legal fine-tune</th><td>{llm ? (llm.legally_fine_tuned && llm.checkpoint_evidence ? "verified" : "no — no checkpoint evidence") : "—"}</td></tr>
              <tr><th>Tier</th><td>{llm?.tier === "addon" ? "optional add-on" : "standard (base install)"}</td></tr>
              <tr><th>Prompt</th><td className="mono">{llm?.prompt_version ?? "—"}</td></tr>
              <tr><th>Licence</th><td>{llm?.license ?? "—"}</td></tr>
              <tr>
                <th>Cloud</th>
                <td>never</td>
              </tr>
              <tr>
                <th>Detail</th>
                <td className="muted">{llm?.detail || "—"}</td>
              </tr>
            </tbody>
          </table>
          {!llmReady && (
            <div className="notice warn" style={{ marginTop: "0.85rem" }}>
              <div>
                {llm?.ready
                  ? "A local model is running, but its configuration is not allowlisted for controlled-beta claims."
                  : <><span className="mono">ollama serve</span> then <span className="mono">ollama pull {llm?.model || "<model in contracts/supported_models.json>"}</span></>}
              </div>
              <div style={{ marginTop: "0.35rem" }}>
                Or set <span className="mono">CASEFORGE_GGUF_PATH</span> /{" "}
                <span className="mono">CASEFORGE_OPENAI_BASE</span>
              </div>
            </div>
          )}
          {tauri && <ModelTierSelector llm={llm} onChanged={() => onRefreshLlm()} />}
          <div className="row">
            <button type="button" className="primary" onClick={onRefreshLlm}>
              Probe LLM
            </button>
          </div>
        </div>

        <div className="card">
          <h3>Vault</h3>
          <p style={{ marginBottom: "0.85rem" }}>
            SQLCipher + Argon2id. Passphrase never leaves this device. Wrong passphrase cannot
            decrypt case data.
          </p>
          <table className="table">
            <tbody>
              <tr>
                <th>Root</th>
                <td className="mono" style={{ wordBreak: "break-all" }}>
                  {vault?.root_dir ?? "—"}
                </td>
              </tr>
              <tr>
                <th>Unlocked</th>
                <td>{vault ? String(vault.app_unlocked) : "—"}</td>
              </tr>
              <tr>
                <th>Cases</th>
                <td>{vault?.case_count_registered ?? "—"}</td>
              </tr>
              <tr>
                <th>Crypto</th>
                <td className="mono">
                  {vault
                    ? `${vault.crypto.engine} · m=${vault.crypto.memory_kib} · t=${vault.crypto.iterations} · p=${vault.crypto.parallelism}`
                    : "—"}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <div className="card">
        <h3>Sidecars</h3>
        <p style={{ marginBottom: "0.85rem" }}>
          Local AI workers. Models load one at a time under choreography limits. Until vision
          bake-off freezes a classifier, docpipe triage stays heuristic.
        </p>
        <div className="table-wrap">
          <table className="table">
            <thead>
              <tr>
                <th>Sidecar</th>
                <th>State</th>
                <th>Detail</th>
              </tr>
            </thead>
            <tbody>
              {workers.map((s) => (
                <tr key={s.name}>
                  <td className="mono">{s.name}</td>
                  <td>
                    <span
                      className={`chip ${s.state === "error" ? "warn" : s.state === "idle" ? "muted" : "ok"}`}
                    >
                      {s.state}
                    </span>
                  </td>
                  <td className="muted">{s.detail}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      <div className="system-pulse-bar">
        <span>
          Shell · {tauri ? "ok" : "preview"} · Vault · {vaultOpen ? "unlocked" : "locked"} · Docpipe
          · idle · LLM · {llmReady ? "ready" : "offline"}
        </span>
        <span>Heavy work never freezes the UI &gt; 200 ms</span>
      </div>
    </div>
  );
}
