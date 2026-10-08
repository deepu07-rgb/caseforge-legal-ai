//! Signed rule tables (B-S4-03 core, PRD v4.0 §5).
//!
//! Jurisdiction-specific legal data (statute succession, limitation periods, deadlines, …) ships inside OFFICIAL packs
//! (akpkg v2.1, `metadata/rule_tables/*.json`, bound to the pack signature through the manifest hashes). A table is
//! accepted only if every row cites authorities that resolve to installed OFFICIAL_VERIFIED chunks, windows for one
//! rule never overlap, and a SIGNED_OFF row carries a legal-editor sign-off. Rows without sign-off are
//! DEVELOPMENT_ONLY and can never make an output verified (AC-S4-RT.2). Custom packs may not carry rule tables.
//! Contract: contracts/rule_table.schema.json. Golden: golden/G-CODEBRIDGE (rule_table_exploits).

use crate::packs::{self, PackManifest};
use crate::retrieval::CorpusChunk;
use crate::vault::VaultPaths;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

pub const ERR_INVALID: &str = "CF_PACK_RULE_TABLE_INVALID";
pub const ERR_CUSTOM: &str = "CF_PACK_RULE_TABLE_IN_CUSTOM_PACK";
const TABLE_KINDS: [&str; 6] = ["STATUTE_SUCCESSION", "LIMITATION_PERIOD", "PROCEDURAL_DEADLINE", "COURT_CALENDAR", "ADMISSIBILITY_REQUIREMENT", "ISSUE_FRAMING_TEMPLATE"];
const EQUIVALENCE: [&str; 5] = ["IDENTICAL", "RENUMBERED", "MODIFIED", "NEW", "OMITTED"];
const GOVERNED_BY: [&str; 5] = ["OFFENCE_DATE", "INSTITUTION_DATE", "FILING_DATE", "CAUSE_OF_ACTION_DATE", "EVENT_DATE"];
static TABLE_ID: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-z0-9][a-z0-9._-]+$").unwrap());
static RULE_ID: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-z0-9][a-z0-9._:-]+$").unwrap());
static FILE_PATH: Lazy<Regex> = Lazy::new(|| Regex::new(r"^metadata/rule_tables/[A-Za-z0-9._-]+\.json$").unwrap());

/// An installed chunk a rule row may cite.
#[derive(Debug, Clone)]
pub struct ChunkRef {
    pub pack_id: String,
    pub source_id: String,
    pub trust_level: String,
    pub text: String,
}

pub type ChunkIndex = HashMap<String, ChunkRef>;

pub fn index_corpus(corpus: &[CorpusChunk]) -> ChunkIndex {
    corpus
        .iter()
        .map(|c| {
            (c.chunk_id.clone(), ChunkRef { pack_id: c.pack_id.clone(), source_id: c.metadata.source_id.clone(), trust_level: c.metadata.trust_level.clone(), text: c.text.clone() })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleRow {
    pub rule_id: String,
    pub table_id: String,
    pub table_kind: String,
    pub jurisdiction: String,
    pub effective_from: String,
    pub effective_to: Option<String>,
    pub authorities: Vec<Value>,
    /// SIGNED_OFF or DEVELOPMENT_ONLY.
    pub trust_state: String,
    pub signed_by: Option<String>,
    pub payload: Value,
}

impl RuleRow {
    pub fn signed(&self) -> bool {
        self.trust_state == "SIGNED_OFF" && self.signed_by.is_some()
    }
    pub fn effective_on(&self, date: &str) -> bool {
        self.effective_from.as_str() <= date && self.effective_to.as_deref().map(|to| date <= to).unwrap_or(true)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleTable {
    pub table_id: String,
    pub table_kind: String,
    pub jurisdiction: String,
    pub pack_id: String,
    pub rows: Vec<RuleRow>,
}

pub fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let num = |r: std::ops::Range<usize>| s.get(r).and_then(|x| x.parse::<u32>().ok());
    matches!((num(0..4), num(5..7), num(8..10)), (Some(_), Some(m), Some(d)) if (1..=12).contains(&m) && (1..=31).contains(&d))
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn provision_ok(v: &Value) -> bool {
    !str_of(v, "code").is_empty() && !str_of(v, "provision").is_empty()
}

/// Rejects rule tables that can never be installed, before any file is read (AC-S4-RT.1).
pub fn precheck_manifest(man: &PackManifest) -> Result<(), String> {
    if man.rule_tables_files.is_empty() {
        return Ok(());
    }
    if man.pack_kind != "official" {
        return Err(format!("{ERR_CUSTOM}: custom packs may not carry rule tables"));
    }
    if man.schema_version != "2.1.0" {
        return Err(format!("{ERR_INVALID}: rule tables require pack manifest schema 2.1.0"));
    }
    Ok(())
}

/// Validates one rule table against the frozen contract rules and the installed chunk index.
pub fn validate_table(table: &Value, pack_kind: &str, chunks: &ChunkIndex) -> Result<RuleTable, String> {
    if pack_kind != "official" {
        return Err(format!("{ERR_CUSTOM}: custom packs may not carry rule tables"));
    }
    let mut errors: Vec<String> = Vec::new();
    let table_id = str_of(table, "table_id").to_string();
    let table_kind = str_of(table, "table_kind").to_string();
    if str_of(table, "schema_version") != "1.0.0" {
        errors.push("schema_version must be 1.0.0".into());
    }
    if !TABLE_ID.is_match(&table_id) {
        errors.push(format!("table_id '{table_id}' is not a valid id"));
    }
    if !TABLE_KINDS.contains(&table_kind.as_str()) {
        errors.push(format!("table_kind '{table_kind}' is not a known kind"));
    }
    let jurisdiction = str_of(table, "jurisdiction").to_string();
    let pack_id = str_of(table, "pack_id").to_string();
    if jurisdiction.is_empty() || pack_id.len() < 3 {
        errors.push("jurisdiction and pack_id are required".into());
    }
    let rows_v = table.get("rows").and_then(Value::as_array).cloned().unwrap_or_default();
    if rows_v.is_empty() {
        errors.push("a table needs at least one row".into());
    }
    let mut rows = Vec::new();
    for r in &rows_v {
        let id = str_of(r, "rule_id").to_string();
        let at = |m: &str| format!("row {id}: {m}");
        if !RULE_ID.is_match(&id) {
            errors.push(at("rule_id is not a valid id"));
        }
        if str_of(r, "table_kind") != table_kind {
            errors.push(at(&format!("table_kind '{}' differs from the table's '{table_kind}'", str_of(r, "table_kind"))));
        }
        if str_of(r, "jurisdiction").is_empty() {
            errors.push(at("jurisdiction is required"));
        }
        let from = str_of(r, "effective_from").to_string();
        let to = r.get("effective_to").and_then(Value::as_str).map(str::to_string);
        if !is_date(&from) || to.as_deref().map(|t| !is_date(t)).unwrap_or(false) || !r.get("effective_to").map(|v| v.is_null() || v.is_string()).unwrap_or(false) {
            errors.push(at("effective dates must be YYYY-MM-DD (effective_to may be null)"));
        } else if to.as_deref().map(|t| t < from.as_str()).unwrap_or(false) {
            errors.push(at("effective window ends before it starts"));
        }
        let authorities = r.get("authorities").and_then(Value::as_array).cloned().unwrap_or_default();
        if authorities.is_empty() {
            errors.push(at("at least one authority is required"));
        }
        let mut resolved = 0;
        for a in &authorities {
            let chunk_id = str_of(a, "chunk_id");
            match chunks.get(chunk_id) {
                None => errors.push(at(&format!("authority {chunk_id} does not resolve to an installed chunk"))),
                Some(c) if c.pack_id != str_of(a, "pack_id") || c.source_id != str_of(a, "source_id") => {
                    errors.push(at(&format!("authority {chunk_id} names the wrong pack or source")))
                }
                Some(c) if c.trust_level != "OFFICIAL_VERIFIED" => errors.push(at(&format!("authority {chunk_id} is {}, not OFFICIAL_VERIFIED", c.trust_level))),
                Some(c) => {
                    let len = c.text.chars().count() as u64;
                    let (s, e) = (a.get("char_start").and_then(Value::as_u64), a.get("char_end").and_then(Value::as_u64));
                    if matches!((s, e), (Some(s), Some(e)) if s >= e || e > len) || s.is_some() != e.is_some() {
                        errors.push(at(&format!("authority {chunk_id} has an invalid character range")));
                    } else {
                        resolved += 1;
                    }
                }
            }
        }
        let signoff = r.get("legal_editor_signoff").cloned().unwrap_or(Value::Null);
        let trust_state = str_of(r, "trust_state").to_string();
        let signed_by = if signoff.is_null() {
            if trust_state != "DEVELOPMENT_ONLY" {
                errors.push(at("SIGNED_OFF without a legal-editor sign-off (rows without sign-off must be DEVELOPMENT_ONLY)"));
            }
            None
        } else {
            if str_of(&signoff, "editor_name").is_empty() || str_of(&signoff, "editor_role").is_empty() || !is_date(str_of(&signoff, "signed_on")) {
                errors.push(at("legal-editor sign-off needs editor_name, editor_role and signed_on"));
            }
            if !matches!(trust_state.as_str(), "SIGNED_OFF" | "DEVELOPMENT_ONLY") {
                errors.push(at("trust_state must be SIGNED_OFF or DEVELOPMENT_ONLY"));
            }
            Some(str_of(&signoff, "editor_name").to_string())
        };
        let payload = r.get("payload").cloned().unwrap_or(Value::Null);
        if !payload.is_object() {
            errors.push(at("payload must be an object"));
        } else if table_kind == "STATUTE_SUCCESSION" {
            match str_of(&payload, "entry_type") {
                "MAPPING" => {
                    let eq = str_of(&payload, "equivalence");
                    let (pred, succ) = (payload.get("predecessor").cloned().unwrap_or(Value::Null), payload.get("successor").cloned().unwrap_or(Value::Null));
                    let shape = match eq {
                        "NEW" => pred.is_null() && provision_ok(&succ),
                        "OMITTED" => provision_ok(&pred) && succ.is_null(),
                        e if EQUIVALENCE.contains(&e) => provision_ok(&pred) && provision_ok(&succ),
                        _ => false,
                    };
                    if !shape {
                        errors.push(at(&format!("mapping '{eq}' has the wrong predecessor/successor shape")));
                    }
                    if eq == "MODIFIED" && resolved < 2 {
                        errors.push(at("a MODIFIED mapping must cite both texts (old and new provision)"));
                    }
                }
                "TRANSITION" => {
                    if str_of(&payload, "before_code").is_empty() || str_of(&payload, "after_code").is_empty() || !is_date(str_of(&payload, "switch_date")) || !GOVERNED_BY.contains(&str_of(&payload, "governed_by")) {
                        errors.push(at("transition needs before_code, after_code, switch_date and governed_by"));
                    }
                }
                other => errors.push(at(&format!("entry_type '{other}' is not MAPPING or TRANSITION"))),
            }
        }
        rows.push(RuleRow {
            rule_id: id,
            table_id: table_id.clone(),
            table_kind: str_of(r, "table_kind").to_string(),
            jurisdiction: str_of(r, "jurisdiction").to_string(),
            effective_from: from,
            effective_to: to,
            authorities,
            trust_state,
            signed_by,
            payload,
        });
    }
    // Windows for one rule_id may not overlap.
    let mut by_rule: HashMap<&str, Vec<&RuleRow>> = HashMap::new();
    for r in &rows {
        by_rule.entry(r.rule_id.as_str()).or_default().push(r);
    }
    for (id, mut list) in by_rule {
        list.sort_by(|a, b| a.effective_from.cmp(&b.effective_from));
        for w in list.windows(2) {
            if w[0].effective_to.as_deref().map(|to| w[1].effective_from.as_str() <= to).unwrap_or(true) {
                errors.push(format!("row {id}: effective windows overlap"));
            }
        }
    }
    if errors.is_empty() {
        Ok(RuleTable { table_id, table_kind, jurisdiction, pack_id, rows })
    } else {
        Err(format!("{ERR_INVALID}: {}", errors.join("; ")))
    }
}

/// Install-time check for every rule table a pack declares (AC-S4-RT.1).
pub fn validate_pack(paths: &VaultPaths, pack_dir: &Path, man: &PackManifest) -> Result<(), String> {
    precheck_manifest(man)?;
    let mut index = crate::retrieval::build_corpus(paths, &[]).map(|c| index_corpus(&c)).unwrap_or_default();
    for c in packs::load_legal_chunks(pack_dir)? {
        index.insert(c.chunk_id.clone(), ChunkRef { pack_id: c.pack_id, source_id: c.source_id, trust_level: c.trust_level, text: c.exact_text });
    }
    for file in &man.rule_tables_files {
        validate_file(pack_dir, man, file, &index)?;
    }
    Ok(())
}

fn validate_file(pack_dir: &Path, man: &PackManifest, file: &str, index: &ChunkIndex) -> Result<RuleTable, String> {
    if !FILE_PATH.is_match(file) || !man.files.iter().any(|f| f.path == file && f.role.as_deref() == Some("RULE_TABLE")) {
        return Err(format!("{ERR_INVALID}: {file} must be listed in the signed manifest with role RULE_TABLE"));
    }
    let raw = std::fs::read_to_string(pack_dir.join(file)).map_err(|e| format!("{ERR_INVALID}: {file}: {e}"))?;
    let v: Value = serde_json::from_str(&raw).map_err(|e| format!("{ERR_INVALID}: {file}: {e}"))?;
    let table = validate_table(&v, &man.pack_kind, index)?;
    if table.pack_id != man.pack_id {
        return Err(format!("{ERR_INVALID}: {file} names pack {} but ships in {}", table.pack_id, man.pack_id));
    }
    Ok(table)
}

/// Rule tables of installed official packs, re-validated on every load (defence in depth). Returns the accepted
/// tables and a reason for each rejected one.
pub fn load_installed(paths: &VaultPaths) -> (Vec<RuleTable>, Vec<String>) {
    let index = crate::retrieval::build_corpus(paths, &[]).map(|c| index_corpus(&c)).unwrap_or_default();
    let mut tables = Vec::new();
    let mut rejected = Vec::new();
    for p in packs::list_installed(paths).into_iter().filter(|p| p.pack_kind == "official") {
        let dir = Path::new(&p.install_path);
        let Ok(man) = packs::read_manifest(dir) else { continue };
        // Code-name aliases ("CrPC" = "Code of Criminal Procedure"), only if bound to the signed manifest.
        let alias_file = "metadata/code_aliases.json";
        if man.files.iter().any(|f| f.path == alias_file) {
            if let Ok(v) = std::fs::read_to_string(dir.join(alias_file)).map(|raw| serde_json::from_str::<Value>(&raw).unwrap_or(Value::Null)) {
                let groups: Vec<Vec<String>> = v["codes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|c| std::iter::once(str_of(c, "code").to_string()).chain(c["aliases"].as_array().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_string))).collect())
                    .collect();
                crate::code_bridge::add_code_aliases(&groups);
            }
        }
        for file in &man.rule_tables_files {
            match validate_file(dir, &man, file, &index) {
                Ok(t) => tables.push(t),
                Err(e) => rejected.push(format!("{}/{file}: {e}", p.pack_id)),
            }
        }
    }
    (tables, rejected)
}

/// Rows of one kind effective on `as_of`, optionally limited to one jurisdiction (string equality on data).
pub fn query<'a>(tables: &'a [RuleTable], kind: &str, jurisdiction: Option<&str>, as_of: &str) -> Vec<&'a RuleRow> {
    tables
        .iter()
        .filter(|t| t.table_kind == kind && jurisdiction.map(|j| t.jurisdiction.eq_ignore_ascii_case(j)).unwrap_or(true))
        .flat_map(|t| t.rows.iter())
        .filter(|r| r.effective_on(as_of))
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    pub const FIXTURE: &str = include_str!("../../golden/G-CODEBRIDGE/fixtures/code_bridge_core.json");

    pub fn index(fx: &Value) -> ChunkIndex {
        fx["chunks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                let s = |k: &str| c[k].as_str().unwrap().to_string();
                (s("chunk_id"), ChunkRef { pack_id: s("pack_id"), source_id: s("source_id"), trust_level: s("trust_level"), text: s("text") })
            })
            .collect()
    }

    pub fn dev_only(fx: &Value) -> Value {
        let mut t = fx["rule_table"].clone();
        for r in t["rows"].as_array_mut().unwrap() {
            for (path, value) in fx["development_only_variant"]["patch_all_rows"].as_object().unwrap() {
                *r.pointer_mut(path).unwrap() = value.clone();
            }
        }
        t
    }

    #[test]
    fn golden_table_validates_and_queries_by_date() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let t = validate_table(&fx["rule_table"], "official", &index(&fx)).unwrap();
        assert_eq!(t.rows.len(), 8);
        assert!(t.rows.iter().all(|r| r.signed()));
        let tables = vec![t];
        assert_eq!(query(&tables, "STATUTE_SUCCESSION", Some("demoland"), "2026-10-08").len(), 8);
        assert_eq!(query(&tables, "STATUTE_SUCCESSION", Some("Demoland"), "2024-06-30").len(), 0, "not yet in force");
        assert_eq!(query(&tables, "STATUTE_SUCCESSION", Some("India"), "2026-10-08").len(), 0, "other jurisdiction");
        assert_eq!(query(&tables, "LIMITATION_PERIOD", None, "2026-10-08").len(), 0);
    }

    #[test]
    fn every_rule_table_exploit_is_rejected() {
        // AC-S4-RT.1
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let idx = index(&fx);
        for x in fx["rule_table_exploits"].as_array().unwrap() {
            let mut t = fx["rule_table"].clone();
            for p in x.get("patch").and_then(Value::as_array).into_iter().flatten() {
                *t.pointer_mut(p["path"].as_str().unwrap()).unwrap_or_else(|| panic!("{x}")) = p["value"].clone();
            }
            if let Some(copy) = x.get("append_row_copy") {
                let mut row = t["rows"][copy["from"].as_u64().unwrap() as usize].clone();
                for (path, value) in copy["set"].as_object().unwrap() {
                    *row.pointer_mut(path).unwrap() = value.clone();
                }
                t["rows"].as_array_mut().unwrap().push(row);
            }
            let kind = x.get("pack_kind").and_then(Value::as_str).unwrap_or("official");
            let err = validate_table(&t, kind, &idx).expect_err(x["name"].as_str().unwrap());
            assert!(err.starts_with(x["error"].as_str().unwrap()), "{}: {err}", x["name"]);
            assert!(err.contains(x["detail"].as_str().unwrap()), "{}: {err}", x["name"]);
        }
    }

    #[test]
    fn unsigned_rows_install_only_as_development_only() {
        // AC-S4-RT.2 (data side): unsigned rows are accepted only as DEVELOPMENT_ONLY and never report signed().
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let t = validate_table(&dev_only(&fx), "official", &index(&fx)).unwrap();
        assert!(t.rows.iter().all(|r| r.trust_state == "DEVELOPMENT_ONLY" && !r.signed()));
    }

    #[test]
    fn manifest_precheck_rejects_custom_and_v1_rule_tables() {
        let base: PackManifest = serde_json::from_value(json!({
            "schema_version": "2.1.0", "pack_id": "fx.demo-official", "pack_kind": "custom", "jurisdiction": "Demoland", "version": "1.0.0",
            "display_name": "x", "description": null, "content_sha256": "0", "files": [], "headnotes_stripped": true, "signature_alg": null,
            "signer_key_id": null, "rule_tables_files": ["metadata/rule_tables/succession.json"]
        }))
        .unwrap();
        assert!(precheck_manifest(&base).unwrap_err().starts_with(ERR_CUSTOM));
        let mut v1 = base.clone();
        v1.pack_kind = "official".into();
        v1.schema_version = "1.0.0".into();
        assert!(precheck_manifest(&v1).unwrap_err().contains("2.1.0"));
        let mut ok = v1.clone();
        ok.schema_version = "2.1.0".into();
        assert!(precheck_manifest(&ok).is_ok());
    }
}
