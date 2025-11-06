use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub description: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub message: String,
    pub execution_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchReport {
    pub query: String,
    pub findings: Vec<String>,
    pub sources: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Codex Core integration bridge
///
/// This module provides integration with the existing Codex Rust core
/// Option A: CLI subprocess calls (current implementation)
/// Option B: Direct crate dependency (future optimization)
pub struct CodexBridge {
    codex_bin_path: String,
}

impl CodexBridge {
    pub fn new() -> Result<Self> {
        // Try to find codex binary in PATH or workspace
        let codex_bin_path = Self::find_codex_binary()?;

        info!("Codex binary found at: {}", codex_bin_path);

        Ok(Self { codex_bin_path })
    }

    fn find_codex_binary() -> Result<String> {
        // Check if codex is in PATH
        if let Ok(output) = Command::new("codex").arg("--version").output() {
            if output.status.success() {
                return Ok("codex".to_string());
            }
        }

        // Check in ../codex-rs/target/release/
        let workspace_path = std::env::current_exe()?
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|p| p.join("codex-rs/target/release/codex.exe"));

        if let Some(path) = workspace_path {
            if path.exists() {
                return Ok(path.to_string_lossy().to_string());
            }
        }

        Err(anyhow::anyhow!(
            "Codex binary not found. Please install codex-cli."
        ))
    }

    /// Create a new plan
    pub async fn create_plan(
        &self,
        description: String,
        mode: Option<String>,
    ) -> Result<Plan> {
        info!("Creating plan: {}", description);

        let mode = mode.unwrap_or_else(|| "orchestrated".to_string());

        let output = Command::new(&self.codex_bin_path)
            .arg("plan")
            .arg("create")
            .arg(&description)
            .arg("--mode")
            .arg(&mode)
            .output()?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to create plan: {}", error));
        }

        // Parse output (simplified - assumes JSON output)
        let _stdout = String::from_utf8_lossy(&output.stdout);
        let plan = Plan {
            id: "plan-placeholder".to_string(), // Extract from output
            description,
            status: "Pending".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        Ok(plan)
    }

    /// Execute a plan
    pub async fn execute_plan(&self, id: String) -> Result<ExecutionResult> {
        info!("Executing plan: {}", id);

        let output = Command::new(&self.codex_bin_path)
            .arg("plan")
            .arg("execute")
            .arg(&id)
            .output()?;

        let success = output.status.success();
        let message = if success {
            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            String::from_utf8_lossy(&output.stderr).to_string()
        };

        Ok(ExecutionResult {
            success,
            message,
            execution_id: Some(format!("exec-{}", id)),
        })
    }

    /// List all plans
    pub async fn list_plans(&self) -> Result<Vec<Plan>> {
        info!("Listing plans");

        let output = Command::new(&self.codex_bin_path)
            .arg("plan")
            .arg("list")
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to list plans"));
        }

        // Parse output (simplified)
        // In a real implementation, this would parse JSON output
        Ok(vec![])
    }

    /// Perform deep research
    pub async fn research(&self, query: String, depth: u8) -> Result<ResearchReport> {
        info!("Performing research: {} (depth: {})", query, depth);

        let output = Command::new(&self.codex_bin_path)
            .arg("research")
            .arg(&query)
            .arg("--depth")
            .arg(depth.to_string())
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Research failed"));
        }

        // Parse output (simplified)
        Ok(ResearchReport {
            query,
            findings: vec!["Research finding 1".to_string()],
            sources: vec!["https://example.com".to_string()],
            confidence: 0.85,
        })
    }

    /// List available MCP tools
    pub async fn list_mcp_tools(&self) -> Result<Vec<Tool>> {
        info!("Listing MCP tools");

        // This would call the MCP server endpoint
        // For now, return placeholder
        Ok(vec![Tool {
            name: "code_review".to_string(),
            description: "Review code for best practices".to_string(),
            parameters: serde_json::json!({}),
        }])
    }

    /// Invoke an MCP tool
    pub async fn invoke_mcp_tool(
        &self,
        name: String,
        _args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        info!("Invoking MCP tool: {}", name);

        // This would call the MCP server
        // For now, return placeholder
        Ok(serde_json::json!({
            "success": true,
            "result": "Tool execution completed"
        }))
    }
}

// Tauri commands that use CodexBridge
#[tauri::command]
pub async fn codex_create_plan(
    description: String,
    mode: Option<String>,
) -> Result<Plan, String> {
    let bridge = CodexBridge::new().map_err(|e| e.to_string())?;
    bridge
        .create_plan(description, mode)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn codex_execute_plan(id: String) -> Result<ExecutionResult, String> {
    let bridge = CodexBridge::new().map_err(|e| e.to_string())?;
    bridge
        .execute_plan(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn codex_list_plans() -> Result<Vec<Plan>, String> {
    let bridge = CodexBridge::new().map_err(|e| e.to_string())?;
    bridge.list_plans().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn codex_research(query: String, depth: u8) -> Result<ResearchReport, String> {
    let bridge = CodexBridge::new().map_err(|e| e.to_string())?;
    bridge
        .research(query, depth)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn codex_list_mcp_tools() -> Result<Vec<Tool>, String> {
    let bridge = CodexBridge::new().map_err(|e| e.to_string())?;
    bridge.list_mcp_tools().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn codex_invoke_mcp_tool(
    name: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let bridge = CodexBridge::new().map_err(|e| e.to_string())?;
    bridge
        .invoke_mcp_tool(name, args)
        .await
        .map_err(|e| e.to_string())
}
