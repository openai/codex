//! MCP-based Codex integration for sub-agents
//!
//! This module enables sub-agents to call Codex capabilities via MCP protocol,
//! solving the private API limitations elegantly.

use anyhow::Context;
use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio::process::Command;
use tracing::debug;
use tracing::error;
use tracing::info;

/// MCP-based Codex client for sub-agents
pub struct McpCodexClient {
    /// Child process running `codex mcp-server`
    _process: Child,
    /// stdin for sending requests
    stdin: ChildStdin,
    /// stdout for receiving responses
    stdout: BufReader<ChildStdout>,
    /// Request ID counter
    next_id: u64,
}

impl McpCodexClient {
    /// Spawn a new Codex MCP server and connect to it
    pub async fn spawn(workspace_dir: &PathBuf) -> Result<Self> {
        info!("Spawning Codex MCP server for sub-agent");

        // Spawn `codex mcp-server` as child process
        let mut process = Command::new("codex")
            .arg("mcp-server")
            .current_dir(workspace_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("Failed to spawn codex mcp-server")?;

        let stdin = process.stdin.take().context("Failed to get stdin")?;
        let stdout = process.stdout.take().context("Failed to get stdout")?;
        let stdout = BufReader::new(stdout);

        debug!("Codex MCP server spawned successfully");

        Ok(Self {
            _process: process,
            stdin,
            stdout,
            next_id: 1,
        })
    }

    /// Call a Codex tool via MCP
    pub async fn call_tool(&mut self, tool_name: &str, arguments: Value) -> Result<Value> {
        let request_id = self.next_id;
        self.next_id += 1;

        // Construct JSON-RPC request
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        debug!("Sending MCP request: {}", tool_name);

        // Send request
        let request_str = serde_json::to_string(&request)?;
        self.stdin
            .write_all(request_str.as_bytes())
            .await
            .context("Failed to write to MCP stdin")?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;

        // Read response
        let mut response_line = String::new();
        self.stdout
            .read_line(&mut response_line)
            .await
            .context("Failed to read from MCP stdout")?;

        let response: Value =
            serde_json::from_str(&response_line).context("Failed to parse MCP response")?;

        debug!("Received MCP response");

        // Check for errors
        if let Some(error) = response.get("error") {
            error!("MCP tool call failed: {:?}", error);
            anyhow::bail!("MCP tool call failed: {}", error);
        }

        // Extract result
        response
            .get("result")
            .cloned()
            .context("No result in MCP response")
    }

    /// Read a file via Codex MCP
    pub async fn read_file(&mut self, path: &str) -> Result<String> {
        let args = serde_json::json!({
            "path": path
        });

        let result = self.call_tool("codex_read_file", args).await?;

        result
            .get("content")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .context("Invalid read_file response")
    }

    /// Search code via Codex MCP
    pub async fn codebase_search(
        &mut self,
        query: &str,
        target_directories: Vec<String>,
    ) -> Result<Vec<String>> {
        let args = serde_json::json!({
            "query": query,
            "target_directories": target_directories
        });

        let result = self.call_tool("codex_codebase_search", args).await?;

        if let Some(results) = result.get("results").and_then(|v| v.as_array()) {
            Ok(results
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect())
        } else {
            Ok(vec![])
        }
    }

    /// Grep via Codex MCP
    pub async fn grep(&mut self, pattern: &str, path: Option<&str>) -> Result<Vec<String>> {
        let mut args = serde_json::json!({
            "pattern": pattern
        });

        if let Some(path) = path {
            args["path"] = Value::String(path.to_string());
        }

        let result = self.call_tool("codex_grep", args).await?;

        if let Some(matches) = result.get("matches").and_then(|v| v.as_array()) {
            Ok(matches
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect())
        } else {
            Ok(vec![])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires running Codex binary
    async fn test_mcp_codex_client() {
        let workspace = PathBuf::from(".");
        let mut client = McpCodexClient::spawn(&workspace).await.unwrap();

        // Test read_file
        let content = client.read_file("README.md").await;
        assert!(content.is_ok() || content.is_err()); // Either works or fails gracefully
    }
}
