//! Pack subscription, separate from the app licence (B-S4-22, PRD v4.0 section 10).
//!
//! `pack_subscription_until` says how long NEW law may be installed. When it passes, the installed packs stay exactly as
//! they are: usable, searchable, cited, exported, each citation showing the date its pack was last checked next to an
//! "updates expired" notice. The only thing that stops is installing newer official content. Nothing in this module is
//! consulted when reading or exporting (the gate editions_b_s4_22.py scans the source to keep it that way), and it is
//! independent of the app licence: an expired app licence makes the vault read-only, an expired subscription does not.

use serde::Serialize;

pub const SUBSCRIBED: &str = "SUBSCRIBED";
pub const UPDATES_EXPIRED: &str = "UPDATES_EXPIRED";
pub const NOT_APPLICABLE: &str = "NOT_APPLICABLE";
pub const REFUSAL_CODE: &str = "CF_PACK_UPDATES_EXPIRED";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PackSubscription {
    /// SUBSCRIBED, UPDATES_EXPIRED, or NOT_APPLICABLE (no verified v2 licence, so there is no subscription to speak of).
    pub state: &'static str,
    pub until_unix_ms: Option<i64>,
    /// The last day of the subscription (UTC), when it has an end.
    pub until_date: Option<String>,
    /// What to show the user when updates have expired.
    pub notice: Option<String>,
}

impl PackSubscription {
    pub fn expired(&self) -> bool {
        self.state == UPDATES_EXPIRED
    }
}

pub fn not_applicable() -> PackSubscription {
    PackSubscription { state: NOT_APPLICABLE, until_unix_ms: None, until_date: None, notice: None }
}

/// `until` follows the app licence's rule: no value, or zero, means no end date; the subscription covers the end instant.
pub fn evaluate(until: Option<i64>, now: i64) -> PackSubscription {
    match until {
        Some(u) if u > 0 && now > u => {
            let date = date_of(u);
            PackSubscription { state: UPDATES_EXPIRED, until_unix_ms: Some(u), until_date: Some(date.clone()), notice: Some(notice_text(&date)) }
        }
        Some(u) if u > 0 => PackSubscription { state: SUBSCRIBED, until_unix_ms: Some(u), until_date: Some(date_of(u)), notice: None },
        _ => PackSubscription { state: SUBSCRIBED, until_unix_ms: None, until_date: None, notice: None },
    }
}

pub fn notice_text(date: &str) -> String {
    format!("Updates expired on {date}. Your installed packs stay available, and every citation shows the date its pack was last checked; no new updates can be installed until the pack subscription is renewed. Your work and your documents are not affected.")
}

/// Added to the currency stamp of a citation when updates have expired.
pub fn stamp_suffix(sub: &PackSubscription) -> String {
    match (&sub.until_date, sub.expired()) {
        (Some(d), true) => format!(" \u{2014} updates expired: the pack subscription ended on {d}, so newer law may not be reflected"),
        _ => String::new(),
    }
}

/// Whether a pack may be installed. Only official content that is newer than the subscription is refused: a delta, or a full
/// official pack checked after the subscription ended. An older official pack can be put back (a restore), a pack that
/// states no date cannot claim to be newer, and the lawyer's own custom packs are never blocked.
pub fn check_install(sub: &PackSubscription, pack_kind: &str, is_delta: bool, currency_date: Option<&str>, updated_at: Option<&str>) -> Result<(), String> {
    if !sub.expired() || pack_kind != "official" {
        return Ok(());
    }
    let until = sub.until_date.as_deref().unwrap_or("");
    let refuse = |what: &str| Err(format!("{REFUSAL_CODE}: {what}. The pack subscription ended on {until}, so new official updates cannot be installed. Your installed packs and all your work stay available."));
    if is_delta {
        return refuse("this is an update to an installed pack");
    }
    let stated = currency_date.or_else(|| updated_at.map(|d| d.get(..10).unwrap_or(d)));
    match stated {
        Some(d) if d > until => refuse(&format!("this pack was checked on {d}, after the subscription ended")),
        _ => Ok(()),
    }
}

/// UTC calendar date (YYYY-MM-DD) of a Unix time in milliseconds.
pub fn date_of(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entitlements::tests::fixture;
    use serde_json::Value;

    #[test]
    fn dates_are_right_across_the_calendar() {
        assert_eq!(date_of(0), "1970-01-01");
        assert_eq!(date_of(1_800_000_000_000), "2027-01-15");
        assert_eq!(date_of(951_782_400_000), "2000-02-29"); // a leap day
        assert_eq!(date_of(-1), "1969-12-31");
        assert_eq!(date_of(4_102_444_800_000), "2100-01-01");
    }

    #[test]
    fn golden_the_state_follows_the_end_date_and_the_notice_says_what_is_and_is_not_affected() {
        let g = fixture();
        let now = g["now_ms"].as_i64().unwrap();
        for c in g["subscription_cases"].as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let s = evaluate(c["until_ms"].as_i64(), now);
            assert_eq!(s.state, c["expect_state"].as_str().unwrap(), "{name}");
            assert_eq!(s.notice.is_some(), c["expect_notice"].as_bool().unwrap(), "{name}");
            if let Some(d) = c.get("expect_date").and_then(Value::as_str) {
                assert_eq!(s.until_date.as_deref(), Some(d), "{name}");
                let n = s.notice.unwrap().to_lowercase();
                assert!(n.contains(d), "{name}: the notice names the date");
                for must in g["notice_must_say"].as_array().unwrap() {
                    assert!(n.contains(must.as_str().unwrap()), "{name}: notice says {must}");
                }
            }
        }
    }

    #[test]
    fn golden_only_newer_official_content_is_refused_after_expiry() {
        let g = fixture();
        let now = g["now_ms"].as_i64().unwrap();
        for c in g["pack_install_cases"].as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let sub = match c["state"].as_str().unwrap() {
                "SUBSCRIBED" => evaluate(None, now),
                "UPDATES_EXPIRED" => evaluate(c["until_ms"].as_i64(), now),
                _ => not_applicable(),
            };
            let r = check_install(&sub, c["pack_kind"].as_str().unwrap(), c["delta"].as_bool().unwrap(), c["currency_date"].as_str(), None);
            match c["expect"].as_str().unwrap() {
                "allowed" => assert!(r.is_ok(), "{name}: {r:?}"),
                _ => {
                    let e = r.unwrap_err();
                    assert!(e.starts_with(g["refusal_code"].as_str().unwrap()), "{name}: {e}");
                    assert!(e.contains("stay available"), "{name}: the refusal says the user keeps their work");
                }
            }
        }
    }

    #[test]
    fn an_official_pack_with_only_an_update_date_is_judged_by_that_date() {
        let sub = evaluate(Some(1_800_000_000_000 - 1), 1_800_000_000_000);
        assert!(check_install(&sub, "official", false, None, Some("2027-02-01T00:00:00Z")).is_err());
        assert!(check_install(&sub, "official", false, None, Some("2026-12-31T00:00:00Z")).is_ok());
    }

    #[test]
    fn the_stamp_keeps_the_older_date_and_adds_the_notice_only_when_expired() {
        let now = 1_800_000_000_000;
        let live = evaluate(Some(now + 1000), now);
        assert_eq!(stamp_suffix(&live), "");
        let gone = evaluate(Some(now - 1), now);
        let text = format!("{}{}", crate::treatment::stamp_text("Indian Law Pack", Some("2026-11-30")), stamp_suffix(&gone));
        assert!(text.starts_with("verified against Indian Law Pack as of 2026-11-30"), "{text}");
        assert!(text.contains("updates expired") && text.contains("2027-01-15"), "{text}");
    }
}
