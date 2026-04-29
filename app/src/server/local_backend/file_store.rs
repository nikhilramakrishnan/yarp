//! Atomic JSON-on-disk persistence with per-file in-memory locking.
//!
//! Each logical "store" (agent_tasks.json, conversations/{token}.json, etc.)
//! is read on-demand and written atomically via tempfile + rename. We hold a
//! `parking_lot::Mutex` keyed by absolute path so concurrent writers from
//! different async tasks serialize correctly without blocking other paths.

use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use serde::{de::DeserializeOwned, Serialize};

#[derive(Clone)]
pub struct FileStore {
    inner: Arc<FileStoreInner>,
}

struct FileStoreInner {
    root: PathBuf,
    locks: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
}

impl FileStore {
    pub fn new(root: PathBuf) -> Self {
        Self {
            inner: Arc::new(FileStoreInner {
                root,
                locks: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn root(&self) -> &Path {
        &self.inner.root
    }

    /// Returns a per-path mutex. Holding it while reading + writing prevents
    /// concurrent writes (and read-modify-write races) on the same file.
    fn lock_for(&self, path: &Path) -> Arc<Mutex<()>> {
        let mut guard = self.inner.locks.lock();
        guard
            .entry(path.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Reads JSON from `path`, returning `Ok(None)` if the file does not exist.
    pub fn read_json<T: DeserializeOwned>(&self, path: &Path) -> Result<Option<T>> {
        let lock = self.lock_for(path);
        let _guard = lock.lock();
        match fs::read(path) {
            Ok(bytes) => {
                let value = serde_json::from_slice(&bytes)
                    .with_context(|| format!("parsing JSON at {}", path.display()))?;
                Ok(Some(value))
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
        }
    }

    /// Writes JSON to `path` atomically (tempfile in the same dir + rename).
    pub fn write_json<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        let lock = self.lock_for(path);
        let _guard = lock.lock();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating dir {}", parent.display()))?;
        }
        let bytes =
            serde_json::to_vec_pretty(value).context("serializing JSON for FileStore::write")?;
        let tmp = self.tempfile_for(path);
        {
            let mut file = fs::File::create(&tmp)
                .with_context(|| format!("creating tempfile {}", tmp.display()))?;
            file.write_all(&bytes)
                .with_context(|| format!("writing tempfile {}", tmp.display()))?;
            file.sync_all()
                .with_context(|| format!("fsync tempfile {}", tmp.display()))?;
        }
        fs::rename(&tmp, path)
            .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
        Ok(())
    }

    /// Reads a JSON value, applies `mutator`, and writes it back atomically.
    /// If the file does not exist, `mutator` is given `T::default()`.
    pub fn update_json<T, F>(&self, path: &Path, mutator: F) -> Result<()>
    where
        T: DeserializeOwned + Serialize + Default,
        F: FnOnce(&mut T),
    {
        let lock = self.lock_for(path);
        let _guard = lock.lock();
        let mut value: T = match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .with_context(|| format!("parsing JSON at {}", path.display()))?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => T::default(),
            Err(err) => {
                return Err(err).with_context(|| format!("reading {}", path.display()));
            }
        };
        mutator(&mut value);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating dir {}", parent.display()))?;
        }
        let bytes =
            serde_json::to_vec_pretty(&value).context("serializing JSON for FileStore::update")?;
        let tmp = self.tempfile_for(path);
        {
            let mut file = fs::File::create(&tmp)
                .with_context(|| format!("creating tempfile {}", tmp.display()))?;
            file.write_all(&bytes)
                .with_context(|| format!("writing tempfile {}", tmp.display()))?;
            file.sync_all()
                .with_context(|| format!("fsync tempfile {}", tmp.display()))?;
        }
        fs::rename(&tmp, path)
            .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
        Ok(())
    }

    /// Deletes `path` if it exists. Returns Ok(false) if it didn't.
    pub fn delete(&self, path: &Path) -> Result<bool> {
        let lock = self.lock_for(path);
        let _guard = lock.lock();
        match fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err).with_context(|| format!("deleting {}", path.display())),
        }
    }

    fn tempfile_for(&self, path: &Path) -> PathBuf {
        let mut tmp = path.to_path_buf();
        let file_name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let nonce = uuid::Uuid::new_v4();
        tmp.set_file_name(format!(".{file_name}.{nonce}.tmp"));
        tmp
    }
}
