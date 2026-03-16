use std::collections::HashMap;
use std::sync::Arc;

use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::warn;

use super::CODE_MODE_RUNNER_SOURCE;
use super::PUBLIC_TOOL_NAME;
use super::protocol::CodeModeNotify;
use super::protocol::CodeModeToolCall;
use super::protocol::HostToNodeMessage;
use super::protocol::NodeToHostMessage;
use super::protocol::message_request_id;

pub(super) struct CodeModeProcess {
    pub(super) child: tokio::process::Child,
    pub(super) stdin: Arc<Mutex<tokio::process::ChildStdin>>,
    pub(super) stdout_task: JoinHandle<()>,
    pub(super) response_waiters: Arc<Mutex<HashMap<String, oneshot::Sender<NodeToHostMessage>>>>,
    pub(super) tool_call_rx: Arc<Mutex<mpsc::UnboundedReceiver<CodeModeToolCall>>>,
    pub(super) notify_rx: Arc<Mutex<mpsc::UnboundedReceiver<CodeModeNotify>>>,
}

impl CodeModeProcess {
    pub(super) async fn send(
        &mut self,
        request_id: &str,
        message: &HostToNodeMessage,
    ) -> Result<NodeToHostMessage, std::io::Error> {
        if self.stdout_task.is_finished() {
            return Err(std::io::Error::other(format!(
                "{PUBLIC_TOOL_NAME} runner is not available"
            )));
        }

        let (tx, rx) = oneshot::channel();
        self.response_waiters
            .lock()
            .await
            .insert(request_id.to_string(), tx);
        if let Err(err) = write_message(&self.stdin, message).await {
            self.response_waiters.lock().await.remove(request_id);
            return Err(err);
        }

        match rx.await {
            Ok(message) => Ok(message),
            Err(_) => Err(std::io::Error::other(format!(
                "{PUBLIC_TOOL_NAME} runner is not available"
            ))),
        }
    }

    pub(super) fn has_exited(&mut self) -> Result<bool, std::io::Error> {
        self.child
            .try_wait()
            .map(|status| status.is_some())
            .map_err(std::io::Error::other)
    }
}

pub(super) async fn spawn_code_mode_process(
    node_path: &std::path::Path,
) -> Result<CodeModeProcess, std::io::Error> {
    let mut cmd = tokio::process::Command::new(node_path);
    cmd.arg("--experimental-vm-modules");
    cmd.arg("--eval");
    cmd.arg(CODE_MODE_RUNNER_SOURCE);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(std::io::Error::other)?;
    let stdout = child.stdout.take().ok_or_else(|| {
        std::io::Error::other(format!("{PUBLIC_TOOL_NAME} runner missing stdout"))
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        std::io::Error::other(format!("{PUBLIC_TOOL_NAME} runner missing stderr"))
    })?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other(format!("{PUBLIC_TOOL_NAME} runner missing stdin")))?;
    let stdin = Arc::new(Mutex::new(stdin));
    let response_waiters = Arc::new(Mutex::new(HashMap::<
        String,
        oneshot::Sender<NodeToHostMessage>,
    >::new()));
    let (tool_call_tx, tool_call_rx) = mpsc::unbounded_channel();
    let (notify_tx, notify_rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        match reader.read_to_end(&mut buf).await {
            Ok(_) => {
                let stderr = String::from_utf8_lossy(&buf).trim().to_string();
                if !stderr.is_empty() {
                    warn!("{PUBLIC_TOOL_NAME} runner stderr: {stderr}");
                }
            }
            Err(err) => {
                warn!("failed to read {PUBLIC_TOOL_NAME} stderr: {err}");
            }
        }
    });
    let stdout_task = tokio::spawn({
        let response_waiters = Arc::clone(&response_waiters);
        async move {
            let mut stdout_lines = BufReader::new(stdout).lines();
            loop {
                let line = match stdout_lines.next_line().await {
                    Ok(line) => line,
                    Err(err) => {
                        warn!("failed to read {PUBLIC_TOOL_NAME} stdout: {err}");
                        break;
                    }
                };
                let Some(line) = line else {
                    break;
                };
                if line.trim().is_empty() {
                    continue;
                }
                let message: NodeToHostMessage = match serde_json::from_str(&line) {
                    Ok(message) => message,
                    Err(err) => {
                        warn!("failed to parse {PUBLIC_TOOL_NAME} stdout message: {err}");
                        break;
                    }
                };
                match message {
                    NodeToHostMessage::ToolCall { tool_call } => {
                        let _ = tool_call_tx.send(tool_call);
                    }
                    NodeToHostMessage::Notify { notify } => {
                        let _ = notify_tx.send(notify);
                    }
                    message => {
                        if let Some(request_id) = message_request_id(&message)
                            && let Some(waiter) = response_waiters.lock().await.remove(request_id)
                        {
                            let _ = waiter.send(message);
                        }
                    }
                }
            }
            response_waiters.lock().await.clear();
        }
    });

    Ok(CodeModeProcess {
        child,
        stdin,
        stdout_task,
        response_waiters,
        tool_call_rx: Arc::new(Mutex::new(tool_call_rx)),
        notify_rx: Arc::new(Mutex::new(notify_rx)),
    })
}

pub(super) async fn write_message(
    stdin: &Arc<Mutex<tokio::process::ChildStdin>>,
    message: &HostToNodeMessage,
) -> Result<(), std::io::Error> {
    let line = serde_json::to_string(message).map_err(std::io::Error::other)?;
    let mut stdin = stdin.lock().await;
    stdin.write_all(line.as_bytes()).await?;
    stdin.write_all(b"\n").await?;
    stdin.flush().await?;
    Ok(())
}
