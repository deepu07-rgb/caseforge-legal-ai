//! Host-owned proposition and quotation verifier (B-S3-14).
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const SCHEMA_VERSION: &str = "caseforge.proposition-ledger.v1";
/// Guardrail G10 (B-S4-15): the label a translated statement or quotation carries. Text read from a machine translation never becomes a
/// verified fact or a verified quotation; it is shown with this label until counsel has checked the original.
pub const TRANSLATION_LABEL: &str = "translation - wording not verified";
pub const CLASSES: &[&str] = &[
    "FACTUAL_BASIS",
    "LEGAL_BASIS",
    "INFERENCE",
    "COUNSEL_INPUT",
    "UNSUPPORTED_OR_GAP",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportClaim {
    pub support_id: String,
    pub exact_excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropositionInput {
    pub proposition_id: String,
    pub class: String,
    pub text: String,
    #[serde(default)]
    pub support_ids: Vec<String>,
    #[serde(default)]
    pub basis_ids: Vec<String>,
    #[serde(default)]
    pub support_claims: Vec<SupportClaim>,
    #[serde(default)]
    pub quotation: Option<SupportClaim>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportSource {
    pub support_id: String,
    pub kind: String,
    pub exact_text: String,
    pub source_id: Option<String>,
    pub chunk_id: Option<String>,
    pub authority_id: Option<String>,
    pub doc_id: Option<String>,
    pub page: Option<i64>,
    pub paragraph: Option<String>,
    pub trust_level: Option<String>,
    pub temporal_status: Option<String>,
    pub superseded_status: Option<String>,
    #[serde(default)]
    pub permitted: bool,
    /// B-S4-15 (G10): this text is machine translation of the original, not the original.
    #[serde(default)]
    pub translated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRequest {
    pub output_kind: String,
    #[serde(default)]
    pub propositions: Vec<PropositionInput>,
    #[serde(default)]
    pub support_sources: Vec<SupportSource>,
    #[serde(default)]
    pub permitted_authority_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceOffsets {
    pub support_id: String,
    pub char_start: usize,
    pub char_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedProposition {
    pub proposition_id: String,
    pub class: String,
    pub text: String,
    pub support_ids: Vec<String>,
    pub basis_ids: Vec<String>,
    pub support_claims: Vec<SupportClaim>,
    pub quotation: Option<SupportClaim>,
    pub verification_status: String,
    pub render_action: String,
    pub verified_support_ids: Vec<String>,
    pub rejected_support_ids: Vec<String>,
    pub quote_offsets: Option<SourceOffsets>,
    pub reason: String,
    /// G10: the proposition rests (directly, through a quotation, or through an earlier proposition) on machine-translated text.
    #[serde(default)]
    pub translated_basis: bool,
    /// The quotation, if any, is an exact stored substring of ORIGINAL-language text. A quotation of a translation is never verified.
    #[serde(default)]
    pub quote_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropositionLedger {
    pub schema_version: String,
    pub batch: String,
    pub output_kind: String,
    pub propositions: Vec<VerifiedProposition>,
    pub rendered_text: String,
    pub trusted_count: usize,
    pub gap_count: usize,
    pub blocked_count: usize,
    pub all_legal_grounded: bool,
    pub all_quotes_exact: bool,
}

fn exact_offsets(source: &str, excerpt: &str, support_id: &str) -> Option<SourceOffsets> {
    if excerpt.is_empty() {
        return None;
    }
    let byte_start = source.find(excerpt)?;
    let byte_end = byte_start + excerpt.len();
    Some(SourceOffsets {
        support_id: support_id.into(),
        char_start: source[..byte_start].chars().count(),
        char_end: source[..byte_end].chars().count(),
    })
}

fn legal_source_allowed(source: &SupportSource, permitted: &HashSet<&str>) -> bool {
    source.kind == "LEGAL_AUTHORITY"
        && source.permitted
        && source
            .authority_id
            .as_deref()
            .map(|id| permitted.contains(id))
            .unwrap_or(false)
        && source.trust_level.as_deref() == Some("OFFICIAL_VERIFIED")
        && source.temporal_status.as_deref() == Some("APPLICABLE")
        && source.superseded_status.as_deref() == Some("CURRENT")
        && source
            .source_id
            .as_deref()
            .is_some_and(|id| id.starts_with("cfsrc:"))
        && source
            .chunk_id
            .as_deref()
            .is_some_and(|id| id.starts_with("cfchunk:"))
}

fn render(prop: &VerifiedProposition) -> String {
    let translation = if prop.translated_basis { format!(" [TRANSLATION - {}]", "wording not verified") } else { String::new() };
    match prop.render_action.as_str() {
        "TRUSTED" => format!(
            "{} [VERIFIED: {}]",
            prop.text,
            prop.verified_support_ids.join(", ")
        ),
        "LABELLED" if prop.class == "INFERENCE" => format!(
            "[INFERENCE — bases: {}]{} {}",
            prop.basis_ids.join(", "),
            translation,
            prop.text
        ),
        "LABELLED" if prop.class == "FACTUAL_BASIS" => format!("[TRANSLATION - wording not verified] {}", prop.text),
        "LABELLED" => format!("[COUNSEL INPUT] {}", prop.text),
        "REPOSITORY_GAP" => format!("[REPOSITORY GAP — NOT VERIFIED] {}", prop.text),
        _ => format!("[BLOCKED UNSUPPORTED PROPOSITION] {}", prop.text),
    }
}

pub fn verify(request: VerifyRequest) -> Result<PropositionLedger, String> {
    let allowed_outputs = [
        "DIGEST",
        "DRAFTING",
        "ARGUMENT_INTEGRITY",
        "MOCK_COURT",
        "REASONING",
    ];
    if !allowed_outputs.contains(&request.output_kind.as_str()) {
        return Err("CF_PROPOSITION_OUTPUT_KIND: unsupported output kind".into());
    }
    let mut source_map: HashMap<&str, &SupportSource> = HashMap::new();
    for source in &request.support_sources {
        if source.support_id.trim().is_empty()
            || source_map
                .insert(source.support_id.as_str(), source)
                .is_some()
        {
            return Err("CF_PROPOSITION_SUPPORT_SOURCE: blank or duplicate support_id".into());
        }
    }
    let permitted: HashSet<&str> = request
        .permitted_authority_ids
        .iter()
        .map(String::as_str)
        .collect();
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut translated_props: HashSet<String> = HashSet::new();
    let mut ids = HashSet::new();
    let mut verified = Vec::new();

    for input in request.propositions {
        if input.proposition_id.trim().is_empty() || !ids.insert(input.proposition_id.clone()) {
            return Err("CF_PROPOSITION_ID: proposition IDs must be non-empty and unique".into());
        }
        if !CLASSES.contains(&input.class.as_str()) || input.text.trim().is_empty() {
            return Err("CF_PROPOSITION_SCHEMA: invalid class or empty text".into());
        }
        let mut good = Vec::new();
        let mut bad = Vec::new();
        let mut claims_by_id: HashMap<&str, &SupportClaim> = HashMap::new();
        for claim in &input.support_claims {
            claims_by_id.insert(claim.support_id.as_str(), claim);
        }
        for support_id in &input.support_ids {
            let Some(source) = source_map.get(support_id.as_str()).copied() else {
                bad.push(support_id.clone());
                continue;
            };
            let Some(claim) = claims_by_id.get(support_id.as_str()).copied() else {
                bad.push(support_id.clone());
                continue;
            };
            let exact =
                exact_offsets(&source.exact_text, &claim.exact_excerpt, support_id).is_some();
            let class_ok = match input.class.as_str() {
                "LEGAL_BASIS" => legal_source_allowed(source, &permitted),
                "FACTUAL_BASIS" => source.kind == "MATTER_SOURCE" && source.doc_id.is_some(),
                "COUNSEL_INPUT" => source.kind == "COUNSEL_INPUT",
                _ => true,
            };
            if exact && class_ok {
                good.push(support_id.clone());
            } else {
                bad.push(support_id.clone());
            }
        }
        let quote_offsets = input.quotation.as_ref().and_then(|quote| {
            source_map
                .get(quote.support_id.as_str())
                .and_then(|source| {
                    exact_offsets(&source.exact_text, &quote.exact_excerpt, &quote.support_id)
                })
        });
        let quote_failed = input.quotation.is_some() && quote_offsets.is_none();
        // G10: a quotation is verified only against original-language text.
        let quote_translated = input.quotation.as_ref().is_some_and(|q| source_map.get(q.support_id.as_str()).is_some_and(|s| s.translated));
        let rests_on_translation = quote_translated
            || input.support_ids.iter().any(|id| source_map.get(id.as_str()).is_some_and(|s| s.translated))
            || input.basis_ids.iter().any(|id| translated_props.contains(id));
        let bases_ok = !input.basis_ids.is_empty()
            && input
                .basis_ids
                .iter()
                .all(|id| seen.get(id).is_some_and(|status| status != "RED"));
        let (status, action, reason) = if quote_failed {
            (
                "RED",
                "BLOCKED",
                "Direct quotation did not exactly match stored source text and offsets.",
            )
        } else {
            match input.class.as_str() {
                "LEGAL_BASIS" if !good.is_empty() && bad.is_empty() => ("GREEN", "TRUSTED", "Exact excerpt verified against permitted official, applicable, current authority."),
                "LEGAL_BASIS" => ("RED", "REPOSITORY_GAP", "No permitted verified authority and exact excerpt support this legal proposition."),
                "FACTUAL_BASIS" if !good.is_empty() && bad.is_empty() => ("GREEN", "TRUSTED", "Exact excerpt and matter source pointer verified."),
                "FACTUAL_BASIS" => ("RED", "BLOCKED", "No exact matter-source excerpt and pointer support this factual proposition."),
                "COUNSEL_INPUT" if !good.is_empty() && bad.is_empty() => ("AMBER", "LABELLED", "Exact counsel input is linked and remains distinct from evidence."),
                "COUNSEL_INPUT" => ("RED", "BLOCKED", "Counsel input lacks an exact linked support record."),
                "INFERENCE" if bases_ok => ("AMBER", "LABELLED", "Inference is explicitly linked to previously verified proposition bases."),
                "INFERENCE" => ("RED", "BLOCKED", "Inference has missing, later, or unsupported proposition bases."),
                _ => ("RED", "REPOSITORY_GAP", "Proposition is explicitly unsupported or a repository gap."),
            }
        };
        let (status, action, reason) = if rests_on_translation && status == "GREEN" {
            (
                "AMBER",
                "LABELLED",
                "Rests on machine translation: the wording is not verified and a translated sentence is never a verified quotation (guardrail G10). Check the original.",
            )
        } else {
            (status, action, reason)
        };
        if rests_on_translation {
            translated_props.insert(input.proposition_id.clone());
        }
        let has_quote = input.quotation.is_some();
        seen.insert(input.proposition_id.clone(), status.into());
        verified.push(VerifiedProposition {
            proposition_id: input.proposition_id,
            class: input.class,
            text: input.text,
            support_ids: input.support_ids,
            basis_ids: input.basis_ids,
            support_claims: input.support_claims,
            quotation: input.quotation,
            verification_status: status.into(),
            render_action: action.into(),
            verified_support_ids: good,
            rejected_support_ids: bad,
            quote_offsets,
            reason: reason.into(),
            translated_basis: rests_on_translation,
            quote_verified: has_quote && !quote_failed && !quote_translated,
        });
    }
    let rendered_text = verified.iter().map(render).collect::<Vec<_>>().join("\n\n");
    Ok(PropositionLedger {
        schema_version: SCHEMA_VERSION.into(),
        batch: "B-S3-14".into(),
        output_kind: request.output_kind,
        trusted_count: verified
            .iter()
            .filter(|p| p.render_action == "TRUSTED")
            .count(),
        gap_count: verified
            .iter()
            .filter(|p| p.render_action == "REPOSITORY_GAP")
            .count(),
        blocked_count: verified
            .iter()
            .filter(|p| p.render_action == "BLOCKED")
            .count(),
        all_legal_grounded: verified
            .iter()
            .filter(|p| p.class == "LEGAL_BASIS")
            .all(|p| p.verification_status == "GREEN"),
        all_quotes_exact: verified
            .iter()
            .filter(|p| p.quotation.is_some())
            .all(|p| p.quote_verified),
        propositions: verified,
        rendered_text,
    })
}

pub fn policy() -> serde_json::Value {
    serde_json::json!({"batch":"B-S3-14","schema_version":SCHEMA_VERSION,"classes":CLASSES,"trust_boundary":"Host assigns support state; model source IDs and citations are untrusted input.","legal_green":"permitted + OFFICIAL_VERIFIED + APPLICABLE + CURRENT + stable source/chunk IDs + exact excerpt","quotation":"exact stored substring with character offsets","unsupported":"blocked or visibly rendered as repository gap","translation":{"guardrail":"G10","label":TRANSLATION_LABEL,"rule":"text read from a machine translation is never GREEN and a quotation of it is never verified"}})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn legal() -> SupportSource {
        SupportSource {
            support_id: "auth:1".into(),
            kind: "LEGAL_AUTHORITY".into(),
            exact_text: "Section 4 requires notice before filing.".into(),
            source_id: Some("cfsrc:pack:0123456789abcdef".into()),
            chunk_id: Some("cfchunk:pack:0123456789abcdef".into()),
            authority_id: Some("cfauth:1".into()),
            doc_id: None,
            page: Some(1),
            paragraph: Some("4".into()),
            trust_level: Some("OFFICIAL_VERIFIED".into()),
            temporal_status: Some("APPLICABLE".into()),
            superseded_status: Some("CURRENT".into()),
            permitted: true,
            translated: false,
        }
    }
    fn prop(class: &str, id: &str, support: &str, excerpt: &str) -> PropositionInput {
        PropositionInput {
            proposition_id: id.into(),
            class: class.into(),
            text: excerpt.into(),
            support_ids: vec![support.into()],
            basis_ids: vec![],
            support_claims: vec![SupportClaim {
                support_id: support.into(),
                exact_excerpt: excerpt.into(),
            }],
            quotation: None,
        }
    }
    #[test]
    fn verified_legal_green() {
        let l = verify(VerifyRequest {
            output_kind: "DRAFTING".into(),
            propositions: vec![prop(
                "LEGAL_BASIS",
                "p1",
                "auth:1",
                "Section 4 requires notice before filing.",
            )],
            support_sources: vec![legal()],
            permitted_authority_ids: vec!["cfauth:1".into()],
        })
        .unwrap();
        assert_eq!(l.propositions[0].verification_status, "GREEN");
    }
    #[test]
    fn fabricated_id_is_gap() {
        let l = verify(VerifyRequest {
            output_kind: "DRAFTING".into(),
            propositions: vec![prop("LEGAL_BASIS", "p1", "auth:fake", "Made up rule")],
            support_sources: vec![legal()],
            permitted_authority_ids: vec!["cfauth:1".into()],
        })
        .unwrap();
        assert_eq!(l.propositions[0].render_action, "REPOSITORY_GAP");
    }
    #[test]
    fn wrong_jurisdiction_or_policy_cannot_green() {
        let mut s = legal();
        s.temporal_status = Some("EXPIRED".into());
        let l = verify(VerifyRequest {
            output_kind: "DRAFTING".into(),
            propositions: vec![prop(
                "LEGAL_BASIS",
                "p1",
                "auth:1",
                "Section 4 requires notice before filing.",
            )],
            support_sources: vec![s],
            permitted_authority_ids: vec!["cfauth:1".into()],
        })
        .unwrap();
        assert_eq!(l.propositions[0].verification_status, "RED");
    }
    #[test]
    fn exact_quote_has_unicode_char_offsets() {
        let mut p = prop(
            "LEGAL_BASIS",
            "p1",
            "auth:1",
            "Section 4 requires notice before filing.",
        );
        p.quotation = Some(SupportClaim {
            support_id: "auth:1".into(),
            exact_excerpt: "requires notice".into(),
        });
        let l = verify(VerifyRequest {
            output_kind: "DRAFTING".into(),
            propositions: vec![p],
            support_sources: vec![legal()],
            permitted_authority_ids: vec!["cfauth:1".into()],
        })
        .unwrap();
        assert!(l.all_quotes_exact);
        assert_eq!(
            l.propositions[0].quote_offsets.as_ref().unwrap().char_start,
            10
        );
    }
    #[test]
    fn altered_quote_blocked() {
        let mut p = prop(
            "LEGAL_BASIS",
            "p1",
            "auth:1",
            "Section 4 requires notice before filing.",
        );
        p.quotation = Some(SupportClaim {
            support_id: "auth:1".into(),
            exact_excerpt: "requires prior notice".into(),
        });
        let l = verify(VerifyRequest {
            output_kind: "DRAFTING".into(),
            propositions: vec![p],
            support_sources: vec![legal()],
            permitted_authority_ids: vec!["cfauth:1".into()],
        })
        .unwrap();
        assert_eq!(l.blocked_count, 1);
        assert!(!l.all_quotes_exact);
    }
    #[test]
    fn unsupported_inference_blocked() {
        let p = PropositionInput {
            proposition_id: "i1".into(),
            class: "INFERENCE".into(),
            text: "Therefore ownership follows.".into(),
            support_ids: vec![],
            basis_ids: vec!["missing".into()],
            support_claims: vec![],
            quotation: None,
        };
        let l = verify(VerifyRequest {
            output_kind: "REASONING".into(),
            propositions: vec![p],
            support_sources: vec![],
            permitted_authority_ids: vec![],
        })
        .unwrap();
        assert_eq!(l.blocked_count, 1);
    }

    // ---- guardrail G10 (B-S4-15): a translation never becomes a verified fact or a verified quotation ----
    const HINDI: &str = "आरोपी ने दिनांक १० मार्च २०२४ को ₹ १,२५,००० की मांग की।";
    const ENGLISH: &str = "The accused demanded Rs. 1,25,000 on 10 March 2024.";

    fn matter(id: &str, text: &str, translated: bool) -> SupportSource {
        SupportSource {
            support_id: id.into(),
            kind: "MATTER_SOURCE".into(),
            exact_text: text.into(),
            source_id: None,
            chunk_id: None,
            authority_id: None,
            doc_id: Some("doc-fir".into()),
            page: Some(1),
            paragraph: None,
            trust_level: None,
            temporal_status: None,
            superseded_status: None,
            permitted: true,
            translated,
        }
    }
    fn run(props: Vec<PropositionInput>, sources: Vec<SupportSource>) -> PropositionLedger {
        verify(VerifyRequest { output_kind: "REASONING".into(), propositions: props, support_sources: sources, permitted_authority_ids: vec![] }).unwrap()
    }
    #[test]
    fn g10_translated_fact_is_never_green() {
        let l = run(vec![prop("FACTUAL_BASIS", "p1", "tr:1", ENGLISH)], vec![matter("tr:1", ENGLISH, true)]);
        let p = &l.propositions[0];
        assert_eq!((p.verification_status.as_str(), p.render_action.as_str()), ("AMBER", "LABELLED"));
        assert!(p.translated_basis && l.trusted_count == 0);
        assert!(l.rendered_text.contains("[TRANSLATION - wording not verified]"), "{}", l.rendered_text);
        assert!(!l.rendered_text.contains("[VERIFIED"));
    }
    #[test]
    fn g10_original_language_fact_is_green() {
        let l = run(vec![prop("FACTUAL_BASIS", "p1", "orig:1", HINDI)], vec![matter("orig:1", HINDI, false)]);
        assert_eq!((l.propositions[0].verification_status.as_str(), l.propositions[0].render_action.as_str()), ("GREEN", "TRUSTED"));
        assert!(!l.propositions[0].translated_basis);
    }
    #[test]
    fn g10_translated_quotation_is_not_verified() {
        let mut p = prop("FACTUAL_BASIS", "p1", "tr:1", ENGLISH);
        p.quotation = Some(SupportClaim { support_id: "tr:1".into(), exact_excerpt: "demanded Rs. 1,25,000".into() });
        let l = run(vec![p], vec![matter("tr:1", ENGLISH, true)]);
        assert!(l.propositions[0].quote_offsets.is_some(), "it is an exact substring of the translation, and still not a verified quotation");
        assert!(!l.propositions[0].quote_verified && !l.all_quotes_exact);
        assert_ne!(l.propositions[0].verification_status, "GREEN");
    }
    #[test]
    fn g10_original_quotation_is_verified() {
        let mut p = prop("FACTUAL_BASIS", "p1", "orig:1", HINDI);
        p.quotation = Some(SupportClaim { support_id: "orig:1".into(), exact_excerpt: "की मांग की".into() });
        let l = run(vec![p], vec![matter("orig:1", HINDI, false)]);
        assert!(l.propositions[0].quote_verified && l.all_quotes_exact);
    }
    #[test]
    fn g10_translated_source_cannot_launder_a_fact_through_a_second_source() {
        let mut p = prop("FACTUAL_BASIS", "p1", "orig:1", HINDI);
        p.support_ids.push("tr:1".into());
        p.support_claims.push(SupportClaim { support_id: "tr:1".into(), exact_excerpt: ENGLISH.into() });
        let l = run(vec![p], vec![matter("orig:1", HINDI, false), matter("tr:1", ENGLISH, true)]);
        assert_ne!(l.propositions[0].verification_status, "GREEN");
        assert!(l.propositions[0].translated_basis);
    }
    #[test]
    fn g10_inference_on_a_translated_basis_stays_labelled() {
        let base = prop("FACTUAL_BASIS", "p1", "tr:1", ENGLISH);
        let inf = PropositionInput { proposition_id: "i1".into(), class: "INFERENCE".into(), text: "So the demand preceded the complaint.".into(), support_ids: vec![], basis_ids: vec!["p1".into()], support_claims: vec![], quotation: None };
        let l = run(vec![base, inf], vec![matter("tr:1", ENGLISH, true)]);
        let i = &l.propositions[1];
        assert_eq!((i.verification_status.as_str(), i.render_action.as_str()), ("AMBER", "LABELLED"));
        assert!(i.translated_basis, "the label follows the inference");
        assert!(l.rendered_text.contains("[INFERENCE") && l.rendered_text.matches("TRANSLATION").count() >= 2);
    }
    #[test]
    fn g10_legal_authority_is_unaffected() {
        let l = verify(VerifyRequest {
            output_kind: "DRAFTING".into(),
            propositions: vec![prop("LEGAL_BASIS", "p1", "auth:1", "Section 4 requires notice before filing.")],
            support_sources: vec![legal()],
            permitted_authority_ids: vec!["cfauth:1".into()],
        })
        .unwrap();
        assert_eq!(l.propositions[0].verification_status, "GREEN");
        assert!(!l.propositions[0].translated_basis);
    }
}
