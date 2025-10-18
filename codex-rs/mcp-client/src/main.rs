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

use std::ffi::OsString;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_mcp_client::McpClient;
use mcp_types::ClientCapabilities;
use mcp_types::Implementation;
use mcp_types::InitializeRequestParams;
use mcp_types::ListToolsRequestParams;
use mcp_types::MCP_SCHEMA_VERSION;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let default_level = "debug";
    let _ = tracing_subscriber::fmt()
        // Fallback to the `default_level` log filter if the environment
        // variable is not set _or_ contains an invalid value
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new(default_level))
                .unwrap_or_else(|_| EnvFilter::new(default_level)),
        )
        .with_writer(std::io::stderr)
        .try_init();

    // Collect command-line arguments excluding the program name itself.
    let mut args: Vec<OsString> = std::env::args_os().skip(1).collect();

    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        eprintln!("Usage: mcp-client <program> [args..]\n\nExample: mcp-client codex-mcp-server");
        std::process::exit(1);
    }
    let original_args = args.clone();

    // Spawn the subprocess and connect the client.
    let program = args.remove(0);
    let env = None;
    let client = McpClient::new_stdio_client(program, args, env, &[], None)
        .await
        .with_context(|| format!("failed to spawn subprocess: {original_args:?}"))?;

    let params = InitializeRequestParams {
        capabilities: ClientCapabilities {
            experimental: None,
            roots: None,
            sampling: None,
            elicitation: None,
        },
        client_info: Implementation {
            name: "codex-mcp-client".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            title: Some("Codex".to_string()),
            // This field is used by Codex when it is an MCP server: it should
            // not be used when Codex is an MCP client.
            user_agent: None,
        },
        protocol_version: MCP_SCHEMA_VERSION.to_owned(),
    };
    let timeout = Some(Duration::from_secs(10));
    let response = client.initialize(params, timeout).await?;
    eprintln!("initialize response: {response:?}");

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
