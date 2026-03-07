use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Cannot determine home directory")]
    NoHomeDir,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OntologyConfigInfo {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub metadata_axioms: Vec<String>,
    #[serde(default)]
    pub readonly: bool,
    pub description: Option<String>,
    pub preferred_serialization: Option<String>,
    pub annotation_property: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OWLMcpConfig {
    #[serde(default)]
    pub ontologies: HashMap<String, OntologyConfigInfo>,
}

impl OWLMcpConfig {
    pub fn config_path() -> Result<PathBuf, ConfigError> {
        let home = dirs::home_dir().ok_or(ConfigError::NoHomeDir)?;
        Ok(home.join(".owl-mcp").join("config.yaml"))
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let config: OWLMcpConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_yaml::to_string(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn get_ontology(&self, name: &str) -> Option<&OntologyConfigInfo> {
        self.ontologies.get(name)
    }

    pub fn set_ontology(&mut self, info: OntologyConfigInfo) {
        self.ontologies.insert(info.name.clone(), info);
    }

    pub fn remove_ontology(&mut self, name: &str) -> Option<OntologyConfigInfo> {
        self.ontologies.remove(name)
    }

    pub fn list_ontologies(&self) -> Vec<&OntologyConfigInfo> {
        self.ontologies.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_info(name: &str, path: &str) -> OntologyConfigInfo {
        OntologyConfigInfo {
            name: name.to_string(),
            path: path.to_string(),
            ..Default::default()
        }
    }

    // ── in-memory CRUD ────────────────────────────────────────────────────────

    #[test]
    fn set_and_get_ontology() {
        let mut cfg = OWLMcpConfig::default();
        cfg.set_ontology(sample_info("pizza", "/data/pizza.ofn"));
        let got = cfg.get_ontology("pizza").unwrap();
        assert_eq!(got.name, "pizza");
        assert_eq!(got.path, "/data/pizza.ofn");
    }

    #[test]
    fn get_missing_ontology_returns_none() {
        let cfg = OWLMcpConfig::default();
        assert!(cfg.get_ontology("nonexistent").is_none());
    }

    #[test]
    fn remove_ontology_returns_info_and_deletes() {
        let mut cfg = OWLMcpConfig::default();
        cfg.set_ontology(sample_info("a", "/a.ofn"));
        let removed = cfg.remove_ontology("a").unwrap();
        assert_eq!(removed.name, "a");
        assert!(cfg.get_ontology("a").is_none());
    }

    #[test]
    fn remove_missing_ontology_returns_none() {
        let mut cfg = OWLMcpConfig::default();
        assert!(cfg.remove_ontology("ghost").is_none());
    }

    #[test]
    fn list_ontologies_empty() {
        let cfg = OWLMcpConfig::default();
        assert!(cfg.list_ontologies().is_empty());
    }

    #[test]
    fn list_ontologies_multiple() {
        let mut cfg = OWLMcpConfig::default();
        cfg.set_ontology(sample_info("a", "/a.ofn"));
        cfg.set_ontology(sample_info("b", "/b.ofn"));
        cfg.set_ontology(sample_info("c", "/c.ofn"));
        let mut names: Vec<_> = cfg.list_ontologies().iter().map(|i| i.name.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[test]
    fn set_ontology_overwrites_existing() {
        let mut cfg = OWLMcpConfig::default();
        cfg.set_ontology(sample_info("o", "/old.ofn"));
        cfg.set_ontology(OntologyConfigInfo {
            name: "o".to_string(),
            path: "/new.ofn".to_string(),
            readonly: true,
            ..Default::default()
        });
        let got = cfg.get_ontology("o").unwrap();
        assert_eq!(got.path, "/new.ofn");
        assert!(got.readonly);
    }

    // ── YAML round-trip ───────────────────────────────────────────────────────

    #[test]
    fn yaml_round_trip_empty() {
        let cfg = OWLMcpConfig::default();
        let yaml = serde_yaml::to_string(&cfg).unwrap();
        let loaded: OWLMcpConfig = serde_yaml::from_str(&yaml).unwrap();
        assert!(loaded.ontologies.is_empty());
    }

    #[test]
    fn yaml_round_trip_with_entries() {
        let mut cfg = OWLMcpConfig::default();
        cfg.set_ontology(OntologyConfigInfo {
            name: "pizza".to_string(),
            path: "/data/pizza.ofn".to_string(),
            readonly: false,
            description: Some("Pizza ontology".to_string()),
            metadata_axioms: vec!["SubClassOf(...)".to_string()],
            preferred_serialization: Some("ofn".to_string()),
            annotation_property: None,
        });
        let yaml = serde_yaml::to_string(&cfg).unwrap();
        let loaded: OWLMcpConfig = serde_yaml::from_str(&yaml).unwrap();
        let got = loaded.get_ontology("pizza").unwrap();
        assert_eq!(got.path, "/data/pizza.ofn");
        assert_eq!(got.description.as_deref(), Some("Pizza ontology"));
        assert_eq!(got.metadata_axioms, vec!["SubClassOf(...)"]);
    }

    #[test]
    fn yaml_deserialization_handles_missing_optional_fields() {
        let yaml = r#"
ontologies:
  minimal:
    name: minimal
    path: /minimal.ofn
"#;
        let cfg: OWLMcpConfig = serde_yaml::from_str(yaml).unwrap();
        let got = cfg.get_ontology("minimal").unwrap();
        assert!(!got.readonly);
        assert!(got.description.is_none());
        assert!(got.metadata_axioms.is_empty());
    }

    // ── file save / load ──────────────────────────────────────────────────────

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        // Temporarily override $HOME so config_path() points inside our temp dir.
        // We can't easily do that without env hacks, so instead test save/load
        // by writing/reading manually using the same serde logic.
        let mut cfg = OWLMcpConfig::default();
        cfg.set_ontology(sample_info("t", "/tmp/t.ofn"));

        let yaml = serde_yaml::to_string(&cfg).unwrap();
        let yaml_path = dir.path().join("config.yaml");
        std::fs::write(&yaml_path, &yaml).unwrap();

        let content = std::fs::read_to_string(&yaml_path).unwrap();
        let loaded: OWLMcpConfig = serde_yaml::from_str(&content).unwrap();
        assert!(loaded.get_ontology("t").is_some());
    }
}
