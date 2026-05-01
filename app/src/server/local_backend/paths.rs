//! Path resolution for the local backend's on-disk state.
//!
//! Everything lives under `~/.yarp/` (resolved via
//! `yarp_core::paths::yarp_home_config_dir`, which maps `Channel::Oss` to
//! `.yarp`). When the home directory can't be located we fall back to a temp
//! directory so the app still launches.

use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct LocalPaths {
    pub root: PathBuf,
}

impl LocalPaths {
    pub fn resolve() -> Self {
        let root = yarp_core::paths::yarp_home_config_dir().unwrap_or_else(|| {
            // Last-resort fallback. Should not happen on macOS / Linux / Windows
            // where `dirs::home_dir()` is reliable, but we'd rather start with a
            // temp dir than panic.
            let mut tmp = std::env::temp_dir();
            tmp.push("yarp");
            tmp
        });
        Self { root }
    }

    pub fn ensure_root_exists(&self) {
        if let Err(err) = std::fs::create_dir_all(&self.root) {
            log::warn!(
                "yarp: failed to create root config dir {}: {err}",
                self.root.display()
            );
        }
    }

    pub fn agent_tasks_file(&self) -> PathBuf {
        self.root.join("agent_tasks.json")
    }

    pub fn harness_dir(&self) -> PathBuf {
        self.root.join("harness")
    }

    pub fn harness_run_dir(&self, run_id: &str) -> PathBuf {
        self.harness_dir().join(run_id)
    }

    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    pub fn workspace_file(&self) -> PathBuf {
        self.root.join("workspace.json")
    }

    pub fn secrets_file(&self) -> PathBuf {
        self.root.join("secrets.json")
    }

    pub fn llm_provider_file(&self) -> PathBuf {
        self.root.join("llm_provider.json")
    }
}
