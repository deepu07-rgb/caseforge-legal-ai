//! New Case wizard (B-S1-03) — PRD §5 fields only.
//! Jurisdiction is required and immutable after create.

use super::error::{VaultError, VaultResult};
use serde::{Deserialize, Serialize};

/// Frozen jurisdiction values (PRD §5 / contracts/constants.json).
pub const JURISDICTIONS: &[&str] = &["India", "United States", "Other"];

pub const DEFAULT_LANGUAGE: &str = "English";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseWizardInput {
    pub cause_title: String,
    pub court: String,
    pub jurisdiction: String,
    pub matter_type: String,
    pub user_side: String,
    /// If empty/None, filled by jurisdiction-based stub (PRD: auto-suggested).
    pub code_era: Option<String>,
    /// Defaults to English (PRD: English primary).
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseWizardProfile {
    pub cause_title: String,
    pub court: String,
    pub jurisdiction: String,
    pub matter_type: String,
    pub user_side: String,
    pub code_era: String,
    pub language: String,
}

impl CaseWizardInput {
    pub fn validate_and_normalize(self) -> VaultResult<CaseWizardProfile> {
        let cause_title = self.cause_title.trim().to_string();
        let court = self.court.trim().to_string();
        let jurisdiction = self.jurisdiction.trim().to_string();
        let matter_type = self.matter_type.trim().to_string();
        let user_side = self.user_side.trim().to_string();

        if cause_title.is_empty() {
            return Err(VaultError::Internal {
                detail: "CF_CASE_WIZARD_INVALID: cause_title required".into(),
            });
        }
        if court.is_empty() {
            return Err(VaultError::Internal {
                detail: "CF_CASE_WIZARD_INVALID: court required".into(),
            });
        }
        if !JURISDICTIONS.contains(&jurisdiction.as_str()) {
            return Err(VaultError::Internal {
                detail: format!(
                    "CF_CASE_WIZARD_INVALID: jurisdiction must be one of {:?}",
                    JURISDICTIONS
                ),
            });
        }
        if matter_type.is_empty() {
            return Err(VaultError::Internal {
                detail: "CF_CASE_WIZARD_INVALID: matter_type required".into(),
            });
        }
        if user_side.is_empty() {
            return Err(VaultError::Internal {
                detail: "CF_CASE_WIZARD_INVALID: user_side required".into(),
            });
        }

        let code_era = self
            .code_era
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| suggest_code_era(&jurisdiction));

        let language = self
            .language
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_LANGUAGE.to_string());

        // PRD: English primary — do not invent multi-language product in B-S1-03.
        if language != DEFAULT_LANGUAGE {
            return Err(VaultError::Internal {
                detail: format!(
                    "CF_CASE_WIZARD_INVALID: language must be '{DEFAULT_LANGUAGE}' in Stage 1 (English primary)"
                ),
            });
        }

        Ok(CaseWizardProfile {
            cause_title,
            court,
            jurisdiction,
            matter_type,
            user_side,
            code_era,
            language,
        })
    }
}

/// Auto-suggest stub only (PRD §5). Not legal advice; not pack-aware.
pub fn suggest_code_era(jurisdiction: &str) -> String {
    match jurisdiction {
        "India" => "India — code/era auto-suggest stub (confirm with counsel)".into(),
        "United States" => {
            "United States — federal/state code-era stub (confirm with counsel)".into()
        }
        "Other" => "Other jurisdiction — code/era stub (user to complete)".into(),
        _ => String::new(),
    }
}
