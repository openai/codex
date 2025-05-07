//! Simple command-line utility to exercise `McpClient`.
//!
//! Example usage:
//!
//! ```bash
//! cargo run -p codex-mcp-client -- `codex-mcp-server`
//! ```
//!
//! Any additional arguments after the first one are forwarded to the spawned
//! program. The utility connects, issues a `tools/list` request and prints the
//! server's response as pretty JSON.

use anyhow::Context;
use anyhow::Result;
use codex_mcp_client::McpClient;
use mcp_types::ListToolsRequestParams;

#[tokio::main]
async fn main() -> Result<()> {
    // Collect command-line arguments excluding the program name itself.
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        eprintln!("Usage: mcp-client <program> [args..]\n\nExample: mcp-client codex-mcp-server");
        std::process::exit(1);
    }
    let original_args = args.clone();

    // Spawn the subprocess and connect the client.
    let program = args.remove(0);
    let env = None;
    let client = McpClient::new_stdio_client(program, args, env)
        .await
        .with_context(|| format!("failed to spawn subprocess: {original_args:?}"))?;

    // Issue `tools/list` request (no params).
    let timeout = None;
    let tools = client
        .list_tools(None::<ListToolsRequestParams>, timeout)
        .await
        .context("tools/list request failed")?;

    // Print the result in a human readable form.
    println!("{}", serde_json::to_string_pretty(&tools)?);

    Ok(())
}
