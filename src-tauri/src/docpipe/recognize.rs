//! Offline OCR result contract and host adapter (B-S3-04).

use crate::docpipe::pdf_adapter;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::sync::OnceLock;

pub const RECOGNIZER_ID: &str = "caseforge.ocr.tesseract-eng-fast.v1";

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub struct ConfidenceThresholds {
    pub word_highlight_below: f64,
    pub word_force_review_below: f64,
    pub word_suppress_below: f64,
    pub page_mean_warn_below: f64,
}

#[cfg(test)]
impl Default for ConfidenceThresholds {
    fn default() -> Self {
        Self {
            word_highlight_below: 0.75,
            word_force_review_below: 0.5,
            word_suppress_below: 0.25,
            page_mean_warn_below: 0.7,
        }
    }
}

#[cfg(test)]
fn thresholds() -> ConfidenceThresholds {
    static CACHED: OnceLock<ConfidenceThresholds> = OnceLock::new();
    *CACHED.get_or_init(load_thresholds_from_contracts)
}

#[cfg(test)]
fn load_thresholds_from_contracts() -> ConfidenceThresholds {
    let mut t = ConfidenceThresholds::default();
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../contracts/constants.json"),
        PathBuf::from("contracts/constants.json"),
    ];
    for path in candidates {
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(vals) = v.get("confidence_thresholds").and_then(|c| c.get("values")) {
                    if let Some(x) = vals.get("word_highlight_below").and_then(|x| x.as_f64()) {
                        t.word_highlight_below = x;
                    }
                    if let Some(x) = vals.get("word_force_review_below").and_then(|x| x.as_f64()) {
                        t.word_force_review_below = x;
                    }
                    if let Some(x) = vals.get("word_suppress_below").and_then(|x| x.as_f64()) {
                        t.word_suppress_below = x;
                    }
                    if let Some(x) = vals.get("page_mean_warn_below").and_then(|x| x.as_f64()) {
                        t.page_mean_warn_below = x;
                    }
                    return t;
                }
            }
        }
    }
    t
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordSpan {
    pub word_index: i64,
    pub text: String,
    pub confidence: f64,
    #[serde(default)]
    pub raw_engine_confidence: Option<f64>,
    #[serde(default)]
    pub calibrated_confidence: Option<f64>,
    #[serde(default = "missing_confidence_state")]
    pub confidence_state: String,
    pub char_start: i64,
    pub char_end: i64,
    pub low_confidence: bool,
    pub force_review: bool,
    pub suppress_from_auto: bool,
    pub bbox: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageTextLayerResult {
    pub page_index: i64,
    pub recognizer_id: String,
    pub full_text: String,
    pub mean_confidence: f64,
    pub page_quality_warn: bool,
    pub words: Vec<WordSpan>,
    #[serde(default)]
    pub critical_fields: serde_json::Value,
    pub notes: Option<String>,
}

fn missing_confidence_state() -> String {
    "missing".into()
}

#[cfg(test)]
fn score_word(token: &str) -> f64 {
    let t = token.trim();
    if t.is_empty() {
        return 0.0;
    }
    let lower = t.to_ascii_lowercase();
    if t.contains("???") || lower == "??" || lower == "???" {
        return 0.28;
    }
    if t.chars().all(|c| c.is_ascii_alphabetic()) && t.len() >= 3 {
        return 0.93;
    }
    if t.chars().all(|c| c.is_ascii_alphabetic()) && t.len() == 2 {
        return 0.88;
    }
    if t.len() == 2 && t.ends_with('.') && t.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
    {
        return 0.8;
    }
    if t.chars().all(|c| c.is_ascii_digit()) && t.len() <= 4 {
        return 0.86;
    }
    let has_letter = t.chars().any(|c| c.is_ascii_alphabetic());
    let has_digit = t.chars().any(|c| c.is_ascii_digit());
    if has_letter && has_digit {
        return 0.78;
    }
    let non_ascii = t.chars().filter(|c| *c as u32 > 127).count();
    if non_ascii > 0 {
        let ratio = non_ascii as f64 / t.len().max(1) as f64;
        return (0.82 - 0.25 * ratio).max(0.45);
    }
    if t.chars().count() == 1 {
        return 0.52;
    }
    0.75
}

#[cfg(test)]
fn recognize_text(full: &str, thr: ConfidenceThresholds) -> (f64, bool, Vec<WordSpan>) {
    let mut words = Vec::new();
    let mut word_index: i64 = 0;
    let chars: Vec<char> = full.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        let token: String = chars[start..i].iter().collect();
        if !token.is_empty() {
            let conf = score_word(&token);
            words.push(WordSpan {
                word_index,
                text: token,
                confidence: conf,
                raw_engine_confidence: None,
                calibrated_confidence: None,
                confidence_state: "missing".into(),
                char_start: start as i64,
                char_end: i as i64,
                low_confidence: conf < thr.word_highlight_below,
                force_review: conf < thr.word_force_review_below,
                suppress_from_auto: conf < thr.word_suppress_below,
                bbox: None,
            });
            word_index += 1;
        }
    }

    let mean = if words.is_empty() {
        0.0
    } else {
        words.iter().map(|w| w.confidence).sum::<f64>() / words.len() as f64
    };
    let warn = if words.is_empty() {
        true
    } else {
        mean < thr.page_mean_warn_below
    };
    (mean, warn, words)
}

/// Recognize document bytes into per-page text layers with word confidences.
pub fn recognize_document_bytes(
    bytes: &[u8],
    source_format: &str,
    filename: &str,
) -> Result<Vec<PageTextLayerResult>, String> {
    pdf_adapter::recognize_document(bytes, source_format, filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_words_high_confidence() {
        let thr = ConfidenceThresholds::default();
        let (mean, warn, words) = recognize_text("The plaintiff filed a petition.", thr);
        assert!(!words.is_empty());
        assert!(mean > 0.8);
        assert!(!warn);
        assert!(words
            .iter()
            .any(|w| w.text == "plaintiff" && !w.low_confidence));
    }

    #[test]
    fn uncertain_token_force_review() {
        let thr = ConfidenceThresholds::default();
        let (_, _, words) = recognize_text("date mar??? unclear", thr);
        let hit = words
            .iter()
            .find(|w| w.text.contains("???"))
            .expect("token");
        assert!(hit.force_review);
        assert!(hit.low_confidence);
    }
}
