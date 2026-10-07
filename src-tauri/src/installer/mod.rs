//! Installer / hardware gate (B-S1-22, PRD §3).
//! Refuse < 4 GB RAM or 32-bit OS with a clear message.

use serde::Serialize;

pub const BATCH: &str = "B-S2-07";
pub const MIN_RAM_GB: f64 = 4.0;
pub const FULL_RAM_GB: f64 = 8.0;

#[derive(Debug, Clone, Serialize)]
pub struct HardwareGate {
    pub allowed: bool,
    pub tier: String,
    pub bits: u32,
    pub total_ram_gb: Option<f64>,
    pub is_32bit: bool,
    pub refuse_reason: Option<String>,
    pub messaging: String,
    pub batch: &'static str,
    pub lite_note: String,
    pub full_note: String,
}

fn is_32bit() -> bool {
    cfg!(target_pointer_width = "32")
}

fn total_ram_gb() -> Option<f64> {
    crate::choreography::system_ram_gb()
}

pub fn evaluate() -> HardwareGate {
    let bits = if is_32bit() { 32 } else { 64 };
    let ram = total_ram_gb();
    let is32 = is_32bit();

    let mut allowed = true;
    let mut refuse: Option<String> = None;
    let tier: String;

    if is32 {
        allowed = false;
        refuse = Some(
            "CaseForge does not support 32-bit operating systems. Install a 64-bit OS and reinstall."
                .into(),
        );
        tier = "not_supported".into();
    } else if let Some(g) = ram {
        if g < MIN_RAM_GB {
            allowed = false;
            refuse = Some(format!(
                "This machine has approximately {g:.1} GB RAM. CaseForge requires at least {MIN_RAM_GB} GB. Installer refuses installation/run on unsupported hardware (PRD §3)."
            ));
            tier = "not_supported".into();
        } else if g < FULL_RAM_GB {
            tier = "lite".into();
        } else {
            tier = "full".into();
        }
    } else {
        tier = "unknown".into();
    }

    let messaging = if !allowed {
        refuse
            .clone()
            .unwrap_or_else(|| "Hardware not supported".into())
    } else if tier == "lite" {
        "Lite tier (4–8 GB): Digest vault, drafting, citation checking, lexicon voice. No live Mock Court voice simulation (honesty refuse).".into()
    } else if tier == "full" {
        "Full tier (≥8 GB): Full Digest + Mock Court (text formats, Adaptive Bench, Bench Prep, voice duplex). Sequential model loading. Live voice simulation offline only.".into()
    } else {
        "Hardware tier could not be measured; sequential model choreography and honesty notices. Live Mock Court voice follows Full-tier policy when measured.".into()
    };

    HardwareGate {
        allowed,
        tier,
        bits,
        total_ram_gb: ram,
        is_32bit: is32,
        refuse_reason: refuse,
        messaging,
        batch: BATCH,
        lite_note: "Lite: no live Mock Court voice simulation. Digest + drafting + citations + lexicon dictation only.".into(),
        full_note: "Full: Digest + Mock Court; sequential one-large-model choreography; RSS budget 7.2 GB; live voice duplex offline.".into(),
    }
}

pub fn policy() -> serde_json::Value {
    let g = evaluate();
    serde_json::json!({
        "batch": BATCH,
        "min_ram_gb": MIN_RAM_GB,
        "full_ram_gb": FULL_RAM_GB,
        "refuse_32bit": true,
        "gate": g,
        "prd": "§3",
    })
}
