use std::sync::Arc;

use async_trait::async_trait;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::LocalShellExecAction;
use codex_protocol::models::LocalShellStatus;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::ShellToolCallParams;
use codex_protocol::user_input::UserInput;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::error;
use uuid::Uuid;

use crate::codex::TurnContext;
use crate::protocol::EventMsg;
use crate::protocol::TaskStartedEvent;
use crate::state::TaskKind;
use crate::tools::context::ToolPayload;
use crate::tools::parallel::ToolCallRuntime;
use crate::tools::router::ToolCall;
use crate::tools::router::ToolRouter;
use crate::turn_diff_tracker::TurnDiffTracker;

use super::SessionTask;
use super::SessionTaskContext;

const USER_SHELL_TOOL_NAME: &str = "local_shell";

#[derive(Clone)]
pub(crate) struct UserShellCommandTask {
    command: String,
}

impl UserShellCommandTask {
    pub(crate) fn new(command: String) -> Self {
        Self { command }
    }
}

#[async_trait]
impl SessionTask for UserShellCommandTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        turn_context: Arc<TurnContext>,
        _input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        let event = EventMsg::TaskStarted(TaskStartedEvent {
            model_context_window: turn_context.client.get_model_context_window(),
        });
        let session = session.clone_session();
        session.send_event(turn_context.as_ref(), event).await;

        // Execute the user's script under their default shell when known; this
        // allows commands that use shell features (pipes, &&, redirects, etc.).
        // We do not source rc files or otherwise reformat the script.
        let shell_invocation = match session.user_shell() {
            crate::shell::Shell::Zsh(zsh) => vec![
                zsh.shell_path.clone(),
                "-lc".to_string(),
                self.command.clone(),
            ],
            crate::shell::Shell::Bash(bash) => vec![
                bash.shell_path.clone(),
                "-lc".to_string(),
                self.command.clone(),
            ],
            crate::shell::Shell::PowerShell(ps) => vec![
                ps.exe.clone(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                self.command.clone(),
            ],
            crate::shell::Shell::Unknown => {
                shlex::split(&self.command).unwrap_or_else(|| vec![self.command.clone()])
            }
        };

        let params = ShellToolCallParams {
            command: shell_invocation.clone(),
            workdir: None,
            timeout_ms: None,
            with_escalated_permissions: None,
            justification: None,
        };

        let params_timeout_ms = params.timeout_ms.clone();
        let tool_call = ToolCall {
            tool_name: USER_SHELL_TOOL_NAME.to_string(),
            call_id: Uuid::new_v4().to_string(),
            payload: ToolPayload::LocalShell { params },
        };

        let router = Arc::new(ToolRouter::from_config(&turn_context.tools_config, None));
        let tracker = Arc::new(Mutex::new(TurnDiffTracker::new()));
        let runtime = ToolCallRuntime::new(
            Arc::clone(&router),
            Arc::clone(&session),
            Arc::clone(&turn_context),
            Arc::clone(&tracker),
        );

        let call_id = tool_call.call_id.clone();
        match runtime
            .handle_tool_call(tool_call, cancellation_token)
            .await
        {
            Ok(resp) => {
                let mut status = LocalShellStatus::Completed;

                // Special-case 'aborted' commands to mark failure.
                let mut output_item: ResponseItem = resp.clone().into();
                if let ResponseInputItem::FunctionCallOutput { output, .. } = &resp {
                    if output.content == "aborted" {
                        status = LocalShellStatus::Incomplete;
                        output_item = ResponseItem::FunctionCallOutput {
                            call_id: call_id.clone(),
                            output: FunctionCallOutputPayload {
                                content: "aborted".to_string(),
                                success: Some(false),
                                ..Default::default()
                            },
                        };
                    }
                }

                let local_call = ResponseItem::LocalShellCall {
                    id: None,
                    call_id: Some(call_id.clone()),
                    status,
                    action: LocalShellAction::Exec(LocalShellExecAction {
                        command: shell_invocation.clone(),
                        timeout_ms: params_timeout_ms,
                        working_directory: Some(turn_context.cwd.to_string_lossy().into_owned()),
                        // The Responses API expects `env` to be an object with string keys/values.
                        // Sending `null` here yields an `invalid_type` error, so we send `{}`.
                        env: Some(std::collections::HashMap::new()),
                        user: None,
                    }),
                };

                session
                    .record_conversation_items(turn_context.as_ref(), &[local_call, output_item])
                    .await;
            }
            Err(err) => {
                error!("user shell command failed: {err:?}");

                let local_call = ResponseItem::LocalShellCall {
                    id: None,
                    call_id: Some(call_id.clone()),
                    status: LocalShellStatus::Incomplete,
                    action: LocalShellAction::Exec(LocalShellExecAction {
                        // Clone because the original invocation was moved into `params` above.
                        command: shell_invocation.clone(),
                        timeout_ms: params_timeout_ms,
                        working_directory: Some(turn_context.cwd.to_string_lossy().into_owned()),
                        // Match success path: ensure an empty object instead of `null`.
                        env: Some(std::collections::HashMap::new()),
                        user: None,
                    }),
                };
                let output_item = ResponseItem::FunctionCallOutput {
                    call_id: call_id.clone(),
                    output: FunctionCallOutputPayload {
                        content: err.to_string(),
                        success: Some(false),
                        ..Default::default()
                    },
                };
                session
                    .record_conversation_items(turn_context.as_ref(), &[local_call, output_item])
                    .await;
            }
        }
        None
    }
}
