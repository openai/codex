use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use codex_code_mode::CellId;
use codex_code_mode::CodeModeNestedToolCall;
use codex_code_mode::CodeModeSessionDelegate;
use codex_code_mode::NotificationFuture;
use codex_code_mode::ToolInvocationFuture;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use serde_json::Value as JsonValue;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::PUBLIC_TOOL_NAME;
use super::call_nested_tool;
use crate::tools::ToolRouter;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::parallel::ToolCallRuntime;

type CellHostSender = watch::Sender<Option<Arc<CoreTurnHost>>>;
type CellHostMap = Mutex<HashMap<CellId, CellHostSender>>;

pub(super) struct CodeModeDispatchBroker {
    dispatch_tx: async_channel::Sender<DispatchMessage>,
    dispatch_rx: async_channel::Receiver<DispatchMessage>,
    dispatch_hosts: Arc<CellHostMap>,
    current_host: Arc<Mutex<Option<Arc<CoreTurnHost>>>>,
}

impl CodeModeDispatchBroker {
    pub(super) fn new() -> Self {
        let (dispatch_tx, dispatch_rx) = async_channel::unbounded();
        Self {
            dispatch_tx,
            dispatch_rx,
            dispatch_hosts: Arc::new(Mutex::new(HashMap::new())),
            current_host: Arc::new(Mutex::new(None)),
        }
    }

    pub(super) fn mark_cell_ready_for_dispatch(&self, cell_id: &CellId) -> Result<(), String> {
        let host = match self.current_host.lock() {
            Ok(current_host) => current_host.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
        .ok_or_else(|| "code mode tool dispatcher is unavailable".to_string())?;
        dispatch_host(&self.dispatch_hosts, cell_id).send_replace(Some(host));
        Ok(())
    }

    pub(super) fn close_cell(&self, cell_id: &CellId) {
        remove_dispatch_host(&self.dispatch_hosts, cell_id);
    }

    pub(super) fn start_turn_worker(
        &self,
        session: Arc<crate::session::session::Session>,
        turn: Arc<crate::session::turn_context::TurnContext>,
        router: Arc<ToolRouter>,
        tracker: SharedTurnDiffTracker,
    ) -> CodeModeDispatchWorker {
        let tool_runtime = ToolCallRuntime::new(router, Arc::clone(&session), turn, tracker);
        let host = Arc::new(CoreTurnHost {
            session,
            tool_runtime,
        });
        match self.current_host.lock() {
            Ok(mut current_host) => *current_host = Some(Arc::clone(&host)),
            Err(poisoned) => *poisoned.into_inner() = Some(Arc::clone(&host)),
        }
        let dispatch_rx = self.dispatch_rx.clone();
        let dispatch_hosts = Arc::clone(&self.dispatch_hosts);
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        tokio::spawn(async move {
            loop {
                let message = tokio::select! {
                    _ = &mut shutdown_rx => break,
                    message = dispatch_rx.recv() => message.ok(),
                };
                let Some(message) = message else {
                    break;
                };
                match message {
                    DispatchMessage::Notify {
                        call_id,
                        cell_id,
                        text,
                        cancellation_token,
                        response_tx,
                    } => {
                        let response = if let Some(host) =
                            wait_for_cell_host(&dispatch_hosts, &cell_id, &cancellation_token).await
                        {
                            host.notify(call_id, cell_id, text).await
                        } else {
                            remove_dispatch_host(&dispatch_hosts, &cell_id);
                            Err("code mode notification cancelled".to_string())
                        };
                        let _ = response_tx.send(response);
                    }
                    DispatchMessage::InvokeTool {
                        invocation,
                        cancellation_token,
                        response_tx,
                    } => {
                        let cell_id = invocation.cell_id.clone();
                        let Some(host) =
                            wait_for_cell_host(&dispatch_hosts, &cell_id, &cancellation_token)
                                .await
                        else {
                            remove_dispatch_host(&dispatch_hosts, &cell_id);
                            continue;
                        };
                        tokio::spawn(async move {
                            let response = tokio::select! {
                                response = host.invoke_tool(
                                    invocation,
                                    cancellation_token.clone(),
                                ) => response,
                                _ = cancellation_token.cancelled() => return,
                            };
                            let _ = response_tx.send(response);
                        });
                    }
                }
            }
        });
        CodeModeDispatchWorker {
            shutdown_tx: Some(shutdown_tx),
            current_host: Arc::clone(&self.current_host),
            host,
        }
    }
}

fn dispatch_host(dispatch_hosts: &CellHostMap, cell_id: &CellId) -> CellHostSender {
    let mut dispatch_hosts = match dispatch_hosts.lock() {
        Ok(dispatch_hosts) => dispatch_hosts,
        Err(poisoned) => poisoned.into_inner(),
    };
    dispatch_hosts
        .entry(cell_id.clone())
        .or_insert_with(|| watch::channel(None).0)
        .clone()
}

fn remove_dispatch_host(dispatch_hosts: &CellHostMap, cell_id: &CellId) {
    let mut dispatch_hosts = match dispatch_hosts.lock() {
        Ok(dispatch_hosts) => dispatch_hosts,
        Err(poisoned) => poisoned.into_inner(),
    };
    dispatch_hosts.remove(cell_id);
}

async fn wait_for_cell_host(
    dispatch_hosts: &CellHostMap,
    cell_id: &CellId,
    cancellation_token: &CancellationToken,
) -> Option<Arc<CoreTurnHost>> {
    if cancellation_token.is_cancelled() {
        return None;
    }
    let mut host_rx = dispatch_host(dispatch_hosts, cell_id).subscribe();
    loop {
        if let Some(host) = host_rx.borrow_and_update().clone() {
            return Some(host);
        }
        tokio::select! {
            changed = host_rx.changed() => {
                if changed.is_err() {
                    return None;
                }
            }
            _ = cancellation_token.cancelled() => return None,
        }
    }
}

impl CodeModeSessionDelegate for CodeModeDispatchBroker {
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            if cancellation_token.is_cancelled() {
                return Err("code mode nested tool call cancelled".to_string());
            }
            let (response_tx, response_rx) = oneshot::channel();
            self.dispatch_tx
                .send(DispatchMessage::InvokeTool {
                    invocation,
                    cancellation_token: cancellation_token.clone(),
                    response_tx,
                })
                .await
                .map_err(|_| "code mode nested tool dispatcher is unavailable".to_string())?;
            tokio::select! {
                response = response_rx => response
                    .map_err(|_| "code mode nested tool dispatcher stopped".to_string())?,
                _ = cancellation_token.cancelled() => {
                    Err("code mode nested tool call cancelled".to_string())
                }
            }
        })
    }

    fn notify<'a>(
        &'a self,
        call_id: String,
        cell_id: CellId,
        text: String,
        cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async move {
            if cancellation_token.is_cancelled() {
                return Err("code mode notification cancelled".to_string());
            }
            let (response_tx, response_rx) = oneshot::channel();
            self.dispatch_tx
                .send(DispatchMessage::Notify {
                    call_id,
                    cell_id,
                    text,
                    cancellation_token: cancellation_token.clone(),
                    response_tx,
                })
                .await
                .map_err(|_| "code mode notification dispatcher is unavailable".to_string())?;
            tokio::select! {
                response = response_rx => response
                    .map_err(|_| "code mode notification dispatcher stopped".to_string())?,
                _ = cancellation_token.cancelled() => {
                    Err("code mode notification cancelled".to_string())
                }
            }
        })
    }

    fn cell_closed(&self, cell_id: &CellId) {
        self.close_cell(cell_id);
    }
}

enum DispatchMessage {
    InvokeTool {
        invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
        response_tx: oneshot::Sender<Result<JsonValue, String>>,
    },
    Notify {
        call_id: String,
        cell_id: CellId,
        text: String,
        cancellation_token: CancellationToken,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
}

pub(crate) struct CodeModeDispatchWorker {
    shutdown_tx: Option<oneshot::Sender<()>>,
    current_host: Arc<Mutex<Option<Arc<CoreTurnHost>>>>,
    host: Arc<CoreTurnHost>,
}

impl Drop for CodeModeDispatchWorker {
    fn drop(&mut self) {
        let mut current_host = match self.current_host.lock() {
            Ok(current_host) => current_host,
            Err(poisoned) => poisoned.into_inner(),
        };
        if current_host
            .as_ref()
            .is_some_and(|current_host| Arc::ptr_eq(current_host, &self.host))
        {
            *current_host = None;
        }
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

struct CoreTurnHost {
    session: Arc<crate::session::session::Session>,
    tool_runtime: ToolCallRuntime,
}

impl CoreTurnHost {
    async fn invoke_tool(
        &self,
        invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String> {
        call_nested_tool(self.tool_runtime.clone(), invocation, cancellation_token)
            .await
            .map_err(|error| error.to_string())
    }

    async fn notify(&self, call_id: String, cell_id: CellId, text: String) -> Result<(), String> {
        if text.trim().is_empty() {
            return Ok(());
        }
        self.session
            .inject_if_running(vec![ResponseItem::CustomToolCallOutput {
                call_id,
                name: Some(PUBLIC_TOOL_NAME.to_string()),
                output: FunctionCallOutputPayload::from_text(text),
                metadata: None,
            }])
            .await
            .map_err(|_| {
                format!("failed to inject exec notify message for cell {cell_id}: no active turn")
            })
    }
}
