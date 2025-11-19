use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context as _;
use anyhow::Result;
use rmcp::ErrorData as McpError;
use rmcp::RoleServer;
use rmcp::ServerHandler;
use rmcp::ServiceExt;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::schemars;
use rmcp::service::RequestContext;
use rmcp::service::RunningService;
use rmcp::tool;
use rmcp::tool_handler;
use rmcp::tool_router;
use rmcp::transport::stdio;

use crate::posix::escalate_server;
use crate::posix::escalate_server::EscalateServer;
use crate::posix::escalate_server::ExecPolicy;

/// Path to our patched bash.
const CODEX_BASH_PATH_ENV_VAR: &str = "CODEX_BASH_PATH";

pub(crate) fn get_bash_path() -> Result<PathBuf> {
    std::env::var(CODEX_BASH_PATH_ENV_VAR)
        .map(PathBuf::from)
        .context(format!("{CODEX_BASH_PATH_ENV_VAR} must be set"))
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecParams {
    /// The bash string to execute.
    pub command: String,
    /// The working directory to execute the command in. Must be an absolute path.
    pub workdir: String,
    /// The timeout for the command in milliseconds.
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ExecResult {
    pub exit_code: i32,
    pub output: String,
    pub duration: Duration,
    pub timed_out: bool,
}

impl From<escalate_server::ExecResult> for ExecResult {
    fn from(result: escalate_server::ExecResult) -> Self {
        Self {
            exit_code: result.exit_code,
            output: result.output,
            duration: result.duration,
            timed_out: result.timed_out,
        }
    }
}

#[derive(Clone)]
pub struct ExecTool {
    tool_router: ToolRouter<ExecTool>,
    bash_path: PathBuf,
    policy: ExecPolicy,
}

#[tool_router]
impl ExecTool {
    pub fn new(bash_path: PathBuf, policy: ExecPolicy) -> Self {
        Self {
            tool_router: Self::tool_router(),
            bash_path,
            policy,
        }
    }

    /// Runs a shell command and returns its output. You MUST provide the workdir as an absolute path.
    #[tool]
    async fn shell(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<ExecParams>,
    ) -> Result<CallToolResult, McpError> {
        let escalate_server = EscalateServer::new(self.bash_path.clone(), self.policy);
        let result = escalate_server
            .exec(
                params.command,
                // TODO: use ShellEnvironmentPolicy
                std::env::vars().collect(),
                PathBuf::from(&params.workdir),
                params.timeout_ms,
            )
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::json(
            ExecResult::from(result),
        )?]))
    }

    #[allow(dead_code)]
    async fn prompt(
        &self,
        command: String,
        workdir: String,
        context: RequestContext<RoleServer>,
    ) -> Result<CreateElicitationResult, McpError> {
        context
            .peer
            .create_elicitation(CreateElicitationRequestParam {
                message: format!("Allow Codex to run `{command:?}` in `{workdir:?}`?"),
                #[allow(clippy::expect_used)]
                requested_schema: ElicitationSchema::builder()
                    .property("dummy", PrimitiveSchema::String(StringSchema::new()))
                    .build()
                    .expect("failed to build elicitation schema"),
            })
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))
    }
}

#[tool_handler]
impl ServerHandler for ExecTool {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "This server provides a tool to execute shell commands and return their output."
                    .to_string(),
            ),
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

pub(crate) async fn serve(
    bash_path: PathBuf,
    policy: ExecPolicy,
) -> Result<RunningService<RoleServer, ExecTool>, rmcp::service::ServerInitializeError> {
    let tool = ExecTool::new(bash_path, policy);
    tool.serve(stdio()).await
}
