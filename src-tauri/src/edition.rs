//! Editions and the Chambers Edition shell (B-S4-17, PRD v4.0 §9 and §10).
//!
//! The Chambers Edition is the same application with a `JUDICIARY` licence. What a licence entitles comes only from its
//! signed `features[]` (the edition-to-feature map and prices are owner decisions Q-S4-06, authored in B-S4-22); this
//! module only recognises the `chambers` feature and the invariants the frozen contract gives the JUDICIARY edition:
//! institution-issued, air-gap forced on, no telemetry.
//!
//! Enforcement is in the host, not the screen:
//!   - `check_generation` sits in front of every call to the local model: in Chambers, free generation is refused, and the
//!     only way to the model is the record-only reasoning path (`record_only_permit`), whose output the G7 classifier
//!     (record_only.rs) then confines to the record and installed law;
//!   - `require_chambers` / `require_advocacy` gate commands by edition; the screen merely hides what the host refuses;
//!   - the air-gap switch cannot be turned off under a JUDICIARY licence.
//! An offline licence check: Ed25519 over the canonical payload with the keys in contracts/licence_trust.json, 3-of-4
//! machine fingerprint, no network. A v1 licence grants no Stage 4 surface.
//!
//! Pilot court constraints (Q-S4-10) are configuration: `<vault>/chambers_pilot.json` lists capabilities to switch off.

use crate::licence::{self, FingerprintComponents, LicenceState, FINGERPRINT_REQUIRED};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

pub const EDITIONS: [&str; 4] = ["ADVOCATE", "PRO", "CHAMBERS_FIRM", "JUDICIARY"];
/// Feature ids a signed v2 licence may carry (contracts/licence_payload_v2.schema.json).
pub const KNOWN_FEATURES: [&str; 18] = [
    "digest", "voice", "retrieval", "export", "mock_court", "audit", "audit_certificate", "opposition_scan", "witness_matrix", "impeachment_kit", "code_bridge", "foundation_gaps",
    "deadline_watch", "timed_rehearsal", "currency_treatment", "indic_ingestion", "bundle_exchange", "chambers",
];
/// Every screen the app has, by navigation id.
pub const ALL_SURFACES: [&str; 21] = [
    "home", "cases", "new-case", "workspace", "builder", "integrity", "digest", "voice", "experience", "court", "opposition", "witnesses", "deadlines", "issues", "chambers", "practice",
    "library", "licence", "protection", "guardrails", "settings",
];
/// Screens that exist for the Chambers Edition only (PRD §6 F11-F15).
const CHAMBERS_ONLY: [&str; 2] = ["chambers", "issues"];
/// Advocacy-only screens, hidden in the Chambers Edition: persuasion coaching, mock court, cross-examination and
/// opposition work, argument scoring.
const ADVOCACY_ONLY: [&str; 8] = ["builder", "integrity", "voice", "experience", "court", "opposition", "witnesses", "practice"];
/// Capabilities a pilot court may switch off in `chambers_pilot.json`.
pub const PILOT_CAPABILITIES: [&str; 5] = ["reasoning", "issue_matrix", "memo_export", "citation_audit", "record_lookup"];

/// A verified v2 licence.
#[derive(Debug, Clone)]
pub struct V2Licence {
    pub licence_id: String,
    pub edition: String,
    pub features: Vec<String>,
    pub air_gap_forced: bool,
    pub telemetry_allowed: bool,
    pub expires_at_unix_ms: Option<i64>,
    pub pack_subscription_until_unix_ms: Option<i64>,
}

/// What the rest of the app needs to know about the edition.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EditionContext {
    pub edition: Option<String>,
    pub chambers: bool,
    /// LICENCE (a verified v2 licence), DEV_OVERRIDE (debug builds only) or NONE.
    pub source: &'static str,
    pub licence_state: &'static str,
    pub features: Vec<String>,
    pub air_gap_forced: bool,
    pub telemetry_allowed: bool,
    pub record_only_default: bool,
    pub visible_surfaces: Vec<&'static str>,
    pub hidden_surfaces: Vec<&'static str>,
    pub problem: Option<String>,
}

pub fn surfaces(chambers: bool) -> (Vec<&'static str>, Vec<&'static str>) {
    let hidden: Vec<&'static str> = ALL_SURFACES.iter().copied().filter(|s| if chambers { ADVOCACY_ONLY.contains(s) } else { CHAMBERS_ONLY.contains(s) }).collect();
    let visible = ALL_SURFACES.iter().copied().filter(|s| !hidden.contains(s)).collect();
    (visible, hidden)
}

fn base(chambers: bool, edition: Option<String>, source: &'static str, state: LicenceState, features: Vec<String>, air_gap_forced: bool, telemetry_allowed: bool, problem: Option<String>) -> EditionContext {
    let (visible_surfaces, hidden_surfaces) = surfaces(chambers);
    EditionContext { edition, chambers, source, licence_state: state.as_str(), features, air_gap_forced, telemetry_allowed, record_only_default: chambers, visible_surfaces, hidden_surfaces, problem }
}

/// The context when there is no licence that grants Stage 4 surfaces (no licence, a v1 licence, or one that does not verify).
pub fn none_context(problem: Option<String>) -> EditionContext {
    base(false, None, "NONE", LicenceState::Unlicensed, vec![], false, true, problem)
}

pub fn context_for(l: &V2Licence, state: LicenceState) -> EditionContext {
    let chambers = l.edition == "JUDICIARY" && l.features.iter().any(|f| f == "chambers");
    // Judiciary licences: air-gap forced, no telemetry (the contract makes these constants for JUDICIARY).
    let (forced, telemetry) = if l.edition == "JUDICIARY" { (true, false) } else { (l.air_gap_forced, l.telemetry_allowed) };
    base(chambers, Some(l.edition.clone()), "LICENCE", state, l.features.clone(), forced, telemetry, None)
}

/// Canonical signing bytes of a v2 payload: every key but `signature_hex` and `signature_alg`, keys sorted, compact JSON,
/// non-ASCII escaped as \uXXXX - the rule of v1 (eval/tools/make_licence.py `canonical_bytes`), applied to the whole object.
pub fn canonical_v2(raw: &Value) -> Result<Vec<u8>, String> {
    let obj = raw.as_object().ok_or("CF_LICENCE_ACTIVATION_FAILED: payload is not an object")?;
    let body: BTreeMap<&String, &Value> = obj.iter().filter(|(k, _)| *k != "signature_hex" && *k != "signature_alg").collect();
    let mut out = String::new();
    write_canonical(&Value::Object(body.into_iter().map(|(k, v)| (k.clone(), v.clone())).collect()), &mut out);
    Ok(out.into_bytes())
}

fn write_canonical(v: &Value, out: &mut String) {
    match v {
        Value::Object(m) => {
            let sorted: BTreeMap<&String, &Value> = m.iter().collect();
            out.push('{');
            for (i, (k, val)) in sorted.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_str(k, out);
                out.push(':');
                write_canonical(val, out);
            }
            out.push('}');
        }
        Value::Array(a) => {
            out.push('[');
            for (i, val) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(val, out);
            }
            out.push(']');
        }
        Value::String(s) => write_str(s, out),
        other => out.push_str(&other.to_string()),
    }
}

/// Python json.dumps(ensure_ascii=True) string form.
fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e && (c as u32) != 0x7f => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{:04x}", unit));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn str_of<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// Verifies a v2 licence payload offline. `keys` are the trusted public keys (hex) by key id.
pub fn verify_v2(raw: &Value, keys: &BTreeMap<String, String>, live: &FingerprintComponents, now_ms: i64) -> Result<(V2Licence, LicenceState), String> {
    let fail = |m: &str| Err(format!("CF_LICENCE_ACTIVATION_FAILED: {m}"));
    if str_of(raw, "schema_version") != "2.0.0" {
        return fail("not a version 2 licence");
    }
    if str_of(raw, "product") != "CaseForge" {
        return fail("product mismatch");
    }
    let edition = str_of(raw, "edition");
    if !EDITIONS.contains(&edition) {
        return fail("unknown edition");
    }
    let features: Vec<String> = raw.get("features").and_then(Value::as_array).map(|a| a.iter().filter_map(|f| f.as_str().map(str::to_string)).collect()).ok_or("CF_LICENCE_ACTIVATION_FAILED: features missing")?;
    let mut seen = std::collections::HashSet::new();
    if features.is_empty() || !features.iter().all(|f| KNOWN_FEATURES.contains(&f.as_str()) && seen.insert(f.clone())) {
        return fail("features must be unique and known");
    }
    let (air, tele) = (raw.get("air_gap_forced").and_then(Value::as_bool), raw.get("telemetry_allowed").and_then(Value::as_bool));
    let (Some(air_gap_forced), Some(telemetry_allowed)) = (air, tele) else { return fail("air_gap_forced and telemetry_allowed are required") };
    if edition == "JUDICIARY" && (!air_gap_forced || telemetry_allowed || str_of(raw, "issued_to_kind") != "INSTITUTION") {
        return fail("a JUDICIARY licence is institution-issued, forces air-gap on and allows no telemetry");
    }
    if edition != "JUDICIARY" && features.iter().any(|f| f == "chambers") {
        return fail("only a JUDICIARY licence may carry the chambers feature");
    }
    // signature
    if str_of(raw, "signature_alg") != "Ed25519" {
        return Err("CF_LICENCE_INVALID_SIGNATURE: unsupported alg".into());
    }
    let key_id = str_of(raw, "signer_key_id");
    let pk_hex = keys.get(key_id).ok_or_else(|| format!("CF_LICENCE_INVALID_SIGNATURE: unknown signer_key_id {key_id}"))?;
    let pk: [u8; 32] = hex::decode(pk_hex).ok().and_then(|b| b.try_into().ok()).ok_or("CF_LICENCE_TRUST: bad key")?;
    let vk = VerifyingKey::from_bytes(&pk).map_err(|e| format!("CF_LICENCE_TRUST: {e}"))?;
    let sig: [u8; 64] = hex::decode(str_of(raw, "signature_hex")).ok().and_then(|b| b.try_into().ok()).ok_or("CF_LICENCE_INVALID_SIGNATURE: bad signature")?;
    vk.verify(&canonical_v2(raw)?, &Signature::from_bytes(&sig)).map_err(|_| "CF_LICENCE_INVALID_SIGNATURE: verify failed".to_string())?;
    // machine
    let bound: FingerprintComponents = serde_json::from_value(raw.get("fingerprint_components").cloned().unwrap_or(Value::Null)).map_err(|_| "CF_LICENCE_ACTIVATION_FAILED: fingerprint missing".to_string())?;
    let matches = live.match_count(&bound);
    if matches < FINGERPRINT_REQUIRED {
        return Err(format!("CF_LICENCE_FINGERPRINT_MISMATCH: matched {matches}/4 (need {FINGERPRINT_REQUIRED})"));
    }
    let expires = raw.get("expires_at_unix_ms").and_then(Value::as_i64);
    let state = match expires {
        Some(e) if e > 0 && now_ms > e => LicenceState::Expired,
        _ => LicenceState::Active,
    };
    Ok((
        V2Licence {
            licence_id: str_of(raw, "licence_id").to_string(),
            edition: edition.to_string(),
            features,
            air_gap_forced,
            telemetry_allowed,
            expires_at_unix_ms: expires,
            pack_subscription_until_unix_ms: raw.get("pack_subscription_until_unix_ms").and_then(Value::as_i64),
        },
        state,
    ))
}

/// True if the stored payload is a v2 licence (version 2.0.0).
pub fn is_v2(payload_json: &str) -> bool {
    serde_json::from_str::<Value>(payload_json).map(|v| str_of(&v, "schema_version") == "2.0.0").unwrap_or(false)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// The edition context from what the vault stores (state, payload). Anything that does not verify grants nothing.
pub fn context_from_stored(stored: Option<(&str, &str)>) -> EditionContext {
    if let Some(ctx) = dev_override() {
        return ctx;
    }
    let Some((_, payload)) = stored else { return none_context(None) };
    let Ok(raw) = serde_json::from_str::<Value>(payload) else { return none_context(None) };
    if str_of(&raw, "schema_version") != "2.0.0" {
        return none_context(None); // a v1 licence grants no Stage 4 surface
    }
    let keys = match licence::trust_keys() {
        Ok(k) => k,
        Err(e) => return none_context(Some(e)),
    };
    match verify_v2(&raw, &keys, &live_fingerprint(), now_ms()) {
        Ok((l, state)) => context_for(&l, state),
        Err(e) => none_context(Some(e)),
    }
}

fn live_fingerprint() -> FingerprintComponents {
    static LIVE: once_cell::sync::Lazy<FingerprintComponents> = once_cell::sync::Lazy::new(licence::collect_fingerprint);
    LIVE.clone()
}

/// Debug builds only: CASEFORGE_DEV_EDITION=JUDICIARY previews the Chambers Edition without a licence. Release builds
/// never read it, so it cannot be used to grant or remove an entitlement in the field.
fn dev_override() -> Option<EditionContext> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let e = std::env::var("CASEFORGE_DEV_EDITION").ok()?;
    if e != "JUDICIARY" {
        return None;
    }
    let l = V2Licence { licence_id: "dev-preview".into(), edition: "JUDICIARY".into(), features: vec!["chambers".into()], air_gap_forced: true, telemetry_allowed: false, expires_at_unix_ms: None, pack_subscription_until_unix_ms: None };
    let mut c = context_for(&l, LicenceState::Active);
    c.source = "DEV_OVERRIDE";
    Some(c)
}

// --- host enforcement ---

static CHAMBERS: AtomicBool = AtomicBool::new(false);

/// Called whenever the vault learns its licence (unlock, activation, refresh).
pub fn set_chambers(on: bool) {
    CHAMBERS.store(on, Ordering::SeqCst);
}
pub fn chambers_active() -> bool {
    CHAMBERS.load(Ordering::SeqCst)
}

thread_local! {
    static PERMIT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Held by the record-only reasoning path while it calls the model. Free generation has no permit.
pub struct RecordOnlyPermit(());
pub fn record_only_permit() -> RecordOnlyPermit {
    PERMIT.with(|p| p.set(p.get() + 1));
    RecordOnlyPermit(())
}
impl Drop for RecordOnlyPermit {
    fn drop(&mut self) {
        PERMIT.with(|p| p.set(p.get().saturating_sub(1)));
    }
}

/// In front of every call to the local model.
pub fn check_generation() -> Result<(), String> {
    gate(chambers_active())
}

fn gate(chambers: bool) -> Result<(), String> {
    if chambers && PERMIT.with(|p| p.get()) == 0 {
        return Err("CF_EDITION_G7_FREE_GENERATION: in the Chambers Edition the model is reachable only through record-only reasoning, whose every statement is tied to the record or to installed law".into());
    }
    Ok(())
}

pub fn require_chambers(ctx: &EditionContext) -> Result<(), String> {
    if ctx.chambers {
        Ok(())
    } else {
        Err("CF_EDITION_NOT_ENTITLED: this belongs to the Chambers Edition, which needs a Judiciary licence".into())
    }
}

pub fn require_advocacy(ctx: &EditionContext) -> Result<(), String> {
    if ctx.chambers {
        Err("CF_EDITION_NOT_ENTITLED: this advocacy feature is not part of the Chambers Edition".into())
    } else {
        Ok(())
    }
}

/// The air-gap switch cannot be turned off under a JUDICIARY licence.
pub fn check_air_gap_change(ctx: &EditionContext, enable: bool) -> Result<(), String> {
    if !enable && ctx.air_gap_forced {
        Err("CF_EDITION_AIR_GAP_FORCED: this licence keeps the app offline; the air-gap switch cannot be turned off".into())
    } else {
        Ok(())
    }
}

// --- pilot court configuration (Q-S4-10): data, not code ---

pub const PILOT_FILE: &str = "chambers_pilot.json";

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Pilot {
    pub court: Option<String>,
    pub disabled_capabilities: Vec<String>,
    /// Set when the file exists but cannot be read: every pilot capability is then off (fail closed).
    pub unreadable: bool,
}

pub fn pilot_config(root: &Path) -> Pilot {
    let path = root.join(PILOT_FILE);
    if !path.exists() {
        return Pilot::default();
    }
    let parsed = std::fs::read_to_string(&path).ok().and_then(|raw| serde_json::from_str::<Value>(&raw).ok());
    match parsed {
        Some(v) => Pilot {
            court: v.get("court").and_then(Value::as_str).map(str::to_string),
            disabled_capabilities: v.get("disabled_capabilities").and_then(Value::as_array).map(|a| a.iter().filter_map(|c| c.as_str().map(str::to_string)).collect()).unwrap_or_default(),
            unreadable: false,
        },
        None => Pilot { court: None, disabled_capabilities: PILOT_CAPABILITIES.iter().map(|s| s.to_string()).collect(), unreadable: true },
    }
}

/// A capability a pilot court has switched off is refused, in Chambers only.
pub fn require_capability(ctx: &EditionContext, root: &Path, capability: &str) -> Result<(), String> {
    if !ctx.chambers {
        return Ok(());
    }
    let p = pilot_config(root);
    if p.disabled_capabilities.iter().any(|c| c == capability) {
        return Err(format!("CF_EDITION_PILOT_DISABLED: {capability} is switched off by this court's configuration{}", if p.unreadable { " (the configuration file could not be read)" } else { "" }));
    }
    Ok(())
}

pub fn status_json(ctx: &EditionContext, root: &Path) -> Value {
    let mut v = serde_json::to_value(ctx).unwrap_or(Value::Null);
    v["pilot"] = serde_json::to_value(pilot_config(root)).unwrap_or(Value::Null);
    v["banner"] = json!(if ctx.chambers { "Chambers Edition \u{2014} record only. Everything shown comes from the filings, the evidence and orders on the record, and installed law. This tool never writes analysis, findings or orders." } else { "" });
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn keypair() -> (SigningKey, BTreeMap<String, String>) {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let mut keys = BTreeMap::new();
        keys.insert("test-key".to_string(), hex::encode(sk.verifying_key().to_bytes()));
        (sk, keys)
    }

    fn fp() -> FingerprintComponents {
        FingerprintComponents { hostname: "court-host-1".into(), username: "registrar-1".into(), machine_guid: "guid-12345678".into(), disk_root: "C:\\root-disk".into() }
    }

    fn payload(sk: &SigningKey, edit: impl Fn(&mut Value)) -> Value {
        let mut v = json!({
            "schema_version": "2.0.0", "licence_id": "L-1", "product": "CaseForge", "edition": "JUDICIARY", "issued_to_kind": "INSTITUTION", "issued_at_unix_ms": 1,
            "expires_at_unix_ms": null, "pack_subscription_until_unix_ms": null,
            "fingerprint_components": {"hostname": "court-host-1", "username": "registrar-1", "machine_guid": "guid-12345678", "disk_root": "C:\\root-disk"},
            "features": ["digest", "audit", "chambers"], "air_gap_forced": true, "telemetry_allowed": false, "seat_transfers_per_year": null,
            "signer_key_id": "test-key", "signature_alg": "Ed25519", "notes": "Chambers pilot \u{2014} \u{0939}\u{093f}\u{0902}\u{0926}\u{0940} \u{1f600}",
        });
        edit(&mut v);
        let sig = sk.sign(&canonical_v2(&v).unwrap());
        v["signature_hex"] = json!(hex::encode(sig.to_bytes()));
        v
    }

    #[test]
    fn canonical_bytes_follow_the_python_rule() {
        // json.dumps({"b": 1, "a": "\u2014 x\n", "c": [true, null, {"z": 1, "y": "\\"}]}, sort_keys=True, separators=(",", ":"))
        let v = json!({"b": 1, "a": "\u{2014} x\n", "c": [true, null, {"z": 1, "y": "\\"}], "signature_hex": "x", "signature_alg": "Ed25519"});
        assert_eq!(String::from_utf8(canonical_v2(&v).unwrap()).unwrap(), r#"{"a":"\u2014 x\n","b":1,"c":[true,null,{"y":"\\","z":1}]}"#);
        // a character outside the basic plane is a surrogate pair, lower-case hex
        assert_eq!(String::from_utf8(canonical_v2(&json!({"n": "\u{1f600}"})).unwrap()).unwrap(), r#"{"n":"\ud83d\ude00"}"#);
    }

    #[test]
    fn a_signed_judiciary_licence_enables_chambers_and_hides_advocacy() {
        let (sk, keys) = keypair();
        let (l, state) = verify_v2(&payload(&sk, |_| {}), &keys, &fp(), 1000).unwrap();
        assert_eq!((l.edition.as_str(), state), ("JUDICIARY", LicenceState::Active));
        let ctx = context_for(&l, state);
        assert!(ctx.chambers && ctx.air_gap_forced && !ctx.telemetry_allowed && ctx.record_only_default);
        for s in ["chambers", "issues", "workspace", "digest", "library", "deadlines"] {
            assert!(ctx.visible_surfaces.contains(&s), "{s} visible");
        }
        for s in ADVOCACY_ONLY {
            assert!(ctx.hidden_surfaces.contains(&s), "{s} hidden in Chambers");
        }
        assert_eq!(ctx.visible_surfaces.len() + ctx.hidden_surfaces.len(), ALL_SURFACES.len());
    }

    #[test]
    fn every_other_edition_sees_no_chambers_surface() {
        let (sk, keys) = keypair();
        for e in ["ADVOCATE", "PRO", "CHAMBERS_FIRM"] {
            let p = payload(&sk, |v| {
                v["edition"] = json!(e);
                v["issued_to_kind"] = json!("FIRM");
                v["features"] = json!(["digest", "audit"]);
                v["air_gap_forced"] = json!(false);
                v["telemetry_allowed"] = json!(true);
            });
            let (l, st) = verify_v2(&p, &keys, &fp(), 1000).unwrap();
            let ctx = context_for(&l, st);
            assert!(!ctx.chambers, "{e}");
            assert!(ctx.hidden_surfaces.contains(&"chambers") && ctx.hidden_surfaces.contains(&"issues"), "{e}");
            for s in ADVOCACY_ONLY {
                assert!(ctx.visible_surfaces.contains(&s), "{e}: {s}");
            }
            assert!(require_chambers(&ctx).is_err() && require_advocacy(&ctx).is_ok());
        }
        let none = none_context(None);
        assert!(!none.chambers && none.hidden_surfaces.contains(&"chambers"));
    }

    #[test]
    fn a_licence_that_does_not_verify_grants_nothing() {
        let (sk, keys) = keypair();
        let good = payload(&sk, |_| {});
        // tampered after signing
        let mut t = good.clone();
        t["features"] = json!(["digest", "audit", "chambers", "voice"]);
        assert!(verify_v2(&t, &keys, &fp(), 1000).unwrap_err().starts_with("CF_LICENCE_INVALID_SIGNATURE"));
        // unknown signer
        let mut u = good.clone();
        u["signer_key_id"] = json!("nobody");
        assert!(verify_v2(&u, &keys, &fp(), 1000).unwrap_err().contains("unknown signer"));
        // another machine
        let mut other = fp();
        other.hostname = "x".into();
        other.username = "y".into();
        assert!(verify_v2(&good, &keys, &other, 1000).unwrap_err().starts_with("CF_LICENCE_FINGERPRINT_MISMATCH"));
        // one component may differ (3 of 4)
        let mut one = fp();
        one.hostname = "renamed-host".into();
        assert!(verify_v2(&good, &keys, &one, 1000).is_ok());
        // a v1 payload is not a v2 licence
        assert!(verify_v2(&json!({"schema_version": "1.0.0"}), &keys, &fp(), 1).is_err());
        assert!(!context_from_stored(Some(("ACTIVE", r#"{"schema_version":"1.0.0","tier":"pro"}"#))).chambers, "a v1 licence grants no Stage 4 surface");
        assert!(!context_from_stored(Some(("ACTIVE", "not json"))).chambers);
        assert!(!context_from_stored(None).chambers);
    }

    #[test]
    fn the_contract_invariants_of_the_judiciary_edition_are_enforced() {
        let (sk, keys) = keypair();
        for edit in [
            (|v: &mut Value| v["air_gap_forced"] = json!(false)) as fn(&mut Value),
            |v| v["telemetry_allowed"] = json!(true),
            |v| v["issued_to_kind"] = json!("INDIVIDUAL"),
        ] {
            assert!(verify_v2(&payload(&sk, edit), &keys, &fp(), 1).unwrap_err().contains("JUDICIARY"), "a signed licence that breaks the Judiciary invariants is refused");
        }
        let advocate_with_chambers = payload(&sk, |v| {
            v["edition"] = json!("ADVOCATE");
            v["issued_to_kind"] = json!("INDIVIDUAL");
            v["air_gap_forced"] = json!(false);
            v["telemetry_allowed"] = json!(true);
        });
        assert!(verify_v2(&advocate_with_chambers, &keys, &fp(), 1).unwrap_err().contains("only a JUDICIARY licence"));
        assert!(verify_v2(&payload(&sk, |v| v["features"] = json!(["chambers", "chambers"])), &keys, &fp(), 1).is_err(), "unique features");
        assert!(verify_v2(&payload(&sk, |v| v["features"] = json!(["time_travel"])), &keys, &fp(), 1).is_err(), "known features only");
    }

    #[test]
    fn an_expired_judiciary_licence_keeps_its_edition_and_is_read_only() {
        let (sk, keys) = keypair();
        let p = payload(&sk, |v| v["expires_at_unix_ms"] = json!(500));
        let (l, state) = verify_v2(&p, &keys, &fp(), 1000).unwrap();
        assert_eq!(state, LicenceState::Expired);
        assert!(!state.vault_writable());
        assert!(context_for(&l, state).chambers, "work already on the record stays readable and exportable");
    }

    #[test]
    fn free_generation_is_refused_in_chambers_and_only_the_record_only_path_may_reach_the_model() {
        // (the process-wide flag is flipped by vault tests too, so the rule is tested without it)
        assert!(gate(false).is_ok());
        let e = gate(true).unwrap_err();
        assert!(e.starts_with("CF_EDITION_G7_FREE_GENERATION"), "{e}");
        {
            let _permit = record_only_permit();
            assert!(gate(true).is_ok());
            // a permit is per thread: another thread has none
            assert!(std::thread::spawn(|| gate(true).is_err()).join().unwrap());
        }
        assert!(gate(true).is_err(), "the permit ends with the reasoning call");
    }

    #[test]
    fn the_air_gap_cannot_be_turned_off_under_a_judiciary_licence() {
        let (sk, keys) = keypair();
        let (l, st) = verify_v2(&payload(&sk, |_| {}), &keys, &fp(), 1).unwrap();
        let ctx = context_for(&l, st);
        assert!(check_air_gap_change(&ctx, false).unwrap_err().starts_with("CF_EDITION_AIR_GAP_FORCED"));
        assert!(check_air_gap_change(&ctx, true).is_ok());
        assert!(check_air_gap_change(&none_context(None), false).is_ok());
    }

    #[test]
    fn pilot_constraints_are_configuration_and_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let (sk, keys) = keypair();
        let (l, st) = verify_v2(&payload(&sk, |_| {}), &keys, &fp(), 1).unwrap();
        let ctx = context_for(&l, st);
        for c in PILOT_CAPABILITIES {
            assert!(require_capability(&ctx, dir.path(), c).is_ok(), "no file: nothing is switched off");
        }
        std::fs::write(dir.path().join(PILOT_FILE), r#"{"court":"Pilot Court","disabled_capabilities":["memo_export"]}"#).unwrap();
        assert!(require_capability(&ctx, dir.path(), "memo_export").unwrap_err().starts_with("CF_EDITION_PILOT_DISABLED"));
        assert!(require_capability(&ctx, dir.path(), "reasoning").is_ok());
        assert!(require_capability(&none_context(None), dir.path(), "memo_export").is_ok(), "the file applies to the Chambers Edition only");
        std::fs::write(dir.path().join(PILOT_FILE), "{ not json").unwrap();
        for c in PILOT_CAPABILITIES {
            assert!(require_capability(&ctx, dir.path(), c).is_err(), "an unreadable policy file switches everything off: {c}");
        }
    }
}
