use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::owl_api::{OwlApi, OwlApiError};
use crate::config::{ConfigError, OWLMcpConfig, OntologyConfigInfo};

/// Thread-safe manager holding one `OwlApi` per file path.
/// This struct itself is wrapped in `Arc<Mutex<OntologyManager>>` by the handler.
pub struct OntologyManager {
    /// Map from canonical absolute path → loaded ontology
    pub apis: HashMap<PathBuf, OwlApi>,
    /// Persistent configuration
    pub config: OWLMcpConfig,
}

impl OntologyManager {
    pub fn new() -> Result<Self, ConfigError> {
        let config = OWLMcpConfig::load()?;
        Ok(OntologyManager {
            apis: HashMap::new(),
            config,
        })
    }

    // ── Path-based API ─────────────────────────────────────────────────────────

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

    // ── Name-based API ─────────────────────────────────────────────────────────

    /// Resolve an ontology name to its file path, then get or load it.
    pub fn get_or_load_by_name(&mut self, name: &str) -> Result<&mut OwlApi, OwlApiError> {
        let info = self
            .config
            .get_ontology(name)
            .ok_or_else(|| {
                OwlApiError::Parse(format!("No configured ontology named '{}'", name))
            })?
            .clone();
        let readonly = info.readonly;
        self.get_or_load(&info.path, readonly, false)
    }

    // ── Config management ──────────────────────────────────────────────────────

    pub fn list_configured_ontologies(&self) -> Vec<OntologyConfigInfo> {
        self.config
            .list_ontologies()
            .into_iter()
            .cloned()
            .collect()
    }

    pub fn configure_ontology(&mut self, info: OntologyConfigInfo) -> Result<String, OwlApiError> {
        let name = info.name.clone();
        // Evict cache so it gets reloaded with new settings next access
        let path = canonicalize_or_absolute(Path::new(&info.path));
        self.apis.remove(&path);
        self.config.set_ontology(info);
        self.config.save().map_err(|e| OwlApiError::Parse(e.to_string()))?;
        Ok(format!("Configured ontology '{}'", name))
    }

    pub fn remove_ontology_config(&mut self, name: &str) -> Result<String, OwlApiError> {
        if let Some(info) = self.config.remove_ontology(name) {
            let path = canonicalize_or_absolute(Path::new(&info.path));
            self.apis.remove(&path);
            self.config.save().map_err(|e| OwlApiError::Parse(e.to_string()))?;
            Ok(format!("Removed ontology config '{}'", name))
        } else {
            Ok(format!("No configured ontology named '{}'", name))
        }
    }

    pub fn get_ontology_config(&self, name: &str) -> Option<OntologyConfigInfo> {
        self.config.get_ontology(name).cloned()
    }

    pub fn register_in_config(
        &mut self,
        owl_file_path: &str,
        name: Option<String>,
        readonly: Option<bool>,
        description: Option<String>,
        preferred_serialization: Option<String>,
        annotation_property: Option<String>,
    ) -> Result<String, OwlApiError> {
        let resolved_name = name.unwrap_or_else(|| {
            Path::new(owl_file_path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
        let info = OntologyConfigInfo {
            name: resolved_name.clone(),
            path: owl_file_path.to_string(),
            readonly: readonly.unwrap_or(false),
            description,
            preferred_serialization,
            annotation_property,
            ..Default::default()
        };
        self.config.set_ontology(info);
        self.config.save().map_err(|e| OwlApiError::Parse(e.to_string()))?;
        Ok(format!(
            "Registered ontology '{}' from '{}'",
            resolved_name, owl_file_path
        ))
    }

    pub fn load_and_register(
        &mut self,
        owl_file_path: &str,
        name: Option<String>,
        readonly: bool,
        create_if_not_exists: bool,
        description: Option<String>,
        preferred_serialization: Option<String>,
        metadata_axioms: Option<Vec<String>>,
        annotation_property: Option<String>,
    ) -> Result<String, OwlApiError> {
        // Load (or create) the file
        let api = self.get_or_load(owl_file_path, readonly, create_if_not_exists)?;

        // Add metadata axioms if provided
        if let Some(axioms) = &metadata_axioms {
            if !axioms.is_empty() && !readonly {
                let strs: Vec<String> = axioms.clone();
                api.add_axioms(&strs)?;
            }
        }

        // Register in config
        let resolved_name = name.unwrap_or_else(|| {
            Path::new(owl_file_path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
        let info = OntologyConfigInfo {
            name: resolved_name.clone(),
            path: owl_file_path.to_string(),
            metadata_axioms: metadata_axioms.unwrap_or_default(),
            readonly,
            description,
            preferred_serialization,
            annotation_property,
        };
        self.config.set_ontology(info);
        self.config.save().map_err(|e| OwlApiError::Parse(e.to_string()))?;
        Ok(format!(
            "Loaded and registered ontology '{}' from '{}'",
            resolved_name, owl_file_path
        ))
    }
}

fn canonicalize_or_absolute(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
