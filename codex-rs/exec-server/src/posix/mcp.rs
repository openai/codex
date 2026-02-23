use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use anyhow::Result;
use codex_core::MCP_SANDBOX_STATE_CAPABILITY;
use codex_core::MCP_SANDBOX_STATE_METHOD;
use codex_core::SandboxState;
use codex_execpolicy::Policy;
use codex_protocol::protocol::SandboxPolicy;
use rmcp::ErrorData as McpError;
use rmcp::RoleServer;
use rmcp::ServerHandler;
use rmcp::ServiceExt;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CustomRequest;
use rmcp::model::CustomResult;
use rmcp::model::*;
use rmcp::schemars;
use rmcp::service::RequestContext;
use rmcp::service::RunningService;
use rmcp::tool;
use rmcp::tool_handler;
use rmcp::tool_router;
use rmcp::transport::stdio;
use serde_json::json;
use tokio::sync::RwLock;

use crate::posix::escalate_server::EscalateServer;
use crate::posix::escalate_server::{self};
use crate::posix::mcp_escalation_policy::McpEscalationPolicy;
use crate::posix::stopwatch::Stopwatch;

/// Path to our patched bash.
const CODEX_BASH_PATH_ENV_VAR: &str = "CODEX_BASH_PATH";

const SANDBOX_STATE_CAPABILITY_VERSION: &str = "1.0.0";

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
    /// Launch Bash with -lc instead of -c: defaults to true.
    pub login: Option<bool>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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
    execve_wrapper: PathBuf,
    policy: Arc<RwLock<Policy>>,
    preserve_program_paths: bool,
    sandbox_state: Arc<RwLock<Option<SandboxState>>>,
}

#[tool_router]
impl ExecTool {
    pub fn new(
        bash_path: PathBuf,
        execve_wrapper: PathBuf,
        policy: Arc<RwLock<Policy>>,
        preserve_program_paths: bool,
    ) -> Self {
        Self {
            tool_router: Self::tool_router(),
            bash_path,
            execve_wrapper,
            policy,
            preserve_program_paths,
            sandbox_state: Arc::new(RwLock::new(None)),
        }
    }

    /// Runs a shell command and returns its output. You MUST provide the workdir as an absolute path.
    #[tool]
    async fn shell(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(params): Parameters<ExecParams>,
    ) -> Result<CallToolResult, McpError> {
        let effective_timeout = Duration::from_millis(
            params
                .timeout_ms
                .unwrap_or(codex_core::exec::DEFAULT_EXEC_COMMAND_TIMEOUT_MS),
        );
        let stopwatch = Stopwatch::new(effective_timeout);
        let cancel_token = stopwatch.cancellation_token();
        let sandbox_state =
            self.sandbox_state
                .read()
                .await
                .clone()
                .unwrap_or_else(|| SandboxState {
                    sandbox_policy: SandboxPolicy::new_read_only_policy(),
                    codex_linux_sandbox_exe: None,
                    sandbox_cwd: PathBuf::from(&params.workdir),
                    use_linux_sandbox_bwrap: false,
                });
        let escalate_server = EscalateServer::new(
            self.bash_path.clone(),
            self.execve_wrapper.clone(),
            McpEscalationPolicy::new(
                self.policy.clone(),
                context,
                stopwatch.clone(),
                self.preserve_program_paths,
            ),
        );

        let result = escalate_server
            .exec(params, cancel_token, &sandbox_state)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::json(
            ExecResult::from(result),
        )?]))
    }
}

#[derive(Default)]
pub struct CodexSandboxStateUpdateMethod;

impl rmcp::model::ConstString for CodexSandboxStateUpdateMethod {
    const VALUE: &'static str = MCP_SANDBOX_STATE_METHOD;
}

#[tool_handler]
impl ServerHandler for ExecTool {
    fn get_info(&self) -> ServerInfo {
        let mut experimental_capabilities = ExperimentalCapabilities::new();
        let mut sandbox_state_capability = JsonObject::new();
        sandbox_state_capability.insert(
            "version".to_string(),
            serde_json::Value::String(SANDBOX_STATE_CAPABILITY_VERSION.to_string()),
        );
        experimental_capabilities.insert(
            MCP_SANDBOX_STATE_CAPABILITY.to_string(),
            sandbox_state_capability,
        );
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_experimental_with(experimental_capabilities)
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "This server provides a tool to execute shell commands and return their output."
                    .to_string(),
            ),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Ok(self.get_info())
    }

    async fn on_custom_request(
        &self,
        request: CustomRequest,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<CustomResult, McpError> {
        let CustomRequest { method, params, .. } = request;
        if method != MCP_SANDBOX_STATE_METHOD {
            return Err(McpError::method_not_found::<CodexSandboxStateUpdateMethod>());
        }

        let Some(params) = params else {
            return Err(McpError::invalid_params(
                "missing params for sandbox state request".to_string(),
                None,
            ));
        };

        let Ok(sandbox_state) = serde_json::from_value::<SandboxState>(params.clone()) else {
            return Err(McpError::invalid_params(
                "failed to deserialize sandbox state".to_string(),
                Some(params),
            ));
        };

        *self.sandbox_state.write().await = Some(sandbox_state);

        Ok(CustomResult::new(json!({})))
    }
}

pub(crate) async fn serve(
    bash_path: PathBuf,
    execve_wrapper: PathBuf,
    policy: Arc<RwLock<Policy>>,
    preserve_program_paths: bool,
) -> Result<RunningService<RoleServer, ExecTool>, rmcp::service::ServerInitializeError> {
    let tool = ExecTool::new(bash_path, execve_wrapper, policy, preserve_program_paths);
    tool.serve(stdio()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    /// Verify that the way we use serde does not compromise the desired JSON
    /// schema via schemars. In particular, ensure that the `login` and
    /// `timeout_ms` fields are optional.
    #[test]
    fn exec_params_json_schema_matches_expected() {
        let schema = schemars::schema_for!(ExecParams);
        let actual = serde_json::to_value(schema).expect("schema should serialize");

        assert_eq!(
            actual,
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "title": "ExecParams",
                "type": "object",
                "properties": {
                    "command": {
                        "description": "The bash string to execute.",
                        "type": "string"
                    },
                    "login": {
                        "description": "Launch Bash with -lc instead of -c: defaults to true.",
                        "type": ["boolean", "null"]
                    },
                    "timeout_ms": {
                        "description": "The timeout for the command in milliseconds.",
                        "format": "uint64",
                        "minimum": 0,
                        "type": ["integer", "null"]
                    },
                    "workdir": {
                        "description":
                            "The working directory to execute the command in. Must be an absolute path.",
                        "type": "string"
                    }
                },
                "required": ["command", "workdir"]
            })
        );
    }
}
