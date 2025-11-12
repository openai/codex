use codex_core::exec::SandboxType;
use codex_core::exec::process_exec_tool_call;
use codex_core::get_platform_sandbox;
use codex_core::protocol::SandboxPolicy;
use rmcp::ErrorData as McpError;
use rmcp::RoleServer;
use rmcp::ServerHandler;
use rmcp::ServiceExt;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::schemars;
use rmcp::service::RequestContext;
use rmcp::tool;
use rmcp::tool_handler;
use rmcp::tool_router;
use rmcp::transport::stdio;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::{self};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecParams {
    pub command: String,
    pub workdir: String,
    pub timeout_ms: Option<u64>,
    pub with_escalated_permissions: Option<bool>,
    pub justification: Option<String>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ExecResult {
    pub exit_code: i32,
    pub output: String,
    pub duration: Duration,
    pub timed_out: bool,
}

#[derive(Clone)]
pub struct ExecTool {
    tool_router: ToolRouter<ExecTool>,
}

#[tool_router]
impl ExecTool {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Runs a shell command and returns its output.")]
    async fn shell(
        &self,
        Parameters(ExecParams {
            command,
            workdir,
            timeout_ms,
            with_escalated_permissions,
            justification,
        }): Parameters<ExecParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = process_exec_tool_call(
            codex_core::exec::ExecParams {
                command: vec!["/bin/bash".to_string(), "-lc".to_string(), command],
                cwd: PathBuf::from(&workdir),
                timeout_ms,
                env: HashMap::new(),
                with_escalated_permissions,
                justification,
                arg0: None,
            },
            get_platform_sandbox().unwrap_or(SandboxType::None),
            &SandboxPolicy::ReadOnly,
            &PathBuf::from("/__NONEXISTENT__"), // This is ignored for ReadOnly
            &None,
            None,
        )
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let result = ExecResult {
            exit_code: result.exit_code,
            output: result.aggregated_output.text,
            duration: result.duration,
            timed_out: result.timed_out,
        };
        Ok(CallToolResult::success(vec![Content::json(result)?]))
    }
}

impl Default for ExecTool {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_handler]
impl ServerHandler for ExecTool {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("This server provides counter tools and prompts. Tools: increment, decrement, get_value, say_hello, echo, sum. Prompts: example_prompt (takes a message), counter_analysis (analyzes counter state with a goal).".to_string()),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Ok(self.get_info())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting MCP server");

    let service = ExecTool::new().serve(stdio()).await.inspect_err(|e| {
        tracing::error!("serving error: {:?}", e);
    })?;

    service.waiting().await?;
    Ok(())
}
