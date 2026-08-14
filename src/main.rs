mod cli;
mod handler;
mod ontology;
mod pitfalls;
mod quality;
mod reasoning;
mod sparql;
mod tools;
mod verbalizer;

use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use handler::OwlMcpHandler;
use ontology::{manager::OntologyManager, watcher::spawn_watcher};
use rmcp::{transport::stdio, ServiceExt};

#[derive(Debug, Clone, ValueEnum)]
enum Transport {
    Stdio,
    Http,
}

#[derive(Debug, Parser)]
#[command(
    name = "owl-mcp",
    version,
    about = "High-performance MCP server and CLI for OWL ontology management"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Start the MCP server (stdio or HTTP transport)
    Serve {
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
    },

    #[command(flatten)]
    Cli(cli::CliCommand),
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();

    let manager = Arc::new(OntologyManager::new());

    match args.command {
        Command::Serve {
            transport,
            host,
            port,
            sse_support,
        } => {
            let _watcher = spawn_watcher(manager.clone());
            let handler = OwlMcpHandler::new(manager);

            match transport {
                Transport::Stdio => {
                    if let Err(e) = run_stdio(handler).await {
                        eprintln!("Server error: {}", e);
                        std::process::exit(1);
                    }
                }
                Transport::Http => {
                    if let Err(e) = run_http(handler, host, port, sse_support).await {
                        eprintln!("Server error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Command::Cli(cmd) => {
            cli::dispatch(cmd, manager).await;
        }
    }
}

async fn run_stdio(handler: OwlMcpHandler) -> Result<(), Box<dyn std::error::Error>> {
    let server = handler.serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}

async fn run_http(
    handler: OwlMcpHandler,
    host: String,
    port: u16,
    _sse_support: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use rmcp::transport::streamable_http_server::{
        session::never::NeverSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    };

    let manager = handler.manager.clone();
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true);

    let service = StreamableHttpService::new(
        move || Ok(OwlMcpHandler::new(manager.clone())),
        std::sync::Arc::new(NeverSessionManager::default()),
        config,
    );

    let app = axum::Router::new().nest_service("/mcp", service);
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    eprintln!("OWL MCP Server listening on http://{}:{}/mcp", host, port);
    axum::serve(listener, app).await?;
    Ok(())
}
