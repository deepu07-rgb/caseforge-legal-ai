//! Persistent production offline English speech worker (B-S3-20).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SpeechStatus {
    pub ready: bool,
    pub production_ready: bool,
    pub backend: String,
    pub stt_backend: String,
    pub tts_backend: String,
    pub pipeline_id: String,
    pub whisper_size: String,
    pub whisper_status: String,
    pub primary_language: String,
    pub language_pinning: bool,
    pub tts_roles: Vec<String>,
    pub cloud: bool,
    pub network_required: bool,
    pub real_audio_required: bool,
    pub microphone_capture_required: bool,
    pub developer_stub_enabled: bool,
    pub retention_policy: String,
    pub batch: String,
    pub detail: String,
    pub product_role: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SttRequest {
    /// Developer-only compatibility input; production requires audio_base64.
    pub transcript_stub: Option<String>,
    pub audio_base64: Option<String>,
    pub audio_format: Option<String>,
    /// Must be English / en* (AC-9.4).
    pub language: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SttResult {
    pub raw_transcript: String,
    pub language: String,
    pub language_pinned: bool,
    pub backend: String,
    pub pipeline_id: String,
    pub whisper_size: String,
    pub cloud: bool,
    pub network_used: bool,
    pub elapsed_ms: u64,
    pub confidence: Option<f64>,
    pub input_kind: String,
    pub production_eligible: bool,
    pub audio_sha256: Option<String>,
    pub audio_bytes: u64,
    pub audio_retained: bool,
    pub notes: String,
    pub disclaimer: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TtsRequest {
    pub text: String,
    pub spoken_text: Option<String>,
    pub role: Option<String>,
    pub language: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TtsResult {
    pub role: String,
    pub text: String,
    pub spoken_text: String,
    pub audio_base64: Option<String>,
    pub audio_format: Option<String>,
    pub backend: String,
    pub pipeline_id: String,
    pub cloud: bool,
    pub network_used: bool,
    pub elapsed_ms: u64,
    pub notes: String,
    pub disclaimer: String,
}

fn repo_root_from_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut p = exe.parent()?.to_path_buf();
    for _ in 0..6 {
        if p.join("sidecars")
            .join("infer_speech")
            .join("infer_speech.py")
            .is_file()
        {
            return Some(p);
        }
        if !p.pop() {
            break;
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if cwd
            .join("sidecars")
            .join("infer_speech")
            .join("infer_speech.py")
            .is_file()
        {
            return Some(cwd);
        }
        if cwd
            .parent()
            .map(|p| {
                p.join("sidecars")
                    .join("infer_speech")
                    .join("infer_speech.py")
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
    if let Ok(override_path) = std::env::var("CASEFORGE_INFER_SPEECH_PY") {
        let p = PathBuf::from(override_path);
        if p.is_file() {
            return Ok(p);
        }
        return Err(format!(
            "CASEFORGE_INFER_SPEECH_PY not a file: {}",
            p.display()
        ));
    }
    let root = repo_root_from_exe().ok_or_else(|| {
        "Cannot locate sidecars/infer_speech/infer_speech.py (set CASEFORGE_INFER_SPEECH_PY)"
            .to_string()
    })?;
    let script = root
        .join("sidecars")
        .join("infer_speech")
        .join("infer_speech.py");
    if !script.is_file() {
        return Err(format!("Missing sidecar script: {}", script.display()));
    }
    Ok(script)
}

fn python_cmd() -> String {
    std::env::var("CASEFORGE_PYTHON").unwrap_or_else(|_| {
        if cfg!(windows) {
            "py".into()
        } else {
            "python3".into()
        }
    })
}

struct SpeechWorker {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl SpeechWorker {
    fn spawn() -> Result<Self, String> {
        let script = sidecar_script()?;
        let mut cmd = Command::new(python_cmd());
        if python_cmd() == "py" {
            cmd.arg("-3");
        }
        let mut child = cmd
            .arg(&script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start persistent infer-speech worker: {e}"))?;
        let stdin = child.stdin.take().ok_or("infer-speech stdin unavailable")?;
        let stdout = BufReader::new(
            child
                .stdout
                .take()
                .ok_or("infer-speech stdout unavailable")?,
        );
        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }

    fn call(&mut self, payload: &str) -> Result<Value, String> {
        if let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? {
            return Err(format!("infer-speech exited before request: {status}"));
        }
        self.stdin
            .write_all(payload.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .map_err(|e| format!("infer-speech write failed: {e}"))?;
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|e| format!("infer-speech read failed: {e}"))?;
        if line.trim().is_empty() {
            return Err("infer-speech returned empty stdout".into());
        }
        serde_json::from_str(line.trim()).map_err(|e| {
            format!(
                "infer-speech JSON parse error: {e}; body={}",
                &line[..line.len().min(200)]
            )
        })
    }
}

impl Drop for SpeechWorker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

static SPEECH_WORKER: OnceLock<Mutex<Option<SpeechWorker>>> = OnceLock::new();

fn run_rpc(method: &str, params: Value) -> Result<Value, String> {
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });
    let payload = serde_json::to_string(&req).map_err(|e| e.to_string())?;

    let worker_lock = SPEECH_WORKER.get_or_init(|| Mutex::new(None));
    let mut guard = worker_lock
        .lock()
        .map_err(|_| "infer-speech worker lock poisoned".to_string())?;
    if guard.is_none() {
        *guard = Some(SpeechWorker::spawn()?);
    }
    let mut resp = guard
        .as_mut()
        .ok_or_else(|| "infer-speech worker unavailable".to_string())?
        .call(&payload);
    if resp.is_err() {
        *guard = Some(SpeechWorker::spawn()?);
        resp = guard
            .as_mut()
            .ok_or_else(|| "infer-speech worker restart failed".to_string())?
            .call(&payload);
    }
    let resp = resp?;

    if let Some(err) = resp.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("speech error");
        return Err(msg.to_string());
    }

    resp.get("result")
        .cloned()
        .ok_or_else(|| "infer-speech response missing result".to_string())
}

pub fn status() -> SpeechStatus {
    match run_rpc("speech.status", json!({})) {
        Ok(v) => SpeechStatus {
            ready: v.get("ready").and_then(|x| x.as_bool()).unwrap_or(false),
            production_ready: v
                .get("production_ready")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            backend: v
                .get("backend")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown")
                .into(),
            stt_backend: v
                .get("stt_backend")
                .and_then(|x| x.as_str())
                .unwrap_or("unavailable")
                .into(),
            tts_backend: v
                .get("tts_backend")
                .and_then(|x| x.as_str())
                .unwrap_or("unavailable")
                .into(),
            pipeline_id: v
                .get("pipeline_id")
                .and_then(|x| x.as_str())
                .unwrap_or("caseforge.speech.offline-en.v2")
                .into(),
            whisper_size: v
                .get("whisper_size")
                .and_then(|x| x.as_str())
                .unwrap_or("small")
                .into(),
            whisper_status: v
                .get("whisper_status")
                .and_then(|x| x.as_str())
                .unwrap_or("MODEL_MISSING")
                .into(),
            primary_language: v
                .get("primary_language")
                .and_then(|x| x.as_str())
                .unwrap_or("English")
                .into(),
            language_pinning: v
                .get("language_pinning")
                .and_then(|x| x.as_bool())
                .unwrap_or(true),
            tts_roles: v
                .get("tts_roles")
                .and_then(|x| x.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_else(|| vec!["bench".into(), "opposition".into(), "witness".into()]),
            cloud: v.get("cloud").and_then(|x| x.as_bool()).unwrap_or(false),
            network_required: v
                .get("network_required")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            real_audio_required: v
                .get("real_audio_required")
                .and_then(|x| x.as_bool())
                .unwrap_or(true),
            microphone_capture_required: v
                .get("microphone_capture_required")
                .and_then(|x| x.as_bool())
                .unwrap_or(true),
            developer_stub_enabled: v
                .get("developer_stub_enabled")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            retention_policy: v
                .get("retention_policy")
                .and_then(|x| x.as_str())
                .unwrap_or("Audio is transient and not retained.")
                .into(),
            batch: v
                .get("batch")
                .and_then(|x| x.as_str())
                .unwrap_or("B-S3-20")
                .into(),
            detail: v
                .get("detail")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .into(),
            product_role: v
                .get("product_role")
                .and_then(|x| x.as_str())
                .unwrap_or("Offline STT/TTS")
                .into(),
        },
        Err(e) => SpeechStatus {
            ready: false,
            production_ready: false,
            backend: "none".into(),
            stt_backend: "unavailable".into(),
            tts_backend: "unavailable".into(),
            pipeline_id: "caseforge.speech.offline-en.v2".into(),
            whisper_size: "small".into(),
            whisper_status: "MODEL_MISSING".into(),
            primary_language: "English".into(),
            language_pinning: true,
            tts_roles: vec!["bench".into(), "opposition".into(), "witness".into()],
            cloud: false,
            network_required: false,
            real_audio_required: true,
            microphone_capture_required: true,
            developer_stub_enabled: false,
            retention_policy: "Audio is transient and not retained.".into(),
            batch: "B-S3-20".into(),
            detail: e,
            product_role: "Offline STT/TTS".into(),
        },
    }
}

pub fn status_brief() -> (bool, String) {
    let s = status();
    if s.ready {
        (
            true,
            format!(
                "{} · whisper {} · {} ready",
                s.backend, s.whisper_size, s.primary_language
            ),
        )
    } else {
        (false, format!("offline — {}", s.detail))
    }
}

pub fn warmup() -> Result<Value, String> {
    run_rpc("speech.warmup", json!({}))
}

pub fn stt(req: SttRequest) -> Result<SttResult, String> {
    let mut params = json!({
        "language": req.language.clone().unwrap_or_else(|| "English".into()),
    });
    if let Some(stub) = &req.transcript_stub {
        params["transcript_stub"] = json!(stub);
    }
    if let Some(audio) = &req.audio_base64 {
        params["audio_base64"] = json!(audio);
    }
    if let Some(fmt) = &req.audio_format {
        params["audio_format"] = json!(fmt);
    }
    let v = run_rpc("speech.stt", params)?;
    Ok(SttResult {
        raw_transcript: v
            .get("raw_transcript")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        language: v
            .get("language")
            .and_then(|x| x.as_str())
            .unwrap_or("English")
            .into(),
        language_pinned: v
            .get("language_pinned")
            .and_then(|x| x.as_bool())
            .unwrap_or(true),
        backend: v
            .get("backend")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        pipeline_id: v
            .get("pipeline_id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        whisper_size: v
            .get("whisper_size")
            .and_then(|x| x.as_str())
            .unwrap_or("small")
            .into(),
        cloud: v.get("cloud").and_then(|x| x.as_bool()).unwrap_or(false),
        network_used: v
            .get("network_used")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        elapsed_ms: v.get("elapsed_ms").and_then(|x| x.as_u64()).unwrap_or(0),
        confidence: v.get("confidence").and_then(|x| x.as_f64()),
        input_kind: v
            .get("input_kind")
            .and_then(|x| x.as_str())
            .unwrap_or("UNKNOWN")
            .into(),
        production_eligible: v
            .get("production_eligible")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        audio_sha256: v
            .get("audio_sha256")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        audio_bytes: v.get("audio_bytes").and_then(|x| x.as_u64()).unwrap_or(0),
        audio_retained: v
            .get("audio_retained")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        notes: v.get("notes").and_then(|x| x.as_str()).unwrap_or("").into(),
        disclaimer: v
            .get("disclaimer")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
    })
}

pub fn tts(req: TtsRequest) -> Result<TtsResult, String> {
    let params = json!({
        "text": req.text,
        "spoken_text": req.spoken_text.clone().unwrap_or_else(|| req.text.clone()),
        "role": req.role.clone().unwrap_or_else(|| "bench".into()),
        "language": req.language.clone().unwrap_or_else(|| "English".into()),
    });
    let v = run_rpc("speech.tts", params)?;
    Ok(TtsResult {
        role: v
            .get("role")
            .and_then(|x| x.as_str())
            .unwrap_or("bench")
            .into(),
        text: v.get("text").and_then(|x| x.as_str()).unwrap_or("").into(),
        spoken_text: v
            .get("spoken_text")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        audio_base64: v
            .get("audio_base64")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        audio_format: v
            .get("audio_format")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        backend: v
            .get("backend")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        pipeline_id: v
            .get("pipeline_id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        cloud: v.get("cloud").and_then(|x| x.as_bool()).unwrap_or(false),
        network_used: v
            .get("network_used")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        elapsed_ms: v.get("elapsed_ms").and_then(|x| x.as_u64()).unwrap_or(0),
        notes: v.get("notes").and_then(|x| x.as_str()).unwrap_or("").into(),
        disclaimer: v
            .get("disclaimer")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
    })
}
