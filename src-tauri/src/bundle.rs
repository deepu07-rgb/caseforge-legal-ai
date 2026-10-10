//! Encrypted Bundle Exchange (B-S4-16, PRD v4.0 section 8): `.cfbundle`, a case-work bundle one lawyer can hand to another on a USB
//! stick, encrypted to the recipient's key, signed by the sender, with redactions that cannot be undone.
//!
//! This module is the format and the rules; it holds no vault, no clock and no network (the LAN transfer is bundle_lan.rs, the
//! commands are in lib.rs). No relay, no server, no cloud: keys are exchanged as files and checked by reading a code aloud.
//!
//! Cryptography (contracts/bundle_manifest.schema.json): X25519 key agreement, HKDF-SHA256, AES-256-GCM, Ed25519.
//!   identity     every install derives two keys from its existing per-install secret (the one that signs Audit Certificates), each by
//!                HKDF with its own label, so a bundle key and a certificate key are never the same key: a signing key and an X25519
//!                agreement key. The public halves make the identity card; `key_id` is a hash of both.
//!   encryption   a random content key per bundle encrypts each item with AES-256-GCM (item id and kind bound as associated data, so
//!                an item cannot be moved to another place or bundle). For each recipient the sender makes a fresh X25519 key, agrees
//!                a secret with the recipient's public key, derives a wrapping key by HKDF and wraps the content key. The schema's
//!                `recipients[].agreement_public_key_hex` carries that per-recipient ephemeral public key (the recipient needs it and
//!                the frozen schema has no other field for it); the recipient's own key is named by `key_id`.
//!   signature    Ed25519 over the manifest's canonical bytes (everything but `signature_hex`). The manifest holds the hashes and
//!                lengths of every item, the wrapped keys and the sender, so changing any byte of the file is detected.
//!   container    "CFBUNDLE1\n", u32 big-endian manifest length, the manifest JSON, then each item as nonce(12) || ciphertext || tag(16),
//!                in manifest order, with nothing after.
//! Redaction happens on the plaintext BEFORE encryption, in one pass, by wording (a term matches with optional whitespace between any
//! two of its characters, longest first); only counts are kept. An independent check then looks for each term, in several shapes,
//! in what is about to be encrypted, and the export is refused if any survives.
//!
//! What a bundle does not hide: the manifest is readable without a key (sender name, item kinds and sizes, recipient key ids, redaction
//! counts). A receiver can tell who sent it and when, which is the point; an onlooker learns that much and no more.

use crate::edition::canonical_v2;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine as _;
use curve25519_dalek::constants::X25519_BASEPOINT;
use curve25519_dalek::montgomery::MontgomeryPoint;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::RngCore;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const MAGIC: &[u8; 10] = b"CFBUNDLE1\n";
pub const REDACTED: &str = "[REDACTED]";
pub const KINDS: [&str; 8] = ["DIGEST", "DOCUMENT", "GRAPH_SUBSET", "ARGUMENT_TREE", "IMPEACHMENT_KIT", "AUDIT_REPORT", "NOTES", "COUNSEL_CORRECTIONS"];
const OVERHEAD: usize = 12 + 16;
const MAX_MANIFEST: usize = 4 * 1024 * 1024;
pub const CARD_SCHEMA: &str = "caseforge.bundle-identity.v1";

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

fn sha256(b: &[u8]) -> [u8; 32] {
    Sha256::digest(b).into()
}

// ---------- HMAC-SHA256 and HKDF (RFC 2104, RFC 5869) ----------

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&sha256(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(k.map(|b| b ^ 0x36));
    inner.update(data);
    let ih: [u8; 32] = inner.finalize().into();
    let mut outer = Sha256::new();
    outer.update(k.map(|b| b ^ 0x5c));
    outer.update(ih);
    outer.finalize().into()
}

pub fn hkdf_sha256(ikm: &[u8], salt: &[u8], info: &[u8], len: usize) -> Vec<u8> {
    assert!(len <= 255 * 32, "HKDF output too long");
    let salt = if salt.is_empty() { &[0u8; 32][..] } else { salt };
    let prk = hmac_sha256(salt, ikm);
    let mut okm = Vec::with_capacity(len);
    let mut t: Vec<u8> = vec![];
    let mut i = 1u8;
    while okm.len() < len {
        let mut m = t.clone();
        m.extend_from_slice(info);
        m.push(i);
        t = hmac_sha256(&prk, &m).to_vec();
        okm.extend_from_slice(&t);
        i += 1;
    }
    okm.truncate(len);
    okm
}

// ---------- X25519 (RFC 7748) ----------

pub fn x25519(secret: &[u8; 32], point: &[u8; 32]) -> [u8; 32] {
    MontgomeryPoint(*point).mul_clamped(*secret).to_bytes()
}

pub fn x25519_public(secret: &[u8; 32]) -> [u8; 32] {
    X25519_BASEPOINT.mul_clamped(*secret).to_bytes()
}

fn shared_secret(secret: &[u8; 32], their_public: &[u8; 32]) -> Result<[u8; 32], String> {
    let s = x25519(secret, their_public);
    // an all-zero result means a low-order point: no secret was agreed
    if s.iter().all(|b| *b == 0) {
        return Err(format!("{}: the key is not a valid recipient key", E_TAMPERED));
    }
    Ok(s)
}

// ---------- errors ----------

pub const E_TAMPERED: &str = "CF_BUNDLE_TAMPERED";
pub const E_NOT_FOR_YOU: &str = "CF_BUNDLE_NOT_FOR_YOU";
pub const E_BAD_FILE: &str = "CF_BUNDLE_BAD_FILE";
pub const E_BAD_REDACTION: &str = "CF_BUNDLE_BAD_REDACTION";
pub const E_REDACTION_INCOMPLETE: &str = "CF_BUNDLE_REDACTION_INCOMPLETE";

// ---------- identity ----------

pub struct Identity {
    signing: SigningKey,
    agree_secret: [u8; 32],
    pub signing_public_key_hex: String,
    pub agreement_public_key_hex: String,
    pub key_id: String,
}

pub fn key_id_of(signing_pub: &[u8; 32], agree_pub: &[u8; 32]) -> String {
    let mut h = Sha256::new();
    h.update(signing_pub);
    h.update(agree_pub);
    hex::encode(&h.finalize()[..8])
}

/// Six groups of five digits derived from both public keys; two people read it out to each other to check a card.
pub fn verification_code(signing_pub_hex: &str, agree_pub_hex: &str) -> String {
    let mut h = Sha256::new();
    h.update(signing_pub_hex.as_bytes());
    h.update(b"|");
    h.update(agree_pub_hex.as_bytes());
    let d = h.finalize();
    (0..6).map(|i| format!("{:05}", u32::from_be_bytes([d[4 * i], d[4 * i + 1], d[4 * i + 2], d[4 * i + 3]]) % 100_000)).collect::<Vec<_>>().join(" ")
}

impl Identity {
    pub fn from_seed(seed: &[u8; 32]) -> Identity {
        let sign_seed: [u8; 32] = hkdf_sha256(seed, b"caseforge.bundle.v1", b"signing", 32).try_into().unwrap();
        let agree_secret: [u8; 32] = hkdf_sha256(seed, b"caseforge.bundle.v1", b"agreement", 32).try_into().unwrap();
        let signing = SigningKey::from_bytes(&sign_seed);
        let sp = signing.verifying_key().to_bytes();
        let ap = x25519_public(&agree_secret);
        Identity { signing, agree_secret, signing_public_key_hex: hex::encode(sp), agreement_public_key_hex: hex::encode(ap), key_id: key_id_of(&sp, &ap) }
    }

    pub fn verification_code(&self) -> String {
        verification_code(&self.signing_public_key_hex, &self.agreement_public_key_hex)
    }

    /// The public identity card to hand to people who will send you bundles: signed by its own signing key.
    pub fn card(&self, display_name: &str, created_at_unix_ms: i64) -> Value {
        let mut v = json!({
            "schema": CARD_SCHEMA,
            "display_name": display_name,
            "key_id": self.key_id,
            "signing_public_key_hex": self.signing_public_key_hex,
            "agreement_public_key_hex": self.agreement_public_key_hex,
            "created_at_unix_ms": created_at_unix_ms,
        });
        let sig = self.signing.sign(&canonical_v2(&v).unwrap_or_default());
        v["signature_hex"] = json!(hex::encode(sig.to_bytes()));
        v
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerCard {
    pub display_name: String,
    pub key_id: String,
    pub signing_public_key_hex: String,
    pub agreement_public_key_hex: String,
    pub created_at_unix_ms: i64,
}

fn hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    hex::decode(s).ok()?.try_into().ok()
}

/// Checks a card's own signature and that its key id belongs to its keys. (That the card belongs to the person is for the two of them
/// to check by reading the verification code.)
pub fn read_card(raw: &Value) -> Result<PeerCard, String> {
    let bad = |m: &str| Err(format!("CF_BUNDLE_BAD_CARD: {m}"));
    if raw["schema"] != CARD_SCHEMA {
        return bad("not an identity card");
    }
    let (Some(name), Some(kid), Some(sp), Some(ap), Some(at), Some(sig)) = (raw["display_name"].as_str(), raw["key_id"].as_str(), raw["signing_public_key_hex"].as_str(), raw["agreement_public_key_hex"].as_str(), raw["created_at_unix_ms"].as_i64(), raw["signature_hex"].as_str()) else {
        return bad("a field is missing");
    };
    let (Some(spb), Some(apb)) = (hex32(sp), hex32(ap)) else { return bad("a key is malformed") };
    if name.trim().is_empty() || name.len() > 120 {
        return bad("the name is empty or too long");
    }
    if kid != key_id_of(&spb, &apb) {
        return bad("the key id does not belong to these keys");
    }
    let sigb: [u8; 64] = if sig.len() == 128 && sig.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) { hex::decode(sig).ok().and_then(|b| b.try_into().ok()).ok_or("CF_BUNDLE_BAD_CARD: the signature is malformed")? } else { return bad("the signature is malformed") };
    let vk = VerifyingKey::from_bytes(&spb).map_err(|_| "CF_BUNDLE_BAD_CARD: bad signing key".to_string())?;
    vk.verify(&canonical_v2(raw)?, &Signature::from_bytes(&sigb)).map_err(|_| "CF_BUNDLE_BAD_CARD: the card's signature does not verify".to_string())?;
    Ok(PeerCard { display_name: name.to_string(), key_id: kid.to_string(), signing_public_key_hex: sp.to_string(), agreement_public_key_hex: ap.to_string(), created_at_unix_ms: at })
}

// ---------- redaction ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Redaction {
    pub text: String,
    /// EXACT or CASE_INSENSITIVE
    pub mode: String,
}

struct Term {
    chars: String, // the term without any whitespace, case-folded if case-insensitive
    ci: bool,
}

pub struct Redactor {
    re: Option<Regex>,
    terms: Vec<Term>,
}

fn no_ws(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

impl Redactor {
    pub fn compile(reds: &[Redaction]) -> Result<Redactor, String> {
        let mut terms: Vec<Term> = vec![];
        for r in reds {
            let ci = match r.mode.as_str() {
                "EXACT" => false,
                "CASE_INSENSITIVE" => true,
                other => return Err(format!("{E_BAD_REDACTION}: mode {other:?} is not EXACT or CASE_INSENSITIVE")),
            };
            let bare = no_ws(&r.text);
            if bare.is_empty() {
                return Err(format!("{E_BAD_REDACTION}: a redaction term is empty"));
            }
            terms.push(Term { chars: if ci { bare.to_lowercase() } else { bare }, ci });
        }
        let mut order: Vec<usize> = (0..terms.len()).collect();
        order.sort_by_key(|i| std::cmp::Reverse(terms[*i].chars.chars().count()));
        let mut pats: Vec<String> = vec![];
        for i in order {
            let t = &terms[i];
            // a term matches with optional whitespace between any two of its characters
            let body = t.chars.chars().map(|c| regex::escape(&c.to_string())).collect::<Vec<_>>().join(r"\s*");
            let p = if t.ci { format!("(?i:{body})") } else { format!("(?:{body})") };
            if !pats.contains(&p) {
                pats.push(p);
            }
        }
        let re = if pats.is_empty() { None } else { Some(Regex::new(&pats.join("|")).map_err(|e| format!("{E_BAD_REDACTION}: {e}"))?) };
        Ok(Redactor { re, terms })
    }

    pub fn apply_str(&self, s: &str) -> (String, usize) {
        match &self.re {
            None => (s.to_string(), 0),
            Some(re) => {
                let n = re.find_iter(s).count();
                if n == 0 {
                    (s.to_string(), 0)
                } else {
                    (re.replace_all(s, regex::NoExpand(REDACTED)).into_owned(), n)
                }
            }
        }
    }

    /// Redacts every string value (not the keys) of a JSON value.
    pub fn apply_value(&self, v: &Value) -> (Value, usize) {
        match v {
            Value::String(s) => {
                let (o, n) = self.apply_str(s);
                (Value::String(o), n)
            }
            Value::Array(a) => {
                let mut n = 0;
                let out = a.iter().map(|x| {
                    let (o, k) = self.apply_value(x);
                    n += k;
                    o
                }).collect();
                (Value::Array(out), n)
            }
            Value::Object(m) => {
                let mut n = 0;
                let mut out = Map::new();
                for (k, x) in m {
                    let (o, c) = self.apply_value(x);
                    n += c;
                    out.insert(k.clone(), o);
                }
                (Value::Object(out), n)
            }
            other => (other.clone(), 0),
        }
    }

    /// An independent look (by normalising, not by pattern) for any term in any string or key of the value, and in its serialised bytes.
    pub fn leaks(&self, v: &Value) -> Option<String> {
        let hit = |s: &str| {
            let plain = no_ws(s);
            let low = plain.to_lowercase();
            self.terms.iter().any(|t| if t.ci { low.contains(&t.chars) } else { plain.contains(&t.chars) })
        };
        fn walk(v: &Value, hit: &dyn Fn(&str) -> bool) -> bool {
            match v {
                Value::String(s) => hit(s),
                Value::Array(a) => a.iter().any(|x| walk(x, hit)),
                Value::Object(m) => m.iter().any(|(k, x)| hit(k) || walk(x, hit)),
                _ => false,
            }
        }
        if walk(v, &hit) {
            return Some("a redacted term is still in the text".into());
        }
        let bytes = serde_json::to_string(v).unwrap_or_default();
        // the serialised form: JSON escapes (\n, \", \\) removed so a term split by a line break is still found
        let flat: String = bytes.replace("\\n", "").replace("\\r", "").replace("\\t", "").replace("\\\"", "\"").replace("\\\\", "\\");
        if hit(&flat) {
            return Some("a redacted term is still in the serialised item".into());
        }
        None
    }
}

// ---------- building a bundle ----------

#[derive(Debug, Clone)]
pub struct PlainItem {
    pub kind: String,
    pub payload: Value,
}

#[derive(Debug)]
pub struct Built {
    pub bytes: Vec<u8>,
    pub bundle_id: String,
    pub items: Vec<(String, String, usize, bool)>,
    pub redaction_log: Vec<(String, usize)>,
}

fn rfc3339(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (h, m, s) = ((secs.rem_euclid(86_400)) / 3600, (secs.rem_euclid(3600)) / 60, secs.rem_euclid(60));
    format!("{}T{:02}:{:02}:{:02}Z", crate::subscription::date_of(ms), h, m, s)
}

fn aad(bundle_id: &str, item_id: &str, kind: &str) -> Vec<u8> {
    format!("caseforge.bundle.item.v1\0{bundle_id}\0{item_id}\0{kind}").into_bytes()
}

fn gcm_seal(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], plain: &[u8]) -> Result<Vec<u8>, String> {
    Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?.encrypt(Nonce::from_slice(nonce), Payload { msg: plain, aad }).map_err(|_| "CF_BUNDLE_CRYPTO: encryption failed".to_string())
}

fn gcm_open(key: &[u8; 32], nonce: &[u8], aad: &[u8], ct: &[u8]) -> Result<Vec<u8>, ()> {
    Aes256Gcm::new_from_slice(key).map_err(|_| ())?.decrypt(Nonce::from_slice(nonce), Payload { msg: ct, aad }).map_err(|_| ())
}

fn random<const N: usize>(rng: &mut impl RngCore) -> [u8; N] {
    let mut b = [0u8; N];
    rng.fill_bytes(&mut b);
    b
}

fn wrap_key(shared: &[u8; 32], bundle_id: &str, recipient_key_id: &str) -> [u8; 32] {
    let mut info = b"caseforge.bundle.wrap.v1\0".to_vec();
    info.extend_from_slice(recipient_key_id.as_bytes());
    hkdf_sha256(shared, bundle_id.as_bytes(), &info, 32).try_into().unwrap()
}

/// Builds a `.cfbundle`. Items are redacted, checked, encrypted, listed in a signed manifest, and laid out in the container.
pub fn build(sender: &Identity, sender_name: &str, recipients: &[PeerCard], source_case_id: &str, items: Vec<PlainItem>, redactions: &[Redaction], now_ms: i64, app_version: &str, rng: &mut impl RngCore) -> Result<Built, String> {
    if recipients.is_empty() {
        return Err("CF_BUNDLE_NO_RECIPIENT: choose at least one recipient".into());
    }
    if items.is_empty() {
        return Err("CF_BUNDLE_EMPTY: choose something to send".into());
    }
    let redactor = Redactor::compile(redactions)?;
    let bundle_id = format!("cfb-{}", hex::encode(random::<12>(rng)));
    let content_key: [u8; 32] = random(rng);
    let mut blobs: Vec<Vec<u8>> = vec![];
    let mut contents = vec![];
    let mut log = vec![];
    let mut summary = vec![];
    for (i, it) in items.iter().enumerate() {
        if !KINDS.contains(&it.kind.as_str()) {
            return Err(format!("CF_BUNDLE_BAD_KIND: {:?} is not something a bundle can carry", it.kind));
        }
        let item_id = format!("i{}", i + 1);
        let (payload, n) = redactor.apply_value(&it.payload);
        let envelope = json!({
            "kind": it.kind,
            "item_id": item_id,
            "produced_at_unix_ms": now_ms,
            "produced_by": {"key_id": sender.key_id, "display_name": sender_name},
            "payload": payload,
        });
        if let Some(why) = redactor.leaks(&envelope) {
            return Err(format!("{E_REDACTION_INCOMPLETE}: nothing was written - {why} (item {item_id}, {}); add the missing wording to the redactions", it.kind));
        }
        let plain = serde_json::to_vec(&envelope).map_err(|e| e.to_string())?;
        let nonce: [u8; 12] = random(rng);
        let ct = gcm_seal(&content_key, &nonce, &aad(&bundle_id, &item_id, &it.kind), &plain)?;
        let mut blob = nonce.to_vec();
        blob.extend_from_slice(&ct);
        debug_assert_eq!(blob.len(), plain.len() + OVERHEAD);
        contents.push(json!({"item_id": item_id, "item_kind": it.kind, "path": format!("items/{item_id}.bin"), "ciphertext_sha256": hex::encode(sha256(&blob)), "plaintext_bytes": plain.len(), "redacted": n > 0}));
        if n > 0 {
            log.push((item_id.clone(), n));
        }
        summary.push((item_id, it.kind.clone(), plain.len(), n > 0));
        blobs.push(blob);
    }
    let mut recips = vec![];
    for r in recipients {
        let rp = hex32(&r.agreement_public_key_hex).ok_or("CF_BUNDLE_BAD_CARD: a recipient key is malformed")?;
        let eph: [u8; 32] = random(rng);
        let shared = shared_secret(&eph, &rp)?;
        let wk = wrap_key(&shared, &bundle_id, &r.key_id);
        let nonce: [u8; 12] = random(rng);
        let mut a = bundle_id.clone().into_bytes();
        a.extend_from_slice(r.key_id.as_bytes());
        let ct = gcm_seal(&wk, &nonce, &a, &content_key)?;
        let mut w = nonce.to_vec();
        w.extend_from_slice(&ct);
        recips.push(json!({"key_id": r.key_id, "agreement_public_key_hex": hex::encode(x25519_public(&eph)), "wrapped_content_key_b64": b64().encode(w)}));
    }
    let mut manifest = json!({
        "schema_version": "1.0.0",
        "bundle_id": bundle_id,
        "created_at": rfc3339(now_ms),
        "app_version": app_version,
        "source_case_id": source_case_id,
        "sender": {"key_id": sender.key_id, "signing_public_key_hex": sender.signing_public_key_hex, "display_name": sender_name},
        "recipients": recips,
        "crypto": {"key_agreement": "X25519", "kdf": "HKDF-SHA256", "content_cipher": "AES-256-GCM", "signature": "Ed25519"},
        "contents": contents,
        "redaction_log": log.iter().map(|(id, n)| json!({"item_id": id, "redaction_count": n})).collect::<Vec<_>>(),
    });
    let sig = sender.signing.sign(&canonical_v2(&manifest)?);
    manifest["signature_hex"] = json!(hex::encode(sig.to_bytes()));
    let mjson = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&(mjson.len() as u32).to_be_bytes());
    out.extend_from_slice(&mjson);
    for b in &blobs {
        out.extend_from_slice(b);
    }
    Ok(Built { bytes: out, bundle_id, items: summary, redaction_log: log })
}

// ---------- reading a bundle ----------

pub struct Parsed<'a> {
    pub manifest: Value,
    pub blobs: Vec<&'a [u8]>,
}

fn lower_hex(s: &Value, len: usize) -> bool {
    s.as_str().map(|s| s.len() == len && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))).unwrap_or(false)
}

/// Reads the container and checks it against its manifest: structure, signature, lengths and hashes. Needs no key.
pub fn parse_and_verify(bytes: &[u8]) -> Result<Parsed<'_>, String> {
    let bad = |m: &str| Err(format!("{E_BAD_FILE}: {m}"));
    let tamper = |m: &str| Err(format!("{E_TAMPERED}: {m}"));
    if bytes.len() < MAGIC.len() + 4 || &bytes[..MAGIC.len()] != MAGIC {
        return bad("this is not a CaseForge bundle");
    }
    let mlen = u32::from_be_bytes(bytes[MAGIC.len()..MAGIC.len() + 4].try_into().unwrap()) as usize;
    let body = MAGIC.len() + 4;
    if mlen == 0 || mlen > MAX_MANIFEST || bytes.len() < body + mlen {
        return bad("the file is cut short or its header is damaged");
    }
    let manifest: Value = serde_json::from_slice(&bytes[body..body + mlen]).map_err(|_| format!("{E_BAD_FILE}: the manifest cannot be read"))?;
    let m = &manifest;
    // shape and constants of contracts/bundle_manifest.schema.json
    let crypto_ok = m["crypto"] == json!({"key_agreement": "X25519", "kdf": "HKDF-SHA256", "content_cipher": "AES-256-GCM", "signature": "Ed25519"});
    if m["schema_version"] != "1.0.0" || !crypto_ok {
        return tamper("the manifest is not a version 1 bundle manifest");
    }
    let (Some(contents), Some(recips)) = (m["contents"].as_array(), m["recipients"].as_array()) else { return tamper("the manifest is incomplete") };
    if contents.is_empty() || recips.is_empty() || !m["bundle_id"].is_string() || !m["sender"].is_object() || !lower_hex(&m["signature_hex"], 128) || !lower_hex(&m["sender"]["signing_public_key_hex"], 64) {
        return tamper("the manifest is incomplete");
    }
    for r in recips {
        if !lower_hex(&r["agreement_public_key_hex"], 64) || !r["key_id"].is_string() || !r["wrapped_content_key_b64"].is_string() {
            return tamper("a recipient entry is malformed");
        }
    }
    // signature
    let spk = hex32(m["sender"]["signing_public_key_hex"].as_str().unwrap()).unwrap();
    let vk = VerifyingKey::from_bytes(&spk).map_err(|_| format!("{E_TAMPERED}: the sender's key is invalid"))?;
    let sig: [u8; 64] = hex::decode(m["signature_hex"].as_str().unwrap()).ok().and_then(|b| b.try_into().ok()).ok_or_else(|| format!("{E_TAMPERED}: the signature is malformed"))?;
    vk.verify(&canonical_v2(m)?, &Signature::from_bytes(&sig)).map_err(|_| format!("{E_TAMPERED}: the sender's signature does not verify; the file was changed or is not what the sender made"))?;
    // items
    let mut blobs = vec![];
    let mut at = body + mlen;
    for c in contents {
        let (Some(pt), true) = (c["plaintext_bytes"].as_u64(), lower_hex(&c["ciphertext_sha256"], 64)) else { return tamper("an item entry is malformed") };
        let want = pt as usize + OVERHEAD;
        if bytes.len() < at + want {
            return bad("the file is cut short");
        }
        let blob = &bytes[at..at + want];
        if hex::encode(sha256(blob)) != c["ciphertext_sha256"].as_str().unwrap() {
            return tamper("an item does not match the hash the sender signed");
        }
        blobs.push(blob);
        at += want;
    }
    if at != bytes.len() {
        return bad("there is data after the last item");
    }
    Ok(Parsed { manifest, blobs })
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenedItem {
    pub item_id: String,
    pub kind: String,
    pub envelope: Value,
}

pub struct Opened {
    pub manifest: Value,
    pub items: Vec<OpenedItem>,
}

/// Decrypts a bundle addressed to `me`. Fails without the recipient's key (E_NOT_FOR_YOU) and on any change to the file (E_TAMPERED).
pub fn open(bytes: &[u8], me: &Identity) -> Result<Opened, String> {
    let p = parse_and_verify(bytes)?;
    let m = &p.manifest;
    let bundle_id = m["bundle_id"].as_str().unwrap();
    let entry = m["recipients"].as_array().unwrap().iter().find(|r| r["key_id"] == me.key_id.as_str()).ok_or_else(|| format!("{E_NOT_FOR_YOU}: this bundle was not encrypted to your key"))?;
    let not_you = || format!("{E_NOT_FOR_YOU}: this bundle was not encrypted to your key");
    let eph = hex32(entry["agreement_public_key_hex"].as_str().unwrap_or("")).ok_or_else(not_you)?;
    let shared = shared_secret(&me.agree_secret, &eph).map_err(|_| not_you())?;
    let wk = wrap_key(&shared, bundle_id, &me.key_id);
    let w = b64().decode(entry["wrapped_content_key_b64"].as_str().unwrap_or("")).map_err(|_| not_you())?;
    if w.len() != 12 + 32 + 16 {
        return Err(not_you());
    }
    let mut a = bundle_id.as_bytes().to_vec();
    a.extend_from_slice(me.key_id.as_bytes());
    let ck: [u8; 32] = gcm_open(&wk, &w[..12], &a, &w[12..]).map_err(|_| not_you())?.try_into().map_err(|_| not_you())?;
    let mut items = vec![];
    for (c, blob) in m["contents"].as_array().unwrap().iter().zip(&p.blobs) {
        let (item_id, kind) = (c["item_id"].as_str().unwrap_or(""), c["item_kind"].as_str().unwrap_or(""));
        let plain = gcm_open(&ck, &blob[..12], &aad(bundle_id, item_id, kind), &blob[12..]).map_err(|_| format!("{E_TAMPERED}: an item cannot be decrypted; it was changed"))?;
        let envelope: Value = serde_json::from_slice(&plain).map_err(|_| format!("{E_TAMPERED}: an item is not readable"))?;
        if envelope["item_id"] != item_id || envelope["kind"] != kind {
            return Err(format!("{E_TAMPERED}: an item does not say what the manifest says it is"));
        }
        items.push(OpenedItem { item_id: item_id.to_string(), kind: kind.to_string(), envelope });
    }
    Ok(Opened { manifest: m.clone(), items })
}

// ---------- what the recipient is shown ----------

/// "from <sender>, <date>": shown with every item received.
pub fn attribution_label(display_name: &str, created_at_unix_ms: i64) -> String {
    format!("from {display_name}, {}", crate::subscription::date_of(created_at_unix_ms))
}

/// Classifies the corrections in a bundle against the recipient's own. Nothing here writes anything: a received correction is shown with
/// its sender and never replaces a correction the recipient made. `recipient` maps a target key to the value the recipient's active
/// correction gives it.
pub fn plan_corrections(recipient: &HashMap<String, String>, incoming: &[Value]) -> Vec<Value> {
    incoming
        .iter()
        .filter_map(|c| {
            let key = c["target_key"].as_str()?;
            let theirs = c["counsel_value"].as_str();
            let mine = recipient.get(key);
            let status = match mine {
                None => "SUGGESTED",
                Some(m) if c["event_type"] == "CORRECT" && theirs == Some(m.as_str()) => "SAME_AS_YOURS",
                Some(_) => "CONFLICTS_WITH_YOUR_CORRECTION",
            };
            Some(json!({
                "target_key": key,
                "status": status,
                "recipient_value": mine,
                "sender_value": theirs,
                "event_type": c["event_type"],
                "reason": c["reason"],
                "base_value": c["base_value"],
                "origin": "COUNSEL_INPUT",
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    const FIXTURE: &str = include_str!("../../golden/G-BUNDLE/fixtures/bundle_core.json");
    const FIXTURE_SHA256: &str = "59b8cb0a66e24f11b3b477b8fc4027c61252cdaa531304cab25be542e73ca90c"; // committed before the engine (52749d5)

    fn fx() -> Value {
        assert_eq!(hex::encode(Sha256::digest(FIXTURE.as_bytes())), FIXTURE_SHA256, "the bundle golden changed since it was pinned");
        serde_json::from_str(FIXTURE).unwrap()
    }

    fn id(n: u8) -> Identity {
        Identity::from_seed(&[n; 32])
    }
    fn peer(i: &Identity, name: &str) -> PeerCard {
        read_card(&i.card(name, 1_791_590_400_000)).unwrap()
    }
    fn reds(f: &Value) -> Vec<Redaction> {
        serde_json::from_value(f["redactions"].clone()).unwrap()
    }
    fn all_items(f: &Value) -> Vec<PlainItem> {
        let i = &f["items"];
        let v = vec![
            PlainItem { kind: "DOCUMENT".into(), payload: i["document"].clone() },
            PlainItem { kind: "GRAPH_SUBSET".into(), payload: i["graph"].clone() },
            PlainItem { kind: "ARGUMENT_TREE".into(), payload: i["tree"].clone() },
            PlainItem { kind: "COUNSEL_CORRECTIONS".into(), payload: i["corrections"].clone() },
            PlainItem { kind: "NOTES".into(), payload: f["notes_item"].clone() },
        ];
        v
    }
    fn build_core(f: &Value, to: &[&Identity]) -> Built {
        let sender = id(1);
        let recips: Vec<PeerCard> = to.iter().enumerate().map(|(i, r)| peer(r, &format!("Recipient {i}"))).collect();
        build(&sender, "Priya Nair", &recips, "case-1", all_items(f), &reds(f), f["attribution"]["created_at_unix_ms"].as_i64().unwrap(), "test", &mut OsRng).unwrap()
    }

    #[test]
    fn primitives_match_their_published_test_vectors() {
        // RFC 4231 test case 1
        assert_eq!(hex::encode(hmac_sha256(&[0x0b; 20], b"Hi There")), "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7");
        // RFC 5869 test case 1
        let okm = hkdf_sha256(&[0x0b; 22], &hex::decode("000102030405060708090a0b0c").unwrap(), &hex::decode("f0f1f2f3f4f5f6f7f8f9").unwrap(), 42);
        assert_eq!(hex::encode(okm), "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865");
        // RFC 7748 section 5.2, first vector
        let sk: [u8; 32] = hex::decode("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4").unwrap().try_into().unwrap();
        let pt: [u8; 32] = hex::decode("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c").unwrap().try_into().unwrap();
        assert_eq!(hex::encode(x25519(&sk, &pt)), "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552");
        // RFC 7748 section 6.1: Alice and Bob agree
        let a: [u8; 32] = hex::decode("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a").unwrap().try_into().unwrap();
        let b: [u8; 32] = hex::decode("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb").unwrap().try_into().unwrap();
        assert_eq!(hex::encode(x25519_public(&a)), "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a");
        assert_eq!(hex::encode(x25519_public(&b)), "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f");
        assert_eq!(hex::encode(x25519(&a, &x25519_public(&b))), "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");
        assert_eq!(x25519(&a, &x25519_public(&b)), x25519(&b, &x25519_public(&a)));
        // a low-order point agrees no secret
        assert!(shared_secret(&a, &[0u8; 32]).is_err());
    }

    #[test]
    fn an_identity_has_separate_keys_a_stable_id_and_a_card_that_checks() {
        let i = id(7);
        assert_eq!(i.key_id, id(7).key_id, "derived from the seed, so the same every time");
        assert_ne!(i.key_id, id(8).key_id);
        assert_ne!(i.signing_public_key_hex, i.agreement_public_key_hex);
        let sk: [u8; 32] = [7; 32];
        assert_ne!(i.signing_public_key_hex, hex::encode(SigningKey::from_bytes(&sk).verifying_key().to_bytes()), "not the certificate-signing key");
        let code = i.verification_code();
        assert!(regex::Regex::new(r"^\d{5}( \d{5}){5}$").unwrap().is_match(&code), "{code}");
        let card = i.card("Priya Nair", 5);
        let p = read_card(&card).unwrap();
        assert_eq!((p.display_name.as_str(), p.key_id.as_str()), ("Priya Nair", i.key_id.as_str()));
        for (k, v) in [("display_name", json!("Someone Else")), ("agreement_public_key_hex", json!(id(9).agreement_public_key_hex)), ("key_id", json!("0000000000000000"))] {
            let mut t = card.clone();
            t[k] = v;
            assert!(read_card(&t).unwrap_err().starts_with("CF_BUNDLE_BAD_CARD"), "{k}");
        }
        assert!(read_card(&json!({"schema": "something else"})).is_err());
    }

    #[test]
    fn golden_redaction_rules() {
        let f = fx();
        for c in f["redaction_cases"].as_array().unwrap() {
            let reds: Vec<Redaction> = serde_json::from_value(c["terms"].clone()).unwrap();
            let r = Redactor::compile(&reds).unwrap();
            let (out, n) = r.apply_str(c["input"].as_str().unwrap());
            assert_eq!((out.as_str(), n), (c["output"].as_str().unwrap(), c["count"].as_u64().unwrap() as usize), "{}", c["name"]);
        }
        for b in f["bad_redactions"].as_array().unwrap() {
            let red: Redaction = serde_json::from_value(b["term"].clone()).unwrap();
            assert!(Redactor::compile(&[red]).err().unwrap().starts_with(f["codes"]["bad_redaction"].as_str().unwrap()));
        }
    }

    #[test]
    fn golden_a_bundle_is_redacted_counted_and_the_redacted_text_is_in_no_byte_of_it() {
        // AC-S4-10.2
        let f = fx();
        let alice = id(2);
        let built = build_core(&f, &[&alice]);
        let want = &f["expect_redacted"];
        // the log holds counts only
        let log: HashMap<String, usize> = built.redaction_log.iter().cloned().collect();
        let by_kind = |kind: &str| built.items.iter().find(|i| i.1 == kind).map(|i| i.0.clone()).unwrap();
        for (kind, name) in [("DOCUMENT", "document"), ("GRAPH_SUBSET", "graph"), ("NOTES", "notes")] {
            assert_eq!(log.get(&by_kind(kind)).copied().unwrap_or(0) as u64, want["log"][name].as_u64().unwrap(), "{name}");
        }
        assert!(!log.contains_key(&by_kind("ARGUMENT_TREE")) && !log.contains_key(&by_kind("COUNSEL_CORRECTIONS")), "no entry for an item with nothing redacted");
        let opened = open(&built.bytes, &alice).unwrap();
        let get = |kind: &str| opened.items.iter().find(|i| i.kind == kind).unwrap().envelope["payload"].clone();
        let doc = get("DOCUMENT");
        assert_eq!(doc["filename"], want["document"]["filename"]);
        assert_eq!(doc["text"], want["document"]["text"]);
        let g = get("GRAPH_SUBSET");
        assert_eq!(g["graph"]["parties"][0]["name"], want["graph"]["party_p1_name"]);
        assert_eq!(g["graph"]["parties"][1]["name"], want["graph"]["party_p2_name"]);
        // what was not redacted is still there
        let all_plain: String = opened.items.iter().map(|i| serde_json::to_string(&i.envelope).unwrap()).collect();
        for keep in want["kept_unredacted"].as_array().unwrap() {
            assert!(all_plain.contains(keep.as_str().unwrap()), "{keep} was kept");
        }
        // absent from every byte: the file, and everything decrypted from it, in every shape the golden lists (and JSON-escaped)
        for t in f["must_be_absent_from_every_byte"].as_array().unwrap() {
            for form in t["forms"].as_array().unwrap() {
                let form = form.as_str().unwrap();
                for shape in [form.to_string(), serde_json::to_string(form).unwrap().trim_matches('"').to_string(), form.to_lowercase(), form.to_uppercase()] {
                    assert!(!contains(&built.bytes, shape.as_bytes()), "bundle bytes contain {shape:?}");
                    assert!(!all_plain.contains(&shape), "decrypted items contain {shape:?}");
                }
            }
        }
        // the manifest says only how many
        let m = serde_json::to_string(&opened.manifest).unwrap();
        assert!(!m.contains("Ramesh") && opened.manifest["redaction_log"].as_array().unwrap().iter().all(|e| e.as_object().unwrap().len() == 2));
    }

    fn contains(hay: &[u8], needle: &[u8]) -> bool {
        !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn an_export_that_would_leave_a_redacted_term_behind_is_refused_and_nothing_is_written() {
        // a term hidden in the sender's own name is not redacted by the pass over the payload, so the independent check refuses
        let f = fx();
        let sender = id(1);
        let r = build(&sender, "Ramesh Kumar", &[peer(&id(2), "A")], "c", all_items(&f), &reds(&f), 1, "t", &mut OsRng);
        assert!(r.unwrap_err().starts_with(f["codes"]["redaction_incomplete"].as_str().unwrap()));
    }

    #[test]
    fn golden_the_right_key_opens_it_and_no_other_key_does() {
        // AC-S4-10.1
        let f = fx();
        let (alice, bob, carol) = (id(2), id(3), id(4));
        let built = build_core(&f, &[&alice, &bob]);
        for who in [&alice, &bob] {
            let o = open(&built.bytes, who).unwrap();
            assert_eq!(o.items.len(), 5);
            assert!(o.items.iter().all(|i| i.envelope["produced_by"]["display_name"] == "Priya Nair"));
        }
        let e = open(&built.bytes, &carol).err().unwrap();
        assert!(e.starts_with(f["codes"]["not_for_you"].as_str().unwrap()), "{e}");
        // the sender is not a recipient either
        assert!(open(&built.bytes, &id(1)).err().unwrap().starts_with("CF_BUNDLE_NOT_FOR_YOU"));
        // a key id that matches an entry but the wrong secret behind it
        let impostor = Identity { key_id: alice.key_id.clone(), ..Identity::from_seed(&[5; 32]) };
        assert!(open(&built.bytes, &impostor).err().unwrap().starts_with("CF_BUNDLE_NOT_FOR_YOU"));
        // the ciphertext contains no readable plaintext
        assert!(!contains(&built.bytes, b"Suresh Verma") && !contains(&built.bytes, b"Plaint"));
        // two bundles of the same items differ everywhere (fresh keys and nonces)
        let again = build_core(&f, &[&alice, &bob]);
        assert_ne!(built.bytes, again.bytes);
    }

    #[test]
    fn golden_changing_any_byte_of_a_bundle_is_detected() {
        // AC-S4-10.1: tampered bundle rejected. Every single byte, one at a time, in a small bundle.
        let f = fx();
        let (sender, alice) = (id(1), id(2));
        let small = vec![PlainItem { kind: "NOTES".into(), payload: json!({"text": "hello"}) }];
        let built = build(&sender, "Priya", &[peer(&alice, "A")], "c", small, &[], 1_791_590_400_000, "t", &mut OsRng).unwrap();
        assert!(open(&built.bytes, &alice).is_ok());
        let mut survivors = vec![];
        for i in 0..built.bytes.len() {
            let mut b = built.bytes.clone();
            b[i] ^= 0x01;
            if open(&b, &alice).is_ok() {
                survivors.push(i);
            }
        }
        assert!(survivors.is_empty(), "flipping these bytes was not detected: {survivors:?} (of {})", built.bytes.len());
        // cut short at every length, and extended
        for n in 0..built.bytes.len() {
            assert!(open(&built.bytes[..n], &alice).is_err(), "truncated to {n}");
        }
        let mut longer = built.bytes.clone();
        longer.push(0);
        assert!(open(&longer, &alice).err().unwrap().starts_with("CF_BUNDLE_BAD_FILE"));
        // a bigger bundle: a sample of positions across the manifest and the items
        let big = build_core(&f, &[&alice]);
        let step = (big.bytes.len() / 400).max(1);
        for i in (0..big.bytes.len()).step_by(step) {
            let mut b = big.bytes.clone();
            b[i] ^= 0x80;
            assert!(open(&b, &alice).is_err(), "byte {i} of {}", big.bytes.len());
        }
        // not a bundle at all
        assert!(open(b"hello", &alice).err().unwrap().starts_with(f["codes"]["bad_container"].as_str().unwrap()));
    }

    #[test]
    fn items_cannot_be_swapped_or_moved_between_bundles() {
        let (sender, alice) = (id(1), id(2));
        let two = vec![PlainItem { kind: "NOTES".into(), payload: json!({"text": "AAAA"}) }, PlainItem { kind: "NOTES".into(), payload: json!({"text": "BBBB"}) }];
        let built = build(&sender, "P", &[peer(&alice, "A")], "c", two, &[], 1, "t", &mut OsRng).unwrap();
        let p = parse_and_verify(&built.bytes).unwrap();
        let (a, b) = (p.blobs[0].to_vec(), p.blobs[1].to_vec());
        assert_eq!(a.len(), b.len());
        let start = built.bytes.len() - a.len() - b.len();
        let mut swapped = built.bytes[..start].to_vec();
        swapped.extend_from_slice(&b);
        swapped.extend_from_slice(&a);
        assert!(open(&swapped, &alice).err().unwrap().starts_with("CF_BUNDLE_TAMPERED"), "swapped items");
        // an item re-signed by someone else with a manifest of their own still fails to open for the wrong sender's claims:
        // moving the item blob into another bundle (same content key impossible) fails the authenticated item id
        let other = build(&sender, "P", &[peer(&alice, "A")], "c", vec![PlainItem { kind: "NOTES".into(), payload: json!({"text": "CCCC"}) }, PlainItem { kind: "NOTES".into(), payload: json!({"text": "DDDD"}) }], &[], 1, "t", &mut OsRng).unwrap();
        let po = parse_and_verify(&other.bytes).unwrap();
        let mut grafted = built.bytes[..start].to_vec();
        grafted.extend_from_slice(po.blobs[0]);
        grafted.extend_from_slice(po.blobs[1]);
        assert!(open(&grafted, &alice).is_err());
    }

    #[test]
    fn a_manifest_signed_by_someone_else_still_names_its_real_signer() {
        // anyone can make a bundle to your public key; what protects you is who it says (and proves) it is from
        let (alice, mallory) = (id(2), id(66));
        let b = build(&mallory, "Priya Nair", &[peer(&alice, "A")], "c", vec![PlainItem { kind: "NOTES".into(), payload: json!({"text": "hi"}) }], &[], 1, "t", &mut OsRng).unwrap();
        let o = open(&b.bytes, &alice).unwrap();
        assert_eq!(o.manifest["sender"]["signing_public_key_hex"], mallory.signing_public_key_hex.as_str(), "the key that signed is the key shown, whatever name is claimed");
        assert_ne!(o.manifest["sender"]["signing_public_key_hex"], id(1).signing_public_key_hex.as_str());
    }

    #[test]
    fn golden_a_received_correction_never_replaces_the_recipients_own() {
        // AC-S4-10.4 (the rule; the vault test shows the tables are untouched)
        let f = fx();
        for c in f["correction_merge_cases"].as_array().unwrap() {
            let recipient: HashMap<String, String> = c["recipient"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
            let plan = plan_corrections(&recipient, c["incoming"].as_array().unwrap());
            let want = c["expect"].as_array().unwrap();
            assert_eq!(plan.len(), want.len(), "{}", c["name"]);
            for (p, w) in plan.iter().zip(want) {
                for k in ["target_key", "status", "recipient_value"] {
                    assert_eq!(p[k], w[k], "{} {k}", c["name"]);
                }
                assert_eq!(p["origin"], "COUNSEL_INPUT");
            }
        }
    }

    #[test]
    fn golden_attribution_names_the_sender_and_the_date() {
        let f = fx();
        let a = &f["attribution"];
        assert_eq!(attribution_label(a["display_name"].as_str().unwrap(), a["created_at_unix_ms"].as_i64().unwrap()), a["expect_label"].as_str().unwrap());
        let b = build_core(&f, &[&id(2)]);
        let o = open(&b.bytes, &id(2)).unwrap();
        assert_eq!(attribution_label(o.manifest["sender"]["display_name"].as_str().unwrap(), o.items[0].envelope["produced_at_unix_ms"].as_i64().unwrap()), a["expect_label"].as_str().unwrap());
        assert!(o.manifest["created_at"].as_str().unwrap().starts_with("2026-10-10T00:00:00"));
    }

    #[test]
    fn the_manifest_satisfies_the_frozen_schema_and_the_module_holds_no_network_or_model() {
        let f = fx();
        let b = build_core(&f, &[&id(2), &id(3)]);
        let p = parse_and_verify(&b.bytes).unwrap();
        if let Some(dir) = std::env::var_os("CASEFORGE_EVIDENCE_DIR") {
            std::fs::write(std::path::Path::new(&dir).join("bundle_manifest.json"), serde_json::to_string_pretty(&p.manifest).unwrap()).unwrap();
            let cards = json!({"card": id(2).card("Recipient 0", 1), "verification_code": id(2).verification_code()});
            std::fs::write(std::path::Path::new(&dir).join("bundle_identity.json"), serde_json::to_string_pretty(&cards).unwrap()).unwrap();
        }
        for k in f["kinds"].as_array().unwrap() {
            assert!(KINDS.contains(&k.as_str().unwrap()));
        }
        let src = include_str!("bundle.rs");
        let code = src.split("#[cfg(test)]").next().unwrap();
        for tok in ["infer_llm", "reqwest", "TcpStream", "TcpListener", "UdpSocket", "std::net", "ollama"] {
            assert!(!code.lines().filter(|l| !l.trim_start().starts_with("//")).any(|l| l.contains(tok)), "{tok}");
        }
    }
}
