//! Gemini CLI MCP Server
//!
//! Wraps the Gemini CLI to provide MCP-compatible Google Search functionality
//! for Codex and Cursor IDE.
//!
//! Features:
//! - OAuth 2.0 + PKCE authentication (RFC 7636)
//! - Google Search via Gemini Grounding
//! - Cross-platform support (Windows/Unix)
//! - Rate limit handling with automatic fallback
//! - Token caching and auto-refresh

mod oauth;

use anyhow::Context;
use anyhow::Result;
use mcp_types::CallToolRequestParams;
use mcp_types::CallToolResult;
use mcp_types::ContentBlock;
use mcp_types::Implementation;
use mcp_types::InitializeResult;
use mcp_types::JSONRPCMessage;
use mcp_types::JSONRPCResponse;
use mcp_types::ListToolsResult;
use mcp_types::ServerCapabilities;
use mcp_types::ServerCapabilitiesTools;
use mcp_types::TextContent;
use mcp_types::Tool;
use mcp_types::ToolInputSchema;
use mcp_types::JSONRPC_VERSION;
use serde_json::json;
use std::io::BufRead;
use std::io::Write;
use tracing::debug;
use tracing::error;
use tracing::info;

/// Create a Command to run gemini CLI (cross-platform)
/// Windows: Uses 'cmd /c gemini' because gemini is a .ps1/.cmd script
/// Unix: Uses 'gemini' directly
fn create_gemini_command() -> std::process::Command {
    #[cfg(target_os = "windows")]
    {
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/c", "gemini"]);
        cmd
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("gemini")
    }
}

/// Execute Gemini CLI search with Google Search Grounding
async fn gemini_search(query: &str, model: &str) -> Result<String> {
    info!("🔍 Executing Gemini search via CLI: {}", query);

    let prompt = format!("Search the web for: {query}");

    let mut cmd = create_gemini_command();
    let output = cmd
        .arg("-p")
        .arg(&prompt)
        .arg("-o")
        .arg("text")
        .arg("-m")
        .arg(model)
        .output()
        .context("Failed to execute gemini CLI")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check for errors
    if !output.status.success()
        || stderr.contains("Error when talking to Gemini API")
        || stderr.contains("RESOURCE_EXHAUSTED")
    {
        // Try fallback to gemini-2.5-flash
        if model != "gemini-2.5-flash" {
            info!("⚠️  Rate limit, trying gemini-2.5-flash");
            let mut fallback_cmd = create_gemini_command();
            let fallback_output = fallback_cmd
                .arg("-p")
                .arg(&prompt)
                .arg("-o")
                .arg("text")
                .arg("-m")
                .arg("gemini-2.5-flash")
                .output()
                .context("Fallback also failed")?;

            let fallback_stdout = String::from_utf8_lossy(&fallback_output.stdout).to_string();
            if fallback_output.status.success() {
                return Ok(fallback_stdout);
            }
        }

        anyhow::bail!("Gemini CLI failed: {}", stderr);
    }

    Ok(stdout)
}

/// Handle tools/list request
fn handle_list_tools() -> ListToolsResult {
    ListToolsResult {
        tools: vec![Tool {
            name: "googleSearch".to_string(),
            title: Some("Google Search via Gemini CLI".to_string()),
            description: Some(
                "Search the web using Google Search via Gemini CLI (OAuth 2.0).\n\
                Provides high-quality search results with Google Search Grounding.\n\
                Automatically handles rate limits with fallback to gemini-2.5-flash."
                    .to_string(),
            ),
            input_schema: ToolInputSchema {
                r#type: "object".to_string(),
                properties: Some(json!({
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    },
                    "model": {
                        "type": "string",
                        "description": "Gemini model to use (default: gemini-2.5-pro)",
                        "default": "gemini-2.5-pro"
                    }
                })),
                required: Some(vec!["query".to_string()]),
            },
            annotations: None,
            output_schema: None,
        }],
        next_cursor: None,
    }
}

/// Handle tools/call request
async fn handle_call_tool(params: CallToolRequestParams) -> Result<CallToolResult> {
    debug!("🔧 Calling tool: {}", params.name);

    match params.name.as_str() {
        "googleSearch" => {
            let query = params
                .arguments
                .as_ref()
                .and_then(|args| args.get("query"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing 'query' parameter"))?;

            let model = params
                .arguments
                .as_ref()
                .and_then(|args| args.get("model"))
                .and_then(|v| v.as_str())
                .unwrap_or("gemini-2.5-pro");

            let result = gemini_search(query, model).await?;

            Ok(CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: result,
                    annotations: None,
                })],
                is_error: Some(false),
                structured_content: None,
            })
        }
        _ => {
            error!("❌ Unknown tool: {}", params.name);
            Ok(CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Unknown tool: {}", params.name),
                    annotations: None,
                })],
                is_error: Some(true),
                structured_content: None,
            })
        }
    }
}

/// Process a single JSON-RPC request
async fn process_request(message: JSONRPCMessage) -> Option<JSONRPCMessage> {
    match message {
        JSONRPCMessage::Request(req) => {
            let id = req.id.clone();
            let method = req.method.clone();

            debug!("📨 Received request: {}", method);

            let result = match method.as_str() {
                "initialize" => {
                    info!("🚀 Initializing MCP server");
                    let result = InitializeResult {
                        protocol_version: "2024-11-05".to_string(),
                        capabilities: ServerCapabilities {
                            completions: None,
                            experimental: None,
                            logging: None,
                            prompts: None,
                            resources: None,
                            tools: Some(ServerCapabilitiesTools {
                                list_changed: Some(false),
                            }),
                        },
                        server_info: Implementation {
                            name: "codex-gemini-cli-mcp-server".to_string(),
                            title: Some("Codex Gemini CLI MCP Server".to_string()),
                            version: "0.48.0".to_string(),
                            user_agent: Some("codex-gemini-mcp/0.48.0".to_string()),
                        },
                        instructions: Some(
                            "Gemini CLI MCP Server (OAuth 2.0)\n\
                            Available tools:\n\
                            - googleSearch: Search the web using Google Search via Gemini"
                                .to_string(),
                        ),
                    };
                    serde_json::to_value(result).ok()
                }
                "tools/list" => {
                    debug!("📋 Listing tools");
                    let result = handle_list_tools();
                    serde_json::to_value(result).ok()
                }
                "tools/call" => {
                    debug!("🔧 Calling tool");
                    match serde_json::from_value::<CallToolRequestParams>(
                        req.params.unwrap_or_default(),
                    ) {
                        Ok(params) => match handle_call_tool(params).await {
                            Ok(result) => serde_json::to_value(result).ok(),
                            Err(e) => {
                                error!("❌ Tool call failed: {}", e);
                                Some(json!({
                                    "content": [{
                                        "type": "text",
                                        "text": format!("Error: {}", e)
                                    }],
                                    "isError": true
                                }))
                            }
                        },
                        Err(e) => {
                            error!("❌ Invalid params: {}", e);
                            Some(json!({
                                "content": [{
                                    "type": "text",
                                    "text": format!("Invalid parameters: {}", e)
                                }],
                                "isError": true
                            }))
                        }
                    }
                }
                "notifications/initialized" => {
                    info!("✅ Client initialized");
                    return None; // No response for notifications
                }
                _ => {
                    error!("❌ Unknown method: {}", method);
                    Some(json!({
                        "error": {
                            "code": -32601,
                            "message": format!("Method not found: {}", method)
                        }
                    }))
                }
            };

            result.map(|r| {
                JSONRPCMessage::Response(JSONRPCResponse {
                    jsonrpc: JSONRPC_VERSION.to_string(),
                    id,
                    result: r,
                })
            })
        }
        JSONRPCMessage::Notification(notif) => {
            debug!("📢 Received notification: {}", notif.method);
            None // Notifications don't get responses
        }
        JSONRPCMessage::Response(_) => {
            error!("❌ Unexpected response message");
            None
        }
        JSONRPCMessage::Error(err) => {
            error!("❌ Received error message: {:?}", err);
            None
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("🚀 Starting Gemini CLI MCP Server v0.48.0");
    info!("   OAuth 2.0 authentication (no API key required)");
    info!("   Listening on STDIO...");

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    // Process messages line by line
    for line in stdin.lock().lines() {
        let line = line.context("Failed to read line from stdin")?;

        if line.trim().is_empty() {
            continue;
        }

        debug!("📥 Received: {}", line);

        // Parse JSON-RPC message
        let message: JSONRPCMessage = match serde_json::from_str(&line) {
            Ok(msg) => msg,
            Err(e) => {
                error!("❌ Failed to parse message: {}", e);
                continue;
            }
        };

        // Process request
        if let Some(response) = process_request(message).await {
            let response_json = serde_json::to_string(&response)?;
            debug!("📤 Sending: {}", response_json);
            writeln!(stdout, "{}", response_json)?;
            stdout.flush()?;
        }
    }

    info!("👋 Gemini CLI MCP Server shutting down");
    Ok(())
}
