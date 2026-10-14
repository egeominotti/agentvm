//! Stdio MCP frontend: the existing daemon owns all VM and supervisor lifetimes.
use agentvm::mcp::{McpServer, client::ApiClient};
use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut origin = std::env::var("AGENTVM_URL").unwrap_or_else(|_| "http://127.0.0.1:7777".into());
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--url" => origin = args.next().ok_or_else(|| anyhow::anyhow!("--url requires a loopback URL"))?,
            "--help" | "-h" => {
                eprintln!(
                    "agentvm-mcp [--url http://127.0.0.1:7777]\nAGENTVM_URL also selects the existing daemon. MCP protocol uses stdin/stdout."
                );
                return Ok(());
            }
            _ => anyhow::bail!("unknown option (use --help)"),
        }
    }
    let client = ApiClient::new(&origin).map_err(anyhow::Error::msg)?;
    let service = McpServer::new(client).serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
