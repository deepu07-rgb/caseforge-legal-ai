//! Record-only mode (B-S4-17, PRD v4.0 §9.1, guardrail G7).
//!
//! In the Chambers Edition everything a model says is confined to the record: the filings, the evidence and orders in the
//! matter, and installed law. Every statement the model produces is classified:
//!
//!   RECORD                 a statement the host verified word for word against a document on the record
//!   LAW                    a statement the host verified word for word against an installed official, applicable, current authority
//!   INFERENCE_FROM_RECORD  a statement that rests, by listed bases, only on statements that are themselves RECORD, LAW or INFERENCE_FROM_RECORD
//!   EXTRA_RECORD           everything else - model memory, an invented or altered quotation, a custom pack, something said in the
//!                          prompt, an inference resting on any of those, a statement without support
//!
//! EXTRA_RECORD is never rendered (it is counted and logged, by hash, never by content). With record-only switched off -
//! allowed only until a judgment scaffold exists - such statements show, labelled EXTRA_RECORD, and are never given a RECORD
//! or LAW class. Every rendered statement carries its class and at least one locator.
//!
//! The classifier takes the output of the host's own proposition verifier (proposition.rs); it adds no trust of its own: a
//! statement is RECORD or LAW only if that verifier marked it TRUSTED. Custom packs never supply LAW or RECORD here
//! (stricter than the PRD, which allows law from a custom pack the user marked "law, not facts"; that marking arrives with
//! the first feature that needs it). Contract: guardrails_stage4.json G7. Golden: golden/G-CHAMBERS (record_only).

use crate::proposition::{PropositionLedger, SupportSource, VerifiedProposition};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const CLASSES: [&str; 4] = ["RECORD", "LAW", "INFERENCE_FROM_RECORD", "EXTRA_RECORD"];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Statement {
    pub proposition_id: String,
    pub text: String,
    pub class: &'static str,
    pub locators: Vec<Value>,
    /// For INFERENCE_FROM_RECORD: the statements it rests on.
    pub bases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Withheld {
    pub proposition_id: String,
    /// NO_SUPPORT, NOT_VERIFIED, NOT_FROM_THE_RECORD or RESTS_ON_EXTRA_RECORD.
    pub reason: &'static str,
    pub text_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Classified {
    pub record_only: bool,
    /// Statements that may render, in order. With record-only off this includes EXTRA_RECORD ones, labelled.
    pub statements: Vec<Statement>,
    pub withheld: Vec<Withheld>,
    pub rendered_text: String,
}

fn sha(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

fn record_locator(src: &SupportSource, excerpt: &str) -> Value {
    json!({"kind": "MATTER", "support_id": src.support_id, "doc_id": src.doc_id, "page": src.page, "paragraph": src.paragraph, "excerpt": excerpt})
}

fn law_locator(src: &SupportSource, excerpt: &str) -> Value {
    json!({"kind": "LAW", "support_id": src.support_id, "authority_id": src.authority_id, "source_id": src.source_id, "chunk_id": src.chunk_id, "paragraph": src.paragraph, "excerpt": excerpt})
}

fn locators(p: &VerifiedProposition, by_id: &HashMap<&str, &SupportSource>, law: bool) -> Vec<Value> {
    p.verified_support_ids
        .iter()
        .filter_map(|id| {
            let src = by_id.get(id.as_str())?;
            let excerpt = p.support_claims.iter().find(|c| &c.support_id == id).map(|c| c.exact_excerpt.as_str()).unwrap_or("");
            Some(if law { law_locator(src, excerpt) } else { record_locator(src, excerpt) })
        })
        .collect()
}

fn withheld_reason(p: &VerifiedProposition) -> &'static str {
    if p.support_ids.is_empty() && p.basis_ids.is_empty() {
        "NO_SUPPORT"
    } else {
        "NOT_VERIFIED"
    }
}

fn locator_summary(l: &Value) -> String {
    match l["kind"].as_str() {
        Some("MATTER") => format!("{}{}{}", l["doc_id"].as_str().unwrap_or("record"), l["page"].as_i64().map(|p| format!(" p.{p}")).unwrap_or_default(), l["paragraph"].as_str().map(|p| format!(" {p}")).unwrap_or_default()),
        Some("LAW") => format!("authority {}{}", l["authority_id"].as_str().unwrap_or(""), l["paragraph"].as_str().map(|p| format!(", {p}")).unwrap_or_default()),
        _ => "record".into(),
    }
}

/// Classifies a verified ledger. `sources` are the support sources the verifier was given.
pub fn classify(ledger: &PropositionLedger, sources: &[SupportSource], record_only: bool) -> Classified {
    let by_id: HashMap<&str, &SupportSource> = sources.iter().map(|s| (s.support_id.as_str(), s)).collect();
    let mut known: HashMap<String, (&'static str, Vec<Value>)> = HashMap::new(); // rendered statements by id
    let mut statements = Vec::new();
    let mut withheld = Vec::new();
    for p in &ledger.propositions {
        let trusted = p.render_action == "TRUSTED";
        let verdict: Result<(&'static str, Vec<Value>, Vec<String>), &'static str> = match (p.class.as_str(), p.render_action.as_str()) {
            ("FACTUAL_BASIS", "TRUSTED") => {
                let l = locators(p, &by_id, false);
                // the verifier only trusts a fact whose support is a matter source with a document; require a locator all the same
                if l.is_empty() || p.verified_support_ids.iter().any(|id| by_id.get(id.as_str()).map(|s| s.kind != "MATTER_SOURCE" || s.doc_id.is_none()).unwrap_or(true)) {
                    Err("NOT_VERIFIED")
                } else {
                    Ok(("RECORD", l, vec![]))
                }
            }
            ("LEGAL_BASIS", "TRUSTED") => {
                let l = locators(p, &by_id, true);
                // the verifier already requires an official, applicable, current, permitted authority; a custom pack never gets here
                if l.is_empty() || p.verified_support_ids.iter().any(|id| by_id.get(id.as_str()).map(|s| s.kind != "LEGAL_AUTHORITY" || s.trust_level.as_deref() != Some("OFFICIAL_VERIFIED")).unwrap_or(true)) {
                    Err("NOT_VERIFIED")
                } else {
                    Ok(("LAW", l, vec![]))
                }
            }
            ("INFERENCE", "LABELLED") if !p.basis_ids.is_empty() => {
                let mut ls: Vec<Value> = Vec::new();
                let mut all_record = true;
                for b in &p.basis_ids {
                    match known.get(b) {
                        Some((_, bl)) => {
                            for l in bl {
                                if !ls.contains(l) {
                                    ls.push(l.clone());
                                }
                            }
                        }
                        None => all_record = false,
                    }
                }
                if all_record && !ls.is_empty() {
                    Ok(("INFERENCE_FROM_RECORD", ls, p.basis_ids.clone()))
                } else {
                    Err("RESTS_ON_EXTRA_RECORD")
                }
            }
            ("COUNSEL_INPUT", _) => Err("NOT_FROM_THE_RECORD"),
            _ => Err(if trusted { "NOT_VERIFIED" } else { withheld_reason(p) }),
        };
        match verdict {
            Ok((class, locators, bases)) => {
                known.insert(p.proposition_id.clone(), (class, locators.clone()));
                statements.push(Statement { proposition_id: p.proposition_id.clone(), text: p.text.clone(), class, locators, bases });
            }
            Err(reason) => {
                withheld.push(Withheld { proposition_id: p.proposition_id.clone(), reason, text_sha256: sha(&p.text) });
                if !record_only {
                    statements.push(Statement { proposition_id: p.proposition_id.clone(), text: p.text.clone(), class: "EXTRA_RECORD", locators: vec![], bases: vec![] });
                }
            }
        }
    }
    let rendered_text = render(&statements, if record_only { withheld.len() } else { 0 });
    Classified { record_only, statements, withheld, rendered_text }
}

/// The text the screen shows: each statement with its class and where it comes from.
pub fn render(statements: &[Statement], withheld_count: usize) -> String {
    let mut parts: Vec<String> = statements
        .iter()
        .map(|s| match s.class {
            "EXTRA_RECORD" => format!("[EXTRA_RECORD \u{2014} NOT FROM THE RECORD] {}", s.text),
            "INFERENCE_FROM_RECORD" => format!("{} [INFERENCE_FROM_RECORD \u{2014} rests on {}]", s.text, s.bases.join(", ")),
            class => format!("{} [{class} \u{2014} {}]", s.text, s.locators.iter().map(locator_summary).collect::<Vec<_>>().join("; ")),
        })
        .collect();
    if withheld_count > 0 {
        parts.push(format!("{withheld_count} statement{} withheld: not supported by the record or installed law.", if withheld_count == 1 { "" } else { "s" }));
    }
    parts.join("\n\n")
}

/// Record-only can be switched off only before a judgment scaffold exists; once one does, it stays on.
pub fn check_change(scaffold_exists: bool, enable: bool) -> Result<(), String> {
    if !enable && scaffold_exists {
        Err("CF_G7_LOCKED: a judgment scaffold exists for this matter, so record-only mode stays on".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proposition::{verify, PropositionInput, VerifyRequest};
    use std::collections::BTreeMap;

    const FIXTURE: &str = include_str!("../../golden/G-CHAMBERS/fixtures/record_only_core.json");
    const FIXTURE_SHA256: &str = "0f4ad28924e7617cba6fa14caac2199525837cd0bd88e6ac486bb692adbc2021"; // committed before the engine (d725ba0)

    fn load() -> (Value, Vec<SupportSource>, PropositionLedger) {
        assert_eq!(hex::encode(Sha256::digest(FIXTURE.as_bytes())), FIXTURE_SHA256, "record-only golden changed since it was pinned");
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let sources: Vec<SupportSource> = serde_json::from_value(fx["support_sources"].clone()).unwrap();
        let props: Vec<PropositionInput> = serde_json::from_value(fx["propositions"].clone()).unwrap();
        let permitted = sources.iter().filter_map(|s| s.authority_id.clone()).collect();
        let ledger = verify(VerifyRequest { output_kind: "REASONING".into(), propositions: props, support_sources: sources.clone(), permitted_authority_ids: permitted }).unwrap();
        (fx, sources, ledger)
    }

    #[test]
    fn golden_planted_extra_record_content_never_renders() {
        let (fx, sources, ledger) = load();
        let out = classify(&ledger, &sources, true);
        let want = &fx["expect"]["record_only_on"];
        // AC-S4-11.1: exactly the legitimate statements render, in order, with the right class and locator
        let rendered: Vec<&str> = out.statements.iter().map(|s| s.proposition_id.as_str()).collect();
        assert_eq!(rendered, want["rendered"].as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap()).collect::<Vec<_>>());
        for (s, w) in out.statements.iter().zip(want["rendered"].as_array().unwrap()) {
            assert_eq!(s.class, w["class"].as_str().unwrap(), "{}", s.proposition_id);
            if let Some(loc) = w.get("locator") {
                let l = &s.locators[0];
                for (k, v) in loc.as_object().unwrap() {
                    assert_eq!(&l[k], v, "{} locator {k}", s.proposition_id);
                }
            }
            if let Some(b) = w.get("bases") {
                assert_eq!(s.bases, b.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect::<Vec<_>>());
            }
        }
        // every planted extra-record statement is withheld, for the stated reason
        let got: BTreeMap<String, String> = out.withheld.iter().map(|w| (w.proposition_id.clone(), w.reason.to_string())).collect();
        let exp: BTreeMap<String, String> = want["withheld"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
        assert_eq!(got, exp);
        // none of the planted content is in the text the screen shows, nor in the log
        let planted: Vec<&str> = fx["propositions"].as_array().unwrap().iter().filter(|p| p["proposition_id"].as_str().unwrap().starts_with('x')).map(|p| p["text"].as_str().unwrap()).collect();
        for t in &planted {
            assert!(!out.rendered_text.contains(t), "planted text rendered: {t}");
        }
        assert!(out.rendered_text.contains("9 statements withheld: not supported by the record or installed law."));
        let log = serde_json::to_string(&out.withheld).unwrap();
        assert!(planted.iter().all(|t| !log.contains(t)), "the log keeps a hash, never the content");
        // AC-S4-11.2: every rendered statement has a class (never EXTRA_RECORD) and a locator
        assert!(out.statements.iter().all(|s| s.class != "EXTRA_RECORD" && !s.locators.is_empty() && CLASSES.contains(&s.class)));
        // deterministic
        assert_eq!(classify(&ledger, &sources, true).rendered_text, out.rendered_text);
    }

    #[test]
    fn with_record_only_off_unsupported_statements_show_labelled_and_never_as_record_or_law() {
        let (fx, sources, ledger) = load();
        let out = classify(&ledger, &sources, false);
        let extra: Vec<&str> = out.statements.iter().filter(|s| s.class == "EXTRA_RECORD").map(|s| s.proposition_id.as_str()).collect();
        assert_eq!(extra, fx["expect"]["record_only_off"]["extra_record_rendered"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect::<Vec<_>>());
        for s in out.statements.iter().filter(|s| s.class == "EXTRA_RECORD") {
            assert!(s.locators.is_empty());
            assert!(out.rendered_text.contains(&format!("[EXTRA_RECORD \u{2014} NOT FROM THE RECORD] {}", s.text)));
        }
        // the legitimate statements are identical with the switch on or off
        let on = classify(&ledger, &sources, true);
        let strip = |c: &Classified| c.statements.iter().filter(|s| s.class != "EXTRA_RECORD").cloned().collect::<Vec<_>>();
        assert_eq!(strip(&on), strip(&out));
    }

    #[test]
    fn record_only_cannot_be_switched_off_once_a_scaffold_exists() {
        assert!(check_change(false, false).is_ok(), "before a scaffold it may be switched off");
        assert!(check_change(true, true).is_ok(), "and back on");
        assert!(check_change(true, false).unwrap_err().starts_with("CF_G7_LOCKED"));
    }

    #[test]
    fn an_inference_rests_only_on_what_itself_renders() {
        // a chain: record -> inference -> inference stays; the same inference over a prompt-supplied statement does not
        let (_, sources, ledger) = load();
        let out = classify(&ledger, &sources, true);
        let p4 = out.statements.iter().find(|s| s.proposition_id == "p4").unwrap();
        assert_eq!(p4.locators.len(), 2, "an inference carries its bases' locators");
        assert!(out.withheld.iter().any(|w| w.proposition_id == "x5" && w.reason == "RESTS_ON_EXTRA_RECORD"));
    }
}
