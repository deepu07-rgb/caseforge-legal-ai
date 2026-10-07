use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct VaultPaths {
    pub root: PathBuf,
}

impl VaultPaths {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn app_dir(&self) -> PathBuf {
        self.root.join("app")
    }

    pub fn app_db_path(&self) -> PathBuf {
        self.app_dir().join("app.db")
    }

    pub fn app_kdf_path(&self) -> PathBuf {
        self.app_dir().join("kdf.json")
    }

    pub fn cases_dir(&self) -> PathBuf {
        self.root.join("cases")
    }

    pub fn case_dir(&self, case_id: &str) -> PathBuf {
        self.cases_dir().join(case_id)
    }

    pub fn case_db_path(&self, case_id: &str) -> PathBuf {
        self.case_dir(case_id).join("case.db")
    }

    pub fn case_kdf_path(&self, case_id: &str) -> PathBuf {
        self.case_dir(case_id).join("kdf.json")
    }

    pub fn is_under_root(&self, path: &Path) -> bool {
        path.starts_with(&self.root)
    }

    /// Official knowledge packs root (B-S1-12).
    pub fn packs_official_dir(&self) -> PathBuf {
        self.root.join("packs").join("official")
    }

    /// Custom (user) knowledge packs root — isolated from official (B-S1-12).
    pub fn packs_custom_dir(&self) -> PathBuf {
        self.root.join("packs").join("custom")
    }

    pub fn packs_registry_path(&self) -> PathBuf {
        self.root.join("packs").join("registry.json")
    }

    /// Local model weights / GGUF cache (B-S1-19 protection). Never user case data.
    pub fn models_dir(&self) -> PathBuf {
        self.root.join("models")
    }

    /// Protection manifests / seals (B-S1-19).
    pub fn protection_dir(&self) -> PathBuf {
        self.root.join("protection")
    }
}
