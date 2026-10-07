//! Document intake (B-S1-04+) — PDF / DOCX / JPG / PNG / TXT.
//! TXT is for synthetic case packets and counsel notes (feeds local LLM context).
//! No OCR. Manual type tag + one-click correction. Phone Capture out of scope.

use super::error::{VaultError, VaultResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Source formats listed in PRD §5, plus TXT notes/packets for local reasoner context.
pub const ALLOWED_EXTENSIONS: &[&str] = &["pdf", "docx", "jpg", "jpeg", "png", "txt"];

/// Suggested type tags from extension (stub only — not AC-5.1 ML).
pub const TYPE_TAG_SUGGESTIONS: &[&str] = &["PDF", "DOCX", "JPG", "PNG", "TXT", "OTHER"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub doc_id: String,
    pub original_filename: String,
    pub type_tag: String,
    pub language_tag: String,
    pub source_format: String,
    pub byte_size: i64,
    /// Null until pipeline counts pages (placeholder for B-S1-04).
    pub page_count: Option<i64>,
    /// Placeholder until quality grading exists.
    pub per_page_quality_grade: Option<String>,
    pub ingested_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentIngestRequest {
    pub original_filename: String,
    /// Standard base64 (not URL-safe) of file bytes.
    pub content_base64: String,
    /// If omitted/empty, suggested from extension.
    pub type_tag: Option<String>,
    /// Defaults to "en" (English primary).
    pub language_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocTextPreview {
    pub doc_id: String,
    pub original_filename: String,
    pub source_format: String,
    pub type_tag: String,
    pub is_binary_stub: bool,
    pub text: String,
    pub char_count: u64,
}

pub fn extension_of(filename: &str) -> Option<String> {
    Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
}

/// Returns normalized source_format: pdf | docx | jpg | png | txt
pub fn validate_extension(filename: &str) -> VaultResult<String> {
    let ext = extension_of(filename).ok_or_else(|| VaultError::Internal {
        detail: "CF_DOCPIPE_UNSUPPORTED_SOURCE: missing file extension".into(),
    })?;
    let source_format = match ext.as_str() {
        "pdf" => "pdf",
        "docx" => "docx",
        "jpg" | "jpeg" => "jpg",
        "png" => "png",
        "txt" => "txt",
        other => {
            return Err(VaultError::Internal {
                detail: format!(
                    "CF_DOCPIPE_UNSUPPORTED_SOURCE: only PDF, DOCX, JPG, PNG, TXT allowed (got .{other})"
                ),
            });
        }
    };
    Ok(source_format.into())
}

pub fn suggest_type_tag(source_format: &str) -> String {
    match source_format {
        "pdf" => "PDF".into(),
        "docx" => "DOCX".into(),
        "jpg" => "JPG".into(),
        "png" => "PNG".into(),
        "txt" => "TXT".into(),
        _ => "OTHER".into(),
    }
}

pub fn decode_base64(content_base64: &str) -> VaultResult<Vec<u8>> {
    let s = content_base64.trim();
    let s = s
        .strip_prefix("data:")
        .and_then(|rest| rest.split(',').nth(1))
        .unwrap_or(s);
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s).map_err(|e| {
        VaultError::Internal {
            detail: format!("CF_DOCPIPE_UNSUPPORTED_SOURCE: invalid base64: {e}"),
        }
    })
}
