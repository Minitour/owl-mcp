use std::collections::HashSet;
use std::sync::Arc;

use rust_mcp_sdk::{
    macros::{mcp_tool, JsonSchema},
    schema::{schema_utils::CallToolError, CallToolResult, TextContent},
    tool_box,
};
use tokio::sync::Mutex;

use crate::config::OntologyConfigInfo;
use crate::ontology::manager::OntologyManager;

pub type Manager = Arc<Mutex<OntologyManager>>;

fn text_result(s: impl Into<String>) -> Result<CallToolResult, CallToolError> {
    Ok(CallToolResult::text_content(vec![TextContent::from(
        s.into(),
    )]))
}

fn list_result(items: Vec<String>) -> Result<CallToolResult, CallToolError> {
    let content: Vec<TextContent> = items.into_iter().map(TextContent::from).collect();
    Ok(CallToolResult::text_content(content))
}

// ── Path-based axiom operations ───────────────────────────────────────────────

#[mcp_tool(
    name = "add_axiom",
    description = "Add a single OWL axiom in functional syntax to the ontology file. E.g. SubClassOf(:Dog :Animal)"
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct AddAxiom {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Axiom in OWL functional syntax, e.g. SubClassOf(:Dog :Animal)
    pub axiom_str: String,
}

impl AddAxiom {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load(&params.owl_file_path, false, true)
            .map_err(CallToolError::new)?;
        let msg = api
            .add_axiom(&params.axiom_str)
            .map_err(CallToolError::new)?;
        text_result(msg)
    }
}

#[mcp_tool(
    name = "add_axioms",
    description = "Add multiple OWL axioms in functional syntax to the ontology file. Stops on the first failure."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct AddAxioms {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// List of axioms in OWL functional syntax
    pub axiom_strs: Vec<String>,
}

impl AddAxioms {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load(&params.owl_file_path, false, true)
            .map_err(CallToolError::new)?;
        let msg = api
            .add_axioms(&params.axiom_strs)
            .map_err(CallToolError::new)?;
        text_result(msg)
    }
}

#[mcp_tool(
    name = "remove_axiom",
    description = "Remove a single OWL axiom (given in functional syntax) from the ontology file."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct RemoveAxiom {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Axiom in OWL functional syntax to remove
    pub axiom_str: String,
}

impl RemoveAxiom {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load(&params.owl_file_path, false, false)
            .map_err(CallToolError::new)?;
        let msg = api
            .remove_axiom(&params.axiom_str)
            .map_err(CallToolError::new)?;
        text_result(msg)
    }
}

#[mcp_tool(
    name = "find_axioms",
    description = "Search axioms in an OWL file using a regex pattern. Returns matching axioms (up to limit)."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct FindAxioms {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Regex pattern to match against functional-syntax axiom strings
    pub pattern: String,
    /// Maximum number of results to return (default: 100)
    #[serde(default = "default_limit")]
    pub limit: u64,
    /// If true, append human-readable labels after ## comments
    #[serde(default)]
    pub include_labels: bool,
    /// IRI or CURIE of the annotation property to use for labels (default: rdfs:label)
    pub annotation_property: Option<String>,
}

impl FindAxioms {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load(&params.owl_file_path, false, false)
            .map_err(CallToolError::new)?;
        let results = api
            .find_axioms(
                &params.pattern,
                params.limit as usize,
                params.include_labels,
                params.annotation_property.as_deref(),
            )
            .map_err(CallToolError::new)?;
        list_result(results)
    }
}

#[mcp_tool(
    name = "get_all_axioms",
    description = "Return all axioms in the OWL file (up to limit)."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct GetAllAxioms {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Maximum number of results to return (default: 100)
    #[serde(default = "default_limit")]
    pub limit: u64,
    /// If true, append human-readable labels after ## comments
    #[serde(default)]
    pub include_labels: bool,
    /// IRI or CURIE of the annotation property to use for labels (default: rdfs:label)
    pub annotation_property: Option<String>,
}

impl GetAllAxioms {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load(&params.owl_file_path, false, false)
            .map_err(CallToolError::new)?;
        let results = api.get_all_axioms(
            params.limit as usize,
            params.include_labels,
            params.annotation_property.as_deref(),
        );
        list_result(results)
    }
}

// ── Path-based metadata operations ────────────────────────────────────────────

#[mcp_tool(
    name = "add_prefix",
    description = "Add a prefix mapping (e.g. prefix='ex:' uri='http://example.org/') to the ontology file."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct AddPrefix {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Prefix name including colon, e.g. 'ex:'
    pub prefix: String,
    /// The full IRI the prefix expands to, e.g. 'http://example.org/'
    pub uri: String,
}

impl AddPrefix {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load(&params.owl_file_path, false, true)
            .map_err(CallToolError::new)?;
        let msg = api
            .add_prefix(&params.prefix, &params.uri)
            .map_err(CallToolError::new)?;
        text_result(msg)
    }
}

#[mcp_tool(
    name = "ontology_metadata",
    description = "Return the ontology-level annotation axioms (metadata header) for the given OWL file."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct OntologyMetadata {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
}

impl OntologyMetadata {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load(&params.owl_file_path, false, false)
            .map_err(CallToolError::new)?;
        let results = api.ontology_metadata();
        list_result(results)
    }
}

#[mcp_tool(
    name = "get_labels_for_iri",
    description = "Return all label values for a given IRI or CURIE in the ontology file. Defaults to rdfs:label."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct GetLabelsForIri {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Full IRI or CURIE (e.g. 'ex:Dog' or '<http://example.org/Dog>')
    pub iri: String,
    /// IRI or CURIE of the annotation property (default: rdfs:label)
    pub annotation_property: Option<String>,
}

impl GetLabelsForIri {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load(&params.owl_file_path, false, false)
            .map_err(CallToolError::new)?;
        let results = api.get_labels_for_iri(&params.iri, params.annotation_property.as_deref());
        list_result(results)
    }
}

// ── Name-based variants ────────────────────────────────────────────────────────

#[mcp_tool(
    name = "add_axiom_by_name",
    description = "Add an OWL axiom to a configured ontology referenced by its registered name."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct AddAxiomByName {
    /// Name of a configured ontology (from list_configured_ontologies)
    pub ontology_name: String,
    /// Axiom in OWL functional syntax
    pub axiom_str: String,
}

impl AddAxiomByName {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load_by_name(&params.ontology_name)
            .map_err(CallToolError::new)?;
        let msg = api
            .add_axiom(&params.axiom_str)
            .map_err(CallToolError::new)?;
        text_result(msg)
    }
}

#[mcp_tool(
    name = "remove_axiom_by_name",
    description = "Remove an OWL axiom from a configured ontology referenced by its registered name."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct RemoveAxiomByName {
    /// Name of a configured ontology
    pub ontology_name: String,
    /// Axiom in OWL functional syntax to remove
    pub axiom_str: String,
}

impl RemoveAxiomByName {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load_by_name(&params.ontology_name)
            .map_err(CallToolError::new)?;
        let msg = api
            .remove_axiom(&params.axiom_str)
            .map_err(CallToolError::new)?;
        text_result(msg)
    }
}

#[mcp_tool(
    name = "find_axioms_by_name",
    description = "Search axioms in a configured ontology by name using a regex pattern."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct FindAxiomsByName {
    /// Name of a configured ontology
    pub ontology_name: String,
    /// Regex pattern to match against functional-syntax axiom strings
    pub pattern: String,
    /// Maximum number of results to return (default: 100)
    #[serde(default = "default_limit")]
    pub limit: u64,
    /// If true, append human-readable labels after ## comments
    #[serde(default)]
    pub include_labels: bool,
    /// IRI or CURIE of the annotation property to use for labels (default: rdfs:label)
    pub annotation_property: Option<String>,
}

impl FindAxiomsByName {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load_by_name(&params.ontology_name)
            .map_err(CallToolError::new)?;
        let results = api
            .find_axioms(
                &params.pattern,
                params.limit as usize,
                params.include_labels,
                params.annotation_property.as_deref(),
            )
            .map_err(CallToolError::new)?;
        list_result(results)
    }
}

#[mcp_tool(
    name = "add_prefix_by_name",
    description = "Add a prefix mapping to a configured ontology referenced by its registered name."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct AddPrefixByName {
    /// Name of a configured ontology
    pub ontology_name: String,
    /// Prefix name including colon, e.g. 'ex:'
    pub prefix: String,
    /// The full IRI the prefix expands to
    pub uri: String,
}

impl AddPrefixByName {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load_by_name(&params.ontology_name)
            .map_err(CallToolError::new)?;
        let msg = api
            .add_prefix(&params.prefix, &params.uri)
            .map_err(CallToolError::new)?;
        text_result(msg)
    }
}

#[mcp_tool(
    name = "get_labels_for_iri_by_name",
    description = "Return all label values for an IRI or CURIE in a configured ontology referenced by name."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct GetLabelsForIriByName {
    /// Name of a configured ontology
    pub ontology_name: String,
    /// Full IRI or CURIE
    pub iri: String,
    /// IRI or CURIE of the annotation property (default: rdfs:label)
    pub annotation_property: Option<String>,
}

impl GetLabelsForIriByName {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load_by_name(&params.ontology_name)
            .map_err(CallToolError::new)?;
        let results = api.get_labels_for_iri(&params.iri, params.annotation_property.as_deref());
        list_result(results)
    }
}

// ── Configuration tools ────────────────────────────────────────────────────────

#[mcp_tool(
    name = "list_configured_ontologies",
    description = "List all ontologies registered in the ~/.owl-mcp/config.yaml configuration."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct ListConfiguredOntologies {}

impl ListConfiguredOntologies {
    pub async fn run_tool(
        _params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mgr = manager.lock().await;
        let ontologies = mgr.list_configured_ontologies();
        let results: Vec<String> = ontologies
            .iter()
            .map(|o| {
                serde_json::to_string_pretty(o)
                    .unwrap_or_else(|_| format!("{}: {}", o.name, o.path))
            })
            .collect();
        if results.is_empty() {
            text_result("No configured ontologies.")
        } else {
            list_result(results)
        }
    }
}

#[mcp_tool(
    name = "configure_ontology",
    description = "Add or update a named ontology in the configuration. The ontology will be reloaded if already active."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct ConfigureOntology {
    /// Unique name for this ontology
    pub name: String,
    /// Absolute path to the OWL file
    pub path: String,
    /// Optional metadata axioms to add when loading
    pub metadata_axioms: Option<Vec<String>>,
    /// If true, the ontology cannot be modified through this server
    #[serde(default)]
    pub readonly: bool,
    /// Human-readable description
    pub description: Option<String>,
    /// Preferred serialization format (e.g. 'ofn', 'rdf')
    pub preferred_serialization: Option<String>,
    /// IRI or CURIE of the annotation property for labels
    pub annotation_property: Option<String>,
}

impl ConfigureOntology {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let info = OntologyConfigInfo {
            name: params.name,
            path: params.path,
            metadata_axioms: params.metadata_axioms.unwrap_or_default(),
            readonly: params.readonly,
            description: params.description,
            preferred_serialization: params.preferred_serialization,
            annotation_property: params.annotation_property,
        };
        let mut mgr = manager.lock().await;
        let msg = mgr.configure_ontology(info).map_err(CallToolError::new)?;
        text_result(msg)
    }
}

#[mcp_tool(
    name = "remove_ontology_config",
    description = "Remove an ontology from the configuration (and stop it if currently active)."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct RemoveOntologyConfig {
    /// Name of the configured ontology to remove
    pub name: String,
}

impl RemoveOntologyConfig {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let msg = mgr
            .remove_ontology_config(&params.name)
            .map_err(CallToolError::new)?;
        text_result(msg)
    }
}

#[mcp_tool(
    name = "get_ontology_config",
    description = "Retrieve the configuration details for a specific named ontology."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct GetOntologyConfig {
    /// Name of the configured ontology
    pub name: String,
}

impl GetOntologyConfig {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mgr = manager.lock().await;
        match mgr.get_ontology_config(&params.name) {
            Some(info) => {
                let json = serde_json::to_string_pretty(&info)
                    .unwrap_or_else(|_| format!("{}: {}", info.name, info.path));
                text_result(json)
            }
            None => text_result(format!("No configured ontology named '{}'", params.name)),
        }
    }
}

#[mcp_tool(
    name = "register_ontology_in_config",
    description = "Register an OWL file into the configuration so it can be referenced by name in future sessions."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct RegisterOntologyInConfig {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Name to register under (defaults to the file stem)
    pub name: Option<String>,
    /// If true, the ontology cannot be modified
    pub readonly: Option<bool>,
    /// Human-readable description
    pub description: Option<String>,
    /// Preferred serialization format
    pub preferred_serialization: Option<String>,
    /// IRI or CURIE of the annotation property for labels
    pub annotation_property: Option<String>,
}

impl RegisterOntologyInConfig {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let msg = mgr
            .register_in_config(
                &params.owl_file_path,
                params.name,
                params.readonly,
                params.description,
                params.preferred_serialization,
                params.annotation_property,
            )
            .map_err(CallToolError::new)?;
        text_result(msg)
    }
}

#[mcp_tool(
    name = "load_and_register_ontology",
    description = "Load (or create) an OWL file, optionally add metadata axioms, and register it in the configuration."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct LoadAndRegisterOntology {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Name to register under (defaults to the file stem)
    pub name: Option<String>,
    /// If true, the ontology cannot be modified
    #[serde(default)]
    pub readonly: bool,
    /// If true, create the file if it does not exist (default: true)
    #[serde(default = "default_true")]
    pub create_if_not_exists: bool,
    /// Human-readable description
    pub description: Option<String>,
    /// Preferred serialization format
    pub preferred_serialization: Option<String>,
    /// Optional metadata axioms to add when loading
    pub metadata_axioms: Option<Vec<String>>,
    /// IRI or CURIE of the annotation property for labels
    pub annotation_property: Option<String>,
}

impl LoadAndRegisterOntology {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let msg = mgr
            .load_and_register(
                &params.owl_file_path,
                params.name,
                params.readonly,
                params.create_if_not_exists,
                params.description,
                params.preferred_serialization,
                params.metadata_axioms,
                params.annotation_property,
            )
            .map_err(CallToolError::new)?;
        text_result(msg)
    }
}

// ── Ontology IRI ───────────────────────────────────────────────────────────

#[mcp_tool(
    name = "set_ontology_iri",
    description = "Set or update the ontology IRI (and optional version IRI) for an OWL file. \
    Pass iri=null to clear the ontology IRI."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct SetOntologyIri {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// The ontology IRI to set (e.g. 'http://example.org/my-ontology')
    pub iri: Option<String>,
    /// Optional version IRI (e.g. 'http://example.org/my-ontology/1.0')
    pub version_iri: Option<String>,
}

impl SetOntologyIri {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load(&params.owl_file_path, false, true)
            .map_err(CallToolError::new)?;
        let msg = api
            .set_ontology_iri(params.iri.as_deref(), params.version_iri.as_deref())
            .map_err(CallToolError::new)?;
        text_result(msg)
    }
}

#[mcp_tool(
    name = "set_ontology_iri_by_name",
    description = "Set or update the ontology IRI (and optional version IRI) for a configured ontology \
    referenced by its registered name."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct SetOntologyIriByName {
    /// Name of a configured ontology
    pub ontology_name: String,
    /// The ontology IRI to set
    pub iri: Option<String>,
    /// Optional version IRI
    pub version_iri: Option<String>,
}

impl SetOntologyIriByName {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load_by_name(&params.ontology_name)
            .map_err(CallToolError::new)?;
        let msg = api
            .set_ontology_iri(params.iri.as_deref(), params.version_iri.as_deref())
            .map_err(CallToolError::new)?;
        text_result(msg)
    }
}

// ── Pitfall scanner ────────────────────────────────────────────────────────────

#[mcp_tool(
    name = "test_pitfalls",
    description = "Scan an OWL ontology for common modeling pitfalls (inspired by OOPS! - OntOlogy Pitfall Scanner). \
    Returns a JSON report listing detected issues, their severity, and affected elements. \
    31 checks: P02 (synonym classes), P03 (\"is\" relationship), P04 (unconnected elements), \
    P05 (wrong inverses), P06 (class hierarchy cycles), P07 (merged concepts), \
    P08 (missing annotations), P10 (missing disjointness), P11 (missing domain/range), \
    P12 (undeclared equivalent properties), P13 (missing inverses, with sub-variants Y/N/S), \
    P19 (multiple domains/ranges), P20 (misused annotations), P21 (miscellaneous class), \
    P22 (inconsistent naming), P24 (recursive definitions), P25 (self-inverse), \
    P26 (inverse of symmetric), P27 (wrong equivalent properties), P28 (wrong symmetric), \
    P29 (wrong transitive), P30 (undeclared equivalent classes), P31 (wrong equivalent classes), \
    P32 (duplicate labels), P33 (single-property chain), P34 (untyped class), \
    P35 (untyped property), P36 (URI file extension), P38 (no ontology declaration), \
    P39 (ambiguous namespace), P41 (no license)."
)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct TestPitfalls {
    /// Absolute path to the OWL file (provide either this or ontology_name)
    pub owl_file_path: Option<String>,
    /// Name of a configured ontology (provide either this or owl_file_path)
    pub ontology_name: Option<String>,
    /// Comma-separated list of pitfall IDs to check (e.g. "P04,P08,P11"). If omitted, all checks run.
    pub pitfalls: Option<String>,
}

impl TestPitfalls {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = if let Some(name) = &params.ontology_name {
            mgr.get_or_load_by_name(name).map_err(CallToolError::new)?
        } else if let Some(path) = &params.owl_file_path {
            mgr.get_or_load(path, false, false)
                .map_err(CallToolError::new)?
        } else {
            return Err(CallToolError::new(
                crate::ontology::owl_api::OwlApiError::Parse(
                    "Provide either owl_file_path or ontology_name".to_string(),
                ),
            ));
        };

        let filter = params.pitfalls.as_ref().map(|s| {
            s.split(',')
                .map(|p| p.trim().to_uppercase())
                .collect::<HashSet<_>>()
        });

        let report = crate::pitfalls::scan(&api.ontology, filter.as_ref());
        let json = serde_json::to_string_pretty(&report).map_err(|e| {
            CallToolError::new(crate::ontology::owl_api::OwlApiError::Parse(e.to_string()))
        })?;

        text_result(json)
    }
}

// ── Tool box (enum + dispatch) ─────────────────────────────────────────────────

fn default_limit() -> u64 {
    100
}

fn default_true() -> bool {
    true
}

tool_box!(
    OwlTools,
    [
        AddAxiom,
        AddAxioms,
        RemoveAxiom,
        FindAxioms,
        GetAllAxioms,
        AddPrefix,
        OntologyMetadata,
        GetLabelsForIri,
        SetOntologyIri,
        AddAxiomByName,
        RemoveAxiomByName,
        FindAxiomsByName,
        AddPrefixByName,
        GetLabelsForIriByName,
        SetOntologyIriByName,
        ListConfiguredOntologies,
        ConfigureOntology,
        RemoveOntologyConfig,
        GetOntologyConfig,
        RegisterOntologyInConfig,
        LoadAndRegisterOntology,
        TestPitfalls,
    ]
);
