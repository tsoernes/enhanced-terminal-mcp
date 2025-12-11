mod detection;
mod server;
mod tools;

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use server::EnhancedTerminalServer;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with RUST_LOG env var support
    // Set RUST_LOG=enhanced_terminal_mcp=debug for detailed logs
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("enhanced_terminal_mcp=info")),
        )
        .init();

    tracing::info!("Enhanced Terminal MCP Server starting");

    let server = EnhancedTerminalServer::new();
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
