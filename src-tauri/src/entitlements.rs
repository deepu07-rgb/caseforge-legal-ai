//! The edition entitlement map (B-S4-22, PRD v4.0 section 10; owner decision Q-S4-06 is open, so the shipped map is
//! DEVELOPMENT_ONLY). One map in `contracts/edition_entitlements.json` says which features each edition includes, which
//! screen needs which feature and which host command needs which feature. Nothing else in the app decides what an edition
//! gets.
//!
//! Rules:
//!   - what a licence entitles is its signed `features[]` INTERSECTED with its edition's list: a feature the licence carries
//!     but its edition does not include is ignored (and reported), a feature the edition includes but the licence omits is not
//!     granted;
//!   - an install with no verified v2 licence follows `unlicensed_stage4_policy`: OPEN_DEVELOPMENT keeps everything but
//!     Chambers, LOCKED refuses the Stage 4 features. A feature the map does not know is refused either way;
//!   - an expired licence keeps what it entitles. Expiry limits writing, never reading or exporting (that rule lives in the
//!     vault; nothing here consults the licence state).

use serde::Deserialize;
use std::collections::BTreeMap;

pub const MAP_JSON: &str = include_str!("../../contracts/edition_entitlements.json");

#[derive(Debug, Clone, Deserialize)]
pub struct EditionEntry {
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Entitlements {
    pub status: String,
    #[serde(default)]
    pub question: Option<String>,
    pub unlicensed_stage4_policy: String,
    pub stage4_features: Vec<String>,
    #[serde(default)]
    pub feature_labels: BTreeMap<String, String>,
    pub editions: BTreeMap<String, EditionEntry>,
    pub surfaces: BTreeMap<String, Option<String>>,
    pub commands: BTreeMap<String, String>,
}

impl Entitlements {
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("CF_ENTITLEMENT_MAP_INVALID: {e}"))
    }

    /// The map shipped in the app. If it cannot be read nothing is entitled (fail closed), and a test fails.
    pub fn builtin() -> &'static Entitlements {
        static MAP: once_cell::sync::Lazy<Entitlements> = once_cell::sync::Lazy::new(|| {
            Entitlements::parse(MAP_JSON).unwrap_or(Entitlements {
                status: "UNREADABLE".into(),
                question: None,
                unlicensed_stage4_policy: "LOCKED".into(),
                stage4_features: vec![],
                feature_labels: BTreeMap::new(),
                editions: BTreeMap::new(),
                surfaces: BTreeMap::new(),
                commands: BTreeMap::new(),
            })
        });
        &MAP
    }

    pub fn is_development_only(&self) -> bool {
        self.status != "APPROVED"
    }

    /// The edition's feature list (empty for an edition the map does not know).
    pub fn edition_features(&self, edition: &str) -> &[String] {
        self.editions.get(edition).map(|e| e.features.as_slice()).unwrap_or(&[])
    }

    /// (effective, ignored): the licence's features the edition includes, in the licence's order, and those it does not.
    pub fn effective(&self, edition: &str, licence_features: &[String]) -> (Vec<String>, Vec<String>) {
        let allowed = self.edition_features(edition);
        let mut eff = vec![];
        let mut ignored = vec![];
        for f in licence_features {
            if allowed.contains(f) {
                eff.push(f.clone());
            } else {
                ignored.push(f.clone());
            }
        }
        (eff, ignored)
    }

    /// True if the screen needs a feature the entitled set lacks. `None` = no verified licence. A screen the map does not
    /// list is hidden (the map is the single source).
    pub fn surface_hidden(&self, surface: &str, effective: Option<&[String]>) -> bool {
        match self.surfaces.get(surface) {
            None => true,
            Some(None) => false,
            Some(Some(need)) => !self.feature_allowed(effective, need),
        }
    }

    /// May a feature be used? `None` = no verified v2 licence.
    pub fn feature_allowed(&self, effective: Option<&[String]>, feature: &str) -> bool {
        let known = self.editions.values().any(|e| e.features.iter().any(|f| f == feature));
        if !known {
            return false;
        }
        match effective {
            Some(eff) => eff.iter().any(|f| f == feature),
            None => {
                if feature == "chambers" {
                    return false;
                }
                !(self.unlicensed_stage4_policy == "LOCKED" && self.stage4_features.iter().any(|f| f == feature))
            }
        }
    }

    /// The feature a host command needs, if the map lists it.
    pub fn command_feature(&self, command: &str) -> Option<&str> {
        self.commands.get(command).map(String::as_str)
    }

    pub fn label(&self, feature: &str) -> String {
        self.feature_labels.get(feature).cloned().unwrap_or_else(|| feature.to_string())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::Value;

    pub const FIXTURE: &str = include_str!("../../golden/G-EDITIONS/fixtures/editions_matrix.json");

    pub fn fixture() -> Value {
        serde_json::from_str(FIXTURE).unwrap()
    }
    pub fn strs(v: &Value) -> Vec<String> {
        v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect()
    }
    pub fn golden_map() -> Entitlements {
        Entitlements::parse(&fixture()["map"].to_string()).unwrap()
    }

    #[test]
    fn the_shipped_map_parses_and_is_marked_development_only() {
        let m = Entitlements::builtin();
        assert_ne!(m.status, "UNREADABLE", "the shipped entitlement map must parse");
        assert!(m.is_development_only(), "Q-S4-06 is undecided: the map must say so");
        assert_eq!(m.editions.len(), 4);
    }

    #[test]
    fn golden_a_licence_only_ever_narrows_its_edition() {
        let g = fixture();
        let m = golden_map();
        for c in g["surface_cases"].as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let (eff, ignored) = m.effective(c["edition"].as_str().unwrap(), &strs(&c["licence_features"]));
            assert_eq!(eff, strs(&c["expect_effective"]), "{name}");
            assert_eq!(ignored, strs(&c["expect_ignored"]), "{name}");
        }
    }

    #[test]
    fn golden_commands_need_their_feature_and_unknown_features_are_refused() {
        let g = fixture();
        let m = golden_map();
        for c in g["command_cases"].as_array().unwrap() {
            let (eff, _) = m.effective(c["edition"].as_str().unwrap(), &strs(&c["licence_features"]));
            let ok = m.feature_allowed(Some(&eff), c["feature"].as_str().unwrap());
            assert_eq!(ok, c["expect"] == "ok", "{c}");
        }
        for c in g["command_map_cases"].as_array().unwrap() {
            assert_eq!(m.command_feature(c["command"].as_str().unwrap()), c["needs"].as_str(), "{c}");
        }
    }

    #[test]
    fn golden_an_install_with_no_licence_follows_the_stated_policy() {
        let g = fixture();
        for c in g["unlicensed_cases"].as_array().unwrap() {
            let mut m = golden_map();
            m.unlicensed_stage4_policy = c["policy"].as_str().unwrap().to_string();
            assert_eq!(m.feature_allowed(None, c["feature"].as_str().unwrap()), c["expect"] == "ok", "{c}");
        }
    }
}
