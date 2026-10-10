//! B-S3-03 PDFium adapter. Document bytes travel over local stdio only; no
//! plaintext temporary file is created and passwords are never included in errors.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub const PDF_PARSER_ID: &str = "caseforge.pdfium.page-source.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPdfPage {
    pub schema_version: String,
    pub doc_id: String,
    pub page_index: i64,
    pub source_kind: String,
    pub state: String,
    pub width_points: f64,
    pub height_points: f64,
    pub rotation_degrees: i64,
    pub native_text: String,
    pub text_spans: Value,
    pub layout_blocks: Value,
    pub ocr: Value,
    pub capabilities: Value,
    pub parser_id: String,
    pub source_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPdfDocument {
    pub schema_version: String,
    pub parser_id: String,
    pub document_state: String,
    pub source_sha256: String,
    pub page_count: usize,
    pub pages: Vec<ParsedPdfPage>,
    pub checkpoint: Value,
}

fn script_path() -> Result<PathBuf, String> {
    if let Ok(value) = std::env::var("CASEFORGE_DOCPIPE_PY") {
        let path = PathBuf::from(value);
        return path.is_file().then_some(path).ok_or_else(|| {
            "CF_DOCPIPE_RUNTIME_MISSING: configured docpipe script does not exist".into()
        });
    }
    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.clone());
        if let Some(parent) = cwd.parent() {
            candidates.push(parent.to_path_buf());
        }
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."));
    if let Ok(exe) = std::env::current_exe() {
        let mut current = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..7 {
            if let Some(path) = current.clone() {
                candidates.push(path.clone());
                current = path.parent().map(|p| p.to_path_buf());
            }
        }
    }
    for root in candidates {
        let script = root.join("sidecars").join("docpipe").join("docpipe.py");
        if script.is_file() {
            return Ok(script);
        }
    }
    Err("CF_DOCPIPE_RUNTIME_MISSING: cannot locate sidecars/docpipe/docpipe.py".into())
}

fn python_command() -> String {
    std::env::var("CASEFORGE_PYTHON").unwrap_or_else(|_| {
        if cfg!(windows) {
            "py".into()
        } else {
            "python3".into()
        }
    })
}

pub fn parse_pdf(
    bytes: &[u8],
    doc_id: &str,
    password: Option<&str>,
) -> Result<ParsedPdfDocument, String> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let request = json!({"jsonrpc":"2.0","id":1,"method":"docpipe.parse_pdf","params":{
        "content_base64": encoded, "doc_id": doc_id, "password": password,
        "render_ocr_pages": false, "resume_from_page": 0
    }});
    let payload = serde_json::to_string(&request)
        .map_err(|_| "CF_PDF_INPUT_INVALID: failed to encode parser request".to_string())?;
    let python = python_command();
    let mut command = Command::new(&python);
    if python == "py" {
        command.arg("-3");
    }
    let mut child = command
        .arg(script_path()?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "CF_DOCPIPE_RUNTIME_MISSING: failed to start local PDF parser".to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload.as_bytes())
            .map_err(|_| "CF_PDF_PARSE_FAILED: could not send document to parser".to_string())?;
        stdin
            .write_all(b"\n")
            .map_err(|_| "CF_PDF_PARSE_FAILED: could not finish parser request".to_string())?;
    }
    let output = child
        .wait_with_output()
        .map_err(|_| "CF_PDF_PARSE_FAILED: local PDF parser did not finish".to_string())?;
    if !output.status.success() {
        return Err("CF_PDF_PARSE_FAILED: local PDF parser exited unexpectedly".into());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| "CF_PDF_PARSE_FAILED: parser returned no response".to_string())?;
    let response: Value = serde_json::from_str(line)
        .map_err(|_| "CF_PDF_PARSE_FAILED: parser returned invalid response".to_string())?;
    if let Some(error) = response.get("error") {
        let code = error
            .get("data")
            .and_then(|d| d.get("code"))
            .and_then(Value::as_str)
            .unwrap_or("CF_PDF_PARSE_FAILED");
        let state = error
            .get("data")
            .and_then(|d| d.get("state"))
            .and_then(Value::as_str)
            .unwrap_or("unsupported");
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("PDF could not be parsed");
        return Err(format!("{code}: {message} [state={state}]"));
    }
    serde_json::from_value(response.get("result").cloned().unwrap_or(Value::Null))
        .map_err(|_| "CF_PDF_PARSE_FAILED: parser result violated page-source contract".to_string())
}

pub fn recognize_document(
    bytes: &[u8],
    source_format: &str,
    filename: &str,
) -> Result<Vec<crate::docpipe::recognize::PageTextLayerResult>, String> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let request = json!({"jsonrpc":"2.0","id":1,"method":"docpipe.recognize_document","params":{
        "content_base64": encoded, "media_type": source_format, "filename": filename
    }});
    let payload = serde_json::to_string(&request)
        .map_err(|_| "CF_OCR_INPUT_INVALID: failed to encode recognition request".to_string())?;
    let python = python_command();
    let mut command = Command::new(&python);
    if python == "py" {
        command.arg("-3");
    }
    let mut child = command
        .arg(script_path()?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "CF_OCR_RUNTIME_MISSING: failed to start local OCR adapter".to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload.as_bytes())
            .map_err(|_| "CF_OCR_FAILED: could not send document to OCR".to_string())?;
        stdin
            .write_all(b"\n")
            .map_err(|_| "CF_OCR_FAILED: could not finish OCR request".to_string())?;
    }
    let output = child
        .wait_with_output()
        .map_err(|_| "CF_OCR_FAILED: local OCR did not finish".to_string())?;
    if !output.status.success() {
        return Err("CF_OCR_FAILED: local OCR exited unexpectedly".into());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| "CF_OCR_FAILED: OCR returned no response".to_string())?;
    let response: Value = serde_json::from_str(line)
        .map_err(|_| "CF_OCR_FAILED: OCR returned invalid response".to_string())?;
    if let Some(error) = response.get("error") {
        let code = error
            .get("data")
            .and_then(|d| d.get("code"))
            .and_then(Value::as_str)
            .unwrap_or("CF_OCR_FAILED");
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("OCR could not recognize the document");
        return Err(format!("{code}: {message}"));
    }
    serde_json::from_value(
        response
            .get("result")
            .and_then(|r| r.get("pages"))
            .cloned()
            .unwrap_or(Value::Null),
    )
    .map_err(|_| "CF_OCR_FAILED: OCR result violated text-layer contract".to_string())
}

pub fn render_page(
    bytes: &[u8],
    source_format: &str,
    filename: &str,
    page_index: i64,
) -> Result<Value, String> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let request = json!({"jsonrpc":"2.0","id":1,"method":"docpipe.render_page","params":{
        "content_base64": encoded, "media_type": source_format, "filename": filename, "page_index": page_index
    }});
    let payload = serde_json::to_string(&request)
        .map_err(|_| "CF_OCR_INPUT_INVALID: failed to encode preview request".to_string())?;
    let python = python_command();
    let mut command = Command::new(&python);
    if python == "py" {
        command.arg("-3");
    }
    let mut child = command
        .arg(script_path()?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "CF_OCR_RUNTIME_MISSING: failed to start page renderer".to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload.as_bytes())
            .map_err(|_| "CF_OCR_FAILED: could not send preview request".to_string())?;
        stdin
            .write_all(b"\n")
            .map_err(|_| "CF_OCR_FAILED: could not finish preview request".to_string())?;
    }
    let output = child
        .wait_with_output()
        .map_err(|_| "CF_OCR_FAILED: page renderer did not finish".to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| "CF_OCR_FAILED: renderer returned no response".to_string())?;
    let response: Value = serde_json::from_str(line)
        .map_err(|_| "CF_OCR_FAILED: renderer returned invalid response".to_string())?;
    if let Some(error) = response.get("error") {
        let code = error
            .get("data")
            .and_then(|d| d.get("code"))
            .and_then(Value::as_str)
            .unwrap_or("CF_OCR_FAILED");
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("page preview unavailable");
        return Err(format!("{code}: {message}"));
    }
    Ok(response.get("result").cloned().unwrap_or(Value::Null))
}

/// One JSON-RPC request to the document sidecar over local stdio, for calls that carry text (B-S4-15 translation steps). Nothing is written to disk.
pub fn docpipe_rpc(method: &str, params: Value) -> Result<Value, String> {
    let request = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params});
    let payload = serde_json::to_string(&request).map_err(|_| "CF_DOCPIPE_INPUT_INVALID: failed to encode request".to_string())?;
    let python = python_command();
    let mut command = Command::new(&python);
    if python == "py" {
        command.arg("-3");
    }
    let mut child = command
        .arg(script_path()?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "CF_DOCPIPE_RUNTIME_MISSING: failed to start the local document sidecar".to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(payload.as_bytes()).map_err(|_| "CF_DOCPIPE_FAILED: could not send the request".to_string())?;
        stdin.write_all(b"\n").map_err(|_| "CF_DOCPIPE_FAILED: could not finish the request".to_string())?;
    }
    let output = child.wait_with_output().map_err(|_| "CF_DOCPIPE_FAILED: the document sidecar did not finish".to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().find(|l| !l.trim().is_empty()).ok_or_else(|| "CF_DOCPIPE_FAILED: the document sidecar returned no response".to_string())?;
    let response: Value = serde_json::from_str(line).map_err(|_| "CF_DOCPIPE_FAILED: the document sidecar returned an invalid response".to_string())?;
    if let Some(error) = response.get("error") {
        let code = error.get("data").and_then(|d| d.get("code")).and_then(Value::as_str).unwrap_or("CF_DOCPIPE_FAILED");
        let message = error.get("message").and_then(Value::as_str).unwrap_or("the document sidecar refused the request");
        return Err(format!("{code}: {message}"));
    }
    Ok(response.get("result").cloned().unwrap_or(Value::Null))
}
