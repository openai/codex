use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use codex_app_server::AppServerClientMessage;
use codex_app_server::AppServerEventNotification;
use codex_app_server::AppServerMessage;
use codex_app_server::spawn_in_memory_typed;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::DynamicToolCallResponse;
use codex_app_server_protocol::ElicitationAction as V2ElicitationAction;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::FileChangeRequestApprovalResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::Result as JsonResult;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_app_server_protocol::ToolRequestUserInputResponse;
use codex_core::config::Config;
use codex_core::config_loader::LoaderOverrides;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ReviewDecision;
use codex_feedback::CodexFeedback;
use codex_protocol::ThreadId;
use codex_protocol::request_user_input::RequestUserInputResponse as CoreRequestUserInputResponse;
use mcp_types::RequestId as McpRequestId;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

type PendingResponseMap =
    HashMap<RequestId, oneshot::Sender<std::result::Result<JsonResult, JSONRPCErrorError>>>;

pub(crate) struct PendingResponse {
    receiver: oneshot::Receiver<std::result::Result<JsonResult, JSONRPCErrorError>>,
}

impl PendingResponse {
    pub async fn into_typed<T: serde::de::DeserializeOwned>(
        self,
    ) -> std::result::Result<T, JSONRPCErrorError> {
        let value = self
            .receiver
            .await
            .map_err(|_| internal_error("response channel closed"))??;
        serde_json::from_value(value).map_err(|err| internal_error(err.to_string()))
    }

    pub async fn discard(self) -> std::result::Result<(), JSONRPCErrorError> {
        let _ = self
            .receiver
            .await
            .map_err(|_| internal_error("response channel closed"))??;
        Ok(())
    }
}

#[derive(Default)]
struct PendingServerRequests {
    exec: HashMap<String, RequestId>,
    patch: HashMap<String, RequestId>,
    user_input: HashMap<String, RequestId>,
}

#[derive(Default)]
struct QueuedResponses {
    exec: HashMap<String, ReviewDecision>,
    patch: HashMap<String, ReviewDecision>,
    user_input: HashMap<String, CoreRequestUserInputResponse>,
}

#[derive(Default)]
struct ElicitationState {
    thread_by_request: HashMap<McpRequestId, ThreadId>,
    queued: HashMap<McpRequestId, (String, codex_core::protocol::ElicitationAction)>,
}

#[derive(Default)]
struct TurnState {
    current_turn_by_thread: HashMap<ThreadId, String>,
}

pub(crate) struct AppServerClient {
    sender: mpsc::Sender<AppServerClientMessage>,
    pending: Arc<Mutex<PendingResponseMap>>,
    pending_server_requests: Arc<Mutex<PendingServerRequests>>,
    queued_responses: Arc<Mutex<QueuedResponses>>,
    elicitation_state: Arc<Mutex<ElicitationState>>,
    turn_state: Arc<Mutex<TurnState>>,
    next_request_id: Arc<AtomicI64>,
}

impl AppServerClient {
    pub(crate) fn spawn(
        app_event_tx: AppEventSender,
        config: Arc<Config>,
        cli_overrides: Vec<(String, toml::Value)>,
        loader_overrides: LoaderOverrides,
        feedback: CodexFeedback,
        config_warnings: Vec<codex_app_server_protocol::ConfigWarningNotification>,
        session_source: codex_protocol::protocol::SessionSource,
    ) -> Self {
        let in_process = spawn_in_memory_typed(
            config.codex_linux_sandbox_exe.clone(),
            config,
            cli_overrides,
            loader_overrides,
            feedback,
            config_warnings,
            session_source,
        );

        let client = Self {
            sender: in_process.incoming,
            pending: Arc::new(Mutex::new(HashMap::new())),
            pending_server_requests: Arc::new(Mutex::new(PendingServerRequests::default())),
            queued_responses: Arc::new(Mutex::new(QueuedResponses::default())),
            elicitation_state: Arc::new(Mutex::new(ElicitationState::default())),
            turn_state: Arc::new(Mutex::new(TurnState::default())),
            next_request_id: Arc::new(AtomicI64::new(1)),
        };

        client.spawn_outgoing_handler(app_event_tx, in_process.outgoing);
        client
    }

    pub(crate) async fn request(
        &self,
        build: impl FnOnce(RequestId) -> ClientRequest,
    ) -> std::result::Result<PendingResponse, JSONRPCErrorError> {
        let request_id = RequestId::Integer(self.next_request_id.fetch_add(1, Ordering::Relaxed));
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(request_id.clone(), tx);
        if self
            .sender
            .send(AppServerClientMessage::Request(build(request_id.clone())))
            .await
            .is_err()
        {
            self.pending.lock().await.remove(&request_id);
            return Err(internal_error("app-server request channel closed"));
        }
        Ok(PendingResponse { receiver: rx })
    }

    pub(crate) async fn send_notification(
        &self,
        notification: ClientNotification,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        self.sender
            .send(AppServerClientMessage::Notification(notification))
            .await
            .map_err(|_| internal_error("app-server notification channel closed"))
    }

    pub(crate) async fn respond_exec_approval(
        &self,
        call_id: String,
        decision: ReviewDecision,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        let request_id = {
            let mut pending = self.pending_server_requests.lock().await;
            pending.exec.remove(&call_id)
        };

        let Some(request_id) = request_id else {
            self.queued_responses
                .lock()
                .await
                .exec
                .insert(call_id, decision);
            return Ok(());
        };

        let response = CommandExecutionRequestApprovalResponse {
            decision: map_exec_decision(decision),
        };
        self.send_response(request_id, response).await
    }

    pub(crate) async fn respond_patch_approval(
        &self,
        call_id: String,
        decision: ReviewDecision,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        let request_id = {
            let mut pending = self.pending_server_requests.lock().await;
            pending.patch.remove(&call_id)
        };

        let Some(request_id) = request_id else {
            self.queued_responses
                .lock()
                .await
                .patch
                .insert(call_id, decision);
            return Ok(());
        };

        let response = FileChangeRequestApprovalResponse {
            decision: map_patch_decision(decision),
        };
        self.send_response(request_id, response).await
    }

    pub(crate) async fn respond_user_input(
        &self,
        call_id: String,
        response: CoreRequestUserInputResponse,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        let request_id = {
            let mut pending = self.pending_server_requests.lock().await;
            pending.user_input.remove(&call_id)
        };

        let Some(request_id) = request_id else {
            self.queued_responses
                .lock()
                .await
                .user_input
                .insert(call_id, response);
            return Ok(());
        };

        let response = ToolRequestUserInputResponse {
            answers: response
                .answers
                .into_iter()
                .map(|(id, answer)| {
                    (
                        id,
                        codex_app_server_protocol::ToolRequestUserInputAnswer {
                            answers: answer.answers,
                        },
                    )
                })
                .collect(),
        };

        self.send_response(request_id, response).await
    }

    pub(crate) async fn respond_elicitation(
        &self,
        server_name: String,
        request_id: McpRequestId,
        decision: codex_core::protocol::ElicitationAction,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        let thread_id = {
            let mut state = self.elicitation_state.lock().await;
            state.thread_by_request.remove(&request_id)
        };

        let Some(thread_id) = thread_id else {
            self.elicitation_state
                .lock()
                .await
                .queued
                .insert(request_id, (server_name, decision));
            return Ok(());
        };

        let params = codex_app_server_protocol::McpElicitationResolveParams {
            thread_id: thread_id.to_string(),
            server_name,
            request_id: request_id.clone(),
            decision: V2ElicitationAction::from(decision),
        };

        self.request(|id| ClientRequest::McpElicitationResolve {
            request_id: id,
            params,
        })
        .await?
        .discard()
        .await
    }

    pub(crate) async fn interrupt_current_turn(
        &self,
        thread_id: ThreadId,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        let turn_id = {
            let state = self.turn_state.lock().await;
            state.current_turn_by_thread.get(&thread_id).cloned()
        };
        let Some(turn_id) = turn_id else {
            return Ok(());
        };

        let params = codex_app_server_protocol::TurnInterruptParams {
            thread_id: thread_id.to_string(),
            turn_id,
        };
        self.request(|id| ClientRequest::TurnInterrupt {
            request_id: id,
            params,
        })
        .await?
        .discard()
        .await
    }

    async fn send_response<T: serde::Serialize>(
        &self,
        request_id: RequestId,
        response: T,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        let result =
            serde_json::to_value(response).map_err(|err| internal_error(err.to_string()))?;
        self.sender
            .send(AppServerClientMessage::Response {
                id: request_id,
                result,
            })
            .await
            .map_err(|_| internal_error("app-server response channel closed"))
    }

    fn spawn_outgoing_handler(
        &self,
        app_event_tx: AppEventSender,
        mut outgoing: mpsc::Receiver<AppServerMessage>,
    ) {
        let pending = Arc::clone(&self.pending);
        let pending_server_requests = Arc::clone(&self.pending_server_requests);
        let queued_responses = Arc::clone(&self.queued_responses);
        let elicitation_state = Arc::clone(&self.elicitation_state);
        let turn_state = Arc::clone(&self.turn_state);
        let sender = self.sender.clone();
        let next_request_id = Arc::clone(&self.next_request_id);

        tokio::spawn(async move {
            while let Some(message) = outgoing.recv().await {
                match message {
                    AppServerMessage::EventNotification(notification) => {
                        handle_event_notification(
                            notification,
                            &app_event_tx,
                            &turn_state,
                            &elicitation_state,
                            &sender,
                            next_request_id.as_ref(),
                        )
                        .await;
                    }
                    AppServerMessage::Request(request) => {
                        handle_server_request(
                            request,
                            &pending_server_requests,
                            &queued_responses,
                            &sender,
                        )
                        .await;
                    }
                    AppServerMessage::Response { id, result } => {
                        if let Some(tx) = pending.lock().await.remove(&id) {
                            let _ = tx.send(Ok(result));
                        }
                    }
                    AppServerMessage::Error { id, error } => {
                        if let Some(tx) = pending.lock().await.remove(&id) {
                            let _ = tx.send(Err(error));
                        }
                    }
                    AppServerMessage::Notification(_notification) => {
                        // v2 notifications are currently surfaced through codex/event
                        // for the TUI, so ignore explicit server notifications here.
                    }
                }
            }
        });
    }
}

async fn handle_event_notification(
    notification: AppServerEventNotification,
    app_event_tx: &AppEventSender,
    turn_state: &Mutex<TurnState>,
    elicitation_state: &Mutex<ElicitationState>,
    sender: &mpsc::Sender<AppServerClientMessage>,
    next_request_id: &AtomicI64,
) {
    if !notification.method.starts_with("codex/event/") {
        return;
    }
    let Some(params) = notification.params else {
        return;
    };
    let serde_json::Value::Object(mut map) = params else {
        return;
    };
    let Some(conversation_id) = map.remove("conversationId") else {
        return;
    };
    let thread_id = match conversation_id.as_str() {
        Some(value) => match ThreadId::from_string(value) {
            Ok(thread_id) => thread_id,
            Err(err) => {
                tracing::warn!("invalid thread id in event: {err}");
                return;
            }
        },
        None => return,
    };

    let event_value = serde_json::Value::Object(map);
    let event: Event = match serde_json::from_value(event_value) {
        Ok(event) => event,
        Err(err) => {
            tracing::warn!("failed to parse codex event: {err}");
            return;
        }
    };

    match &event.msg {
        EventMsg::TurnStarted(_) => {
            if !event.id.is_empty() {
                turn_state
                    .lock()
                    .await
                    .current_turn_by_thread
                    .insert(thread_id, event.id.clone());
            }
        }
        EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_) => {
            if !event.id.is_empty() {
                let mut state = turn_state.lock().await;
                if state
                    .current_turn_by_thread
                    .get(&thread_id)
                    .is_some_and(|id| id == &event.id)
                {
                    state.current_turn_by_thread.remove(&thread_id);
                }
            }
        }
        EventMsg::ElicitationRequest(ev) => {
            let mut state = elicitation_state.lock().await;
            state.thread_by_request.insert(ev.id.clone(), thread_id);
            if let Some((server_name, decision)) = state.queued.remove(&ev.id) {
                let params = codex_app_server_protocol::McpElicitationResolveParams {
                    thread_id: thread_id.to_string(),
                    server_name,
                    request_id: ev.id.clone(),
                    decision: V2ElicitationAction::from(decision),
                };
                let request_id =
                    RequestId::Integer(next_request_id.fetch_add(1, Ordering::Relaxed));
                let request = ClientRequest::McpElicitationResolve { request_id, params };
                let _ = sender.send(AppServerClientMessage::Request(request)).await;
            }
        }
        _ => {}
    }

    app_event_tx.send(AppEvent::CodexThreadEvent { thread_id, event });
}

async fn handle_server_request(
    request: ServerRequest,
    pending: &Mutex<PendingServerRequests>,
    queued: &Mutex<QueuedResponses>,
    sender: &mpsc::Sender<AppServerClientMessage>,
) {
    match request {
        ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
            record_server_request(
                request_id,
                params.item_id,
                &mut pending.lock().await.exec,
                &mut queued.lock().await.exec,
                |decision| CommandExecutionRequestApprovalResponse {
                    decision: map_exec_decision(decision),
                },
                sender,
            )
            .await;
        }
        ServerRequest::FileChangeRequestApproval { request_id, params } => {
            record_server_request(
                request_id,
                params.item_id,
                &mut pending.lock().await.patch,
                &mut queued.lock().await.patch,
                |decision| FileChangeRequestApprovalResponse {
                    decision: map_patch_decision(decision),
                },
                sender,
            )
            .await;
        }
        ServerRequest::ToolRequestUserInput { request_id, params } => {
            record_user_input_request(request_id, params, pending, queued, sender).await;
        }
        ServerRequest::DynamicToolCall { request_id, params } => {
            let response = DynamicToolCallResponse {
                output: "Dynamic tools are not supported in the TUI yet.".to_string(),
                success: false,
            };
            let _ = send_response(sender, request_id, response).await;
            tracing::warn!(
                "dynamic tool call {} for tool {} ignored",
                params.call_id,
                params.tool
            );
        }
        _ => {}
    }
}

async fn record_user_input_request(
    request_id: RequestId,
    params: ToolRequestUserInputParams,
    pending: &Mutex<PendingServerRequests>,
    queued: &Mutex<QueuedResponses>,
    sender: &mpsc::Sender<AppServerClientMessage>,
) {
    let call_id = params.item_id;
    let mut pending_guard = pending.lock().await;
    if let Some(response) = queued.lock().await.user_input.remove(&call_id) {
        drop(pending_guard);
        let response = ToolRequestUserInputResponse {
            answers: response
                .answers
                .into_iter()
                .map(|(id, answer)| {
                    (
                        id,
                        codex_app_server_protocol::ToolRequestUserInputAnswer {
                            answers: answer.answers,
                        },
                    )
                })
                .collect(),
        };
        let _ = send_response(sender, request_id, response).await;
    } else {
        pending_guard.user_input.insert(call_id, request_id);
    }
}

async fn record_server_request<T: serde::Serialize>(
    request_id: RequestId,
    call_id: String,
    pending: &mut HashMap<String, RequestId>,
    queued: &mut HashMap<String, ReviewDecision>,
    build_response: impl FnOnce(ReviewDecision) -> T,
    sender: &mpsc::Sender<AppServerClientMessage>,
) {
    if let Some(decision) = queued.remove(&call_id) {
        let response = build_response(decision);
        let _ = send_response(sender, request_id, response).await;
    } else {
        pending.insert(call_id, request_id);
    }
}

async fn send_response<T: serde::Serialize>(
    sender: &mpsc::Sender<AppServerClientMessage>,
    request_id: RequestId,
    response: T,
) -> std::result::Result<(), JSONRPCErrorError> {
    let result = serde_json::to_value(response).map_err(|err| internal_error(err.to_string()))?;
    sender
        .send(AppServerClientMessage::Response {
            id: request_id,
            result,
        })
        .await
        .map_err(|_| internal_error("app-server response channel closed"))
}

fn internal_error(message: impl Into<String>) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32603,
        message: message.into(),
        data: None,
    }
}

fn map_exec_decision(decision: ReviewDecision) -> CommandExecutionApprovalDecision {
    match decision {
        ReviewDecision::Approved => CommandExecutionApprovalDecision::Accept,
        ReviewDecision::ApprovedForSession => CommandExecutionApprovalDecision::AcceptForSession,
        ReviewDecision::ApprovedExecpolicyAmendment {
            proposed_execpolicy_amendment,
        } => CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
            execpolicy_amendment: proposed_execpolicy_amendment.into(),
        },
        ReviewDecision::Denied => CommandExecutionApprovalDecision::Decline,
        ReviewDecision::Abort => CommandExecutionApprovalDecision::Cancel,
    }
}

fn map_patch_decision(decision: ReviewDecision) -> FileChangeApprovalDecision {
    match decision {
        ReviewDecision::Approved => FileChangeApprovalDecision::Accept,
        ReviewDecision::ApprovedForSession => FileChangeApprovalDecision::AcceptForSession,
        ReviewDecision::ApprovedExecpolicyAmendment { .. } => FileChangeApprovalDecision::Accept,
        ReviewDecision::Denied => FileChangeApprovalDecision::Decline,
        ReviewDecision::Abort => FileChangeApprovalDecision::Cancel,
    }
}
