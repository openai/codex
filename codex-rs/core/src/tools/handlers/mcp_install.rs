use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputQuestionOption;
use codex_rmcp_client::perform_oauth_login;
use serde::Deserialize;
use serde::Serialize;
use tokio::time::timeout;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::types::McpServerTransportConfig;
use crate::function_tool::FunctionCallError;
use crate::mcp::McpServerAuthFlow;
use crate::mcp::auth::compute_auth_statuses;
use crate::mcp::install_mcp_server;
use crate::mcp_connection_manager::AddServerParams;
use crate::mcp_connection_manager::DEFAULT_STARTUP_TIMEOUT;
use crate::mcp_connection_manager::SandboxState;
use crate::protocol::SandboxPolicy;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

use super::parse_arguments;

const DEFAULT_OAUTH_TIMEOUT_SECS: i64 = 300;
const RELOAD_TIMEOUT_BUFFER_SECS: u64 = 5;
const MCP_INSTALL_OPTION_INSTALL: &str = "Install tool";
const MCP_INSTALL_OPTION_RUN_ANYWAY: &str = "Continue anyway";

#[derive(Debug, Serialize)]
struct McpInstallApprovalKey {
    name: String,
}

#[derive(Debug, Deserialize)]
struct InstallMcpToolArgs {
    name: String,
    description: String,
    transport: String,
    command: Option<Vec<String>>,
    url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallMcpToolResponse {
    name: String,
    description: String,
    installed: bool,
    replaced: bool,
    reloaded: bool,
    oauth_status: OauthStatus,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    notes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum OauthStatus {
    NotRequired,
    Completed,
    Skipped,
}

pub struct McpInstallHandler;

#[async_trait]
impl ToolHandler for McpInstallHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "install_mcp_tool handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: InstallMcpToolArgs = parse_arguments(&arguments)?;
        let name = normalize_required_string("name", args.name)?;
        let description = normalize_required_string("description", args.description)?;
        let transport = normalize_required_string("transport", args.transport)?;
        let transport_kind = transport.to_ascii_lowercase().replace('-', "_");

        let transport_config = match transport_kind.as_str() {
            "stdio" => {
                let mut command_parts = args.command.unwrap_or_default().into_iter();
                let command_bin = command_parts.next().ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "command is required when transport is stdio".to_string(),
                    )
                })?;
                let command_args = command_parts.collect();
                McpServerTransportConfig::Stdio {
                    command: command_bin,
                    args: command_args,
                    env: None,
                    env_vars: Vec::new(),
                    cwd: None,
                }
            }
            "streamable_http" => {
                let url = normalize_required_option("url", args.url)?;
                McpServerTransportConfig::StreamableHttp {
                    url,
                    bearer_token_env_var: None,
                    http_headers: None,
                    env_http_headers: None,
                }
            }
            other => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "unsupported transport '{other}'; expected 'stdio' or 'streamable_http'",
                )));
            }
        };

        ensure_mcp_install_approval(
            session.as_ref(),
            turn.as_ref(),
            &call_id,
            &name,
            &description,
        )
        .await?;

        session
            .notify_background_event(
                &turn,
                format!("Installing MCP tool '{name}': {description}"),
            )
            .await;

        let config = session.get_config().await;
        let install_result = install_mcp_server(&config.codex_home, name.clone(), transport_config)
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to install MCP tool '{name}': {err}"
                ))
            })?;

        if install_result.replaced {
            session
                .notify_background_event(&turn, format!("Updated existing MCP tool '{name}'."))
                .await;
        } else {
            session
                .notify_background_event(&turn, format!("Added MCP tool '{name}'."))
                .await;
        }

        let mut notes = Vec::new();
        let mut oauth_status = OauthStatus::NotRequired;

        match install_result.auth_flow {
            McpServerAuthFlow::OAuth {
                url,
                http_headers,
                env_http_headers,
            } => {
                let timeout_secs = DEFAULT_OAUTH_TIMEOUT_SECS;
                if timeout_secs <= 0 {
                    return Err(FunctionCallError::RespondToModel(
                        "oauth_timeout_secs must be greater than zero".to_string(),
                    ));
                }

                session
                    .notify_background_event(
                        &turn,
                        format!("Detected OAuth support. Starting OAuth flow for '{name}'..."),
                    )
                    .await;

                session
                    .notify_background_event(
                        &turn,
                        format!("Launching browser for OAuth login for '{name}'..."),
                    )
                    .await;

                let login_result = timeout(
                    Duration::from_secs(timeout_secs as u64),
                    perform_oauth_login(
                        &name,
                        &url,
                        config.mcp_oauth_credentials_store_mode,
                        http_headers,
                        env_http_headers,
                        &[],
                        config.mcp_oauth_callback_port,
                    ),
                )
                .await;

                match login_result {
                    Ok(Ok(())) => {}
                    Ok(Err(err)) => {
                        return Err(FunctionCallError::RespondToModel(format!(
                            "OAuth login failed for '{name}': {err}"
                        )));
                    }
                    Err(_) => {
                        return Err(FunctionCallError::RespondToModel(format!(
                            "timed out while waiting for OAuth login for '{name}'"
                        )));
                    }
                }

                oauth_status = OauthStatus::Completed;
                session
                    .notify_background_event(&turn, format!("OAuth login completed for '{name}'."))
                    .await;
            }
            McpServerAuthFlow::Unknown => {
                oauth_status = OauthStatus::Skipped;
                notes.push(format!(
                    "OAuth support could not be detected. If '{name}' requires login, run `codex mcp login {name}`."
                ));
            }
            McpServerAuthFlow::NotRequired => {}
        }

        let mut updated_config = (*config).clone();
        let servers: HashMap<_, _> = install_result.servers.clone().into_iter().collect();
        updated_config.mcp_servers.set(servers).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to update session config for '{name}': {err}"
            ))
        })?;
        session.replace_config(updated_config).await;

        let server_config = install_result.servers.get(&name).cloned().ok_or_else(|| {
            FunctionCallError::RespondToModel(format!(
                "failed to load MCP server config for '{name}'"
            ))
        })?;

        let auth_entries = compute_auth_statuses(
            std::iter::once((&name, &server_config)),
            config.mcp_oauth_credentials_store_mode,
        )
        .await;
        let auth_entry = auth_entries.get(&name).cloned();

        session
            .notify_background_event(&turn, format!("Reloading MCP server '{name}'..."))
            .await;

        let reload_timeout = server_config
            .startup_timeout_sec
            .unwrap_or(DEFAULT_STARTUP_TIMEOUT)
            + Duration::from_secs(RELOAD_TIMEOUT_BUFFER_SECS);

        let sandbox_state = SandboxState {
            sandbox_policy: turn.sandbox_policy.clone(),
            codex_linux_sandbox_exe: turn.codex_linux_sandbox_exe.clone(),
            sandbox_cwd: turn.cwd.clone(),
        };
        let cancel_token = session
            .services
            .mcp_startup_cancellation_token
            .lock()
            .await
            .clone();
        let startup_handle = {
            let mut manager = session.services.mcp_connection_manager.write().await;
            manager
                .add_server(AddServerParams {
                    server_name: name.clone(),
                    config: server_config,
                    store_mode: config.mcp_oauth_credentials_store_mode,
                    auth_entry,
                    tx_event: session.get_tx_event(),
                    cancel_token,
                    sandbox_state,
                })
                .await
                .map_err(FunctionCallError::RespondToModel)?
        };

        match timeout(reload_timeout, startup_handle.wait()).await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                return Err(FunctionCallError::RespondToModel(err));
            }
            Err(_) => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "timed out while waiting for MCP server '{name}' to start"
                )));
            }
        }

        let response = InstallMcpToolResponse {
            name,
            description,
            installed: true,
            replaced: install_result.replaced,
            reloaded: true,
            oauth_status,
            notes,
        };

        serialize_function_output(response)
    }
}

async fn ensure_mcp_install_approval(
    session: &Session,
    turn: &TurnContext,
    call_id: &str,
    name: &str,
    description: &str,
) -> Result<(), FunctionCallError> {
    let approval_key = McpInstallApprovalKey {
        name: name.to_string(),
    };
    let already_approved = {
        let store = session.services.tool_approvals.lock().await;
        matches!(
            store.get(&approval_key),
            Some(ReviewDecision::ApprovedForSession)
        )
    };
    if already_approved {
        return Ok(());
    }
    if matches!(
        turn.sandbox_policy,
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
    ) {
        return Ok(());
    }

    let question_id = format!("mcp_install_{name}");
    let question = RequestUserInputQuestion {
        id: question_id.clone(),
        header: format!("Install MCP server '{name}'?"),
        question: format!(
            "The agent wants to install the MCP server '{name}' ({description}). This choice will be remembered for the rest of this session."
        ),
        options: Some(vec![
            RequestUserInputQuestionOption {
                label: MCP_INSTALL_OPTION_INSTALL.to_string(),
                description: "Install and load this MCP server.".to_string(),
            },
            RequestUserInputQuestionOption {
                label: MCP_INSTALL_OPTION_RUN_ANYWAY.to_string(),
                description: "Proceed without further review.".to_string(),
            },
        ]),
    };

    let response = session
        .request_user_input(
            turn,
            call_id.to_string(),
            RequestUserInputArgs {
                questions: vec![question],
            },
        )
        .await
        .ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "install_mcp_tool was cancelled before receiving a response".to_string(),
            )
        })?;

    let selection = response
        .answers
        .get(&question_id)
        .and_then(|answer| answer.answers.first())
        .ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "install_mcp_tool requires an explicit selection".to_string(),
            )
        })?;

    if !matches!(
        selection.as_str(),
        MCP_INSTALL_OPTION_INSTALL | MCP_INSTALL_OPTION_RUN_ANYWAY
    ) {
        return Err(FunctionCallError::RespondToModel(format!(
            "install_mcp_tool received unsupported selection '{selection}'"
        )));
    }

    let mut store = session.services.tool_approvals.lock().await;
    store.put(approval_key, ReviewDecision::ApprovedForSession);

    Ok(())
}

fn normalize_optional_string(input: Option<String>) -> Option<String> {
    input.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn normalize_required_string(field: &str, value: String) -> Result<String, FunctionCallError> {
    match normalize_optional_string(Some(value)) {
        Some(normalized) => Ok(normalized),
        None => Err(FunctionCallError::RespondToModel(format!(
            "{field} must be provided"
        ))),
    }
}

fn normalize_required_option(
    field: &str,
    value: Option<String>,
) -> Result<String, FunctionCallError> {
    let value = value.ok_or_else(|| {
        FunctionCallError::RespondToModel(format!(
            "{field} is required when transport is streamable_http"
        ))
    })?;
    normalize_required_string(field, value)
}

fn serialize_function_output<T>(payload: T) -> Result<ToolOutput, FunctionCallError>
where
    T: Serialize,
{
    let content = serde_json::to_string(&payload).map_err(|err| {
        FunctionCallError::RespondToModel(format!(
            "failed to serialize MCP install response: {err}"
        ))
    })?;

    Ok(ToolOutput::Function {
        content,
        content_items: None,
        success: Some(true),
    })
}
