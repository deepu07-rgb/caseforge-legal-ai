import type {
  BatchJob,
  CaseGraph,
  DocumentMeta,
  InterviewSession,
  MockCourtReadiness,
  PageSourceRow,
  PreparationTask,
  TriageJobStatus,
} from "./ipc";

export type WorkflowStageId = "material"|"quality"|"understanding"|"missing_facts"|"digest"|"authorities"|"argument"|"mock_court"|"tasks";
export type WorkflowState = "EMPTY"|"LOADING"|"PARTIAL"|"FAILED"|"BLOCKED"|"CANCELLED"|"STALE"|"READY"|"COMPLETE";
export interface WorkflowStage { id:WorkflowStageId; order:number; label:string; purpose:string; state:WorkflowState; detail:string; actionLabel:string; }
export interface WorkflowInputs {
  hasCase:boolean; documents:DocumentMeta[]; pageSources:PageSourceRow[]; triageJob:TriageJobStatus|null; triageBusy:boolean;
  graph:CaseGraph|null; skeletonJob:TriageJobStatus|null; skeletonBusy:boolean; interview:InterviewSession|null; interviewBusy:boolean;
  readiness:MockCourtReadiness|null; readinessLoading:boolean; readinessError:string|null; argumentTree:Record<string,unknown>|null;
  argumentLoading:boolean; tasks:PreparationTask[]; tasksLoading:boolean; batchJob:BatchJob|null;
}

const jobState=(job:TriageJobStatus|null,busy:boolean):WorkflowState|null=>busy||job?.state==="running"?"LOADING":job?.state==="failed"?"FAILED":null;
const graphStale=(documents:DocumentMeta[],graph:CaseGraph|null):boolean=>{
  if(!graph||documents.length===0)return false;
  const generated=Date.parse(graph.generated_at||"");
  const newest=Math.max(...documents.map((d)=>d.ingested_at_unix_ms));
  return !Number.isFinite(generated)||newest>generated;
};

export function deriveLawyerWorkflow(i:WorkflowInputs):WorkflowStage[]{
  if(!i.hasCase)return [
    {id:"material",order:1,label:"Add Case Material",purpose:"Create or open a matter and add pleadings, exhibits, and records.",state:"EMPTY",detail:"No matter is open.",actionLabel:"Open or create a matter"},
    ...(["quality","understanding","missing_facts","digest","authorities","argument","mock_court","tasks"] as WorkflowStageId[]).map((id,n)=>({id,order:n+2,label:["Review Document Quality","Review What CaseForge Understood","Complete Missing Facts","Case Digest","Legal Authorities","Argument Analysis","Mock Court","Preparation Tasks"][n],purpose:"Complete the earlier preparation stage first.",state:"BLOCKED" as const,detail:"Open a matter and add case material first.",actionLabel:"Go to first required step"}))
  ];
  const docs=i.documents.length;
  const material:WorkflowStage={id:"material",order:1,label:"Add Case Material",purpose:"Keep the encrypted matter file complete.",state:docs?"COMPLETE":"EMPTY",detail:docs?`${docs} encrypted document${docs===1?"":"s"} in this matter.`:"No documents have been added.",actionLabel:docs?"Review case material":"Add files"};
  const qualityJob=jobState(i.triageJob,i.triageBusy);
  const expectedPages=i.documents.reduce((n,d)=>n+(d.page_count||0),0);
  const unsafePages=i.pageSources.filter(p=>["malformed","encrypted","oversized","unsupported"].includes(p.state));
  const ocrPages=i.pageSources.filter(p=>p.state==="ocr_required");
  const incompleteQuality=i.pageSources.length===0||Boolean(expectedPages&&i.pageSources.length<expectedPages);
  const batchStatus=(i.batchJob?.status||"").toLowerCase();
  const batchState:WorkflowState|null=!incompleteQuality?null:batchStatus.includes("cancel")?"CANCELLED":batchStatus.includes("fail")?"FAILED":batchStatus.includes("run")?"LOADING":null;
  const qualityState:WorkflowState=docs===0?"BLOCKED":qualityJob||batchState||i.pageSources.length===0?qualityJob||batchState||"EMPTY":unsafePages.length||ocrPages.length||i.pageSources.length<expectedPages?"PARTIAL":"COMPLETE";
  const quality:WorkflowStage={id:"quality",order:2,label:"Review Document Quality",purpose:"Confirm every page is readable before relying on extraction.",state:qualityState,detail:qualityJob?`Document quality processing is ${qualityJob.toLowerCase()}.`:batchState?`The saved processing batch is ${batchState.toLowerCase()}. Resume or rerun the quality check.`:docs===0?"Add case material first.":i.pageSources.length===0?"Document quality has not been analysed.":`${i.pageSources.length}/${expectedPages||i.pageSources.length} pages analysed · ${ocrPages.length} need OCR · ${unsafePages.length} blocked.`,actionLabel:i.pageSources.length?"Open quality summary":"Analyse document quality"};
  const skeletonState=jobState(i.skeletonJob,i.skeletonBusy);
  const stale=graphStale(i.documents,i.graph);
  const understandingState:WorkflowState=docs===0?"BLOCKED":skeletonState||!i.graph?skeletonState||"EMPTY":stale?"STALE":"COMPLETE";
  const understanding:WorkflowStage={id:"understanding",order:3,label:"Review What CaseForge Understood",purpose:"Verify extracted facts against their source pages.",state:understandingState,detail:skeletonState?`Matter understanding is ${skeletonState.toLowerCase()}.`:!i.graph?"No case understanding has been built from the material.":stale?"Newer material exists than the current case graph. Rebuild before relying on it.":`${i.graph.chronology?.length||0} events · ${i.graph.parties?.length||0} parties · source-linked review ready.`,actionLabel:!i.graph||stale?"Build or refresh understanding":"Review extracted facts"};
  const openGaps=(i.graph?.gaps||[]).filter(g=>g.status==="open").length;
  const missingState:WorkflowState=!i.graph?"BLOCKED":i.interviewBusy?"LOADING":openGaps===0?"COMPLETE":i.interview?.status==="completed"?"PARTIAL":"READY";
  const missing:WorkflowStage={id:"missing_facts",order:4,label:"Complete Missing Facts",purpose:"Resolve explicit gaps with counsel input while keeping it separate from evidence.",state:missingState,detail:!i.graph?"Build and review the matter understanding first.":openGaps===0?"No open graph gaps remain.":`${openGaps} open gap${openGaps===1?"":"s"} · interview ${i.interview?.status||"not started"}.`,actionLabel:"Open guided missing-facts review"};
  const digest:WorkflowStage={id:"digest",order:5,label:"Case Digest",purpose:"Read the source-linked chronology, issues, evidence, and relief.",state:!i.graph?"BLOCKED":stale?"STALE":"READY",detail:!i.graph?"Matter understanding is required.":stale?"Digest is stale because newer material exists.":`Preparation completeness ${i.graph.completeness?.score_0_100??"not scored"}%.`,actionLabel:"Open case digest"};
  const authorityState:WorkflowState=i.readinessLoading?"LOADING":i.readinessError?"FAILED":!i.graph?"BLOCKED":i.readiness?.ready?"COMPLETE":i.readiness?"PARTIAL":"EMPTY";
  const authorities:WorkflowStage={id:"authorities",order:6,label:"Legal Authorities",purpose:"Confirm the applicable-law regime and current official authorities.",state:authorityState,detail:i.readinessError||(!i.graph?"Matter understanding is required.":i.readiness?.ready?`${i.readiness.usable_authority_count} confirmed current authorities available.`:(i.readiness?.blocking_reasons||["Legal working set has not been confirmed."]).join(" · ")),actionLabel:"Open legal authorities"};
  const argument:WorkflowStage={id:"argument",order:7,label:"Argument Analysis",purpose:"Inspect deterministic vulnerabilities and missing links.",state:!i.graph?"BLOCKED":i.argumentLoading?"LOADING":i.argumentTree?"COMPLETE":"READY",detail:!i.graph?"Matter understanding is required.":i.argumentTree?"Stored argument tree is available.":"Build the argument tree from the verified case graph.",actionLabel:"Open argument analysis"};
  const courtState:WorkflowState=!i.readiness?.ready?"BLOCKED":i.tasks.length?"COMPLETE":"READY";
  const court:WorkflowStage={id:"mock_court",order:8,label:"Mock Court",purpose:"Rehearse against the verified matter and legal working set.",state:courtState,detail:i.readiness?.ready?i.tasks.length?"A grounded rehearsal and debrief have been recorded.":"Safe prerequisites are satisfied; grounded rehearsal is available.":(i.readiness?.blocking_reasons||["Confirm the matter graph and legal working set first."]).join(" · "),actionLabel:i.readiness?.ready?"Open Mock Court":"Fix Mock Court prerequisites"};
  const activeTasks=i.tasks.filter(t=>t.status!=="DISMISSED");
  const taskState:WorkflowState=i.tasksLoading?"LOADING":!i.readiness?.ready?"BLOCKED":i.tasks.length===0?"EMPTY":activeTasks.every(t=>t.status==="COMPLETED")?"COMPLETE":"PARTIAL";
  const tasks:WorkflowStage={id:"tasks",order:9,label:"Preparation Tasks",purpose:"Accept, edit, dismiss, and complete source-specific rehearsal work.",state:taskState,detail:i.tasksLoading?"Loading encrypted preparation tasks.":i.tasks.length===0?"Generate a grounded Mock Court debrief first.":`${activeTasks.filter(t=>t.status==="COMPLETED").length}/${activeTasks.length} active tasks complete · ${i.tasks.filter(t=>t.status==="DISMISSED").length} dismissed.`,actionLabel:"Open preparation tasks"};
  return [material,quality,understanding,missing,digest,authorities,argument,court,tasks];
}

export function nextSafeStage(stages:WorkflowStage[]):WorkflowStage {
  return stages.find(s=>s.state!=="COMPLETE")||stages[stages.length-1];
}
