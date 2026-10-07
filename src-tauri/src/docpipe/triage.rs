//! Provisional heuristic page triage (mirrors sidecars/docpipe/triage.py).
//! classifier_id = caseforge.triage.heuristic.v1

use crate::docpipe::pdf_adapter;
use serde::{Deserialize, Serialize};

pub const CLASSIFIER_ID: &str = "caseforge.triage.heuristic.v1";

pub const TRIAGE_CLASSES: &[&str] = &[
    "typed_english",
    "other",
    "mixed",
    "clean_handwriting",
    "messy_handwriting",
    "non_text",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageTriageResult {
    pub page_index: i64,
    pub triage_class: String,
    pub confidence: f64,
    pub classifier_id: String,
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_source_json: Option<String>,
}

/// Classify document bytes into per-page triage results.
pub fn classify_document_bytes(
    bytes: &[u8],
    source_format: &str,
    filename: &str,
) -> Result<Vec<PageTriageResult>, String> {
    let pages = split_pages(bytes, source_format, filename)?;
    Ok(pages
        .into_iter()
        .map(|(idx, text, is_image, content_len, page_source_json)| {
            let (cls, conf, notes) = classify_text(&text, is_image, content_len);
            PageTriageResult {
                page_index: idx,
                triage_class: cls.into(),
                confidence: conf,
                classifier_id: CLASSIFIER_ID.into(),
                notes: Some(notes.into()),
                page_source_json,
            }
        })
        .collect())
}

/// Per-page text layers for recognition (B-S1-06). Returns (page_index, text, is_image).
pub fn document_page_texts(
    bytes: &[u8],
    source_format: &str,
    filename: &str,
) -> Result<Vec<(i64, String, bool)>, String> {
    Ok(split_pages(bytes, source_format, filename)?
        .into_iter()
        .map(|(idx, text, is_image, _, _)| (idx, text, is_image))
        .collect())
}

fn split_pages(
    bytes: &[u8],
    source_format: &str,
    filename: &str,
) -> Result<Vec<(i64, String, bool, usize, Option<String>)>, String> {
    let name = filename.to_ascii_lowercase();
    let fmt = source_format.to_ascii_lowercase();

    if fmt == "pdf" || name.ends_with(".pdf") {
        return split_pdf(bytes, filename);
    }
    if fmt == "docx" || name.ends_with(".docx") {
        let text = extract_docx_text(bytes);
        return Ok(vec![(0, text, false, bytes.len(), None)]);
    }
    if fmt == "jpg"
        || fmt == "png"
        || name.ends_with(".jpg")
        || name.ends_with(".jpeg")
        || name.ends_with(".png")
    {
        return Ok(vec![(0, String::new(), true, bytes.len(), None)]);
    }
    // text / unknown
    let text = String::from_utf8_lossy(bytes).into_owned();
    Ok(vec![(0, text, false, bytes.len(), None)])
}

fn split_pdf(
    bytes: &[u8],
    filename: &str,
) -> Result<Vec<(i64, String, bool, usize, Option<String>)>, String> {
    let parsed = pdf_adapter::parse_pdf(bytes, filename, None)?;
    if parsed.document_state != "ready" {
        return Err(format!(
            "CF_PDF_PARSE_INCOMPLETE: state={}",
            parsed.document_state
        ));
    }
    Ok(parsed
        .pages
        .into_iter()
        .map(|page| {
            let needs_ocr = page.source_kind == "scanned" || page.source_kind == "mixed";
            let json = serde_json::to_string(&page).ok();
            (
                page.page_index,
                page.native_text,
                needs_ocr,
                bytes.len(),
                json,
            )
        })
        .collect())
}

fn extract_docx_text(bytes: &[u8]) -> String {
    // Minimal: find <w:t>...</w:t> even inside zip binary.
    let raw = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    let mut rest = raw.as_ref();
    while let Some(start) = rest.find("<w:t") {
        let after = &rest[start..];
        if let Some(gt) = after.find('>') {
            let content_start = &after[gt + 1..];
            if let Some(end) = content_start.find("</w:t>") {
                let t = &content_start[..end];
                if !t.is_empty() {
                    if !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(t);
                }
                rest = &content_start[end + 6..];
                continue;
            }
        }
        rest = &rest[start + 4..];
    }
    if out.is_empty() {
        // ASCII runs
        let mut run = String::new();
        let mut runs = Vec::new();
        for b in bytes {
            let c = *b as char;
            if c.is_ascii_graphic() || c == ' ' {
                run.push(c);
            } else if run.len() >= 8 {
                runs.push(run.clone());
                run.clear();
                if runs.len() >= 40 {
                    break;
                }
            } else {
                run.clear();
            }
        }
        return runs.join(" ");
    }
    out
}

fn classify_text(
    text: &str,
    is_image: bool,
    content_len: usize,
) -> (&'static str, f64, &'static str) {
    if let Some(cls) = extract_marker(text) {
        return (cls, 0.99, "explicit fixture marker");
    }

    let t = text.trim();
    if is_image && t.is_empty() {
        if content_len < 64 {
            return ("non_text", 0.55, "tiny image bytes; provisional non_text");
        }
        return (
            "typed_english",
            0.35,
            "image without text layer; provisional low-confidence default",
        );
    }
    if t.is_empty() {
        return ("non_text", 0.9, "empty page text");
    }

    let printable = t
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .count();
    if printable < 12 {
        return ("non_text", 0.88, "too little printable content");
    }

    // Strip synthetic scaffold footer so footer tokens never drive the class.
    let body = t.split("[CASEFORGE_SYNTHETIC_PAGE]").next().unwrap_or(t);
    let body_lower = body.to_ascii_lowercase();

    let has_hand = body_lower.contains("handwrit")
        || body_lower.contains("cursive")
        || body_lower.contains("margin notes")
        || body_lower.contains("hand notes")
        || body_lower.contains("scrawl");
    let messy = has_hand
        && (body_lower.contains("messy")
            || body_lower.contains("scrawl")
            || body_lower.contains("crossouts")
            || body_lower.contains("hard to read")
            || body_lower.contains("???"));
    let clean_hand = has_hand
        && (body_lower.contains("clean handwriting")
            || (body_lower.contains("neat") && body_lower.contains("hand"))
            || body_lower.contains("clear lines"));
    let typed_block = [
        "order",
        "plaintiff",
        "defendant",
        "affidavit",
        "petition",
        "subject:",
        "in the court",
        "please take notice",
        "civil miscellaneous",
        "applicant:",
        "respondent:",
    ]
    .iter()
    .any(|k| body_lower.contains(k));

    if messy {
        return ("messy_handwriting", 0.82, "messy handwriting cues");
    }
    if typed_block && has_hand {
        return ("mixed", 0.8, "typed body + handwriting margin cues");
    }
    if clean_hand || (has_hand && !typed_block) {
        return (
            "clean_handwriting",
            0.78,
            "handwriting without typed court block",
        );
    }

    let non_ascii = body.chars().filter(|c| *c as u32 > 127).count();
    let ascii_letters = body.chars().filter(|c| c.is_ascii_alphabetic()).count();
    let has_typed = ascii_letters >= 3;

    if non_ascii > 8 && non_ascii > ascii_letters / 2 && !has_hand {
        return ("other", 0.8, "high non-ascii density");
    }
    if [
        "demanda",
        "demandante",
        "demandado",
        "expediente",
        "reclamaci",
        "sintético",
        "sintetico",
        "página",
        "aviso",
    ]
    .iter()
    .any(|k| body_lower.contains(k))
    {
        return ("other", 0.78, "non-english legal language cues");
    }

    let words: Vec<&str> = body_lower
        .split(|c: char| !c.is_ascii_alphabetic() && c != '\'')
        .filter(|w| !w.is_empty())
        .collect();
    let eng = [
        "the",
        "and",
        "of",
        "to",
        "in",
        "that",
        "is",
        "for",
        "court",
        "plaintiff",
        "defendant",
        "affidavit",
        "petition",
        "order",
        "section",
    ];
    let eng_hits = words.iter().filter(|w| eng.contains(w)).count();

    if eng_hits >= 2 || (has_typed && ascii_letters > 40) {
        return ("typed_english", 0.72, "english-like typed text");
    }
    if has_typed {
        return ("typed_english", 0.55, "letter runs present");
    }
    ("other", 0.5, "fallback other")
}

fn extract_marker(text: &str) -> Option<&'static str> {
    let upper = text; // case-insensitive search
    let needle = "[CASEFORGE_TRIAGE_CLASS:";
    let lower_text = upper.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    if let Some(pos) = lower_text.find(&lower_needle) {
        let after = &upper[pos + needle.len()..];
        if let Some(end) = after.find(']') {
            let raw = after[..end].trim().to_ascii_lowercase();
            return match raw.as_str() {
                "typed_english" => Some("typed_english"),
                "other" => Some("other"),
                "mixed" => Some("mixed"),
                "clean_handwriting" => Some("clean_handwriting"),
                "messy_handwriting" => Some("messy_handwriting"),
                "non_text" => Some("non_text"),
                _ => None,
            };
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_typed() {
        let r = classify_document_bytes(
            b"[CASEFORGE_TRIAGE_CLASS:typed_english]\nHello court",
            "txt",
            "p.txt",
        )
        .unwrap();
        assert_eq!(r[0].triage_class, "typed_english");
        assert!(r[0].confidence >= 0.99);
    }

    #[test]
    fn messy_handwriting_keyword() {
        let r = classify_document_bytes(b"messy handwriting sample", "txt", "p.txt").unwrap();
        assert_eq!(r[0].triage_class, "messy_handwriting");
    }
}
