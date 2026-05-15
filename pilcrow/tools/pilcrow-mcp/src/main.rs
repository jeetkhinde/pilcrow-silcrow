use anyhow::Result;
use pilcrow_mcp::server::PilcrowServer;
use rmcp::{transport::stdio, ServiceExt};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let server = PilcrowServer::new()?.serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
