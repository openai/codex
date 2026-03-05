use std::sync::Arc;

use codex_app_server_client::ClientSurface;
use codex_app_server_client::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_client::InProcessClientStartArgs;
use codex_app_server_client::InProcessServerEvent;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::LoaderOverrides;
use codex_feedback::CodexFeedback;
use codex_protocol::ThreadId;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::WarningEvent;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::version::CODEX_CLI_VERSION;

const TUI_NOTIFY_CLIENT: &str = "codex-tui";

async fn initialize_app_server_client_name(thread: &CodexThread) {
    if let Err(err) = thread
        .set_app_server_client_name(Some(TUI_NOTIFY_CLIENT.to_string()))
        .await
    {
        tracing::error!("failed to set app server client name: {err}");
    }
}

fn in_process_start_args(config: &Config) -> InProcessClientStartArgs {
    let config_warnings: Vec<ConfigWarningNotification> = config
        .startup_warnings
        .iter()
        .map(|warning| ConfigWarningNotification {
            summary: warning.clone(),
            details: None,
            path: None,
            range: None,
        })
        .collect();

    InProcessClientStartArgs {
        arg0_paths: codex_arg0::Arg0DispatchPaths::default(),
        config: Arc::new(config.clone()),
        cli_overrides: Vec::new(),
        loader_overrides: LoaderOverrides::default(),
        cloud_requirements: CloudRequirementsLoader::default(),
        feedback: CodexFeedback::new(),
        config_warnings,
        surface: ClientSurface::Tui,
        client_name: Some(TUI_NOTIFY_CLIENT.to_string()),
        client_version: CODEX_CLI_VERSION.to_string(),
        experimental_api: true,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    }
}

struct RequestIdSequencer {
    next: i64,
}

impl RequestIdSequencer {
    fn new() -> Self {
        Self { next: 1 }
    }

    fn next(&mut self) -> RequestId {
        let id = self.next;
        self.next += 1;
        RequestId::Integer(id)
    }
}

fn send_codex_event(app_event_tx: &AppEventSender, msg: EventMsg) {
    app_event_tx.send(AppEvent::CodexEvent(Event {
        id: String::new(),
        msg,
    }));
}

fn send_warning_event(app_event_tx: &AppEventSender, message: String) {
    send_codex_event(app_event_tx, EventMsg::Warning(WarningEvent { message }));
}

fn send_error_event(app_event_tx: &AppEventSender, message: String) {
    send_codex_event(
        app_event_tx,
        EventMsg::Error(codex_protocol::protocol::ErrorEvent {
            message,
            codex_error_info: None,
        }),
    );
}

async fn send_request_with_response<T>(
    client: &InProcessAppServerClient,
    request: ClientRequest,
    method: &str,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    client.request_typed(request).await.map_err(|err| {
        if method.is_empty() {
            err.to_string()
        } else {
            format!("{method}: {err}")
        }
    })
}

fn session_configured_from_thread_start_response(
    response: ThreadStartResponse,
) -> Result<SessionConfiguredEvent, String> {
    let session_id = ThreadId::from_string(&response.thread.id)
        .map_err(|err| format!("thread/start returned invalid thread id: {err}"))?;

    Ok(SessionConfiguredEvent {
        session_id,
        forked_from_id: None,
        thread_name: response.thread.name,
        model: response.model,
        model_provider_id: response.model_provider,
        service_tier: response.service_tier,
        approval_policy: response.approval_policy.to_core(),
        sandbox_policy: response.sandbox.to_core(),
        cwd: response.cwd,
        reasoning_effort: response.reasoning_effort,
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: response.thread.path,
    })
}

fn active_turn_id_from_turns(turns: &[codex_app_server_protocol::Turn]) -> Option<String> {
    turns.iter().rev().find_map(|turn| {
        if turn.status == TurnStatus::InProgress {
            Some(turn.id.clone())
        } else {
            None
        }
    })
}

fn server_request_method_name(request: &ServerRequest) -> String {
    serde_json::to_value(request)
        .ok()
        .and_then(|value| {
            value
                .get("method")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn legacy_notification_to_event(notification: JSONRPCNotification) -> Result<Event, String> {
    let mut value = notification
        .params
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let serde_json::Value::Object(ref mut object) = value else {
        return Err(format!(
            "legacy notification `{}` params were not an object",
            notification.method
        ));
    };
    object.insert(
        "type".to_string(),
        serde_json::Value::String(notification.method),
    );

    let msg: EventMsg =
        serde_json::from_value(value).map_err(|err| format!("failed to decode event: {err}"))?;
    Ok(Event {
        id: String::new(),
        msg,
    })
}

async fn process_phase_2_1_command(
    op: Op,
    thread_id: &str,
    current_turn_id: &mut Option<String>,
    request_ids: &mut RequestIdSequencer,
    client: &InProcessAppServerClient,
    app_event_tx: &AppEventSender,
) -> bool {
    match op {
        Op::Interrupt => {
            let Some(turn_id) = current_turn_id.clone() else {
                send_warning_event(
                    app_event_tx,
                    "turn/interrupt skipped because there is no active turn".to_string(),
                );
                return false;
            };
            let request = ClientRequest::TurnInterrupt {
                request_id: request_ids.next(),
                params: TurnInterruptParams {
                    thread_id: thread_id.to_string(),
                    turn_id,
                },
            };
            if let Err(err) = send_request_with_response::<TurnInterruptResponse>(
                client,
                request,
                "turn/interrupt",
            )
            .await
            {
                send_error_event(app_event_tx, err);
            }
        }
        Op::UserInput {
            items,
            final_output_json_schema,
        } => {
            let request = ClientRequest::TurnStart {
                request_id: request_ids.next(),
                params: TurnStartParams {
                    thread_id: thread_id.to_string(),
                    input: items.into_iter().map(Into::into).collect(),
                    output_schema: final_output_json_schema,
                    ..TurnStartParams::default()
                },
            };
            match send_request_with_response::<TurnStartResponse>(client, request, "turn/start")
                .await
            {
                Ok(response) => {
                    *current_turn_id = Some(response.turn.id);
                }
                Err(err) => send_error_event(app_event_tx, err),
            }
        }
        Op::UserTurn {
            items,
            cwd,
            approval_policy,
            sandbox_policy,
            model,
            effort,
            summary,
            service_tier,
            final_output_json_schema,
            collaboration_mode,
            personality,
        } => {
            let request = ClientRequest::TurnStart {
                request_id: request_ids.next(),
                params: TurnStartParams {
                    thread_id: thread_id.to_string(),
                    input: items.into_iter().map(Into::into).collect(),
                    cwd: Some(cwd),
                    approval_policy: Some(approval_policy.into()),
                    sandbox_policy: Some(sandbox_policy.into()),
                    model: Some(model),
                    service_tier,
                    effort,
                    summary,
                    personality,
                    output_schema: final_output_json_schema,
                    collaboration_mode,
                },
            };
            match send_request_with_response::<TurnStartResponse>(client, request, "turn/start")
                .await
            {
                Ok(response) => {
                    *current_turn_id = Some(response.turn.id);
                }
                Err(err) => send_error_event(app_event_tx, err),
            }
        }
        Op::Shutdown => {
            let request = ClientRequest::ThreadUnsubscribe {
                request_id: request_ids.next(),
                params: ThreadUnsubscribeParams {
                    thread_id: thread_id.to_string(),
                },
            };
            if let Err(err) = send_request_with_response::<ThreadUnsubscribeResponse>(
                client,
                request,
                "thread/unsubscribe",
            )
            .await
            {
                send_warning_event(
                    app_event_tx,
                    format!("thread/unsubscribe failed during shutdown: {err}"),
                );
            }
            return true;
        }
        unsupported => {
            send_warning_event(
                app_event_tx,
                format!(
                    "op `{}` is not migrated to in-process yet (Phase 2.1 supports UserTurn/UserInput/Interrupt/Shutdown)",
                    serde_json::to_value(&unsupported)
                        .ok()
                        .and_then(|value| value
                            .get("type")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_owned))
                        .unwrap_or_else(|| "unknown".to_string())
                ),
            );
        }
    }

    false
}

async fn run_in_process_agent_loop(
    mut codex_op_rx: tokio::sync::mpsc::UnboundedReceiver<Op>,
    mut client: InProcessAppServerClient,
    thread_id: String,
    _session_id: ThreadId,
    app_event_tx: AppEventSender,
    mut request_ids: RequestIdSequencer,
    mut current_turn_id: Option<String>,
) {
    let mut pending_shutdown_complete = false;
    loop {
        tokio::select! {
            maybe_op = codex_op_rx.recv() => {
                match maybe_op {
                    Some(op) => {
                        let should_shutdown = process_phase_2_1_command(
                            op,
                            &thread_id,
                            &mut current_turn_id,
                            &mut request_ids,
                            &client,
                            &app_event_tx,
                        ).await;
                        if should_shutdown {
                            pending_shutdown_complete = true;
                            break;
                        }
                    }
                    None => break,
                }
            }
            maybe_event = client.next_event() => {
                let Some(event) = maybe_event else {
                    break;
                };

                match event {
                    InProcessServerEvent::ServerRequest(request) => {
                        let method = server_request_method_name(&request);
                        if let Err(err) = client.reject_server_request(
                            request.id().clone(),
                            JSONRPCErrorError {
                                code: -32000,
                                message: format!(
                                    "phase 2.1 in-process TUI does not support `{method}` interactive server requests"
                                ),
                                data: None,
                            },
                        ).await {
                            send_error_event(
                                &app_event_tx,
                                format!("failed to reject server request `{method}`: {err}"),
                            );
                        }
                    }
                    InProcessServerEvent::ServerNotification(_notification) => {}
                    InProcessServerEvent::LegacyNotification(notification) => {
                        let event = match legacy_notification_to_event(notification) {
                            Ok(event) => event,
                            Err(err) => {
                                send_warning_event(&app_event_tx, err);
                                continue;
                            }
                        };
                        if matches!(event.msg, EventMsg::SessionConfigured(_)) {
                            continue;
                        }

                        match &event.msg {
                            EventMsg::TurnStarted(payload) => {
                                current_turn_id = Some(payload.turn_id.clone());
                            }
                            EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_) => {
                                current_turn_id = None;
                            }
                            _ => {}
                        }

                        let shutdown_complete = matches!(event.msg, EventMsg::ShutdownComplete);
                        if shutdown_complete {
                            pending_shutdown_complete = true;
                            break;
                        }
                        app_event_tx.send(AppEvent::CodexEvent(event));
                    }
                    InProcessServerEvent::Lagged { skipped } => {
                        send_warning_event(
                            &app_event_tx,
                            format!("in-process app-server event stream lagged; dropped {skipped} events"),
                        );
                    }
                }
            }
        }
    }

    let shutdown_error = match client.shutdown().await {
        Ok(()) => None,
        Err(err) => Some(err),
    };
    if let Some(err) = &shutdown_error {
        send_warning_event(
            &app_event_tx,
            format!("in-process app-server shutdown failed: {err}"),
        );
    }
    if pending_shutdown_complete {
        if shutdown_error.is_some() {
            send_warning_event(
                &app_event_tx,
                "emitting shutdown complete after shutdown error to preserve TUI shutdown flow"
                    .to_string(),
            );
        }
        send_codex_event(&app_event_tx, EventMsg::ShutdownComplete);
    }
}

/// Spawn the agent bootstrapper and op forwarding loop, returning the
/// `UnboundedSender<Op>` used by the UI to submit operations.
pub(crate) fn spawn_agent(
    config: Config,
    app_event_tx: AppEventSender,
    _server: Arc<ThreadManager>,
) -> UnboundedSender<Op> {
    let (codex_op_tx, codex_op_rx) = unbounded_channel::<Op>();

    let app_event_tx_clone = app_event_tx;
    tokio::spawn(async move {
        let mut request_ids = RequestIdSequencer::new();
        let client = match InProcessAppServerClient::start(in_process_start_args(&config)).await {
            Ok(client) => client,
            Err(err) => {
                let message = format!("Failed to initialize in-process app-server client: {err}");
                tracing::error!("{message}");
                send_error_event(&app_event_tx_clone, message.clone());
                app_event_tx_clone.send(AppEvent::FatalExitRequest(message));
                return;
            }
        };

        let thread_start = match send_request_with_response::<ThreadStartResponse>(
            &client,
            ClientRequest::ThreadStart {
                request_id: request_ids.next(),
                params: ThreadStartParams::default(),
            },
            "thread/start",
        )
        .await
        {
            Ok(response) => response,
            Err(err) => {
                send_error_event(&app_event_tx_clone, err.clone());
                app_event_tx_clone.send(AppEvent::FatalExitRequest(err));
                let _ = client.shutdown().await;
                return;
            }
        };

        let session_configured = match session_configured_from_thread_start_response(thread_start) {
            Ok(event) => event,
            Err(message) => {
                send_error_event(&app_event_tx_clone, message.clone());
                app_event_tx_clone.send(AppEvent::FatalExitRequest(message));
                let _ = client.shutdown().await;
                return;
            }
        };

        let thread_id = session_configured.session_id.to_string();
        let session_id = session_configured.session_id;
        send_codex_event(
            &app_event_tx_clone,
            EventMsg::SessionConfigured(session_configured),
        );

        run_in_process_agent_loop(
            codex_op_rx,
            client,
            thread_id,
            session_id,
            app_event_tx_clone,
            request_ids,
            None,
        )
        .await;
    });

    codex_op_tx
}

/// Spawn agent loops for an existing thread (e.g., a forked thread).
/// Sends the provided `SessionConfiguredEvent` immediately, then forwards subsequent
/// events and accepts Ops for submission.
pub(crate) fn spawn_agent_from_existing(
    config: Config,
    mut session_configured: codex_protocol::protocol::SessionConfiguredEvent,
    app_event_tx: AppEventSender,
) -> UnboundedSender<Op> {
    let (codex_op_tx, codex_op_rx) = unbounded_channel::<Op>();

    let app_event_tx_clone = app_event_tx;
    tokio::spawn(async move {
        let mut request_ids = RequestIdSequencer::new();
        let client = match InProcessAppServerClient::start(in_process_start_args(&config)).await {
            Ok(client) => client,
            Err(err) => {
                let message = format!("failed to initialize in-process app-server client: {err}");
                send_error_event(&app_event_tx_clone, message.clone());
                app_event_tx_clone.send(AppEvent::FatalExitRequest(message));
                return;
            }
        };

        let expected_thread_id = session_configured.session_id.to_string();
        let thread_resume = match send_request_with_response::<ThreadResumeResponse>(
            &client,
            ClientRequest::ThreadResume {
                request_id: request_ids.next(),
                params: ThreadResumeParams {
                    thread_id: expected_thread_id.clone(),
                    path: session_configured.rollout_path.clone(),
                    ..ThreadResumeParams::default()
                },
            },
            "thread/resume",
        )
        .await
        {
            Ok(response) => response,
            Err(err) => {
                let message = format!("in-process thread resume failed: {err}");
                send_error_event(&app_event_tx_clone, message.clone());
                app_event_tx_clone.send(AppEvent::FatalExitRequest(message));
                let _ = client.shutdown().await;
                return;
            }
        };

        if thread_resume.thread.id != expected_thread_id {
            match ThreadId::from_string(&thread_resume.thread.id) {
                Ok(parsed) => {
                    send_warning_event(
                        &app_event_tx_clone,
                        format!(
                            "in-process thread/resume returned `{}` instead of `{expected_thread_id}`; using resumed id",
                            thread_resume.thread.id
                        ),
                    );
                    session_configured.session_id = parsed;
                }
                Err(err) => {
                    let message = format!(
                        "in-process thread/resume returned invalid thread id `{}` ({err})",
                        thread_resume.thread.id
                    );
                    send_error_event(&app_event_tx_clone, message.clone());
                    app_event_tx_clone.send(AppEvent::FatalExitRequest(message));
                    let _ = client.shutdown().await;
                    return;
                }
            }
        }

        if session_configured.thread_name.is_none() {
            session_configured.thread_name = thread_resume.thread.name;
        }
        if session_configured.rollout_path.is_none() {
            session_configured.rollout_path = thread_resume.thread.path;
        }

        let session_id = session_configured.session_id;
        let thread_id = session_id.to_string();
        let current_turn_id = active_turn_id_from_turns(&thread_resume.thread.turns);
        send_codex_event(
            &app_event_tx_clone,
            EventMsg::SessionConfigured(session_configured),
        );

        run_in_process_agent_loop(
            codex_op_rx,
            client,
            thread_id,
            session_id,
            app_event_tx_clone,
            request_ids,
            current_turn_id,
        )
        .await;
    });

    codex_op_tx
}

/// Spawn an op-forwarding loop for an existing thread without subscribing to events.
pub(crate) fn spawn_op_forwarder(thread: std::sync::Arc<CodexThread>) -> UnboundedSender<Op> {
    let (codex_op_tx, mut codex_op_rx) = unbounded_channel::<Op>();

    tokio::spawn(async move {
        initialize_app_server_client_name(thread.as_ref()).await;
        while let Some(op) = codex_op_rx.recv().await {
            if let Err(e) = thread.submit(op).await {
                tracing::error!("failed to submit op: {e}");
            }
        }
    });

    codex_op_tx
}
