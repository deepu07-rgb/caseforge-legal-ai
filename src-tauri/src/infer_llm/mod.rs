//! Local infer-llm host — spawns Python sidecar (Ollama / GGUF / OpenAI-compat).
//! PRD §4.1: offline reasoner; one model path; no cloud.

use crate::proposition::{PropositionInput, PropositionLedger, SupportSource};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

/// B-S4-00 model tiers: `default` ships in the base install; `addon` is the optional larger
/// model. The sidecar enforces the add-on RAM floor and fails closed on unknown tiers.
pub const MODEL_TIERS: [&str; 2] = ["default", "addon"];

/// User-selected tier for this app session. `None` = environment or `default`. Not persisted:
/// every restart returns to the base-install tier.
static MODEL_TIER: Mutex<Option<String>> = Mutex::new(None);

pub fn current_tier() -> String {
    if let Some(tier) = MODEL_TIER.lock().ok().and_then(|t| t.clone()) {
        return tier;
    }
    std::env::var("CASEFORGE_LLM_TIER")
        .ok()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "default".into())
}

pub fn set_tier(tier: &str) -> Result<String, String> {
    let tier = tier.trim().to_ascii_lowercase();
    if !MODEL_TIERS.contains(&tier.as_str()) {
        return Err(format!("CF_LLM_TIER_UNKNOWN: {tier:?}; expected one of {MODEL_TIERS:?}"));
    }
    *MODEL_TIER.lock().map_err(|e| e.to_string())? = Some(tier.clone());
    Ok(tier)
}

/// Tier catalogue from the sidecar (`llm.tiers`): models, licences, install policy.
pub fn tiers() -> Result<Value, String> {
    run_rpc("llm.tiers", json!({}), 15)
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LlmStatus {
    pub ready: bool,
    pub controlled_beta_ready: bool,
    pub backend: String,
    pub backend_version: String,
    pub model: String,
    pub model_digest: String,
    pub quantization: String,
    pub parameters: String,
    pub context_length: u64,
    pub classification: String,
    pub license: String,
    pub legally_fine_tuned: bool,
    pub checkpoint_evidence: bool,
    pub prompt_version: String,
    pub system_prompt_hash: String,
    pub generation: Value,
    pub detail: String,
    pub cloud: bool,
    pub product_role: String,
    /// B-S4-00: active model tier and, if the tier cannot be used, why.
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub tier_error: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LlmChatRequest {
    pub user_text: String,
    pub role: Option<String>,
    pub format: Option<String>,
    pub case_title: Option<String>,
    pub jurisdiction: Option<String>,
    pub user_side: Option<String>,
    /// Free-text matter file excerpt for the local reasoner.
    pub case_context: Option<String>,
    /// Exact retrieved authority text selected from the matter working set.
    pub authority_context: Option<String>,
    /// Stable authority identifiers permitted for citation in this turn.
    pub authority_ids: Option<Vec<String>>,
    /// Version of the encrypted matter working set used for this output.
    pub working_set_version: Option<String>,
    /// B-S3-14: controlled generation must return a structured proposition ledger candidate.
    pub structured_propositions: Option<bool>,
    pub output_kind: Option<String>,
    pub support_sources: Option<Vec<SupportSource>>,
    pub permitted_authority_ids: Option<Vec<String>>,
    /// B-S3-16: require the Mock Court structured-turn v2 envelope.
    pub mock_court_schema_v2: Option<bool>,
    /// Prior turns: [{role, content}, ...]
    pub messages: Option<Vec<LlmMessage>>,
    pub max_tokens: Option<u32>,
    /// B-S4-28: best-of-n sample number (0 = the contract's settings, 1-7 = deterministic seed/temperature variants).
    #[serde(default)]
    pub variant: Option<u32>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LlmChatResult {
    pub content: String,
    pub model: String,
    pub backend: String,
    pub elapsed_ms: u64,
    pub role: String,
    pub format: String,
    pub disclaimer: String,
    pub source: String,
    pub controlled_beta_ready: bool,
    pub provenance: Value,
    pub eval_count: Option<u64>,
    pub tokens_per_second: Option<f64>,
    pub propositions: Vec<PropositionInput>,
    pub proposition_ledger: Option<PropositionLedger>,
    pub mock_court_turn: Option<Value>,
}

fn repo_root_from_exe() -> Option<PathBuf> {
    // dev: target/debug/caseforge.exe → ../../../
    let exe = std::env::current_exe().ok()?;
    let mut p = exe.parent()?.to_path_buf();
    for _ in 0..6 {
        if p.join("sidecars")
            .join("infer_llm")
            .join("infer_llm.py")
            .is_file()
        {
            return Some(p);
        }
        if !p.pop() {
            break;
        }
    }
    // cwd fallback
    if let Ok(cwd) = std::env::current_dir() {
        if cwd
            .join("sidecars")
            .join("infer_llm")
            .join("infer_llm.py")
            .is_file()
        {
            return Some(cwd);
        }
        if cwd
            .parent()
            .map(|p| {
                p.join("sidecars")
                    .join("infer_llm")
                    .join("infer_llm.py")
                    .is_file()
            })
            .unwrap_or(false)
        {
            return cwd.parent().map(|p| p.to_path_buf());
        }
    }
    None
}

fn sidecar_script() -> Result<PathBuf, String> {
    if let Ok(override_path) = std::env::var("CASEFORGE_INFER_LLM_PY") {
        let p = PathBuf::from(override_path);
        if p.is_file() {
            return Ok(p);
        }
        return Err(format!(
            "CASEFORGE_INFER_LLM_PY not a file: {}",
            p.display()
        ));
    }
    let root = repo_root_from_exe().ok_or_else(|| {
        "Cannot locate sidecars/infer_llm/infer_llm.py (set CASEFORGE_INFER_LLM_PY)".to_string()
    })?;
    let script = root.join("sidecars").join("infer_llm").join("infer_llm.py");
    if !script.is_file() {
        return Err(format!("Missing sidecar script: {}", script.display()));
    }
    Ok(script)
}

fn python_cmd() -> String {
    std::env::var("CASEFORGE_PYTHON").unwrap_or_else(|_| {
        // Prefer py launcher on Windows, else python
        if cfg!(windows) {
            "py".into()
        } else {
            "python3".into()
        }
    })
}

fn run_rpc(method: &str, params: Value, timeout_secs: u64) -> Result<Value, String> {
    let script = sidecar_script()?;
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });
    let payload = serde_json::to_string(&req).map_err(|e| e.to_string())?;

    let mut cmd = Command::new(python_cmd());
    if python_cmd() == "py" {
        cmd.arg("-3");
    }
    cmd.env("CASEFORGE_LLM_TIER", current_tier())
        .arg(&script)
        .arg("--once")
        .arg(&payload)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // On Windows, CREATE_NO_WINDOW is nice but needs extra crate; skip.

    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "Failed to spawn Python infer-llm ({}) : {}. Install Python 3 or set CASEFORGE_PYTHON.",
            python_cmd(),
            e
        )
    })?;

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < Duration::from_secs(timeout_secs) => {
                thread::sleep(Duration::from_millis(25));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("CF_LLM_TIMEOUT: local model exceeded {timeout_secs}s; no partial output rendered"));
            }
            Err(e) => return Err(format!("infer-llm status failed: {e}")),
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("infer-llm wait failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "infer-llm exited {}: {}",
            output.status,
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| "infer-llm returned empty stdout".to_string())?;

    let resp: Value = serde_json::from_str(line).map_err(|e| {
        format!(
            "infer-llm JSON parse error: {e}; body={}",
            &line[..line.len().min(200)]
        )
    })?;

    if let Some(err) = resp.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("LLM error");
        let data = err.get("data").cloned().unwrap_or(Value::Null);
        return Err(format!("{msg} | {data}"));
    }

    resp.get("result")
        .cloned()
        .ok_or_else(|| "infer-llm response missing result".to_string())
}

pub fn status() -> LlmStatus {
    match run_rpc("llm.status", json!({}), 15) {
        Ok(v) => LlmStatus {
            ready: v.get("ready").and_then(|x| x.as_bool()).unwrap_or(false),
            controlled_beta_ready: v
                .get("controlled_beta_ready")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            backend: v
                .get("backend")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown")
                .into(),
            backend_version: v
                .get("backend_version")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown")
                .into(),
            model: v.get("model").and_then(|x| x.as_str()).unwrap_or("").into(),
            model_digest: v
                .get("model_digest")
                .and_then(|x| x.as_str())
                .unwrap_or("unverified")
                .into(),
            quantization: v
                .get("quantization")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown")
                .into(),
            parameters: v
                .get("parameters")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown")
                .into(),
            context_length: v
                .get("context_length")
                .and_then(|x| x.as_u64())
                .unwrap_or(0),
            classification: v
                .get("classification")
                .and_then(|x| x.as_str())
                .unwrap_or("unvalidated")
                .into(),
            license: v
                .get("license")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown")
                .into(),
            legally_fine_tuned: v
                .get("legally_fine_tuned")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            checkpoint_evidence: v
                .get("checkpoint_evidence")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            prompt_version: v
                .get("prompt_version")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown")
                .into(),
            system_prompt_hash: v
                .get("system_prompt_hash")
                .and_then(|x| x.as_str())
                .unwrap_or("unverified")
                .into(),
            generation: v.get("generation").cloned().unwrap_or(Value::Null),
            detail: v
                .get("detail")
                .map(|d| d.to_string())
                .unwrap_or_else(|| v.to_string()),
            cloud: false,
            product_role: v
                .get("product_role")
                .and_then(|x| x.as_str())
                .unwrap_or("Primary local reasoner")
                .into(),
            tier: v
                .get("tier")
                .and_then(|x| x.as_str())
                .unwrap_or("default")
                .into(),
            tier_error: v
                .get("tier_error")
                .and_then(|x| x.as_str())
                .map(String::from),
        },
        Err(e) => LlmStatus {
            ready: false,
            controlled_beta_ready: false,
            backend: "none".into(),
            backend_version: "unknown".into(),
            model: String::new(),
            model_digest: "unverified".into(),
            quantization: "unknown".into(),
            parameters: "unknown".into(),
            context_length: 0,
            classification: "unavailable".into(),
            license: "unknown".into(),
            legally_fine_tuned: false,
            checkpoint_evidence: false,
            prompt_version: "unknown".into(),
            system_prompt_hash: "unverified".into(),
            generation: Value::Null,
            detail: e,
            cloud: false,
            product_role: "Primary local reasoner".into(),
            tier: current_tier(),
            tier_error: None,
        },
    }
}

pub fn chat(req: LlmChatRequest) -> Result<LlmChatResult, String> {
    crate::edition::check_generation()?; // B-S4-17: Chambers reaches the model only through record-only reasoning
    let mut params = json!({
        "user_text": req.user_text,
        "role": req.role.clone().unwrap_or_else(|| "counsel".into()),
        "format": req.format.clone().unwrap_or_else(|| "oral_argument".into()),
        "case_title": req.case_title.clone().unwrap_or_default(),
        "jurisdiction": req.jurisdiction.clone().unwrap_or_default(),
        "user_side": req.user_side.clone().unwrap_or_default(),
        "case_context": req.case_context.clone().unwrap_or_default(),
        "authority_context": req.authority_context.clone().unwrap_or_default(),
        "authority_ids": req.authority_ids.clone().unwrap_or_default(),
        "working_set_version": req.working_set_version.clone().unwrap_or_default(),
        "structured_propositions": req.structured_propositions.unwrap_or(false),
        "output_kind": req.output_kind.clone().unwrap_or_else(|| "REASONING".into()),
        "support_sources": req.support_sources.clone().unwrap_or_default(),
        "permitted_authority_ids": req.permitted_authority_ids.clone().unwrap_or_default(),
        "mock_court_schema_v2": req.mock_court_schema_v2.unwrap_or(false),
        "max_tokens": req.max_tokens.unwrap_or(512),
        "variant": req.variant.unwrap_or(0),
    });
    if let Some(msgs) = &req.messages {
        params["messages"] = serde_json::to_value(msgs).unwrap_or(json!([]));
    }

    let v = run_rpc("llm.chat", params, 300)?;
    Ok(LlmChatResult {
        content: v
            .get("content")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        model: v.get("model").and_then(|x| x.as_str()).unwrap_or("").into(),
        backend: v
            .get("backend")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        elapsed_ms: v.get("elapsed_ms").and_then(|x| x.as_u64()).unwrap_or(0),
        role: v
            .get("role")
            .and_then(|x| x.as_str())
            .unwrap_or("counsel")
            .into(),
        format: v
            .get("format")
            .and_then(|x| x.as_str())
            .unwrap_or("oral_argument")
            .into(),
        disclaimer: v
            .get("disclaimer")
            .and_then(|x| x.as_str())
            .unwrap_or("Local model output for preparation only.")
            .into(),
        source: "infer-llm".into(),
        controlled_beta_ready: v
            .get("controlled_beta_ready")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        provenance: v.get("provenance").cloned().unwrap_or(Value::Null),
        eval_count: v.get("eval_count").and_then(|x| x.as_u64()),
        tokens_per_second: v.get("tokens_per_second").and_then(|x| x.as_f64()),
        propositions: v
            .get("propositions")
            .cloned()
            .and_then(|x| serde_json::from_value(x).ok())
            .unwrap_or_default(),
        proposition_ledger: None,
        mock_court_turn: v
            .get("mock_court_turn")
            .cloned()
            .filter(|value| !value.is_null()),
    })
}

pub fn chat_with_timeout(req: LlmChatRequest, timeout_secs: u64) -> Result<LlmChatResult, String> {
    crate::edition::check_generation()?; // B-S4-17
    let mut params = json!({
        "user_text": req.user_text,
        "role": req.role.clone().unwrap_or_else(|| "counsel".into()),
        "format": req.format.clone().unwrap_or_else(|| "oral_argument".into()),
        "case_title": req.case_title.clone().unwrap_or_default(),
        "jurisdiction": req.jurisdiction.clone().unwrap_or_default(),
        "user_side": req.user_side.clone().unwrap_or_default(),
        "case_context": req.case_context.clone().unwrap_or_default(),
        "authority_context": req.authority_context.clone().unwrap_or_default(),
        "authority_ids": req.authority_ids.clone().unwrap_or_default(),
        "working_set_version": req.working_set_version.clone().unwrap_or_default(),
        "structured_propositions": req.structured_propositions.unwrap_or(false),
        "output_kind": req.output_kind.clone().unwrap_or_else(|| "REASONING".into()),
        "support_sources": req.support_sources.clone().unwrap_or_default(),
        "permitted_authority_ids": req.permitted_authority_ids.clone().unwrap_or_default(),
        "mock_court_schema_v2": req.mock_court_schema_v2.unwrap_or(false),
        "max_tokens": req.max_tokens.unwrap_or(512),
        "variant": req.variant.unwrap_or(0),
    });
    if let Some(msgs) = &req.messages {
        params["messages"] = serde_json::to_value(msgs).unwrap_or(json!([]));
    }
    let v = run_rpc("llm.chat", params, timeout_secs)?;
    Ok(LlmChatResult {
        content: v
            .get("content")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        model: v.get("model").and_then(|x| x.as_str()).unwrap_or("").into(),
        backend: v
            .get("backend")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        elapsed_ms: v.get("elapsed_ms").and_then(|x| x.as_u64()).unwrap_or(0),
        role: v
            .get("role")
            .and_then(|x| x.as_str())
            .unwrap_or("counsel")
            .into(),
        format: v
            .get("format")
            .and_then(|x| x.as_str())
            .unwrap_or("oral_argument")
            .into(),
        disclaimer: v
            .get("disclaimer")
            .and_then(|x| x.as_str())
            .unwrap_or("Local model output for preparation only.")
            .into(),
        source: "infer-llm".into(),
        controlled_beta_ready: v
            .get("controlled_beta_ready")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        provenance: v.get("provenance").cloned().unwrap_or(Value::Null),
        eval_count: v.get("eval_count").and_then(|x| x.as_u64()),
        tokens_per_second: v.get("tokens_per_second").and_then(|x| x.as_f64()),
        propositions: v
            .get("propositions")
            .cloned()
            .and_then(|x| serde_json::from_value(x).ok())
            .unwrap_or_default(),
        proposition_ledger: None,
        mock_court_turn: v
            .get("mock_court_turn")
            .cloned()
            .filter(|value| !value.is_null()),
    })
}

/// Quick path used when only checking readiness for shell_sidecar_status.
pub fn status_brief() -> (bool, String) {
    let s = status();
    if s.ready {
        (true, format!("{} · {} ready", s.backend, s.model))
    } else {
        (
            false,
            format!(
                "not ready — {}",
                s.detail.chars().take(160).collect::<String>()
            ),
        )
    }
}

#[allow(dead_code)]
pub fn script_exists() -> bool {
    sidecar_script()
        .map(|p| Path::new(&p).is_file())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_selection_is_validated_and_never_silently_falls_back() {
        assert!(set_tier("bogus").is_err());
        assert!(set_tier("").is_err());
        assert_eq!(set_tier(" ADDON ").unwrap(), "addon");
        assert_eq!(current_tier(), "addon");
        assert_eq!(set_tier("default").unwrap(), "default");
        assert_eq!(current_tier(), "default");
    }
}

/// B-S4-15: one masked Hindi segment in, English prose out. A fixed translation prompt, greedy decoding, no case context (the reasoner's
/// prompt is not involved). Edition rule B-S4-17 applies: Chambers reaches the model only through record-only reasoning.
pub fn translate(text: &str) -> Result<String, String> {
    crate::edition::check_generation()?;
    let v = run_rpc("llm.translate", json!({ "text": text }), 300)?;
    v.get("content")
        .and_then(|x| x.as_str())
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "CF_LLM_EMPTY: the model returned no translation".to_string())
}
