use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::owl_api::{OwlApi, OwlApiError};

/// Thread-safe manager holding one `OwlApi` per file path.
/// This struct itself is wrapped in `Arc<Mutex<OntologyManager>>` by the handler.
pub struct OntologyManager {
    /// Map from canonical absolute path → loaded ontology
    pub apis: HashMap<PathBuf, OwlApi>,
}

impl OntologyManager {
    pub fn new() -> Self {
        OntologyManager {
            apis: HashMap::new(),
        }
    }

    /// Get or load an ontology by file path.
    pub fn get_or_load(
        &mut self,
        path: impl AsRef<Path>,
        readonly: bool,
        create_if_not_exists: bool,
    ) -> Result<&mut OwlApi, OwlApiError> {
        let path = canonicalize_or_absolute(path.as_ref());
        if !self.apis.contains_key(&path) {
            let api = OwlApi::load(&path, readonly, create_if_not_exists)?;
            self.apis.insert(path.clone(), api);
        }
        Ok(self.apis.get_mut(&path).unwrap())
    }

    /// Reload an ontology from disk if it's currently loaded (called by file watcher).
    #[allow(dead_code)]
    pub fn reload_if_loaded(&mut self, path: impl AsRef<Path>) -> Result<(), OwlApiError> {
        let path = canonicalize_or_absolute(path.as_ref());
        if let Some(api) = self.apis.get_mut(&path) {
            api.reload()?;
        }
        Ok(())
    }

    /// List all currently loaded ontology file paths.
    pub fn active_paths(&self) -> Vec<String> {
        self.apis
            .keys()
            .map(|p| p.to_string_lossy().into_owned())
            .collect()
    }
}

fn canonicalize_or_absolute(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
