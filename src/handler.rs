use std::sync::Arc;

use async_trait::async_trait;
use rust_mcp_sdk::{
    McpServer,
    mcp_server::ServerHandler,
    schema::{
        CallToolRequestParams, ContentBlock, GetPromptRequestParams, GetPromptResult,
        ListPromptsResult, ListResourcesResult, ListToolsResult, PaginatedRequestParams, Prompt,
        PromptArgument, PromptMessage, ReadResourceContent, ReadResourceRequestParams,
        ReadResourceResult, Resource, Role, RpcError, TextContent, TextResourceContents,
        schema_utils::CallToolError,
        CallToolResult,
    },
};
use tokio::sync::Mutex;

use crate::ontology::manager::OntologyManager;
use crate::tools::*;

pub struct OwlMcpHandler {
    pub manager: Arc<Mutex<OntologyManager>>,
}

impl OwlMcpHandler {
    pub fn new(manager: Arc<Mutex<OntologyManager>>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl ServerHandler for OwlMcpHandler {
    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: OwlTools::tools(),
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        let tool = OwlTools::try_from(params).map_err(CallToolError::new)?;
        let mgr = &self.manager;

        match tool {
            OwlTools::AddAxiom(p) => AddAxiom::run_tool(p, mgr).await,
            OwlTools::AddAxioms(p) => AddAxioms::run_tool(p, mgr).await,
            OwlTools::RemoveAxiom(p) => RemoveAxiom::run_tool(p, mgr).await,
            OwlTools::FindAxioms(p) => FindAxioms::run_tool(p, mgr).await,
            OwlTools::GetAllAxioms(p) => GetAllAxioms::run_tool(p, mgr).await,
            OwlTools::AddPrefix(p) => AddPrefix::run_tool(p, mgr).await,
            OwlTools::OntologyMetadata(p) => OntologyMetadata::run_tool(p, mgr).await,
            OwlTools::GetLabelsForIri(p) => GetLabelsForIri::run_tool(p, mgr).await,
            OwlTools::AddAxiomByName(p) => AddAxiomByName::run_tool(p, mgr).await,
            OwlTools::RemoveAxiomByName(p) => RemoveAxiomByName::run_tool(p, mgr).await,
            OwlTools::FindAxiomsByName(p) => FindAxiomsByName::run_tool(p, mgr).await,
            OwlTools::AddPrefixByName(p) => AddPrefixByName::run_tool(p, mgr).await,
            OwlTools::GetLabelsForIriByName(p) => GetLabelsForIriByName::run_tool(p, mgr).await,
            OwlTools::ListConfiguredOntologies(p) => {
                ListConfiguredOntologies::run_tool(p, mgr).await
            }
            OwlTools::ConfigureOntology(p) => ConfigureOntology::run_tool(p, mgr).await,
            OwlTools::RemoveOntologyConfig(p) => RemoveOntologyConfig::run_tool(p, mgr).await,
            OwlTools::GetOntologyConfig(p) => GetOntologyConfig::run_tool(p, mgr).await,
            OwlTools::RegisterOntologyInConfig(p) => {
                RegisterOntologyInConfig::run_tool(p, mgr).await
            }
            OwlTools::LoadAndRegisterOntology(p) => {
                LoadAndRegisterOntology::run_tool(p, mgr).await
            }
        }
    }

    async fn handle_list_resources_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListResourcesResult, RpcError> {
        Ok(ListResourcesResult {
            resources: vec![
                Resource {
                    uri: "resource://config/ontologies".to_string(),
                    name: "config/ontologies".to_string(),
                    description: Some(
                        "Full OWLMcpConfig with all configured ontologies".to_string(),
                    ),
                    mime_type: Some("application/json".to_string()),
                    annotations: None,
                    meta: None,
                    size: None,
                    title: None,
                    icons: vec![],
                },
                Resource {
                    uri: "resource://active".to_string(),
                    name: "active".to_string(),
                    description: Some("List of currently loaded ontology file paths".to_string()),
                    mime_type: Some("application/json".to_string()),
                    annotations: None,
                    meta: None,
                    size: None,
                    title: None,
                    icons: vec![],
                },
            ],
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_read_resource_request(
        &self,
        params: ReadResourceRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ReadResourceResult, RpcError> {
        let uri = &params.uri;
        let mgr = self.manager.lock().await;

        let text = if uri == "resource://config/ontologies" {
            serde_json::to_string_pretty(&mgr.config)
                .unwrap_or_else(|e| format!("Error serializing config: {}", e))
        } else if uri == "resource://active" {
            serde_json::to_string_pretty(&mgr.active_paths())
                .unwrap_or_else(|e| format!("Error: {}", e))
        } else if let Some(name) = uri.strip_prefix("resource://config/ontology/") {
            match mgr.get_ontology_config(name) {
                Some(info) => serde_json::to_string_pretty(&info)
                    .unwrap_or_else(|e| format!("Error: {}", e)),
                None => format!("No configured ontology named '{}'", name),
            }
        } else {
            return Err(RpcError::invalid_params()
                .with_message(format!("Unknown resource URI: {}", uri)));
        };

        Ok(ReadResourceResult {
            contents: vec![ReadResourceContent::TextResourceContents(
                TextResourceContents {
                    uri: uri.clone(),
                    mime_type: Some("application/json".to_string()),
                    text,
                    meta: None,
                },
            )],
            meta: None,
        })
    }

    async fn handle_list_prompts_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListPromptsResult, RpcError> {
        Ok(ListPromptsResult {
            prompts: vec![
                Prompt {
                    name: "ask_for_axioms_about".to_string(),
                    description: Some(
                        "Generate a prompt asking what axioms include a given topic string"
                            .to_string(),
                    ),
                    arguments: vec![PromptArgument {
                        name: "topic".to_string(),
                        description: Some("The topic to search axioms for".to_string()),
                        required: Some(true),
                        title: None,
                    }],
                    meta: None,
                    title: None,
                    icons: vec![],
                },
                Prompt {
                    name: "add_subclass_of".to_string(),
                    description: Some("Generate a prompt to add a subClassOf axiom".to_string()),
                    arguments: vec![
                        PromptArgument {
                            name: "child".to_string(),
                            description: Some("The subclass (child class)".to_string()),
                            required: Some(true),
                            title: None,
                        },
                        PromptArgument {
                            name: "parent".to_string(),
                            description: Some("The superclass (parent class)".to_string()),
                            required: Some(true),
                            title: None,
                        },
                    ],
                    meta: None,
                    title: None,
                    icons: vec![],
                },
            ],
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_get_prompt_request(
        &self,
        params: GetPromptRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<GetPromptResult, RpcError> {
        let args = params.arguments.unwrap_or_default();

        let message_text = match params.name.as_str() {
            "ask_for_axioms_about" => {
                let topic = args.get("topic").map(|s| s.as_str()).unwrap_or("?");
                format!("What axioms include the string '{}'?", topic)
            }
            "add_subclass_of" => {
                let child = args.get("child").map(|s| s.as_str()).unwrap_or("?");
                let parent = args.get("parent").map(|s| s.as_str()).unwrap_or("?");
                format!(
                    "Add a subClassOf axiom where the subclass is '{}' and the superclass is '{}'",
                    child, parent
                )
            }
            _ => {
                return Err(RpcError::invalid_params()
                    .with_message(format!("Unknown prompt: {}", params.name)));
            }
        };

        Ok(GetPromptResult {
            description: None,
            messages: vec![PromptMessage {
                role: Role::User,
                content: ContentBlock::TextContent(TextContent::from(message_text)),
            }],
            meta: None,
        })
    }
}
