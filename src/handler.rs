use std::sync::Arc;

use rmcp::{
    handler::server::wrapper::Parameters, model::*, prompt, prompt_handler, prompt_router, tool,
    tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
};
use serde::Deserialize;

use crate::ontology::manager::OntologyManager;
use crate::ontology::owl_api::OwlApiError;
use crate::tools::*;

#[derive(Clone)]
pub struct OwlMcpHandler {
    pub manager: Arc<OntologyManager>,
    /// Preferred protocol version advertised in `initialize` (stdio vs HTTP differ).
    protocol_version: ProtocolVersion,
}

impl OwlMcpHandler {
    pub fn for_stdio(manager: Arc<OntologyManager>) -> Self {
        Self {
            manager,
            protocol_version: ProtocolVersion::V_2025_11_25,
        }
    }

    pub fn for_http(manager: Arc<OntologyManager>) -> Self {
        Self {
            manager,
            protocol_version: ProtocolVersion::V_2026_07_28,
        }
    }
}

/// Map ontology/tool failures to MCP tool-level errors (`isError: true`) so clients
/// surface the message instead of an opaque JSON-RPC `-32603`.
fn map_tool(result: Result<Vec<String>, OwlApiError>) -> Result<CallToolResult, McpError> {
    match result {
        Ok(lines) => Ok(CallToolResult::success(
            lines.into_iter().map(ContentBlock::text).collect(),
        )),
        Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(e.to_string())])),
    }
}

#[tool_router]
impl OwlMcpHandler {
    #[tool(
        description = "Add a single OWL axiom in functional syntax to the ontology file. E.g. SubClassOf(:Dog :Animal)"
    )]
    async fn add_axiom(
        &self,
        Parameters(params): Parameters<AddAxiom>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(AddAxiom::run(params, &self.manager).await)
    }

    #[tool(
        description = "Add multiple OWL axioms in functional syntax to the ontology file. Stops on the first failure."
    )]
    async fn add_axioms(
        &self,
        Parameters(params): Parameters<AddAxioms>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(AddAxioms::run(params, &self.manager).await)
    }

    #[tool(
        description = "Add a data property assertion (DataPropertyAssertion) where the literal VALUE is supplied as a separate field. Use this instead of add_axiom for long or special-character values."
    )]
    async fn add_data_property_assertion(
        &self,
        Parameters(params): Parameters<AddDataPropertyAssertion>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(AddDataPropertyAssertion::run(params, &self.manager).await)
    }

    #[tool(
        description = "Add an annotation assertion (AnnotationAssertion) where the literal VALUE is supplied as a separate field. Use this instead of add_axiom for long or special-character values."
    )]
    async fn add_annotation_assertion(
        &self,
        Parameters(params): Parameters<AddAnnotationAssertion>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(AddAnnotationAssertion::run(params, &self.manager).await)
    }

    #[tool(
        description = "Add an object property assertion (ObjectPropertyAssertion) linking a subject individual to a target individual via an object property."
    )]
    async fn add_object_property_assertion(
        &self,
        Parameters(params): Parameters<AddObjectPropertyAssertion>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(AddObjectPropertyAssertion::run(params, &self.manager).await)
    }

    #[tool(
        description = "Add a class assertion (ClassAssertion) stating that an individual is an instance of a class."
    )]
    async fn add_class_assertion(
        &self,
        Parameters(params): Parameters<AddClassAssertion>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(AddClassAssertion::run(params, &self.manager).await)
    }

    #[tool(
        description = "Remove a single OWL axiom (given in functional syntax) from the ontology file."
    )]
    async fn remove_axiom(
        &self,
        Parameters(params): Parameters<RemoveAxiom>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(RemoveAxiom::run(params, &self.manager).await)
    }

    #[tool(
        description = "Search axioms in an OWL file using a regex pattern. Returns matching axioms (up to limit)."
    )]
    async fn find_axioms(
        &self,
        Parameters(params): Parameters<FindAxioms>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(FindAxioms::run(params, &self.manager).await)
    }

    #[tool(description = "Return all axioms in the OWL file (up to limit).")]
    async fn get_all_axioms(
        &self,
        Parameters(params): Parameters<GetAllAxioms>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(GetAllAxioms::run(params, &self.manager).await)
    }

    #[tool(
        description = "Add a prefix mapping (e.g. prefix='ex:' uri='http://example.org/') to the ontology file."
    )]
    async fn add_prefix(
        &self,
        Parameters(params): Parameters<AddPrefix>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(AddPrefix::run(params, &self.manager).await)
    }

    #[tool(
        description = "Return the ontology-level annotation axioms (metadata header) for the given OWL file."
    )]
    async fn ontology_metadata(
        &self,
        Parameters(params): Parameters<OntologyMetadata>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(OntologyMetadata::run(params, &self.manager).await)
    }

    #[tool(
        description = "Return all label values for a given IRI or CURIE in the ontology file. Defaults to rdfs:label."
    )]
    async fn get_labels_for_iri(
        &self,
        Parameters(params): Parameters<GetLabelsForIri>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(GetLabelsForIri::run(params, &self.manager).await)
    }

    #[tool(
        description = "Set or update the ontology IRI (and optional version IRI) for an OWL file. Pass iri=null to clear the ontology IRI."
    )]
    async fn set_ontology_iri(
        &self,
        Parameters(params): Parameters<SetOntologyIri>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(SetOntologyIri::run(params, &self.manager).await)
    }

    #[tool(
        description = "Evaluate the quality of an OWL ontology using the OQuaRE framework (based on ISO/IEC 25000 SQuaRE). Returns a JSON report with metrics, characteristics, and an overall 1-5 score."
    )]
    async fn test_quality(
        &self,
        Parameters(params): Parameters<TestQuality>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(TestQuality::run(params, &self.manager).await)
    }

    #[tool(
        description = "Scan an OWL ontology for common modeling pitfalls (inspired by OOPS!). Returns a JSON report listing detected issues, their severity, and affected elements."
    )]
    async fn test_pitfalls(
        &self,
        Parameters(params): Parameters<TestPitfalls>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(TestPitfalls::run(params, &self.manager).await)
    }

    #[tool(
        description = "Run a SPARQL query over one or more OWL files. Returns SPARQL 1.1 JSON results for SELECT/ASK, and N-Triples for CONSTRUCT/DESCRIBE. Set with_reasoning=true to materialize OWL 2 EL entailments before querying."
    )]
    async fn sparql_query(
        &self,
        Parameters(params): Parameters<SparqlQuery>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(SparqlQuery::run(params, &self.manager).await)
    }

    #[tool(
        description = "Run an OWL 2 EL reasoner (whelk; alias elk) over one or more OWL files and report logical consistency. Reasoning is limited to the OWL 2 EL profile."
    )]
    async fn check_consistency(
        &self,
        Parameters(params): Parameters<CheckConsistency>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(CheckConsistency::run(params, &self.manager).await)
    }

    #[tool(
        description = "Convert OWL axioms into Controlled Natural Language (pseudo-text) and a Turtle fragment. If iri is omitted, verbalizes owl:Class and owl:NamedIndividual entities (up to limit)."
    )]
    async fn verbalize(
        &self,
        Parameters(params): Parameters<Verbalize>,
    ) -> Result<CallToolResult, McpError> {
        map_tool(Verbalize::run(params, &self.manager).await)
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct TopicArgs {
    /// The topic to search axioms for
    topic: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SubclassArgs {
    /// The subclass (child class)
    child: String,
    /// The superclass (parent class)
    parent: String,
}

#[prompt_router]
impl OwlMcpHandler {
    #[prompt(
        name = "ask_for_axioms_about",
        description = "Generate a prompt asking what axioms include a given topic string"
    )]
    async fn ask_for_axioms_about(
        &self,
        Parameters(args): Parameters<TopicArgs>,
    ) -> Vec<PromptMessage> {
        vec![PromptMessage::new_text(
            Role::User,
            format!("What axioms include the string '{}'?", args.topic),
        )]
    }

    #[prompt(
        name = "add_subclass_of",
        description = "Generate a prompt to add a subClassOf axiom"
    )]
    async fn add_subclass_of(
        &self,
        Parameters(args): Parameters<SubclassArgs>,
    ) -> Vec<PromptMessage> {
        vec![PromptMessage::new_text(
            Role::User,
            format!(
                "Add a subClassOf axiom where the subclass is '{}' and the superclass is '{}'",
                args.child, args.parent
            ),
        )]
    }
}

#[tool_handler]
#[prompt_handler]
impl ServerHandler for OwlMcpHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_prompts()
                .enable_resources()
                .enable_tools()
                .build(),
        )
        .with_server_info(
            Implementation::new("owl-mcp", env!("CARGO_PKG_VERSION"))
                .with_title("OWL MCP Server")
                .with_description(
                    "High-performance MCP server for OWL ontology management, written in Rust.",
                ),
        )
        .with_protocol_version(self.protocol_version.clone())
        .with_instructions(
            "Use the OWL tools to load, query and modify OWL ontology files. \
             Axioms are expressed in OWL Functional Syntax. \
             resource://active lists ontology files currently cached in this process \
             (not durable state)."
                .to_string(),
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![Resource::new(
                "resource://active",
                "active".to_string(),
            )
            .with_description(
                "List of ontology file paths currently cached in this process (not durable state)"
                    .to_string(),
            )
            .with_mime_type("application/json".to_string())],
            ..Default::default()
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _ctx: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        let uri = &request.uri;
        if uri != "resource://active" {
            return Err(McpError::invalid_params(
                format!("Unknown resource URI: {uri}"),
                None,
            ));
        }

        let paths = self.manager.active_paths().await;
        let text = serde_json::to_string_pretty(&paths).unwrap_or_else(|e| format!("Error: {e}"));
        Ok(ReadResourceResult::new(vec![ResourceContents::text(text, uri.clone())]).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ontology::owl_api::OwlApiError;

    #[test]
    fn stdio_advertises_2025_11_25() {
        let handler = OwlMcpHandler::for_stdio(Arc::new(OntologyManager::new()));
        assert_eq!(
            handler.get_info().protocol_version,
            ProtocolVersion::V_2025_11_25
        );
    }

    #[test]
    fn http_advertises_2026_07_28() {
        let handler = OwlMcpHandler::for_http(Arc::new(OntologyManager::new()));
        assert_eq!(
            handler.get_info().protocol_version,
            ProtocolVersion::V_2026_07_28
        );
    }

    #[test]
    fn map_tool_returns_is_error_not_protocol_error() {
        let err = OwlApiError::Parse("missing file".into());
        let result = map_tool(Err(err)).unwrap();
        assert_eq!(result.is_error, Some(true));
        assert!(!result.content.is_empty());
    }
}
