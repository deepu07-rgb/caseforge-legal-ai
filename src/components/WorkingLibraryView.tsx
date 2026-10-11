import { useEffect, useState } from "react";
import { CurrencyStamp, TreatmentBlock } from "./CurrencyStamp";
import {
  workingSetAddChunk, workingSetConfirmRegime, workingSetGet, workingSetSuggest, workingSetUpdateItem,
  type LegalWorkingSet, type RetrievalResult, type WorkingSetDecision,
} from "../lib/ipc";

export default function WorkingLibraryView({ caseId, retrievalResult, onError, onNotice }:{caseId:string|null;retrievalResult:RetrievalResult|null;onError:(s:string)=>void;onNotice:(s:string)=>void}){
  const [set,setSet]=useState<LegalWorkingSet|null>(null); const [busy,setBusy]=useState(false); const [expanded,setExpanded]=useState<string|null>(null);
  const act=async(fn:()=>Promise<LegalWorkingSet>,notice:string)=>{setBusy(true);try{const next=await fn();setSet(next);onNotice(notice);}catch(e){onError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}};
  useEffect(()=>{if(!caseId){setSet(null);return;} void workingSetGet(caseId).then(setSet).catch(()=>setSet(null));},[caseId]);
  if(!caseId)return <div className="notice warn">Open a matter to create its counsel-controlled legal working set.</div>;
  const update=(itemId:string,decision:WorkingSetDecision,pinned:boolean,controlling:boolean)=>act(()=>workingSetUpdateItem({caseId,itemId,decision,pinned,controlling}),"Working-set counsel decision saved.");
  return <section className="card" data-testid="working-library">
    <div className="row" style={{justifyContent:"space-between"}}><div><span className="eyebrow">matter legal working set</span><h3>Law relied on for this matter</h3></div><span className="chip muted">version {set?.version??0}</span></div>
    <p className="muted">Proposals never become controlling automatically. Counsel decisions survive retrieval, index, model, and pack reruns.</p>
    <div className="row" style={{gap:".5rem",flexWrap:"wrap"}}><button className="primary" disabled={busy} onClick={()=>void act(()=>workingSetSuggest(caseId),"Authority proposals refreshed without overwriting counsel decisions.")}>Suggest from matter</button>
      {retrievalResult?.retrieved.map((h)=><button key={h.chunk_id} disabled={busy} onClick={()=>void act(()=>workingSetAddChunk(caseId,h.chunk_id),"Retrieved authority added as a proposal.")}>Add retrieval #{retrievalResult.retrieved.indexOf(h)+1}</button>)}</div>
    {set && <>
      {set.suppressed_proposals && set.suppressed_proposals.length>0 && <p className="muted" style={{fontSize:".85rem"}} data-testid="suppressed-proposals">Left out of the suggestions as not fitting this matter ({set.suppressed_proposals[0].reason}): {set.suppressed_proposals.map((s)=>`${s.title} ${s.section_label}`.trim()).join("; ")}. Search for them yourself if you need them.</p>}
      <div className={`notice ${set.regime_status==="COUNSEL_CONFIRMED"?"":"warn"}`} style={{marginTop:"1rem"}} data-testid="regime-confirmation">
        <strong>Applicable-law regime: {set.regime_status.replaceAll("_"," ")}</strong>
        {set.regime_candidates.map((r)=><div key={r.regime_id} style={{marginTop:".5rem"}}><div><strong>{r.label}</strong> — {r.reason}{r.regime_id.endsWith("_REQUIRED")?" Selecting this identifies a repository gap and blocks downstream use until its authoritative pack is installed.":""}</div><button disabled={busy||set.confirmed_regime_id===r.regime_id} onClick={()=>void act(()=>workingSetConfirmRegime(caseId,r.regime_id),"Applicable-law regime confirmed by counsel.")}>{set.confirmed_regime_id===r.regime_id?"Confirmed":"Confirm this regime"}</button></div>)}
      </div>
      <div className="stat-label" style={{marginTop:"1rem"}}>{set.items.length} authorities · {set.usable_authority_ids.length} controlling and usable</div>
      {set.items.length===0?<p className="muted">No authorities yet. Generate proposals or add a retrieved result.</p>:<ul className="retrieval-hits">{set.items.map((item)=><li key={item.item_id} data-testid="working-set-item">
        <div className="row" style={{justifyContent:"space-between",gap:".5rem"}}><strong>{item.title} · {item.section_label}</strong><span className="chip muted">{item.decision}</span></div>
        <div className="mono muted" style={{fontSize:".7rem"}}>{item.citation} · {item.jurisdiction} · {item.authority_type} · {item.trust_level} · temporal={item.temporal_status} · supersession={item.superseded_status}</div>
        <CurrencyStamp packId={item.pack_id}/>
        {item.authority_type==="JUDGMENT"&&<TreatmentBlock sourceId={item.source_id} chunkId={item.chunk_id}/>}
        <div className="row" style={{gap:".4rem",flexWrap:"wrap",marginTop:".5rem"}}>
          <button onClick={()=>void update(item.item_id,"INCLUDED",item.pinned,false)}>Include</button><button onClick={()=>void update(item.item_id,"EXCLUDED",item.pinned,false)}>Exclude</button><button onClick={()=>void update(item.item_id,"IRRELEVANT",item.pinned,false)}>Irrelevant</button><button onClick={()=>void update(item.item_id,"REMOVED",false,false)}>Remove</button>
          <button onClick={()=>void update(item.item_id,item.decision,!item.pinned,item.controlling)}>{item.pinned?"Unpin":"Pin"}</button><button disabled={item.decision!=="INCLUDED"||item.temporal_status!=="APPLICABLE"||item.superseded_status!=="CURRENT"} onClick={()=>void update(item.item_id,"INCLUDED",item.pinned,!item.controlling)}>{item.controlling?"Unconfirm controlling":"Confirm controlling"}</button>
          <button onClick={()=>setExpanded(expanded===item.item_id?null:item.item_id)}>{expanded===item.item_id?"Hide exact text":"Inspect exact text"}</button>
        </div>
        {expanded===item.item_id&&<div className="notice" style={{marginTop:".6rem"}} data-testid="working-set-exact-text"><div className="mono muted" style={{fontSize:".68rem"}}>source={item.source_id} · chunk={item.chunk_id}<br/>source checksum={item.source_checksum}<br/>chunk checksum={item.chunk_checksum}<br/>pack={item.pack_id} · path={item.source_path}<br/>official reference={item.source_reference}</div><pre style={{whiteSpace:"pre-wrap",maxHeight:"22rem",overflow:"auto"}}>{item.exact_text}</pre></div>}
      </li>)}</ul>}
    </>}
  </section>;
}
