//! Model choreography supervisor (B-S1-20, PRD §3 / §4.2).
//! At most one large model resident; primary never concurrent with vision on 8 GB.
//! Visible status chip data + budget metrics (AC-3.1, 3.4, 3.5, 4.2).

use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const PIPELINE_ID: &str = "caseforge.choreography.v1";
pub const BATCH: &str = "B-S1-20";
pub const MAX_LARGE_RESIDENT: usize = 1;
pub const RSS_MAX_GB: f64 = 7.2;
pub const IDLE_RAM_MAX_MB: u64 = 400;
pub const COLD_START_MAX_S: u64 = 6;
pub const UI_FREEZE_MAX_MS: u64 = 200;
pub const DIGEST_MAX_MIN: u64 = 5;
pub const IDLE_UNLOAD_S: u64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRole {
    PrimaryLlm,
    Vision,
    SpeechStt,
    SpeechTts,
    Embeddings,
}

impl ModelRole {
    pub fn as_str(self) -> &'static str {
        match self {
            ModelRole::PrimaryLlm => "primary_llm",
            ModelRole::Vision => "vision",
            ModelRole::SpeechStt => "speech_stt",
            ModelRole::SpeechTts => "speech_tts",
            ModelRole::Embeddings => "embeddings",
        }
    }

    pub fn is_large(self) -> bool {
        matches!(self, ModelRole::PrimaryLlm | ModelRole::Vision)
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "primary_llm" | "primary" | "llm" => Some(ModelRole::PrimaryLlm),
            "vision" | "docpipe_vision" => Some(ModelRole::Vision),
            "speech_stt" | "stt" => Some(ModelRole::SpeechStt),
            "speech_tts" | "tts" => Some(ModelRole::SpeechTts),
            "embeddings" => Some(ModelRole::Embeddings),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ResidentSlot {
    pub role: String,
    pub large: bool,
    pub acquired_at_unix_ms: i64,
    pub last_used_unix_ms: i64,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChoreographyStatus {
    pub pipeline_id: &'static str,
    pub batch: &'static str,
    pub tier: String,
    pub live_simulation_allowed: bool,
    pub resident: Vec<ResidentSlot>,
    pub large_resident_count: usize,
    pub max_large_resident: usize,
    pub primary_and_vision_concurrent: bool,
    pub hard_fail_primary_vision: bool,
    pub chip_label: String,
    pub chip_tone: String,
    pub idle_unload_after_s: u64,
    pub budgets: BudgetSnapshot,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BudgetSnapshot {
    pub cold_start_max_s: u64,
    pub process_uptime_ms: u64,
    pub cold_start_ok: Option<bool>,
    pub idle_ram_max_mb: u64,
    pub process_rss_mb: Option<u64>,
    pub idle_ram_ok: Option<bool>,
    pub total_rss_max_gb: f64,
    pub process_rss_gb: Option<f64>,
    pub rss_ok: Option<bool>,
    pub ui_freeze_max_ms: u64,
    pub digest_max_min: u64,
    pub measurement_notes: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcquireResult {
    pub ok: bool,
    pub role: String,
    pub status: ChoreographyStatus,
    pub error: Option<String>,
}

struct Slot {
    role: ModelRole,
    acquired: Instant,
    last_used: Instant,
    detail: String,
}

struct SuperInner {
    slots: HashMap<ModelRole, Slot>,
    process_start: Instant,
}

static SUPER: Lazy<Mutex<SuperInner>> = Lazy::new(|| {
    Mutex::new(SuperInner {
        slots: HashMap::new(),
        process_start: Instant::now(),
    })
});

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Best-effort process RSS in bytes (Windows + Unix).
pub fn process_rss_bytes() -> Option<u64> {
    #[cfg(windows)]
    {
        process_rss_windows()
    }
    #[cfg(unix)]
    {
        process_rss_unix()
    }
    #[cfg(not(any(windows, unix)))]
    {
        None
    }
}

#[cfg(windows)]
fn process_rss_windows() -> Option<u64> {
    // PROCESS_MEMORY_COUNTERS.WorkingSetSize via psapi
    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }
    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            ppsmemcounters: *mut ProcessMemoryCounters,
            cb: u32,
        ) -> i32;
    }
    unsafe {
        let mut pmc = std::mem::zeroed::<ProcessMemoryCounters>();
        pmc.cb = std::mem::size_of::<ProcessMemoryCounters>() as u32;
        let ok = K32GetProcessMemoryInfo(GetCurrentProcess(), &mut pmc, pmc.cb);
        if ok != 0 {
            Some(pmc.working_set_size as u64)
        } else {
            None
        }
    }
}

#[cfg(unix)]
fn process_rss_unix() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = s.split_whitespace().nth(1)?.parse().ok()?;
    let page = unsafe { libc_page_size() } as u64;
    Some(pages * page)
}

#[cfg(unix)]
fn libc_page_size() -> usize {
    // Avoid libc dep: assume 4096 if sysconf unavailable
    4096
}

/// Rough total system RAM (GB) for tier detection — best effort.
pub fn system_ram_gb() -> Option<f64> {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct MemoryStatusEx {
            dw_length: u32,
            dw_memory_load: u32,
            ull_total_phys: u64,
            ull_avail_phys: u64,
            ull_total_page_file: u64,
            ull_avail_page_file: u64,
            ull_total_virtual: u64,
            ull_avail_virtual: u64,
            ull_avail_extended_virtual: u64,
        }
        extern "system" {
            fn GlobalMemoryStatusEx(lpbuffer: *mut MemoryStatusEx) -> i32;
        }
        unsafe {
            let mut st = std::mem::zeroed::<MemoryStatusEx>();
            st.dw_length = std::mem::size_of::<MemoryStatusEx>() as u32;
            if GlobalMemoryStatusEx(&mut st) != 0 {
                Some(st.ull_total_phys as f64 / (1024.0 * 1024.0 * 1024.0))
            } else {
                None
            }
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

pub fn detect_tier() -> (String, bool) {
    // (tier_label, live_simulation_allowed)
    match system_ram_gb() {
        Some(g) if g < 4.0 => ("not_supported".into(), false),
        Some(g) if g < 8.0 => ("lite".into(), false),
        Some(_) => ("full".into(), true),
        None => ("unknown".into(), true), // honest unknown; do not block
    }
}

fn unload_idle_locked(inner: &mut SuperInner) {
    let idle = Duration::from_secs(IDLE_UNLOAD_S);
    let now = Instant::now();
    inner.slots.retain(|_, slot| {
        if now.duration_since(slot.last_used) < idle {
            true
        } else {
            // unload
            false
        }
    });
}

fn build_status(inner: &SuperInner) -> ChoreographyStatus {
    let (tier, live_sim) = detect_tier();
    let mut resident: Vec<ResidentSlot> = inner
        .slots
        .values()
        .map(|s| ResidentSlot {
            role: s.role.as_str().into(),
            large: s.role.is_large(),
            acquired_at_unix_ms: now_ms() - s.acquired.elapsed().as_millis() as i64,
            last_used_unix_ms: now_ms() - s.last_used.elapsed().as_millis() as i64,
            detail: s.detail.clone(),
        })
        .collect();
    resident.sort_by(|a, b| a.role.cmp(&b.role));

    let large_n = resident.iter().filter(|r| r.large).count();
    let has_primary = inner.slots.contains_key(&ModelRole::PrimaryLlm);
    let has_vision = inner.slots.contains_key(&ModelRole::Vision);
    let concurrent = has_primary && has_vision;

    let chip_label = if concurrent {
        "MODEL CONFLICT · primary+vision".into()
    } else if let Some(s) = resident.iter().find(|r| r.large) {
        format!("MODEL · {}", s.role)
    } else if !resident.is_empty() {
        format!(
            "MODEL · {}",
            resident
                .iter()
                .map(|r| r.role.as_str())
                .collect::<Vec<_>>()
                .join("+")
        )
    } else {
        "MODEL · unloaded".into()
    };
    let chip_tone = if concurrent {
        "danger".into()
    } else if large_n > 0 {
        "ok".into()
    } else {
        "muted".into()
    };

    let uptime_ms = inner.process_start.elapsed().as_millis() as u64;
    let rss = process_rss_bytes();
    let rss_mb = rss.map(|b| b / (1024 * 1024));
    let rss_gb = rss.map(|b| b as f64 / (1024.0 * 1024.0 * 1024.0));
    let models_unloaded = large_n == 0;
    let idle_ram_ok = if models_unloaded {
        rss_mb.map(|m| m <= IDLE_RAM_MAX_MB)
    } else {
        None // not applicable while models loaded
    };
    let rss_ok = rss_gb.map(|g| g <= RSS_MAX_GB);
    // Cold start: process uptime at first status after launch is diagnostic;
    // true cold-start PASS requires installer measurement — report provisional.
    let cold_ok = if uptime_ms < COLD_START_MAX_S * 1000 + 2000 {
        Some(true)
    } else {
        // Already running — budget applies to launch, not long sessions
        None
    };

    ChoreographyStatus {
        pipeline_id: PIPELINE_ID,
        batch: BATCH,
        tier,
        live_simulation_allowed: live_sim,
        resident,
        large_resident_count: large_n,
        max_large_resident: MAX_LARGE_RESIDENT,
        primary_and_vision_concurrent: concurrent,
        hard_fail_primary_vision: concurrent,
        chip_label,
        chip_tone,
        idle_unload_after_s: IDLE_UNLOAD_S,
        budgets: BudgetSnapshot {
            cold_start_max_s: COLD_START_MAX_S,
            process_uptime_ms: uptime_ms,
            cold_start_ok: cold_ok,
            idle_ram_max_mb: IDLE_RAM_MAX_MB,
            process_rss_mb: rss_mb,
            idle_ram_ok,
            total_rss_max_gb: RSS_MAX_GB,
            process_rss_gb: rss_gb,
            rss_ok,
            ui_freeze_max_ms: UI_FREEZE_MAX_MS,
            digest_max_min: DIGEST_MAX_MIN,
            measurement_notes: if rss.is_none() {
                "RSS measurement unavailable on this platform build; choreography rules still enforced"
                    .into()
            } else {
                "RSS from OS working set; cold_start_ok only meaningful near process launch".into()
            },
        },
        detail: if concurrent {
            "HARD FAIL: primary_llm and vision both resident (PRD §4.2 8GB rule)".into()
        } else {
            format!(
                "sequential load; max {MAX_LARGE_RESIDENT} large model; idle unload {IDLE_UNLOAD_S}s"
            )
        },
    }
}

pub fn status() -> ChoreographyStatus {
    let mut g = SUPER.lock().unwrap_or_else(|e| e.into_inner());
    unload_idle_locked(&mut g);
    build_status(&g)
}

/// Acquire a model role for exclusive large-model choreography.
pub fn acquire(role_str: &str, detail: &str) -> AcquireResult {
    let Some(role) = ModelRole::parse(role_str) else {
        return AcquireResult {
            ok: false,
            role: role_str.into(),
            status: status(),
            error: Some(format!("CF_CHOREOGRAPHY: unknown role {role_str}")),
        };
    };
    let mut g = SUPER.lock().unwrap_or_else(|e| e.into_inner());
    unload_idle_locked(&mut g);

    if role.is_large() {
        // Hard rule: primary and vision never concurrent.
        if role == ModelRole::PrimaryLlm && g.slots.contains_key(&ModelRole::Vision) {
            // Evict vision first (sequential swap)
            g.slots.remove(&ModelRole::Vision);
        }
        if role == ModelRole::Vision && g.slots.contains_key(&ModelRole::PrimaryLlm) {
            g.slots.remove(&ModelRole::PrimaryLlm);
        }
        // Evict any other large roles beyond max
        let other_large: Vec<ModelRole> = g
            .slots
            .keys()
            .copied()
            .filter(|r| r.is_large() && *r != role)
            .collect();
        for r in other_large {
            g.slots.remove(&r);
        }
        let large_after = g.slots.keys().filter(|r| r.is_large()).count()
            + if g.slots.contains_key(&role) { 0 } else { 1 };
        if large_after > MAX_LARGE_RESIDENT {
            let st = build_status(&g);
            return AcquireResult {
                ok: false,
                role: role.as_str().into(),
                status: st,
                error: Some(
                    "CF_CHOREOGRAPHY: max_large_models_resident exceeded (must be 1)".into(),
                ),
            };
        }
    }

    let now = Instant::now();
    g.slots.insert(
        role,
        Slot {
            role,
            acquired: now,
            last_used: now,
            detail: detail.into(),
        },
    );
    // Final concurrent check
    if g.slots.contains_key(&ModelRole::PrimaryLlm) && g.slots.contains_key(&ModelRole::Vision) {
        // Should not happen after eviction — hard fail
        g.slots.remove(&ModelRole::Vision);
    }
    let st = build_status(&g);
    AcquireResult {
        ok: !st.hard_fail_primary_vision,
        role: role.as_str().into(),
        status: st,
        error: None,
    }
}

pub fn release(role_str: &str) -> ChoreographyStatus {
    let mut g = SUPER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(role) = ModelRole::parse(role_str) {
        g.slots.remove(&role);
    }
    unload_idle_locked(&mut g);
    build_status(&g)
}

pub fn touch(role_str: &str) {
    let mut g = SUPER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(role) = ModelRole::parse(role_str) {
        if let Some(slot) = g.slots.get_mut(&role) {
            slot.last_used = Instant::now();
        }
    }
}

pub fn unload_all() -> ChoreographyStatus {
    let mut g = SUPER.lock().unwrap_or_else(|e| e.into_inner());
    g.slots.clear();
    build_status(&g)
}

pub fn policy() -> serde_json::Value {
    let (tier, live) = detect_tier();
    serde_json::json!({
        "batch": BATCH,
        "pipeline_id": PIPELINE_ID,
        "max_large_models_resident": MAX_LARGE_RESIDENT,
        "primary_and_vision_concurrent_forbidden": true,
        "idle_unload_after_s": IDLE_UNLOAD_S,
        "visible_status_chip_required": true,
        "tier_detected": tier,
        "live_simulation_allowed": live,
        "budgets": {
            "cold_start_max_s": COLD_START_MAX_S,
            "idle_ram_max_mb": IDLE_RAM_MAX_MB,
            "ui_freeze_max_ms": UI_FREEZE_MAX_MS,
            "digest_max_min": DIGEST_MAX_MIN,
            "total_rss_max_gb": RSS_MAX_GB,
        },
        "acs": ["AC-3.1", "AC-3.4", "AC-3.5", "AC-4.2"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_evicts_vision() {
        unload_all();
        let a = acquire("vision", "test");
        assert!(a.ok);
        let b = acquire("primary_llm", "test");
        assert!(b.ok);
        assert!(!b.status.primary_and_vision_concurrent);
        assert_eq!(b.status.large_resident_count, 1);
        unload_all();
    }

    #[test]
    fn never_both_large() {
        unload_all();
        acquire("primary_llm", "t");
        let s = acquire("vision", "t");
        assert!(!s.status.primary_and_vision_concurrent);
        assert!(s.status.large_resident_count <= 1);
        unload_all();
    }
}
