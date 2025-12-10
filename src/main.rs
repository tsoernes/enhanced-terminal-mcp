mod detection;
mod server;
mod tools;

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use server::EnhancedTerminalServer;

#[tokio::main]
async fn main() -> Result<()> {
    let server = EnhancedTerminalServer::new();
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
