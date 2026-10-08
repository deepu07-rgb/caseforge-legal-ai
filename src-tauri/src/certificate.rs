//! Audit Certificate (B-S4-05, PRD v4.0 §7.1): a tamper-evident, honestly scoped record that a citation & quote
//! audit was run on a specific draft against specific installed packs on a specific date.
//!
//! It certifies the PROCEDURE, never legal correctness or compliance with any court's rules (fixed scope statement,
//! taken verbatim from the frozen contract). Signed with a per-install Ed25519 key kept in the encrypted app vault.
//! Verification is offline and independent: `scripts/verify_certificate.py` re-implements it with the Python standard
//! library only. Contract: contracts/audit_certificate.schema.json.

use crate::audit::AuditReport;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

pub const SCHEMA_VERSION: &str = "1.0.0";
pub const CANONICALIZATION: &str = "JSON object without signing.signature_hex, keys sorted, compact separators, UTF-8";

static CONTRACT: Lazy<Value> =
    Lazy::new(|| serde_json::from_str(include_str!("../../contracts/audit_certificate.schema.json")).expect("certificate contract"));
/// The scope statement is a `const` in the frozen contract; the certificate uses exactly that text.
pub static SCOPE_STATEMENT: Lazy<String> =
    Lazy::new(|| CONTRACT["properties"]["scope_statement"]["const"].as_str().expect("scope const").to_string());
static PROHIBITED: Lazy<Vec<Regex>> = Lazy::new(|| {
    let g: Value = serde_json::from_str(include_str!("../../contracts/guardrails_stage4.json")).expect("guardrails stage4");
    g["prohibited_copy"]["phrases"]
        .as_array()
        .expect("phrases")
        .iter()
        .map(|p| Regex::new(&format!(r"(?i)\b{}\b", regex::escape(p.as_str().unwrap()))).unwrap())
        .collect()
});

/// Canonical JSON: object keys sorted (explicitly, independent of serde_json feature flags), compact, UTF-8.
/// Python equivalent: json.dumps(v, sort_keys=True, separators=(",", ":"), ensure_ascii=False).
pub fn canonical_json(v: &Value) -> String {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let parts: Vec<String> = keys
                .into_iter()
                .map(|k| format!("{}:{}", serde_json::to_string(k).unwrap(), canonical_json(&map[k])))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::Array(items) => format!("[{}]", items.iter().map(canonical_json).collect::<Vec<_>>().join(",")),
        other => serde_json::to_string(other).unwrap(),
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Stable short identifier of a public key, shown to recipients so they can compare it out of band.
pub fn key_id(public_key: &[u8; 32]) -> String {
    format!("cf-install-{}", &sha256_hex(public_key)[..16])
}

pub fn prohibited_hits(text: &str) -> Vec<String> {
    PROHIBITED.iter().filter_map(|re| re.find(text).map(|m| m.as_str().to_string())).collect()
}

fn signing_bytes(cert: &Value) -> Vec<u8> {
    let mut unsigned = cert.clone();
    if let Some(signing) = unsigned.get_mut("signing").and_then(Value::as_object_mut) {
        signing.remove("signature_hex");
    }
    canonical_json(&unsigned).into_bytes()
}

/// Optional AI-use disclosure written or edited by counsel. Never included unless counsel confirmed it.
pub struct Disclosure {
    pub text: String,
    pub template_id: Option<String>,
    pub confirmed: bool,
}

pub fn issue(
    report: &AuditReport,
    draft_text: &str,
    seed: &[u8; 32],
    app_version: &str,
    disclosure: Option<Disclosure>,
    issued_at: &str,
) -> Result<Value, String> {
    let draft_sha = sha256_hex(draft_text.as_bytes());
    if draft_sha != report.subject.sha256 {
        return Err("CF_AUDIT_CERT_HASH_MISMATCH: this audit report was produced for a different version of the draft; re-run the audit".into());
    }
    let disclosure_value = match disclosure {
        Some(d) if !d.text.trim().is_empty() => {
            if !d.confirmed {
                return Err("CF_AUDIT_CERT_DISCLOSURE_UNCONFIRMED: confirm the disclosure text before it is placed on the certificate".into());
            }
            let hits = prohibited_hits(&d.text);
            if !hits.is_empty() {
                return Err(format!("CF_AUDIT_CERT_PROHIBITED_COPY: disclosure contains prohibited wording {hits:?}"));
            }
            json!({"included": true, "template_id": d.template_id, "text": d.text.trim(), "user_confirmed": true})
        }
        _ => json!({"included": false, "template_id": Value::Null, "text": Value::Null, "user_confirmed": false}),
    };
    let report_value = serde_json::to_value(report).map_err(|e| e.to_string())?;
    let signing_key = SigningKey::from_bytes(seed);
    let public_key = signing_key.verifying_key().to_bytes();
    let mut summary_counts = Map::new();
    for (k, v) in &report.counts {
        summary_counts.insert(k.clone(), json!(v));
    }
    let mut cert = json!({
        "schema_version": SCHEMA_VERSION,
        "certificate_id": format!("cert-{}", &sha256_hex(format!("{}|{}|{}", report.audit_id, issued_at, hex_of(&public_key)).as_bytes())[..20]),
        "issued_at": issued_at,
        "app_version": app_version,
        "draft_sha256": draft_sha,
        "audit_report_sha256": sha256_hex(canonical_json(&report_value).as_bytes()),
        "audit_report": report_value,
        "pack_snapshot": report.pack_snapshot.iter().map(|p| json!({"pack_id": p.pack_id, "version": p.version, "currency_date": p.currency_date})).collect::<Vec<_>>(),
        "model_fingerprint": report.model_fingerprint,
        "summary": {"total_items": report.items.len(), "counts": Value::Object(summary_counts)},
        "scope_statement": SCOPE_STATEMENT.as_str(),
        "disclosure": disclosure_value,
        "export_footer_id": "caseforge.export_footer.v1",
        "signing": {
            "alg": "Ed25519",
            "key_kind": "PER_INSTALL",
            "key_id": key_id(&public_key),
            "public_key_hex": hex_of(&public_key),
            "canonicalization": CANONICALIZATION,
            "signature_hex": ""
        }
    });
    if cert["pack_snapshot"].as_array().map(|a| a.is_empty()).unwrap_or(true) {
        return Err("CF_AUDIT_CERT_NO_PACKS: no legal packs are installed, so there is nothing to certify against".into());
    }
    let signature = signing_key.sign(&signing_bytes(&cert));
    cert["signing"]["signature_hex"] = json!(hex_of(&signature.to_bytes()));
    Ok(cert)
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyOutcome {
    pub valid: bool,
    pub key_id: Option<String>,
    pub checks: Vec<VerifyCheck>,
    pub note: String,
}

fn decode_hex<const N: usize>(s: &str) -> Option<[u8; N]> {
    let bytes = hex::decode(s).ok()?;
    bytes.try_into().ok()
}

/// Offline verification. `draft_text` is optional: without it the document link cannot be checked.
pub fn verify(cert: &Value, draft_text: Option<&str>) -> VerifyOutcome {
    let mut checks = Vec::new();
    let mut check = |name: &str, passed: bool, detail: String| checks.push(VerifyCheck { name: name.into(), passed, detail });

    check("scope_statement_exact", cert["scope_statement"].as_str() == Some(SCOPE_STATEMENT.as_str()), "scope statement equals the frozen contract text".into());
    let disclosure_text = cert["disclosure"]["text"].as_str().unwrap_or("");
    let hits = prohibited_hits(disclosure_text);
    check("no_prohibited_wording", hits.is_empty(), if hits.is_empty() { "none".into() } else { format!("{hits:?}") });
    let report_hash = sha256_hex(canonical_json(&cert["audit_report"]).as_bytes());
    check("audit_report_hash", cert["audit_report_sha256"].as_str() == Some(report_hash.as_str()), "embedded audit report matches its recorded SHA-256".into());
    check("draft_hash_consistent", cert["draft_sha256"] == cert["audit_report"]["subject"]["sha256"], "certificate and audit report name the same draft".into());
    match draft_text {
        Some(t) => {
            let ok = cert["draft_sha256"].as_str() == Some(sha256_hex(t.as_bytes()).as_str());
            check("draft_matches_document", ok, if ok { "the supplied document is the certified draft".into() } else { "the supplied document differs from the certified draft".into() });
        }
        None => check("draft_matches_document", true, "not checked: no document supplied".into()),
    }
    let pk = cert["signing"]["public_key_hex"].as_str().and_then(decode_hex::<32>);
    let sig = cert["signing"]["signature_hex"].as_str().and_then(decode_hex::<64>);
    let key_ok = pk.map(|p| cert["signing"]["key_id"].as_str() == Some(key_id(&p).as_str())).unwrap_or(false);
    check("key_id_matches_key", key_ok, "key_id is the fingerprint of the embedded public key".into());
    let sig_ok = match (pk.and_then(|p| VerifyingKey::from_bytes(&p).ok()), sig) {
        (Some(vk), Some(s)) => vk.verify(&signing_bytes(cert), &Signature::from_bytes(&s)).is_ok(),
        _ => false,
    };
    check("signature", sig_ok, "Ed25519 signature over the canonical certificate".into());
    let valid = checks.iter().all(|c| c.passed);
    VerifyOutcome {
        valid,
        key_id: cert["signing"]["key_id"].as_str().map(str::to_string),
        checks,
        note: "A valid signature proves the certificate is unaltered since this install signed it. To trust WHO signed it, compare the key_id with the one the signing advocate publishes.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::{CorpusChunk, LegalMetadata};

    const FIXTURE: &str = include_str!("../../golden/G-AUDIT/fixtures/audit_core.json");
    const REFERENCE: &str = include_str!("../../golden/G-AUDIT/fixtures/certificate_reference.json");
    const SEED: [u8; 32] = [42u8; 32];
    const FIXED_TIME: &str = "2026-10-08T00:00:00Z";

    fn corpus() -> Vec<CorpusChunk> {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        fx["corpus"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
                CorpusChunk {
                    chunk_id: s("chunk_id"), pack_id: s("pack_id"), path: String::new(), jurisdiction: None, text: s("text"), authority_hint: s("title"),
                    metadata: LegalMetadata {
                        source_id: s("source_id"), title: s("title"), jurisdiction_path: vec![], authority_type: s("authority_type"),
                        issuing_authority: String::new(), citation: s("citation"), section: String::new(), effective_from: String::new(),
                        effective_to: String::new(), version: "fixture".into(), amendment: String::new(), page: v["page"].clone(),
                        paragraph: s("paragraph"), heading_path: vec![], checksum: String::new(), source_checksum: String::new(),
                        pack_checksum: String::new(), trust_level: s("trust_level"), superseded_status: s("superseded_status"),
                        publisher: String::new(), source_reference: String::new(), content_kind: "AUTHORITATIVE_TEXT".into(),
                        temporal_status: s("temporal_status"),
                    },
                }
            })
            .collect()
    }

    fn reference_inputs() -> (String, AuditReport) {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let text = fx["drafts"][0]["text"].as_str().unwrap().to_string();
        let mut report = crate::audit::audit(&text, &corpus(), "fixture.txt", "txt", None);
        report.generated_at = FIXED_TIME.into();
        (text, report)
    }

    fn issue_reference() -> (String, Value) {
        let (text, report) = reference_inputs();
        let cert = issue(&report, &text, &SEED, "test", None, FIXED_TIME).unwrap();
        (text, cert)
    }

    #[test]
    fn issued_certificate_verifies_and_matches_contract() {
        let (text, cert) = issue_reference();
        let out = verify(&cert, Some(&text));
        assert!(out.valid, "{:#?}", out.checks);
        for k in CONTRACT["required"].as_array().unwrap() {
            assert!(cert.get(k.as_str().unwrap()).is_some(), "missing {k}");
        }
    }

    #[test]
    fn any_change_breaks_verification() {
        // AC-S4-01.4
        let (text, cert) = issue_reference();
        let mut altered_draft = text.clone();
        altered_draft.push(' ');
        assert!(!verify(&cert, Some(&altered_draft)).valid, "changed draft must fail");
        let mut c = cert.clone();
        c["audit_report"]["items"][0]["result"] = json!("VERIFIED_WITH_NOTE");
        assert!(!verify(&c, Some(&text)).valid, "changed report must fail");
        let mut c = cert.clone();
        c["summary"]["total_items"] = json!(999);
        assert!(!verify(&c, Some(&text)).valid, "changed summary must fail (signature)");
        let mut c = cert.clone();
        let sig = c["signing"]["signature_hex"].as_str().unwrap().to_string();
        let flipped = format!("{}{}", if sig.starts_with('0') { "1" } else { "0" }, &sig[1..]);
        c["signing"]["signature_hex"] = json!(flipped);
        assert!(!verify(&c, Some(&text)).valid, "changed signature must fail");
        let mut c = cert.clone();
        c["scope_statement"] = json!("This draft is court-approved.");
        assert!(!verify(&c, Some(&text)).valid, "changed scope statement must fail");
    }

    #[test]
    fn disclosure_rules() {
        // AC-S4-01.5: prohibited wording never reaches a certificate; unconfirmed disclosure is refused.
        let (text, report) = reference_inputs();
        let bad = Disclosure { text: "This filing is guaranteed and court-approved.".into(), template_id: None, confirmed: true };
        assert!(issue(&report, &text, &SEED, "test", Some(bad), FIXED_TIME).unwrap_err().contains("PROHIBITED_COPY"));
        let unconfirmed = Disclosure { text: "AI tools were used to check citations.".into(), template_id: None, confirmed: false };
        assert!(issue(&report, &text, &SEED, "test", Some(unconfirmed), FIXED_TIME).unwrap_err().contains("UNCONFIRMED"));
        let ok = Disclosure { text: "AI tools were used to check citations against installed packs.".into(), template_id: None, confirmed: true };
        let cert = issue(&report, &text, &SEED, "test", Some(ok), FIXED_TIME).unwrap();
        assert_eq!(cert["disclosure"]["included"], json!(true));
        assert!(verify(&cert, Some(&text)).valid);
        assert!(prohibited_hits(SCOPE_STATEMENT.as_str()).is_empty(), "scope statement itself must be clean");
    }

    #[test]
    fn stale_report_is_refused() {
        let (text, report) = reference_inputs();
        let err = issue(&report, &format!("{text} edited"), &SEED, "test", None, FIXED_TIME).unwrap_err();
        assert!(err.contains("HASH_MISMATCH"));
    }

    #[test]
    fn reference_certificate_is_reproducible_and_cross_verifiable() {
        // The committed reference (verified independently by scripts/verify_certificate.py) must be exactly what
        // this signer produces: same canonical bytes, same signature.
        let (_, cert) = issue_reference();
        if std::env::var("CF_WRITE_CERT_REFERENCE").is_ok() {
            let (text, _) = reference_inputs();
            let out = json!({"note": "Generated by certificate::tests with a fixed TEST seed ([42; 32]) and time. Never a real install key.", "draft_text": text, "certificate": cert});
            std::fs::write(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../golden/G-AUDIT/fixtures/certificate_reference.json"),
                serde_json::to_string_pretty(&out).unwrap() + "\n",
            )
            .unwrap();
            return;
        }
        let reference: Value = serde_json::from_str(REFERENCE).unwrap();
        assert_eq!(canonical_json(&reference["certificate"]), canonical_json(&cert), "signer output drifted from the committed reference");
        assert!(verify(&reference["certificate"], reference["draft_text"].as_str()).valid);
    }
}
