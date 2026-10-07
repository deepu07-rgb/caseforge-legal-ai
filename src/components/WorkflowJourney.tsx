import { nextSafeStage, type WorkflowStage, type WorkflowStageId } from "../lib/lawyerWorkflow";

const STATE_LABEL:Record<WorkflowStage["state"],string>={EMPTY:"Not started",LOADING:"Working",PARTIAL:"Needs attention",FAILED:"Failed",BLOCKED:"Blocked",CANCELLED:"Cancelled",STALE:"Stale",READY:"Ready",COMPLETE:"Complete"};
export default function WorkflowJourney({stages,onOpen}:{stages:WorkflowStage[];onOpen:(id:WorkflowStageId)=>void}){
  const next=nextSafeStage(stages);
  return <section className="workflow-journey" aria-labelledby="workflow-title" data-testid="lawyer-workflow">
    <div className="workflow-head"><div><span className="eyebrow">Case preparation journey</span><h3 id="workflow-title">What to do next</h3><p className="muted">Progress comes from saved matter state and verified readiness—not from page visits.</p></div><button type="button" className="primary" onClick={()=>onOpen(next.id)} data-testid="workflow-next">{next.actionLabel}</button></div>
    <ol className="workflow-list">
      {stages.map(stage=><li key={stage.id} className={`workflow-step state-${stage.state.toLowerCase()}`} data-workflow-state={stage.state}>
        <button type="button" onClick={()=>onOpen(stage.id)} aria-label={`${stage.order}. ${stage.label}. ${STATE_LABEL[stage.state]}. ${stage.detail}`}>
          <span className="workflow-number" aria-hidden="true">{stage.order}</span><span className="workflow-copy"><strong>{stage.label}</strong><span>{stage.purpose}</span><small>{stage.detail}</small></span><span className="workflow-state"><span aria-hidden="true">{stage.state==="COMPLETE"?"✓":stage.state==="BLOCKED"?"⊘":stage.state==="STALE"?"↻":"→"}</span> {STATE_LABEL[stage.state]}</span>
        </button>
      </li>)}
    </ol>
  </section>;
}
