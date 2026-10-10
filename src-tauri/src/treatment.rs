//! Law Currency Stamp and Treatment Graph (B-S4-13, PRD v4.0 §7.8).
//!
//! - **Currency stamp.** Every citation shows the pack it was checked against and that pack's currency date (the date up
//!   to which its legal content was checked). A pack that states none says so; nothing is assumed.
//! - **Treatment edges.** A pack may carry `metadata/treatment_edges.json`: pack-build artefacts saying that one installed
//!   judgment treats another (overruled, reversed, distinguished, followed, doubted, per incuriam, referred to a larger
//!   Bench), each with the exact citing excerpt. They are DERIVED content: the app never trusts the builder. Whenever an
//!   edge is looked up it is re-verified here — the excerpt must be the exact text at the stated place in the installed
//!   citing passage, the cited and citing passages must be installed and different, and the excerpt must really contain
//!   the treatment wording (same rules as `eval/tools/treatment_patterns.py`) without negating it or putting it in a
//!   submission or a question. An edge that fails is not shown.
//! - **Nothing here changes a citation's standing.** Edges never alter supersession; they add notes. A cited judgment with
//!   adverse treatment (anything but "followed") shows VERIFIED_WITH_NOTE instead of VERIFIED. No edge means "no treatment
//!   found in the installed corpus" — a statement about the library, never "good law".
//!
//! Deterministic: no model, no network. Contract: contracts/treatment_edge.schema.json. Golden: golden/G-TREATMENT.

use crate::audit::{AuditItem, Note, PackSnapshot};
use crate::packs::{is_v2, list_installed, read_manifest, PackManifest};
use crate::retrieval::CorpusChunk;
use crate::vault::VaultPaths;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub const TREATMENTS: [&str; 7] = ["OVERRULED", "REVERSED", "PER_INCURIAM", "REFERRED_TO_LARGER_BENCH", "DOUBTED", "DISTINGUISHED", "FOLLOWED"];
pub const EDGES_FILE: &str = "metadata/treatment_edges.json";
/// Notes (and the VERIFIED_WITH_NOTE result) come from every treatment except "followed", which is listed on request only.
const MAX_NOTES_PER_CITATION: usize = 6;

/// (treatment, pattern id, regex) in priority order. Keep identical to eval/tools/treatment_patterns.py.
static CUES: Lazy<Vec<(&'static str, &'static str, Regex)>> = Lazy::new(|| {
    let r = |s: &str| Regex::new(s).unwrap();
    vec![
        ("PER_INCURIAM", "per-incuriam.v2", r(r"(?i)\bper\s+incuriam\b")),
        ("OVERRULED", "overruled.v2", r(r"(?i)\b(?:is|are|stands?|be|been|was|were)\s+(?:(?:hereby|accordingly|expressly|thus)\s+)?overruled\b|\bwe\s+(?:hereby\s+)?overrule\b")),
        ("REFERRED_TO_LARGER_BENCH", "larger-bench.v2", r(r"(?i)\b(?:refer|referred|referring|placed?)\b[^.]{0,80}\blarger\s+bench\b")),
        ("REVERSED", "reversed.v2", r(r"(?i)\b(?:decision|judgment|order|view)\b.{0,120}\b(?:was|were|stands|has\s+been|have\s+been)\s+reversed\s+(?:in\s+appeal|on\s+appeal|by\s+(?:this\s+court|the\s+supreme\s+court|a\s+larger\s+bench|a\s+constitution\s+bench))")),
        ("DOUBTED", "doubted.v2", r(r"(?i)\bdoubt(?:ed)?\s+the\s+correctness\b|\bcorrectness\s+of\b.{0,60}\bdoubted\b|\bwe\s+have\s+our\s+doubts\b|\b(?:has|have|had|is|are|was|were|been)\s+(?:(?:since|often|also|now|been)\s+)*doubted\b")),
        ("DISTINGUISHED", "distinguished.v2", r(r"(?i)\bdistinguishable\b|\b(?:is|are|was|were|stands?|be|been)\s+(?:(?:clearly|therefore|thus|easily)\s+)?distinguished\b|\bdistinguish(?:ed|ing)\s+(?:the\s+)?(?:decision|judgment|case|ruling)\b|\bdistinguish(?:ed|ing)\s+on\s+(?:the\s+)?facts\b")),
        ("FOLLOWED", "followed.v2", r(r"(?i)\b(?:follow|followed|following)\s+(?:the\s+)?(?:decision|judgment|view|ratio|law\s+laid\s+down|dictum|ruling)\b|\b(?:respectfully\s+)?agree\s+with\s+(?:the\s+)?(?:view|decision|judgment)\b")),
    ]
});
static NEGATION: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(?:not|never|no|neither|nor|cannot|hardly|without)\b").unwrap());
static ARGUMENT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:submit(?:s|ted)?|contend(?:s|ed)?|urge[ds]?|argu(?:e|es|ed)|plea|prayer|contention|counsel|if|whether|should|ought|may|would|seek(?:s|ing)?|propos(?:e|es|ed|al))\b").unwrap()
});
const NEGATION_WINDOW: usize = 40;

/// True if the cue (byte range `start..end` of `sentence`) is negated, or sits in a submission or a question.
fn guarded(sentence: &str, start: usize, end: usize) -> bool {
    let skip = sentence[..start].chars().count().saturating_sub(NEGATION_WINDOW);
    let from = sentence.char_indices().nth(skip).map(|(i, _)| i).unwrap_or(0);
    NEGATION.is_match(&sentence[from..end]) || ARGUMENT.is_match(&sentence[..start])
}

/// The treatment named by the first wording in `sentence` that survives the guards.
pub fn detect(sentence: &str) -> Option<(&'static str, &'static str)> {
    CUES.iter().find_map(|(name, pid, rx)| rx.find_iter(sentence).any(|m| !guarded(sentence, m.start(), m.end())).then_some((*name, *pid)))
}

/// Ok(pattern id) if `sentence` supports `treatment`; else CUE_NOT_FOUND or NEGATED_OR_ARGUED.
pub fn verify_excerpt(treatment: &str, sentence: &str) -> Result<&'static str, &'static str> {
    let mut seen = false;
    for (_, pid, rx) in CUES.iter().filter(|c| c.0 == treatment) {
        for m in rx.find_iter(sentence) {
            seen = true;
            if !guarded(sentence, m.start(), m.end()) {
                return Ok(*pid);
            }
        }
    }
    Err(if seen { "NEGATED_OR_ARGUED" } else { "CUE_NOT_FOUND" })
}

#[derive(Debug, Clone, Deserialize)]
pub struct Citing {
    pub source_id: String,
    pub chunk_id: String,
    #[serde(default)]
    pub paragraph: Option<String>,
    pub char_start: usize,
    pub char_end: usize,
    pub excerpt: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Detection {
    pub method: String,
    pub pattern_id: String,
}

/// An edge as stored in a pack (contracts/treatment_edge.schema.json).
#[derive(Debug, Clone, Deserialize)]
pub struct RawEdge {
    pub schema_version: String,
    pub edge_id: String,
    pub citing: Citing,
    pub cited_source_id: String,
    pub treatment: String,
    pub detection: Detection,
    pub content_kind: String,
    pub affects_supersession: bool,
    #[serde(default)]
    pub pack_build: Value,
}

/// An edge the app has verified against the installed text.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VerifiedEdge {
    pub edge_id: String,
    pub treatment: String,
    /// The host's own pattern that matched (the stored pattern id is only the builder's claim).
    pub pattern_id: String,
    pub cited_source_id: String,
    pub citing_pack_id: String,
    pub citing_source_id: String,
    pub citing_chunk_id: String,
    pub citing_title: String,
    pub citing_citation: String,
    pub citing_decision_date: Option<String>,
    pub paragraph: Option<String>,
    pub char_start: usize,
    pub char_end: usize,
    pub excerpt: String,
}

fn shape_ok(e: &RawEdge) -> bool {
    e.schema_version == "1.0.0"
        && !e.edge_id.is_empty()
        && !e.cited_source_id.is_empty()
        && !e.citing.source_id.is_empty()
        && !e.citing.chunk_id.is_empty()
        && !e.citing.excerpt.is_empty()
        && e.citing.char_end > e.citing.char_start
        && TREATMENTS.contains(&e.treatment.as_str())
        && e.detection.method == "PATTERN_PLUS_VERIFIER"
        && !e.detection.pattern_id.is_empty()
        && e.content_kind == "DERIVED"
        && !e.affects_supersession
}

/// Verifies one stored edge against the installed passage it cites from. `citing` is that passage (None if it is not
/// installed); `known` says whether a source is installed.
pub fn verify_edge(e: &RawEdge, citing: Option<&CorpusChunk>, known: &dyn Fn(&str) -> bool) -> Result<VerifiedEdge, &'static str> {
    if !shape_ok(e) {
        return Err("BAD_SHAPE");
    }
    let chunk = citing.ok_or("CHUNK_NOT_INSTALLED")?;
    if chunk.chunk_id != e.citing.chunk_id || chunk.metadata.source_id != e.citing.source_id {
        return Err("SOURCE_MISMATCH");
    }
    if e.cited_source_id == e.citing.source_id {
        return Err("SELF_REFERENCE");
    }
    if !known(&e.cited_source_id) {
        return Err("CITED_NOT_INSTALLED");
    }
    let (s, t) = (e.citing.char_start, e.citing.char_end);
    let at: String = chunk.text.chars().skip(s).take(t - s).collect();
    if at != e.citing.excerpt {
        return Err("EXCERPT_MISMATCH");
    }
    let pattern = verify_excerpt(&e.treatment, &e.citing.excerpt)?;
    Ok(VerifiedEdge {
        edge_id: e.edge_id.clone(),
        treatment: e.treatment.clone(),
        pattern_id: pattern.to_string(),
        cited_source_id: e.cited_source_id.clone(),
        citing_pack_id: chunk.pack_id.clone(),
        citing_source_id: e.citing.source_id.clone(),
        citing_chunk_id: chunk.chunk_id.clone(),
        citing_title: chunk.metadata.title.clone(),
        citing_citation: chunk.metadata.citation.clone(),
        citing_decision_date: Some(chunk.metadata.effective_from.clone()).filter(|d| d != "UNKNOWN"),
        // the installed passage knows its own paragraph label; the stored one is only the builder's claim
        paragraph: Some(chunk.metadata.paragraph.clone()).filter(|p| !p.is_empty()).or_else(|| e.citing.paragraph.clone()),
        char_start: s,
        char_end: t,
        excerpt: e.citing.excerpt.clone(),
    })
}

fn rank(treatment: &str) -> usize {
    ["OVERRULED", "REVERSED", "PER_INCURIAM", "REFERRED_TO_LARGER_BENCH", "DOUBTED", "DISTINGUISHED", "FOLLOWED"].iter().position(|t| *t == treatment).unwrap_or(99)
}

/// Verifies edges; returns the verified ones (adverse first, then by edge id) and the reason for each rejected one.
pub fn verify_edges<'a>(edges: impl IntoIterator<Item = &'a RawEdge>, get_chunk: &dyn Fn(&str) -> Option<CorpusChunk>, known: &dyn Fn(&str) -> bool) -> (Vec<VerifiedEdge>, Vec<(String, &'static str)>) {
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    for e in edges {
        let chunk = get_chunk(&e.citing.chunk_id);
        match verify_edge(e, chunk.as_ref(), known) {
            Ok(v) => ok.push(v),
            Err(r) => bad.push((e.edge_id.clone(), r)),
        }
    }
    ok.sort_by(|a, b| rank(&a.treatment).cmp(&rank(&b.treatment)).then(a.edge_id.cmp(&b.edge_id)));
    bad.sort();
    (ok, bad)
}

/// Treatment edges of the installed packs, indexed by the source they treat. Read only from files bound by the pack's
/// signed manifest.
#[derive(Default)]
pub struct TreatmentSet {
    edges: Vec<RawEdge>,
    by_cited: HashMap<String, Vec<usize>>,
    /// Packs that carry a treatment edges file (their judgments have been scanned, even if no edge was found).
    pub scanned_packs: Vec<String>,
    /// Stored entries that were not even well-formed JSON of the right shape.
    pub malformed: usize,
}

static LOADED: Lazy<Mutex<HashMap<String, Arc<TreatmentSet>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

impl TreatmentSet {
    pub fn from_edges(edges: Vec<RawEdge>, scanned_packs: Vec<String>) -> Self {
        let mut by_cited: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, e) in edges.iter().enumerate() {
            by_cited.entry(e.cited_source_id.clone()).or_default().push(i);
        }
        TreatmentSet { edges, by_cited, scanned_packs, malformed: 0 }
    }

    pub fn load(paths: &VaultPaths) -> Arc<TreatmentSet> {
        let mut installed: Vec<_> = list_installed(paths).into_iter().filter(|p| is_v2(&p.schema_version)).collect();
        installed.sort_by(|a, b| a.pack_id.cmp(&b.pack_id));
        let key = format!("{}|{}", paths.root.display(), installed.iter().map(|p| format!("{}:{}", p.pack_id, p.content_sha256)).collect::<Vec<_>>().join(","));
        if let Some(s) = LOADED.lock().ok().and_then(|m| m.get(&key).cloned()) {
            return s;
        }
        let (mut edges, mut scanned, mut malformed) = (Vec::new(), Vec::new(), 0usize);
        for p in &installed {
            let dir = Path::new(&p.install_path);
            let Ok(man) = read_manifest(dir) else { continue };
            let Some(file) = man.treatment_edges_file.as_deref() else { continue };
            if !man.files.iter().any(|f| f.path == file && f.role.as_deref() == Some("TREATMENT_EDGES")) {
                continue;
            }
            let Some(values) = std::fs::read_to_string(dir.join(file)).ok().and_then(|raw| serde_json::from_str::<Vec<Value>>(&raw).ok()) else { continue };
            scanned.push(p.pack_id.clone());
            for v in values {
                match serde_json::from_value::<RawEdge>(v) {
                    Ok(e) => edges.push(e),
                    Err(_) => malformed += 1,
                }
            }
        }
        let mut set = TreatmentSet::from_edges(edges, scanned);
        set.malformed = malformed;
        let set = Arc::new(set);
        if let Ok(mut m) = LOADED.lock() {
            if m.len() >= 16 {
                m.clear();
            }
            m.insert(key, set.clone());
        }
        set
    }

    /// Stored edges that name `source_id` as the treated judgment (not yet verified).
    pub fn raw_for(&self, source_id: &str) -> Vec<&RawEdge> {
        self.by_cited.get(source_id).into_iter().flatten().map(|&i| &self.edges[i]).collect()
    }

    pub fn corpus_scanned(&self) -> bool {
        !self.scanned_packs.is_empty()
    }

    /// Verified treatment of one installed source. `authority_type` is the type of the cited passage (treatment is about
    /// judgments); `get_chunk` finds installed passages.
    pub fn lookup(&self, source_id: &str, authority_type: &str, get_chunk: &dyn Fn(&str) -> Option<CorpusChunk>) -> Lookup {
        if authority_type != "JUDGMENT" {
            return Lookup { source_id: source_id.into(), state: "NOT_APPLICABLE", edges: vec![], rejected: vec![] };
        }
        let (edges, rejected) = verify_edges(self.raw_for(source_id), get_chunk, &|id| id == source_id);
        let state = if !edges.is_empty() {
            "FOUND"
        } else if self.corpus_scanned() {
            "NONE_FOUND"
        } else {
            "NO_DATA"
        };
        Lookup { source_id: source_id.into(), state, edges, rejected: rejected.into_iter().map(|(e, r)| (e, r.to_string())).collect() }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Lookup {
    pub source_id: String,
    /// FOUND, NONE_FOUND ("no treatment found in the installed corpus"), NO_DATA (no installed pack carries treatment
    /// data) or NOT_APPLICABLE (not a judgment).
    pub state: &'static str,
    pub edges: Vec<VerifiedEdge>,
    pub rejected: Vec<(String, String)>,
}

fn label(treatment: &str) -> &'static str {
    match treatment {
        "OVERRULED" => "Overruled",
        "REVERSED" => "Reversed",
        "PER_INCURIAM" => "Per incuriam",
        "REFERRED_TO_LARGER_BENCH" => "Referred to a larger Bench",
        "DOUBTED" => "Doubted",
        "DISTINGUISHED" => "Distinguished",
        _ => "Followed",
    }
}

/// Adds a TREATMENT note for each adverse treatment verified for a cited judgment, and turns VERIFIED into
/// VERIFIED_WITH_NOTE. Other results (misquoted, superseded...) keep their result and gain the notes. "Followed" is not a
/// note. `treated` returns the verified edges for a source id.
pub fn add_audit_notes(items: &mut [AuditItem], counts: &mut BTreeMap<String, usize>, treated: &dyn Fn(&str) -> Vec<VerifiedEdge>) {
    for item in items.iter_mut().filter(|i| i.kind == "CITATION") {
        let Some(loc) = item.resolved.as_ref() else { continue };
        let notes: Vec<Note> = treated(&loc.source_id)
            .iter()
            .filter(|e| e.treatment != "FOLLOWED")
            .take(MAX_NOTES_PER_CITATION)
            .map(|e| Note {
                note_kind: "TREATMENT".into(),
                text: format!(
                    "{} in the installed corpus: {} ({}){}: \u{201c}{}\u{201d}",
                    label(&e.treatment),
                    e.citing_title,
                    e.citing_citation,
                    e.paragraph.as_deref().map(|p| format!(", {p}")).unwrap_or_default(),
                    e.excerpt
                ),
                ref_id: Some(e.edge_id.clone()),
            })
            .collect();
        if notes.is_empty() {
            continue;
        }
        item.notes.extend(notes);
        if item.result == "VERIFIED" {
            item.result = "VERIFIED_WITH_NOTE".into();
            let verified = counts.get("VERIFIED").copied().unwrap_or(1).saturating_sub(1);
            counts.insert("VERIFIED".into(), verified);
            *counts.entry("VERIFIED_WITH_NOTE".into()).or_default() += 1;
        }
    }
}

/// Decorates an audit report with what the installed corpus says: the currency date of each pack and a TREATMENT note
/// for each adverse treatment verified for a cited judgment. Changes no citation's standing except VERIFIED ->
/// VERIFIED_WITH_NOTE.
pub fn decorate_report(paths: &VaultPaths, store: &crate::retrieval::store::Store, report: &mut crate::audit::AuditReport) {
    stamp_snapshots(&mut report.pack_snapshot, &pack_stamps(paths));
    let set = TreatmentSet::load(paths);
    if set.corpus_scanned() {
        let get = |id: &str| store.get_by_chunk_id(id);
        add_audit_notes(&mut report.items, &mut report.counts, &|source: &str| set.lookup(source, "JUDGMENT", &get).edges);
    }
}

/// A pack as a citation surface needs to stamp it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PackStamp {
    pub pack_id: String,
    pub version: String,
    pub display_name: String,
    /// Date up to which the pack's legal content was checked, if the pack states one.
    pub currency_date: Option<String>,
    pub updated_at: Option<String>,
    pub scanned_for_treatment: bool,
    /// B-S4-22: the pack subscription has ended, so this stamp keeps the older date and the screen says updates have expired.
    pub updates_expired: bool,
    pub updates_notice: Option<String>,
    /// The last day of the pack subscription, when it has ended.
    pub updates_expired_on: Option<String>,
}

pub fn pack_stamps(paths: &VaultPaths) -> Vec<PackStamp> {
    let mut v: Vec<PackStamp> = list_installed(paths)
        .into_iter()
        .filter_map(|p| {
            let man = read_manifest(Path::new(&p.install_path)).ok()?;
            Some(PackStamp { pack_id: p.pack_id, version: p.version, display_name: p.display_name, currency_date: man.currency_date.clone(), updated_at: man.updated_at.clone(), scanned_for_treatment: man.treatment_edges_file.is_some(), updates_expired: false, updates_notice: None, updates_expired_on: None })
        })
        .collect();
    v.sort_by(|a, b| a.pack_id.cmp(&b.pack_id));
    v
}

/// Fills in the currency date of each pack in an audit report's snapshot from the pack's own manifest.
pub fn stamp_snapshots(snapshots: &mut [PackSnapshot], stamps: &[PackStamp]) {
    for s in snapshots.iter_mut().filter(|s| s.currency_date.is_none()) {
        s.currency_date = stamps.iter().find(|p| p.pack_id == s.pack_id).and_then(|p| p.currency_date.clone());
    }
}

/// The wording every surface shows for a pack: "verified against <pack> as of <date>", or what is not known.
pub fn stamp_text(pack_name: &str, currency_date: Option<&str>) -> String {
    match currency_date {
        Some(d) => format!("verified against {pack_name} as of {d}"),
        None => format!("verified against {pack_name}; this pack does not state a currency date"),
    }
}

/// Install-time check of a pack's treatment edges file (structure only; each edge is verified against the installed text
/// when it is looked up). A pack that carries one must say how current it is.
pub fn precheck_pack(dir: &Path, man: &PackManifest) -> Result<(), String> {
    let Some(file) = man.treatment_edges_file.as_deref() else { return Ok(()) };
    let bad = |why: &str| Err(format!("CF_PACK_TREATMENT_INVALID: {why}"));
    if file != EDGES_FILE {
        return bad("the treatment edges file must be metadata/treatment_edges.json");
    }
    if man.schema_version != "2.1.0" {
        return bad("treatment edges require pack manifest schema 2.1.0");
    }
    if !man.currency_date.as_deref().map(crate::rule_tables::is_date).unwrap_or(false) {
        return bad("a pack with treatment edges must state its currency_date (YYYY-MM-DD)");
    }
    if !man.files.iter().any(|f| f.path == file && f.role.as_deref() == Some("TREATMENT_EDGES")) {
        return bad("the treatment edges file must be listed with role TREATMENT_EDGES");
    }
    let values: Vec<Value> = std::fs::read_to_string(dir.join(file)).ok().and_then(|raw| serde_json::from_str(&raw).ok()).ok_or_else(|| "CF_PACK_TREATMENT_INVALID: the treatment edges file is not a JSON array".to_string())?;
    for (i, v) in values.into_iter().enumerate() {
        match serde_json::from_value::<RawEdge>(v) {
            Ok(e) if shape_ok(&e) => {}
            _ => return bad(&format!("edge {} does not satisfy contracts/treatment_edge.schema.json (derived content, affects_supersession false)", i + 1)),
        }
    }
    Ok(())
}

/// The screen's view of a lookup: the same edges plus the sentence the screen must show when none are found.
pub fn lookup_json(l: &Lookup) -> Value {
    json!({
        "source_id": l.source_id, "state": l.state, "edges": l.edges, "rejected_count": l.rejected.len(),
        "none_found_text": "no treatment found in installed corpus",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::LegalMetadata;
    use sha2::{Digest, Sha256};
    use std::collections::HashSet;

    const FIXTURE: &str = include_str!("../../golden/G-TREATMENT/fixtures/treatment_core.json");
    const FIXTURE_SHA256: &str = "e99813cbb2b3026af2aec8148d03fcc842e693e6bcbc7856cb0d620d42a54d19"; // committed before the engine (45eba02)

    fn corpus_chunk(chunk_id: &str, source: &Value, paragraph: &str, text: &str, pack_id: &str) -> CorpusChunk {
        CorpusChunk {
            chunk_id: chunk_id.into(),
            pack_id: pack_id.into(),
            path: String::new(),
            jurisdiction: Some("India".into()),
            text: text.into(),
            authority_hint: String::new(),
            metadata: LegalMetadata {
                source_id: source["source_id"].as_str().unwrap().into(),
                title: source["title"].as_str().unwrap().into(),
                jurisdiction_path: vec!["India".into()],
                authority_type: "JUDGMENT".into(),
                issuing_authority: "Supreme Court of India".into(),
                citation: source["citation"].as_str().unwrap().into(),
                section: "judgment".into(),
                effective_from: source["decision_date"].as_str().unwrap().into(),
                effective_to: "UNKNOWN".into(),
                version: "1".into(),
                amendment: "NONE".into(),
                page: json!(1),
                paragraph: paragraph.into(),
                heading_path: vec![],
                checksum: String::new(),
                source_checksum: String::new(),
                pack_checksum: String::new(),
                trust_level: "OFFICIAL_VERIFIED".into(),
                superseded_status: "UNKNOWN".into(),
                publisher: String::new(),
                source_reference: String::new(),
                content_kind: "AUTHORITATIVE_TEXT".into(),
                temporal_status: "APPLICABLE".into(),
            },
        }
    }

    struct Scenario {
        chunks: HashMap<String, CorpusChunk>,
        edges: Vec<RawEdge>,
        known: HashSet<String>,
        fx: Value,
    }

    fn scenario() -> Scenario {
        assert_eq!(hex::encode(Sha256::digest(FIXTURE.as_bytes())), FIXTURE_SHA256, "G-TREATMENT fixture changed since it was pinned");
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let sc = &fx["scenario"];
        let sources: HashMap<String, Value> = sc["sources"].as_array().unwrap().iter().map(|s| (s["source_id"].as_str().unwrap().to_string(), s.clone())).collect();
        let chunks = sc["chunks"].as_array().unwrap().iter().map(|c| (c["chunk_id"].as_str().unwrap().to_string(), corpus_chunk(c["chunk_id"].as_str().unwrap(), &sources[c["source_id"].as_str().unwrap()], c["paragraph"].as_str().unwrap(), c["text"].as_str().unwrap(), "test.treatment"))).collect();
        let edges = sc["edges"].as_array().unwrap().iter().map(|e| serde_json::from_value(e.clone()).unwrap()).collect();
        let known = sc["known_sources"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect();
        Scenario { chunks, edges, known, fx }
    }

    #[test]
    fn golden_wording_finds_planted_treatments_and_ignores_traps() {
        assert_eq!(hex::encode(Sha256::digest(FIXTURE.as_bytes())), FIXTURE_SHA256);
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let (mut planted, mut traps) = (0, 0);
        for s in fx["sentences"].as_array().unwrap() {
            let id = s["id"].as_str().unwrap();
            let got = detect(s["text"].as_str().unwrap());
            match s["expect"].as_object() {
                Some(e) => {
                    planted += 1;
                    assert_eq!(got, Some((TREATMENTS.iter().find(|t| **t == e["treatment"].as_str().unwrap()).unwrap().to_owned(), e["pattern_id"].as_str().unwrap())), "{id}");
                    assert_eq!(verify_excerpt(e["treatment"].as_str().unwrap(), s["text"].as_str().unwrap()), Ok(e["pattern_id"].as_str().unwrap()), "{id}: verifier agrees");
                }
                None => {
                    traps += 1;
                    assert_eq!(got, None, "{id}: {}", s["why"]);
                }
            }
        }
        assert!(planted >= 9 && traps >= 9, "all seven treatments are planted, with traps ({planted}/{traps})");
        let kinds: HashSet<&str> = fx["sentences"].as_array().unwrap().iter().filter_map(|s| s["expect"]["treatment"].as_str()).collect();
        assert_eq!(kinds.len(), 7, "every treatment is exercised");
    }

    #[test]
    fn golden_edges_are_verified_against_the_installed_text() {
        let sc = scenario();
        let expect = &sc.fx["scenario"]["expect"];
        let (ok, bad) = verify_edges(&sc.edges, &|id| sc.chunks.get(id).cloned(), &|id| sc.known.contains(id));
        let mut by_cited: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for v in &ok {
            by_cited.entry(v.cited_source_id.clone()).or_default().push(v.edge_id.clone());
        }
        let want: BTreeMap<String, Vec<String>> = expect["verified"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_array().unwrap().iter().map(|e| e.as_str().unwrap().to_string()).collect())).collect();
        assert_eq!(by_cited, want, "verified edges per treated source, adverse first");
        let rejected: BTreeMap<String, String> = bad.iter().map(|(e, r)| (e.clone(), r.to_string())).collect();
        let want: BTreeMap<String, String> = expect["rejected"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
        assert_eq!(rejected, want, "every bad edge is rejected for the right reason");
        // AC-S4-08.1: the planted overruling surfaces with the exact citing paragraph.
        let overruling = ok.iter().find(|v| v.edge_id == "e01").unwrap();
        assert_eq!((overruling.treatment.as_str(), overruling.cited_source_id.as_str(), overruling.citing_title.as_str(), overruling.paragraph.as_deref()), ("OVERRULED", "S-A", "Rho v. Sigma", Some("para 12")));
        let chunk = &sc.chunks["c-B-12"];
        let at: String = chunk.text.chars().skip(overruling.char_start).take(overruling.char_end - overruling.char_start).collect();
        assert_eq!(at, overruling.excerpt, "the excerpt is the exact text at the stated place");
        assert!(overruling.excerpt.contains("is hereby overruled"));
        // Nothing the host verified can change a citation's standing.
        assert!(sc.edges.iter().filter(|e| ok.iter().any(|v| v.edge_id == e.edge_id)).all(|e| !e.affects_supersession && e.content_kind == "DERIVED"));
    }

    #[test]
    fn golden_audit_notes_and_lookup_states() {
        use crate::audit::{LegalLocator, Pinpoint};
        let sc = scenario();
        let expect = &sc.fx["scenario"]["expect"];
        let set = TreatmentSet::from_edges(sc.edges.clone(), vec!["test.treatment".into()]);
        let get = |id: &str| sc.chunks.get(id).cloned();
        let treated = |source: &str| set.lookup(source, "JUDGMENT", &get).edges;
        let mut items: Vec<AuditItem> = expect["audit"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| AuditItem {
                item_id: c["item"].as_str().unwrap().into(), kind: "CITATION".into(), citation_kind: Some("REPORTER".into()), extraction_rule_id: None, draft_char_start: 0, draft_char_end: 1, draft_text: "x".into(),
                attributed_citation_item_id: None, result: c["result"].as_str().unwrap().into(),
                resolved: Some(LegalLocator { pack_id: "test.treatment".into(), source_id: c["source_id"].as_str().unwrap().into(), chunk_id: "c".into(), char_start: 0, char_end: 1, page_index: None, paragraph: None, excerpt: "x".into() }),
                pinpoint: Pinpoint { claimed: None, status: "NO_PINPOINT".into() }, notes: vec![], misquote: None, proposition_ids: vec![],
            })
            .collect();
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for i in &items {
            *counts.entry(i.result.clone()).or_default() += 1;
        }
        add_audit_notes(&mut items, &mut counts, &treated);
        for (item, case) in items.iter().zip(expect["audit"].as_array().unwrap()) {
            let id = case["item"].as_str().unwrap();
            assert_eq!(item.result, case["expect_result"].as_str().unwrap(), "{id}: result");
            let got: Vec<(String, String)> = item.notes.iter().map(|n| (n.note_kind.clone(), n.ref_id.clone().unwrap())).collect();
            let want: Vec<(String, String)> = case["expect_notes"].as_array().unwrap().iter().map(|n| (n["kind"].as_str().unwrap().to_string(), n["ref_id"].as_str().unwrap().to_string())).collect();
            assert_eq!(got, want, "{id}: notes");
            for (n, w) in item.notes.iter().zip(case["expect_notes"].as_array().unwrap()) {
                let edge = sc.edges.iter().find(|e| e.edge_id == w["ref_id"].as_str().unwrap()).unwrap();
                assert!(n.text.contains(&edge.citing.excerpt) && n.text.starts_with(&format!("{} in the installed corpus", label(w["treatment"].as_str().unwrap()))), "{id}: the note quotes the citing text");
            }
        }
        // Counts follow the results.
        assert_eq!((counts["VERIFIED"], counts["VERIFIED_WITH_NOTE"], counts["SUPERSEDED"], counts["MISQUOTED"]), (2, 2, 1, 1));
        // Lookup states.
        for (source, want) in expect["lookup"].as_object().unwrap() {
            let l = set.lookup(source, "JUDGMENT", &get);
            assert_eq!(l.state, want["state"].as_str().unwrap(), "{source}");
            assert_eq!(l.edges.iter().map(|e| e.edge_id.clone()).collect::<Vec<_>>(), want["edges"].as_array().unwrap().iter().map(|e| e.as_str().unwrap().to_string()).collect::<Vec<_>>(), "{source}");
            assert_eq!(lookup_json(&l)["none_found_text"], "no treatment found in installed corpus");
        }
        assert_eq!(set.lookup("S-K", "STATUTE", &get).state, "NOT_APPLICABLE", "treatment is about judgments");
        let none = TreatmentSet::from_edges(vec![], vec![]);
        assert_eq!(none.lookup("S-K", "JUDGMENT", &get).state, "NO_DATA", "no pack carries treatment data: that is not 'none found'");
    }

    #[test]
    fn currency_is_stamped_from_the_pack_and_missing_dates_are_stated() {
        let stamps = vec![
            PackStamp { pack_id: "a".into(), version: "1".into(), display_name: "Pack A".into(), currency_date: Some("2026-03-31".into()), updated_at: None, scanned_for_treatment: false, updates_expired: false, updates_notice: None, updates_expired_on: None },
            PackStamp { pack_id: "b".into(), version: "1".into(), display_name: "Pack B".into(), currency_date: None, updated_at: Some("2026-01-01".into()), scanned_for_treatment: false, updates_expired: false, updates_notice: None, updates_expired_on: None },
        ];
        let snap = |id: &str| PackSnapshot { pack_id: id.into(), version: "1".into(), currency_date: None, trust_level: "OFFICIAL_VERIFIED".into() };
        let mut s = vec![snap("a"), snap("b"), snap("c")];
        stamp_snapshots(&mut s, &stamps);
        assert_eq!(s.iter().map(|x| x.currency_date.clone()).collect::<Vec<_>>(), vec![Some("2026-03-31".to_string()), None, None]);
        assert_eq!(stamp_text("Pack A", Some("2026-03-31")), "verified against Pack A as of 2026-03-31");
        assert!(stamp_text("Pack B", None).contains("does not state a currency date"));
    }

    #[test]
    fn labels_never_say_a_case_is_good_law() {
        for t in TREATMENTS {
            let l = label(t).to_lowercase();
            for w in ["good law", "valid", "safe", "reliable", "bad law"] {
                assert!(!l.contains(w), "{t}: {w}");
            }
        }
    }
}
