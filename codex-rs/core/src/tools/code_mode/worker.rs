use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::warn;

use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;

use super::ExecContext;
use super::PUBLIC_TOOL_NAME;
use super::call_nested_tool;
use super::process::CodeModeProcess;
use super::process::write_message;
use super::protocol::HostToNodeMessage;
use super::protocol::NodeToHostMessage;
use crate::tools::parallel::ToolCallRuntime;

pub(crate) struct CodeModeWorker {
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl Drop for CodeModeWorker {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

impl CodeModeProcess {
    pub(super) fn worker(
        &self,
        exec: ExecContext,
        tool_runtime: ToolCallRuntime,
    ) -> CodeModeWorker {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let stdin = self.stdin.clone();
        let message_rx = self.message_rx.clone();
        tokio::spawn(async move {
            loop {
                let next_message = tokio::select! {
                    _ = &mut shutdown_rx => break,
                    message = async {
                        let mut message_rx = message_rx.lock().await;
                        message_rx.recv().await
                    } => message,
                };
                let Some(next_message) = next_message else {
                    break;
                };
                match next_message {
                    NodeToHostMessage::ToolCall { tool_call } => {
                        let exec = exec.clone();
                        let tool_runtime = tool_runtime.clone();
                        let stdin = stdin.clone();
                        tokio::spawn(async move {
                            let result = call_nested_tool(
                                exec,
                                tool_runtime,
                                tool_call.name,
                                tool_call.input,
                                CancellationToken::new(),
                            )
                            .await;
                            let (code_mode_result, error_text) = match result {
                                Ok(code_mode_result) => (code_mode_result, None),
                                Err(error) => (serde_json::Value::Null, Some(error.to_string())),
                            };
                            let response = HostToNodeMessage::Response {
                                request_id: tool_call.request_id,
                                id: tool_call.id,
                                code_mode_result,
                                error_text,
                            };
                            if let Err(err) = write_message(&stdin, &response).await {
                                warn!("failed to write {PUBLIC_TOOL_NAME} tool response: {err}");
                            }
                        });
                    }
                    NodeToHostMessage::Notify { notify } => {
                        if notify.text.trim().is_empty() {
                            continue;
                        }
                        if exec
                            .session
                            .inject_response_items(vec![ResponseInputItem::CustomToolCallOutput {
                                call_id: notify.call_id.clone(),
                                name: Some(PUBLIC_TOOL_NAME.to_string()),
                                output: FunctionCallOutputPayload::from_text(notify.text),
                            }])
                            .await
                            .is_err()
                        {
                            warn!(
                                "failed to inject {PUBLIC_TOOL_NAME} notify message for cell {}: no active turn",
                                notify.cell_id
                            );
                        }
                    }
                    unexpected_message @ (NodeToHostMessage::Yielded { .. }
                    | NodeToHostMessage::Terminated { .. }
                    | NodeToHostMessage::Result { .. }) => {
                        error!(
                            "received unexpected {PUBLIC_TOOL_NAME} message in worker loop: {unexpected_message:?}"
                        );
                        break;
                    }
                }
            }
        });

        CodeModeWorker {
            shutdown_tx: Some(shutdown_tx),
        }
    }
}
