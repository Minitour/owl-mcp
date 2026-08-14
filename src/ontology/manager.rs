use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;

use super::owl_api::{OwlApi, OwlApiError};

/// Cache of loaded ontologies, keyed by canonical absolute path.
///
/// Disk is the source of truth: [`get_or_load`] reloads from mtime on each access.
/// Each file has its own mutex so unrelated paths are not serialized.
pub struct OntologyManager {
    apis: Mutex<HashMap<PathBuf, Arc<Mutex<OwlApi>>>>,
}

impl OntologyManager {
    pub fn new() -> Self {
        OntologyManager {
            apis: Mutex::new(HashMap::new()),
        }
    }

    /// Get or load an ontology by file path, reloading from disk if mtime changed.
    pub async fn get_or_load(
        &self,
        path: impl AsRef<Path>,
        readonly: bool,
        create_if_not_exists: bool,
    ) -> Result<Arc<Mutex<OwlApi>>, OwlApiError> {
        let path = canonicalize_or_absolute(path.as_ref());

        {
            let map = self.apis.lock().await;
            if let Some(handle) = map.get(&path).cloned() {
                drop(map);
                {
                    let mut api = handle.lock().await;
                    api.check_and_reload_if_modified()?;
                }
                return Ok(handle);
            }
        }

        let api = OwlApi::load(&path, readonly, create_if_not_exists)?;
        let loaded = Arc::new(Mutex::new(api));

        let mut map = self.apis.lock().await;
        let handle = map.entry(path).or_insert_with(|| loaded.clone()).clone();
        drop(map);

        {
            let mut api = handle.lock().await;
            api.check_and_reload_if_modified()?;
        }
        Ok(handle)
    }

    /// Reload an ontology from disk if it's currently loaded (called by file watcher).
    #[allow(dead_code)]
    pub async fn reload_if_loaded(&self, path: impl AsRef<Path>) -> Result<(), OwlApiError> {
        let path = canonicalize_or_absolute(path.as_ref());
        let handle = {
            let map = self.apis.lock().await;
            map.get(&path).cloned()
        };
        if let Some(handle) = handle {
            let mut api = handle.lock().await;
            api.reload()?;
        }
        Ok(())
    }

    /// Snapshot of cached handles (for the file watcher).
    pub async fn loaded_handles(&self) -> Vec<(PathBuf, Arc<Mutex<OwlApi>>)> {
        let map = self.apis.lock().await;
        map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    /// List all currently cached ontology file paths (this process only).
    pub async fn active_paths(&self) -> Vec<String> {
        let map = self.apis.lock().await;
        map.keys()
            .map(|p| p.to_string_lossy().into_owned())
            .collect()
    }
}

fn canonicalize_or_absolute(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
