use std::collections::HashSet;
use std::sync::Arc;

use rust_mcp_sdk::{
    macros::{mcp_tool, JsonSchema},
    schema::{schema_utils::CallToolError, CallToolResult, TextContent},
    tool_box,
};
use tokio::sync::Mutex;

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

// ── Axiom operations ──────────────────────────────────────────────────────────

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

// ── Metadata operations ───────────────────────────────────────────────────────

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
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Comma-separated list of pitfall IDs to check (e.g. "P04,P08,P11"). If omitted, all checks run.
    pub pitfalls: Option<String>,
}

impl TestPitfalls {
    pub async fn run_tool(
        params: Self,
        manager: &Manager,
    ) -> Result<CallToolResult, CallToolError> {
        let mut mgr = manager.lock().await;
        let api = mgr
            .get_or_load(&params.owl_file_path, false, false)
            .map_err(CallToolError::new)?;

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
        TestPitfalls,
    ]
);
