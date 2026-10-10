//! Offline seat transfer (B-S4-22, PRD v4.0 section 10; the yearly limit is owner decision Q-S4-07, PROPOSED 2).
//!
//! A licence is bound to one machine by its fingerprint, and only the issuer's key can bind it to another, so a transfer is a
//! paper trail that needs no network and no server:
//!   1. the old machine DEACTIVATES: it writes a signed release receipt (signed by the install's own key), forgets the licence
//!      and remembers that this seat was given away. Nothing the user made is touched: cases, documents and exports stay;
//!   2. the issuer receives the receipt and the new machine's fingerprint (files, by any means) and re-issues the licence for
//!      the new machine with the transfer recorded in the licence's signed `seat_transfers`;
//!   3. the new machine ACTIVATES the re-issued licence.
//! The yearly limit is counted over a rolling 365 days from `seat_transfers`, by the issuer's tool when it re-issues and by the
//! app when it activates (so a licence with too many transfers is refused even if an issuer slips). An app also refuses to take
//! back a licence file that it gave away: the old file carries fewer transfers than the vault remembers releasing.
//! Limits of an offline scheme, stated plainly: someone who deletes the vault can re-use an old licence file on the old
//! machine, and a copied licence file cannot be told from the original except by its fingerprint. That is true of every
//! offline licence; the receipt and the record are what make a transfer honest, not what make cheating impossible.

use crate::licence::FingerprintComponents;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const WINDOW_DAYS: i64 = 365;
const DAY_MS: i64 = 86_400_000;
pub const LIMIT_CODE: &str = "CF_SEAT_TRANSFER_LIMIT";
pub const RELEASED_CODE: &str = "CF_SEAT_RELEASED";
pub const RECEIPT_KIND: &str = "CASEFORGE_SEAT_RELEASE";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeatTransfer {
    pub transferred_at_unix_ms: i64,
    pub receipt_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SeatStatus {
    pub enabled: bool,
    pub limit_per_year: Option<i64>,
    pub used_in_window: usize,
    pub remaining: Option<i64>,
    pub window_days: i64,
    pub can_deactivate: bool,
    pub reason: Option<String>,
}

/// A transfer counts while it is less than a year old. A transfer dated in the future (a wrong clock) counts too.
pub fn used_in_window(transfers: &[SeatTransfer], now: i64) -> usize {
    transfers.iter().filter(|t| t.transferred_at_unix_ms > now - WINDOW_DAYS * DAY_MS).count()
}

pub fn status(limit: Option<i64>, transfers: &[SeatTransfer], now: i64) -> SeatStatus {
    let used = used_in_window(transfers, now);
    match limit {
        None => SeatStatus { enabled: false, limit_per_year: None, used_in_window: used, remaining: None, window_days: WINDOW_DAYS, can_deactivate: false, reason: Some("Seat transfer is not enabled for this licence.".into()) },
        Some(l) => {
            let remaining = (l - used as i64).max(0);
            let can = remaining > 0;
            SeatStatus {
                enabled: true,
                limit_per_year: Some(l),
                used_in_window: used,
                remaining: Some(remaining),
                window_days: WINDOW_DAYS,
                can_deactivate: can,
                reason: if can { None } else { Some(format!("This licence has no transfers left in the past {WINDOW_DAYS} days (limit {l}).")) },
            }
        }
    }
}

/// A licence whose signed transfers exceed its own limit in the window is refused.
pub fn check_licence(limit: Option<i64>, transfers: &[SeatTransfer], now: i64) -> Result<(), String> {
    let used = used_in_window(transfers, now) as i64;
    match limit {
        Some(l) if used > l => Err(format!("{LIMIT_CODE}: this licence records {used} seat transfers in the past {WINDOW_DAYS} days, more than its limit of {l}")),
        None if !transfers.is_empty() => Err(format!("{LIMIT_CODE}: this licence records seat transfers although seat transfer is not enabled for it")),
        _ => Ok(()),
    }
}

/// This vault remembers giving the seat away (`released_ordinal` = how many transfers the licence had once it was given
/// away). A licence file with fewer transfers than that is the one that was released.
pub fn check_not_released(released_ordinal: Option<usize>, licence_transfers: usize) -> Result<(), String> {
    match released_ordinal {
        Some(r) if licence_transfers < r => Err(format!("{RELEASED_CODE}: this licence file was transferred away from this machine; ask the issuer for the re-issued licence")),
        _ => Ok(()),
    }
}

/// The signed release receipt the old machine hands to the issuer.
pub fn issue_receipt(licence_id: &str, edition: &str, ordinal: usize, live: &FingerprintComponents, seed: &[u8; 32], now: i64, receipt_id: &str) -> Value {
    let sk = SigningKey::from_bytes(seed);
    let mut v = json!({
        "schema_version": "1.0.0",
        "kind": RECEIPT_KIND,
        "receipt_id": receipt_id,
        "licence_id": licence_id,
        "edition": edition,
        "transfer_ordinal": ordinal,
        "deactivated_at_unix_ms": now,
        "fingerprint_components": live,
        "public_key_hex": hex::encode(sk.verifying_key().to_bytes()),
        "signature_alg": "Ed25519",
    });
    let sig = sk.sign(&crate::edition::canonical_v2(&v).unwrap_or_default());
    v["signature_hex"] = json!(hex::encode(sig.to_bytes()));
    v
}

/// Checks a receipt's own signature (the issuer's tool does the same in Python). Whether the signing key belongs to the
/// licence's machine is the issuer's judgement, from the fingerprint inside the receipt.
pub fn verify_receipt(raw: &Value) -> Result<(), String> {
    let bad = |m: &str| Err(format!("CF_SEAT_RECEIPT_INVALID: {m}"));
    if raw.get("kind").and_then(Value::as_str) != Some(RECEIPT_KIND) || raw.get("signature_alg").and_then(Value::as_str) != Some("Ed25519") {
        return bad("not a seat release receipt");
    }
    let pk: Option<[u8; 32]> = raw.get("public_key_hex").and_then(Value::as_str).and_then(|h| hex::decode(h).ok()).and_then(|b| b.try_into().ok());
    let sig: Option<[u8; 64]> = raw.get("signature_hex").and_then(Value::as_str).and_then(|h| hex::decode(h).ok()).and_then(|b| b.try_into().ok());
    let (Some(pk), Some(sig)) = (pk, sig) else { return bad("key or signature missing") };
    let Ok(vk) = VerifyingKey::from_bytes(&pk) else { return bad("bad public key") };
    let bytes = crate::edition::canonical_v2(raw).map_err(|e| e.to_string())?;
    vk.verify(&bytes, &Signature::from_bytes(&sig)).map_err(|_| "CF_SEAT_RECEIPT_INVALID: signature does not verify".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entitlements::tests::fixture;

    fn transfers(g: &Value, days_ago: &Value) -> Vec<SeatTransfer> {
        let (now, day) = (g["now_ms"].as_i64().unwrap(), g["day_ms"].as_i64().unwrap());
        days_ago.as_array().unwrap().iter().enumerate().map(|(i, d)| SeatTransfer { transferred_at_unix_ms: now - d.as_i64().unwrap() * day, receipt_id: format!("r{i}") }).collect()
    }

    #[test]
    fn golden_transfers_are_counted_over_a_rolling_year() {
        let g = fixture();
        assert_eq!(g["seat_window_days"].as_i64().unwrap(), WINDOW_DAYS);
        let now = g["now_ms"].as_i64().unwrap();
        for c in g["seat_cases"].as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let s = status(c["limit"].as_i64(), &transfers(&g, &c["transfers_days_ago"]), now);
            let e = &c["expect"];
            assert_eq!(s.enabled, e["enabled"].as_bool().unwrap(), "{name}");
            assert_eq!(s.used_in_window as i64, e["used"].as_i64().unwrap(), "{name}");
            assert_eq!(s.remaining, e["remaining"].as_i64(), "{name}");
            assert_eq!(s.can_deactivate, e["can_deactivate"].as_bool().unwrap(), "{name}");
            if let Some(r) = e.get("reason_contains").and_then(Value::as_str) {
                assert!(s.reason.unwrap().to_lowercase().contains(r), "{name}");
            } else if s.can_deactivate {
                assert!(s.reason.is_none(), "{name}");
            }
        }
    }

    #[test]
    fn golden_a_licence_over_its_own_limit_is_refused() {
        let g = fixture();
        let now = g["now_ms"].as_i64().unwrap();
        for c in g["seat_licence_cases"].as_array().unwrap() {
            let r = check_licence(c["limit"].as_i64(), &transfers(&g, &c["transfers_days_ago"]), now);
            match c["expect"].as_str().unwrap() {
                "ok" => assert!(r.is_ok(), "{c}: {r:?}"),
                code => assert!(r.unwrap_err().starts_with(code), "{c}"),
            }
        }
    }

    #[test]
    fn golden_an_old_licence_file_cannot_take_the_seat_back() {
        let g = fixture();
        for c in g["seat_released_cases"].as_array().unwrap() {
            let r = check_not_released(c["released_ordinal"].as_u64().map(|v| v as usize), c["licence_transfers"].as_u64().unwrap() as usize);
            match c["expect"].as_str().unwrap() {
                "ok" => assert!(r.is_ok(), "{c}: {r:?}"),
                code => assert!(r.unwrap_err().starts_with(code), "{c}"),
            }
        }
    }

    fn fp() -> FingerprintComponents {
        FingerprintComponents { hostname: "old-host-1".into(), username: "user-0001".into(), machine_guid: "guid-12345678".into(), disk_root: "C:\\root-disk".into() }
    }

    #[test]
    fn a_release_receipt_verifies_and_any_change_to_it_breaks_it() {
        let r = issue_receipt("L-1", "PRO", 1, &fp(), &[9u8; 32], 1_800_000_000_000, "rcpt-1");
        assert!(verify_receipt(&r).is_ok());
        // the gate verifies a receipt written here with the Python licence tool, and re-issues a licence from it
        if let Some(dir) = std::env::var_os("CASEFORGE_EVIDENCE_DIR") {
            let ev = issue_receipt("L-seat-evidence", "PRO", 1, &fp(), &[9u8; 32], 1_800_000_000_000, "rcpt-evidence");
            std::fs::write(std::path::Path::new(&dir).join("seat_receipt.json"), serde_json::to_string_pretty(&ev).unwrap()).unwrap();
        }
        assert_eq!(r["transfer_ordinal"], 1);
        assert_eq!(r["kind"], RECEIPT_KIND);
        for edit in [
            (|v: &mut Value| v["licence_id"] = json!("L-2")) as fn(&mut Value),
            |v| v["transfer_ordinal"] = json!(2),
            |v| v["deactivated_at_unix_ms"] = json!(5),
            |v| v["fingerprint_components"]["hostname"] = json!("other-host"),
        ] {
            let mut t = r.clone();
            edit(&mut t);
            assert!(verify_receipt(&t).unwrap_err().starts_with("CF_SEAT_RECEIPT_INVALID"), "{t}");
        }
        let mut other_key = r.clone();
        other_key["public_key_hex"] = json!(hex::encode(SigningKey::from_bytes(&[3u8; 32]).verifying_key().to_bytes()));
        assert!(verify_receipt(&other_key).is_err(), "a receipt signed by one key does not pass under another");
        assert!(verify_receipt(&json!({"kind": "something else"})).is_err());
    }
}
