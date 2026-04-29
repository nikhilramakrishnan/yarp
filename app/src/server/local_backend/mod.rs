//! Local-first backend for Yarp (the warp-oss fork).
//!
//! Yarp does not talk to `app.warp.dev`. Every server-side capability that the
//! rest of the app reaches for via [`ServerApi`] (cloud objects, agent harness
//! coordination, conversation history, AI inference, etc.) is satisfied here,
//! on disk and through user-supplied LLM providers.
//!
//! Layout:
//!
//! ```text
//! ~/.yarp/
//! ├── install_id              # stable per-install UUID
//! ├── agent_tasks.json        # locally-managed ambient/harness task registry
//! ├── conversations/          # AI conversation snapshots (per-token JSON)
//! ├── harness/{run_id}/       # transcript + block snapshot for each harness run
//! ├── objects/                # workflows, notebooks, generic objects, folders
//! └── settings.json           # local-only settings (LLM provider config etc.)
//! ```
//!
//! Submodules each implement one of the `*Client` traits that today live on
//! `ServerApi`. They share the [`LocalBackend`] context, which holds path
//! roots and the file-store helper.

pub mod ai_client;
pub mod file_store;
pub mod harness_support;
pub mod llm_provider;
pub mod paths;

pub use ai_client::OssAiClient;
pub use file_store::FileStore;
pub use harness_support::OssHarnessSupportClient;
pub use paths::LocalPaths;

use std::sync::Arc;

/// Shared context for every local-backend client. Cheap to clone (`Arc` inside).
#[derive(Clone)]
pub struct LocalBackend {
    inner: Arc<LocalBackendInner>,
}

struct LocalBackendInner {
    paths: LocalPaths,
    file_store: FileStore,
}

impl LocalBackend {
    /// Builds the backend, ensuring the on-disk root directory exists. Falls
    /// back to a temp directory if the user's home directory cannot be
    /// determined (rare on the platforms we support).
    pub fn new() -> Self {
        let paths = LocalPaths::resolve();
        paths.ensure_root_exists();
        let file_store = FileStore::new(paths.root.clone());
        Self {
            inner: Arc::new(LocalBackendInner { paths, file_store }),
        }
    }

    pub fn paths(&self) -> &LocalPaths {
        &self.inner.paths
    }

    pub fn file_store(&self) -> &FileStore {
        &self.inner.file_store
    }
}

impl Default for LocalBackend {
    fn default() -> Self {
        Self::new()
    }
}
