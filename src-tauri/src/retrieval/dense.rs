//! Dense (meaning-based) retrieval — B-S4-27.
//!
//! Packs may ship passage vectors (metadata/embeddings.e5s.i8 + embeddings.json, signed through the manifest) made with
//! the offline multilingual-e5-small model. They are registered when the corpus is built. A query is embedded with the
//! same model by the local Python helper (sidecars/docpipe/embed.py), kept running between searches; if the model or
//! helper is unavailable, dense retrieval is simply off and lexical/legal-concept retrieval runs unchanged.
//!
//! Memory: one contiguous int8 block per pack (384 bytes per passage) plus a 64-bit key per passage.

use once_cell::sync::Lazy;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

pub const MODEL_ID: &str = "intfloat/multilingual-e5-small@614241f";
const FIRST_ANSWER: Duration = Duration::from_secs(60); // model load on a slow disk
const LATER_ANSWER: Duration = Duration::from_secs(15);
const BACK_OFF: Duration = Duration::from_secs(300);

#[derive(Default)]
struct Store {
    dims: usize,
    blocks: Vec<Vec<i8>>,
    rows: HashMap<u64, (u32, u32)>,
}

static STORE: Lazy<RwLock<Store>> = Lazy::new(|| RwLock::new(Store::default()));
static LOADED: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));
static QUERIES: Lazy<Mutex<HashMap<String, Vec<f32>>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static UNAVAILABLE_UNTIL: Lazy<Mutex<Option<Instant>>> = Lazy::new(|| Mutex::new(None));
static HELPER: Lazy<Mutex<Option<Helper>>> = Lazy::new(|| Mutex::new(None));

pub(crate) fn key(chunk_id: &str) -> u64 {
    let d = Sha256::digest(chunk_id.as_bytes());
    u64::from_le_bytes(d[..8].try_into().unwrap_or([0; 8]))
}

fn insert_block(dims: usize, chunk_ids: &[String], block: Vec<i8>) {
    if let Ok(mut s) = STORE.write() {
        if s.dims != 0 && s.dims != dims {
            eprintln!("dense: ignoring {dims}-dim vectors (store holds {}-dim)", s.dims);
            return;
        }
        s.dims = dims;
        let b = s.blocks.len() as u32;
        for (i, id) in chunk_ids.iter().enumerate() {
            s.rows.insert(key(id), (b, i as u32));
        }
        s.blocks.push(block);
    }
}

/// Registers a pack's passage vectors (row i = i-th chunk of legal_chunks.json). Rejects any mismatch of model,
/// row count or chunk order, so a stale or foreign vector file is never used.
pub fn register_pack(pack_dir: &Path, content_sha256: &str, chunk_ids: &[String]) {
    if !LOADED.lock().map(|mut l| l.insert(content_sha256.to_string())).unwrap_or(false) {
        return;
    }
    let Ok(meta) = std::fs::read_to_string(pack_dir.join("metadata/embeddings.json")).map(|s| serde_json::from_str::<Value>(&s).unwrap_or(Value::Null)) else { return };
    let dims = meta["dims"].as_u64().unwrap_or(0) as usize;
    let order = hex::encode(Sha256::digest(chunk_ids.join("\n").as_bytes()));
    if meta["model"].as_str() != Some(MODEL_ID) || meta["rows"].as_u64() != Some(chunk_ids.len() as u64) || meta["chunk_ids_sha256"].as_str() != Some(order.as_str()) || dims == 0 {
        eprintln!("dense: ignoring vectors in {} (model/rows/order mismatch)", pack_dir.display());
        return;
    }
    let Ok(bytes) = std::fs::read(pack_dir.join("metadata/embeddings.e5s.i8")) else { return };
    if bytes.len() != dims * chunk_ids.len() {
        return;
    }
    insert_block(dims, chunk_ids, bytes.into_iter().map(|b| b as i8).collect());
}

/// The `k` passages nearest to the query among those `keep` accepts (by key), similarity > 0, best first.
pub(crate) fn scan(query: &[f32], k: usize, keep: &dyn Fn(u64) -> bool) -> Vec<(u64, f32)> {
    let Ok(s) = STORE.read() else { return Vec::new() };
    if query.len() != s.dims {
        return Vec::new();
    }
    let mut found: Vec<(u64, f32)> = s
        .rows
        .iter()
        .filter(|(h, _)| keep(**h))
        .filter_map(|(h, &(b, r))| {
            let row = s.blocks.get(b as usize)?.get(r as usize * s.dims..(r as usize + 1) * s.dims)?;
            let sim = row.iter().zip(query).map(|(a, q)| *a as f32 * q).sum::<f32>() / 127.0;
            (sim > 0.0).then_some((*h, sim))
        })
        .collect();
    found.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    found.truncate(k);
    found
}

pub fn has_vectors() -> bool {
    STORE.read().map(|s| !s.rows.is_empty()).unwrap_or(false)
}

/// Cosine similarity of a unit query vector with a stored int8 unit vector (scale 127).
pub fn similarity(query: &[f32], chunk_id: &str) -> Option<f32> {
    let s = STORE.read().ok()?;
    if query.len() != s.dims {
        return None;
    }
    let (b, r) = *s.rows.get(&key(chunk_id))?;
    let v = s.blocks.get(b as usize)?.get(r as usize * s.dims..(r as usize + 1) * s.dims)?;
    Some(v.iter().zip(query).map(|(a, q)| *a as f32 * q).sum::<f32>() / 127.0)
}

fn script_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CASEFORGE_EMBED_PY") {
        return Some(PathBuf::from(p)).filter(|p| p.is_file());
    }
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."));
    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent().map(Path::to_path_buf);
        for _ in 0..7 {
            if let Some(p) = cur.clone() {
                roots.push(p.clone());
                cur = p.parent().map(Path::to_path_buf);
            }
        }
    }
    roots.into_iter().map(|r| r.join("sidecars").join("docpipe").join("embed.py")).find(|p| p.is_file())
}

/// The embedding helper process, answering one JSON-RPC line per request.
struct Helper {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    answered: bool,
    next_id: u64,
}

impl Helper {
    fn start() -> Result<Helper, String> {
        let script = script_path().ok_or("embed helper missing")?;
        let python = std::env::var("CASEFORGE_PYTHON").unwrap_or_else(|_| if cfg!(windows) { "py".into() } else { "python3".into() });
        let mut cmd = Command::new(&python);
        if python == "py" {
            cmd.arg("-3");
        }
        cmd.arg(&script).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd.spawn().map_err(|e| format!("cannot start {python}: {e}"))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Helper { child, stdin, lines, answered: false, next_id: 1 })
    }

    fn ask(&mut self, text: &str) -> Result<Vec<f32>, String> {
        let id = self.next_id;
        self.next_id += 1;
        let req = serde_json::json!({"jsonrpc": "2.0", "id": id, "method": "embed.query", "params": {"text": text}});
        writeln!(self.stdin, "{req}").and_then(|_| self.stdin.flush()).map_err(|e| e.to_string())?;
        let wait = if self.answered { LATER_ANSWER } else { FIRST_ANSWER };
        let line = self.lines.recv_timeout(wait).map_err(|_| "embed helper did not answer in time".to_string())?;
        let v: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        if v["id"].as_u64() != Some(id) || v["result"]["model"].as_str() != Some(MODEL_ID) {
            return Err(format!("embed helper: {}", v["error"]["message"].as_str().unwrap_or("unexpected answer")));
        }
        self.answered = true;
        Ok(v["result"]["vector"].as_array().ok_or("no vector")?.iter().filter_map(Value::as_f64).map(|x| x as f32).collect())
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Embeds a query with the local model, or None if dense retrieval is unavailable (then retried after 5 minutes).
pub fn embed_query(text: &str) -> Option<Vec<f32>> {
    if let Some(v) = QUERIES.lock().ok()?.get(text) {
        return Some(v.clone());
    }
    if UNAVAILABLE_UNTIL.lock().ok()?.map(|t| Instant::now() < t).unwrap_or(false) {
        return None;
    }
    let result = (|| -> Result<Vec<f32>, String> {
        let mut slot = HELPER.lock().map_err(|_| "embed helper lock poisoned".to_string())?;
        if slot.is_none() {
            *slot = Some(Helper::start()?);
        }
        let answer = slot.as_mut().ok_or("no helper")?.ask(text);
        if answer.is_err() {
            *slot = None; // a stuck or failed helper is stopped; the next attempt starts a fresh one
        }
        answer
    })();
    match result {
        Ok(v) => {
            if let Ok(mut q) = QUERIES.lock() {
                if q.len() > 256 {
                    q.clear();
                }
                q.insert(text.to_string(), v.clone());
            }
            Some(v)
        }
        Err(e) => {
            eprintln!("dense retrieval off: {e}");
            if let Ok(mut u) = UNAVAILABLE_UNTIL.lock() {
                *u = Some(Instant::now() + BACK_OFF);
            }
            None
        }
    }
}

#[cfg(test)]
pub(crate) fn register_for_test(chunk_id: &str, v: Vec<i8>) {
    insert_block(v.len(), &[chunk_id.to_string()], v);
}
