mod config;
mod handler;
mod ontology;
mod tools;

use std::sync::Arc;

use clap::{Parser, ValueEnum};
use handler::OwlMcpHandler;
use ontology::{manager::OntologyManager, watcher::spawn_watcher};
use rust_mcp_sdk::{
    mcp_server::{server_runtime, McpServerOptions},
    schema::{
        Implementation, InitializeResult, ProtocolVersion, ServerCapabilities,
        ServerCapabilitiesPrompts, ServerCapabilitiesResources, ServerCapabilitiesTools,
    },
    McpServer, StdioTransport, ToMcpServerHandler, TransportOptions,
};
use tokio::sync::Mutex;

#[derive(Debug, Clone, ValueEnum)]
enum Transport {
    Stdio,
    Http,
}

#[derive(Debug, Parser)]
#[command(
    name = "owl-mcp",
    version,
    about = "High-performance MCP server for OWL ontology management"
)]
struct Args {
    /// Transport to use
    #[arg(long, default_value = "stdio", env = "OWL_MCP_TRANSPORT")]
    transport: Transport,

    /// Host to bind (HTTP transport only)
    #[arg(long, default_value = "127.0.0.1", env = "OWL_MCP_HOST")]
    host: String,

    /// Port to bind (HTTP transport only)
    #[arg(long, default_value_t = 8080, env = "OWL_MCP_PORT")]
    port: u16,

    /// Enable legacy SSE endpoint alongside Streamable HTTP (HTTP transport only)
    #[arg(long, default_value_t = true, env = "OWL_MCP_SSE_SUPPORT")]
    sse_support: bool,
}

fn server_info() -> InitializeResult {
    InitializeResult {
        server_info: Implementation {
            name: "owl-mcp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            title: Some("OWL MCP Server".to_string()),
            description: Some(
                "High-performance MCP server for OWL ontology management, written in Rust."
                    .to_string(),
            ),
            icons: vec![],
            website_url: None,
        },
        capabilities: ServerCapabilities {
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            resources: Some(ServerCapabilitiesResources {
                subscribe: None,
                list_changed: None,
            }),
            prompts: Some(ServerCapabilitiesPrompts { list_changed: None }),
            experimental: None,
            logging: None,
            completions: None,
            tasks: None,
        },
        instructions: Some(
            "Use the OWL tools to load, query and modify OWL ontology files. \
             Axioms are expressed in OWL Functional Syntax."
                .to_string(),
        ),
        meta: None,
        protocol_version: ProtocolVersion::V2025_11_25.to_string(),
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing (logs go to stderr so they don't interfere with stdio MCP transport)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();

    // Build shared state
    let manager = match OntologyManager::new() {
        Ok(m) => Arc::new(Mutex::new(m)),
        Err(e) => {
            eprintln!("Failed to initialize OntologyManager: {}", e);
            std::process::exit(1);
        }
    };

    // Spawn file watcher background task
    let _watcher = spawn_watcher(manager.clone());

    let handler = OwlMcpHandler::new(manager);

    match args.transport {
        Transport::Stdio => {
            if let Err(e) = run_stdio(handler).await {
                eprintln!("Server error: {}", e);
                std::process::exit(1);
            }
        }
        Transport::Http => {
            if let Err(e) = run_http(handler, args.host, args.port, args.sse_support).await {
                eprintln!("Server error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

async fn run_stdio(handler: OwlMcpHandler) -> Result<(), Box<dyn std::error::Error>> {
    let transport = StdioTransport::new(TransportOptions::default())?;
    let server = server_runtime::create_server(McpServerOptions {
        server_details: server_info(),
        handler: handler.to_mcp_server_handler(),
        task_store: None,
        client_task_store: None,
        transport,
    });
    server.start().await?;
    Ok(())
}

async fn run_http(
    handler: OwlMcpHandler,
    host: String,
    port: u16,
    sse_support: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use rust_mcp_sdk::{
        event_store::InMemoryEventStore,
        mcp_server::{hyper_server, HyperServerOptions},
    };

    let server = hyper_server::create_server(
        server_info(),
        handler.to_mcp_server_handler(),
        HyperServerOptions {
            host: host.clone(),
            port,
            sse_support,
            event_store: Some(Arc::new(InMemoryEventStore::default())),
            ..Default::default()
        },
    );

    eprintln!("OWL MCP Server listening on http://{}:{}", host, port);
    if sse_support {
        eprintln!("SSE endpoint enabled (legacy support)");
    }

    server.start().await?;
    Ok(())
}
