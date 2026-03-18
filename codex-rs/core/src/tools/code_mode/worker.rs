use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use super::ExecContext;
use super::PUBLIC_TOOL_NAME;
use super::call_nested_tool;
use super::process::CodeModeProcess;
use super::process::write_message;
use super::protocol::HostToNodeMessage;
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
        let tool_call_rx = self.tool_call_rx.clone();
        tokio::spawn(async move {
            loop {
                let tool_call = tokio::select! {
                    _ = &mut shutdown_rx => break,
                    tool_call = async {
                        let mut tool_call_rx = tool_call_rx.lock().await;
                        tool_call_rx.recv().await
                    } => tool_call,
                };
                let Some(tool_call) = tool_call else {
                    break;
                };
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
        });

        CodeModeWorker {
            shutdown_tx: Some(shutdown_tx),
        }
    }
}
