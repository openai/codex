use std::collections::HashMap;
use std::sync::Arc;

use async_channel::Receiver;
use async_channel::Sender;
use codex_async_utils::OrCancelExt;
use codex_exec_server::EnvironmentManager;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::McpInvocation;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RequestUserInputEvent;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::Submission;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionsArgs;
use codex_protocol::request_permissions::RequestPermissionsEvent;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::codex::Codex;
use crate::codex::CodexSpawnArgs;
use crate::codex::CodexSpawnOk;
use crate::codex::SUBMISSION_CHANNEL_CAPACITY;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex::emit_subagent_session_started;
use crate::config::Config;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::review_approval_request_with_cancel;
use crate::guardian::routes_approval_to_guardian;
use crate::mcp_tool_call::MCP_TOOL_APPROVAL_ACCEPT;
use crate::mcp_tool_call::MCP_TOOL_APPROVAL_ACCEPT_FOR_SESSION;
use crate::mcp_tool_call::MCP_TOOL_APPROVAL_DECLINE_SYNTHETIC;
use crate::mcp_tool_call::build_guardian_mcp_tool_review_request;
use crate::mcp_tool_call::is_mcp_tool_approval_question_id;
use crate::mcp_tool_call::lookup_mcp_tool_metadata;
use codex_login::AuthManager;
use codex_models_manager::manager::ModelsManager;
use codex_protocol::error::CodexErr;
use codex_protocol::protocol::InitialHistory;
use codex_utils_absolute_path::AbsolutePathBuf;

#[cfg(test)]
use crate::codex::completed_session_loop_termination;

/// Start an interactive sub-Codex thread and return IO channels.
///
/// The returned `events_rx` yields non-approval events emitted by the sub-agent.
/// Approval requests are handled via `parent_session` and are not surfaced.
/// The returned `ops_tx` allows the caller to submit additional `Op`s to the sub-agent.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_codex_thread_interactive(
    config: Config,
    auth_manager: Arc<AuthManager>,
    models_manager: Arc<ModelsManager>,
    parent_session: Arc<Session>,
    parent_ctx: Arc<TurnContext>,
    cancel_token: CancellationToken,
    subagent_source: SubAgentSource,
    initial_history: Option<InitialHistory>,
) -> Result<Codex, CodexErr> {
    let (tx_sub, rx_sub) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);
    let (tx_ops, rx_ops) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);

    let CodexSpawnOk { codex, .. } = Codex::spawn(CodexSpawnArgs {
        config,
        auth_manager,
        models_manager,
        environment_manager: Arc::new(EnvironmentManager::from_environment(
            parent_ctx.environment.as_deref(),
        )),
        skills_manager: Arc::clone(&parent_session.services.skills_manager),
        plugins_manager: Arc::clone(&parent_session.services.plugins_manager),
        mcp_manager: Arc::clone(&parent_session.services.mcp_manager),
        skills_watcher: Arc::clone(&parent_session.services.skills_watcher),
        conversation_history: initial_history.unwrap_or(InitialHistory::New),
        session_source: SessionSource::SubAgent(subagent_source.clone()),
        agent_control: parent_session.services.agent_control.clone(),
        dynamic_tools: Vec::new(),
        persist_extended_history: false,
        metrics_service_name: None,
        inherited_shell_snapshot: None,
        user_shell_override: None,
        inherited_exec_policy: Some(Arc::clone(&parent_session.services.exec_policy)),
        parent_trace: None,
    })
    .await?;
    if parent_session.enabled(codex_features::Feature::GeneralAnalytics) {
        let thread_config = codex.thread_config_snapshot().await;
        let client_metadata = parent_session.app_server_client_metadata().await;
        emit_subagent_session_started(
            &parent_session.services.analytics_events_client,
            client_metadata,
            codex.session.conversation_id,
            thread_config,
            subagent_source,
        );
    }
    let codex = Arc::new(codex);

    // Use a child token so parent cancel cascades but we can scope it to this task
    let cancel_token_events = cancel_token.child_token();
    let cancel_token_ops = cancel_token.child_token();

    // Forward events from the sub-agent to the consumer, filtering approvals and
    // routing them to the parent session for decisions.
    let parent_session_clone = Arc::clone(&parent_session);
    let parent_ctx_clone = Arc::clone(&parent_ctx);
    let codex_for_events = Arc::clone(&codex);
    // Cache delegated MCP invocations so guardian can recover the full tool call
    // context when the later legacy RequestUserInput approval event only carries
    // a call_id plus approval question metadata.
    let pending_mcp_invocations = Arc::new(Mutex::new(HashMap::<String, McpInvocation>::new()));
    tokio::spawn(async move {
        forward_events(
            codex_for_events,
            tx_sub,
            parent_session_clone,
            parent_ctx_clone,
            pending_mcp_invocations,
            cancel_token_events,
        )
        .await;
    });

    // Forward ops from the caller to the sub-agent.
    let codex_for_ops = Arc::clone(&codex);
    tokio::spawn(async move {
        forward_ops(codex_for_ops, rx_ops, cancel_token_ops).await;
    });

    Ok(Codex {
        tx_sub: tx_ops,
        rx_event: rx_sub,
        agent_status: codex.agent_status.clone(),
        session: Arc::clone(&codex.session),
        session_loop_termination: codex.session_loop_termination.clone(),
    })
}

/// Convenience wrapper for one-time use with an initial prompt.
///
/// Internally calls the interactive variant, then immediately submits the provided input.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_codex_thread_one_shot(
    config: Config,
    auth_manager: Arc<AuthManager>,
    models_manager: Arc<ModelsManager>,
    input: Vec<UserInput>,
    parent_session: Arc<Session>,
    parent_ctx: Arc<TurnContext>,
    cancel_token: CancellationToken,
    subagent_source: SubAgentSource,
    final_output_json_schema: Option<Value>,
    initial_history: Option<InitialHistory>,
) -> Result<Codex, CodexErr> {
    // Use a child token so we can stop the delegate after completion without
    // requiring the caller to cancel the parent token.
    let child_cancel = cancel_token.child_token();
    let io = run_codex_thread_interactive(
        config,
        auth_manager,
        models_manager,
        parent_session,
        parent_ctx,
        child_cancel.clone(),
        subagent_source,
        initial_history,
    )
    .await?;

    // Send the initial input to kick off the one-shot turn.
    io.submit(Op::UserInput {
        items: input,
        final_output_json_schema,
    })
    .await?;

    // Bridge events so we can observe completion and shut down automatically.
    let (tx_bridge, rx_bridge) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);
    let ops_tx = io.tx_sub.clone();
    let agent_status = io.agent_status.clone();
    let session = Arc::clone(&io.session);
    let session_loop_termination = io.session_loop_termination.clone();
    let io_for_bridge = io;
    tokio::spawn(async move {
        while let Ok(event) = io_for_bridge.next_event().await {
            let should_shutdown = matches!(
                event.msg,
                EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_)
            );
            let _ = tx_bridge.send(event).await;
            if should_shutdown {
                let _ = ops_tx
                    .send(Submission {
                        id: "shutdown".to_string(),
                        op: Op::Shutdown {},
                        trace: None,
                    })
                    .await;
                child_cancel.cancel();
                break;
            }
        }
    });

    // For one-shot usage, return a closed `tx_sub` so callers cannot submit
    // additional ops after the initial request. Create a channel and drop the
    // receiver to close it immediately.
    let (tx_closed, rx_closed) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);
    drop(rx_closed);

    Ok(Codex {
        rx_event: rx_bridge,
        tx_sub: tx_closed,
        agent_status,
        session,
        session_loop_termination,
    })
}

async fn forward_events(
    codex: Arc<Codex>,
    tx_sub: Sender<Event>,
    parent_session: Arc<Session>,
    parent_ctx: Arc<TurnContext>,
    pending_mcp_invocations: Arc<Mutex<HashMap<String, McpInvocation>>>,
    cancel_token: CancellationToken,
) {
    let cancelled = cancel_token.cancelled();
    tokio::pin!(cancelled);

    loop {
        tokio::select! {
            _ = &mut cancelled => {
                shutdown_delegate(&codex).await;
                break;
            }
            event = codex.next_event() => {
                let event = match event {
                    Ok(event) => event,
                    Err(_) => break,
                };
                match event {
                    // ignore all legacy delta events
                    Event {
                        id: _,
                        msg: EventMsg::AgentMessageDelta(_) | EventMsg::AgentReasoningDelta(_),
                    } => {}
                    Event {
                        id: _,
                        msg: EventMsg::TokenCount(_),
                    } => {}
                    Event {
                        id: _,
                        msg: EventMsg::SessionConfigured(_),
                    } => {}
                    Event {
                        id: _,
                        msg: EventMsg::ThreadNameUpdated(_),
                    } => {}
                    Event {
                        id,
                        msg: EventMsg::ExecApprovalRequest(event),
                    } => {
                        // Initiate approval via parent session; do not surface to consumer.
                        handle_exec_approval(
                            &codex,
                            id,
                            &parent_session,
                            &parent_ctx,
                            event,
                            &cancel_token,
                        )
                        .await;
                    }
                    Event {
                        id,
                        msg: EventMsg::ApplyPatchApprovalRequest(event),
                    } => {
                        handle_patch_approval(
                            &codex,
                            id,
                            &parent_session,
                            &parent_ctx,
                            event,
                            &cancel_token,
                        )
                        .await;
                    }
                    Event {
                        msg: EventMsg::RequestPermissions(event),
                        ..
                    } => {
                        handle_request_permissions(
                            &codex,
                            &parent_session,
                            &parent_ctx,
                            event,
                            &cancel_token,
                        )
                        .await;
                    }
                    Event {
                        id,
                        msg: EventMsg::RequestUserInput(event),
                    } => {
                        handle_request_user_input(
                            &codex,
                            id,
                            &parent_session,
                            &parent_ctx,
                            &pending_mcp_invocations,
                            event,
                            &cancel_token,
                        )
                        .await;
                    }
                    Event {
                        id,
                        msg: EventMsg::McpToolCallBegin(event),
                    } => {
                        pending_mcp_invocations
                            .lock()
                            .await
                            .insert(event.call_id.clone(), event.invocation.clone());
                        if !forward_event_or_shutdown(
                            &codex,
                            &tx_sub,
                            &cancel_token,
                            Event {
                                id,
                                msg: EventMsg::McpToolCallBegin(event),
                            },
                        )
                        .await
                        {
                            break;
                        }
                    }
                    Event {
                        id,
                        msg: EventMsg::McpToolCallEnd(event),
                    } => {
                        pending_mcp_invocations.lock().await.remove(&event.call_id);
                        if !forward_event_or_shutdown(
                            &codex,
                            &tx_sub,
                            &cancel_token,
                            Event {
                                id,
                                msg: EventMsg::McpToolCallEnd(event),
                            },
                        )
                        .await
                        {
                            break;
                        }
                    }
                    other => {
                        if !forward_event_or_shutdown(&codex, &tx_sub, &cancel_token, other).await
                        {
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// Ask the delegate to stop and drain its events so background sends do not hit a closed channel.
async fn shutdown_delegate(codex: &Codex) {
    let _ = codex.submit(Op::Interrupt).await;
    let _ = codex.submit(Op::Shutdown {}).await;

    let _ = timeout(Duration::from_millis(500), async {
        while let Ok(event) = codex.next_event().await {
            if matches!(
                event.msg,
                EventMsg::TurnAborted(_) | EventMsg::TurnComplete(_)
            ) {
                break;
            }
        }
    })
    .await;
}

async fn forward_event_or_shutdown(
    codex: &Codex,
    tx_sub: &Sender<Event>,
    cancel_token: &CancellationToken,
    event: Event,
) -> bool {
    match tx_sub.send(event).or_cancel(cancel_token).await {
        Ok(Ok(())) => true,
        _ => {
            shutdown_delegate(codex).await;
            false
        }
    }
}

/// Forward ops from a caller to a sub-agent, respecting cancellation.
async fn forward_ops(
    codex: Arc<Codex>,
    rx_ops: Receiver<Submission>,
    cancel_token_ops: CancellationToken,
) {
    loop {
        let submission = match rx_ops.recv().or_cancel(&cancel_token_ops).await {
            Ok(Ok(submission)) => submission,
            Ok(Err(_)) | Err(_) => break,
        };
        let _ = codex.submit_with_id(submission).await;
    }
}

/// Handle an ExecApprovalRequest by consulting the parent session and replying.
async fn handle_exec_approval(
    codex: &Codex,
    turn_id: String,
    parent_session: &Arc<Session>,
    parent_ctx: &Arc<TurnContext>,
    event: ExecApprovalRequestEvent,
    cancel_token: &CancellationToken,
) {
    let approval_id_for_op = event.effective_approval_id();
    let ExecApprovalRequestEvent {
        call_id,
        approval_id,
        command,
        cwd,
        reason,
        network_approval_context,
        proposed_execpolicy_amendment,
        additional_permissions,
        available_decisions,
        ..
    } = event;
    let cwd = match AbsolutePathBuf::try_from(cwd.as_path()) {
        Ok(cwd) => cwd,
        Err(err) => {
            tracing::warn!("approval cwd is not absolute: {err}");
            parent_ctx.cwd.clone()
        }
    };
    let decision = if routes_approval_to_guardian(parent_ctx) {
        let review_cancel = cancel_token.child_token();
        let review_rx = spawn_guardian_review(
            Arc::clone(parent_session),
            Arc::clone(parent_ctx),
            GuardianApprovalRequest::Shell {
                id: call_id.clone(),
                command,
                cwd,
                sandbox_permissions: if additional_permissions.is_some() {
                    crate::sandboxing::SandboxPermissions::WithAdditionalPermissions
                } else {
                    crate::sandboxing::SandboxPermissions::UseDefault
                },
                additional_permissions,
                justification: None,
            },
            reason,
            review_cancel.clone(),
        );
        await_approval_with_cancel(
            async move { review_rx.await.unwrap_or_default() },
            parent_session,
            &approval_id_for_op,
            cancel_token,
            Some(&review_cancel),
        )
        .await
    } else {
        await_approval_with_cancel(
            parent_session.request_command_approval(
                parent_ctx,
                call_id,
                approval_id,
                command,
                cwd,
                reason,
                network_approval_context,
                proposed_execpolicy_amendment,
                additional_permissions,
                available_decisions,
            ),
            parent_session,
            &approval_id_for_op,
            cancel_token,
            /*review_cancel_token*/ None,
        )
        .await
    };

    let _ = codex
        .submit(Op::ExecApproval {
            id: approval_id_for_op,
            turn_id: Some(turn_id),
            decision,
        })
        .await;
}

/// Handle an ApplyPatchApprovalRequest by consulting the parent session and replying.
async fn handle_patch_approval(
    codex: &Codex,
    _id: String,
    parent_session: &Arc<Session>,
    parent_ctx: &Arc<TurnContext>,
    event: ApplyPatchApprovalRequestEvent,
    cancel_token: &CancellationToken,
) {
    let ApplyPatchApprovalRequestEvent {
        call_id,
        changes,
        reason,
        grant_root,
        ..
    } = event;
    let approval_id = call_id.clone();
    let guardian_decision = if routes_approval_to_guardian(parent_ctx) {
        let files = changes
            .keys()
            .map(|path| parent_ctx.cwd.join(path))
            .collect::<Vec<_>>();
        let review_cancel = cancel_token.child_token();
        let patch = changes
            .iter()
            .map(|(path, change)| match change {
                codex_protocol::protocol::FileChange::Add { content } => {
                    format!("*** Add File: {}\n{}", path.display(), content)
                }
                codex_protocol::protocol::FileChange::Delete { content } => {
                    format!("*** Delete File: {}\n{}", path.display(), content)
                }
                codex_protocol::protocol::FileChange::Update {
                    unified_diff,
                    move_path,
                } => {
                    if let Some(move_path) = move_path {
                        format!(
                            "*** Update File: {}\n*** Move to: {}\n{}",
                            path.display(),
                            move_path.display(),
                            unified_diff
                        )
                    } else {
                        format!("*** Update File: {}\n{}", path.display(), unified_diff)
                    }
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let review_rx = spawn_guardian_review(
            Arc::clone(parent_session),
            Arc::clone(parent_ctx),
            GuardianApprovalRequest::ApplyPatch {
                id: approval_id.clone(),
                cwd: parent_ctx.cwd.clone(),
                files,
                patch,
            },
            reason.clone(),
            review_cancel.clone(),
        );
        Some(
            await_approval_with_cancel(
                async move { review_rx.await.unwrap_or_default() },
                parent_session,
                &approval_id,
                cancel_token,
                Some(&review_cancel),
            )
            .await,
        )
    } else {
        None
    };
    let decision = if let Some(decision) = guardian_decision {
        decision
    } else {
        let decision_rx = parent_session
            .request_patch_approval(parent_ctx, call_id, changes, reason, grant_root)
            .await;
        await_approval_with_cancel(
            async move { decision_rx.await.unwrap_or_default() },
            parent_session,
            &approval_id,
            cancel_token,
            /*review_cancel_token*/ None,
        )
        .await
    };
    let _ = codex
        .submit(Op::PatchApproval {
            id: approval_id,
            decision,
        })
        .await;
}

async fn handle_request_user_input(
    codex: &Codex,
    id: String,
    parent_session: &Arc<Session>,
    parent_ctx: &Arc<TurnContext>,
    pending_mcp_invocations: &Arc<Mutex<HashMap<String, McpInvocation>>>,
    event: RequestUserInputEvent,
    cancel_token: &CancellationToken,
) {
    if routes_approval_to_guardian(parent_ctx)
        && let Some(response) = maybe_auto_review_mcp_request_user_input(
            parent_session,
            parent_ctx,
            pending_mcp_invocations,
            &event,
            cancel_token,
        )
        .await
    {
        let _ = codex.submit(Op::UserInputAnswer { id, response }).await;
        return;
    }

    let args = RequestUserInputArgs {
        questions: event.questions,
    };
    let response_fut =
        parent_session.request_user_input(parent_ctx, parent_ctx.sub_id.clone(), args);
    let response = await_user_input_with_cancel(
        response_fut,
        parent_session,
        &parent_ctx.sub_id,
        cancel_token,
    )
    .await;
    let _ = codex.submit(Op::UserInputAnswer { id, response }).await;
}

/// Intercepts delegated legacy MCP approval prompts on the RequestUserInput
/// compatibility path and, when guardian is active, answers them
/// programmatically after running the guardian review.
///
/// The RequestUserInput event only carries `call_id` plus approval question
/// metadata, so this helper joins it back to the cached `McpToolCallBegin`
/// invocation in order to rebuild the full guardian review request.
async fn maybe_auto_review_mcp_request_user_input(
    parent_session: &Arc<Session>,
    parent_ctx: &Arc<TurnContext>,
    pending_mcp_invocations: &Arc<Mutex<HashMap<String, McpInvocation>>>,
    event: &RequestUserInputEvent,
    cancel_token: &CancellationToken,
) -> Option<RequestUserInputResponse> {
    // TODO(ccunningham): Support delegated MCP approval elicitations here too after
    // coordinating with @fouad. Today guardian only auto-reviews the RequestUserInput
    // compatibility path for delegated MCP approvals.
    let question = event
        .questions
        .iter()
        .find(|question| is_mcp_tool_approval_question_id(&question.id))?;
    let invocation = pending_mcp_invocations
        .lock()
        .await
        .get(&event.call_id)
        .cloned()?;
    let metadata = lookup_mcp_tool_metadata(
        parent_session.as_ref(),
        parent_ctx.as_ref(),
        &invocation.server,
        &invocation.tool,
    )
    .await;
    let review_cancel = cancel_token.child_token();
    let review_rx = spawn_guardian_review(
        Arc::clone(parent_session),
        Arc::clone(parent_ctx),
        build_guardian_mcp_tool_review_request(&event.call_id, &invocation, metadata.as_ref()),
        /*retry_reason*/ None,
        review_cancel.clone(),
    );
    let decision = await_approval_with_cancel(
        async move { review_rx.await.unwrap_or_default() },
        parent_session,
        &event.call_id,
        cancel_token,
        Some(&review_cancel),
    )
    .await;
    let selected_label = match decision {
        ReviewDecision::ApprovedForSession => question
            .options
            .as_ref()
            .and_then(|options| {
                options
                    .iter()
                    .find(|option| option.label == MCP_TOOL_APPROVAL_ACCEPT_FOR_SESSION)
            })
            .map(|option| option.label.clone())
            .unwrap_or_else(|| MCP_TOOL_APPROVAL_ACCEPT.to_string()),
        ReviewDecision::Approved
        | ReviewDecision::ApprovedExecpolicyAmendment { .. }
        | ReviewDecision::NetworkPolicyAmendment { .. } => MCP_TOOL_APPROVAL_ACCEPT.to_string(),
        ReviewDecision::Denied | ReviewDecision::Abort => {
            MCP_TOOL_APPROVAL_DECLINE_SYNTHETIC.to_string()
        }
    };
    Some(RequestUserInputResponse {
        answers: HashMap::from([(
            question.id.clone(),
            codex_protocol::request_user_input::RequestUserInputAnswer {
                answers: vec![selected_label],
            },
        )]),
    })
}

fn spawn_guardian_review(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    request: GuardianApprovalRequest,
    retry_reason: Option<String>,
    cancel_token: CancellationToken,
) -> oneshot::Receiver<ReviewDecision> {
    let (tx, rx) = oneshot::channel();
    std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            let _ = tx.send(ReviewDecision::Denied);
            return;
        };
        let decision = runtime.block_on(review_approval_request_with_cancel(
            &session,
            &turn,
            request,
            retry_reason,
            cancel_token,
        ));
        let _ = tx.send(decision);
    });
    rx
}

async fn handle_request_permissions(
    codex: &Codex,
    parent_session: &Arc<Session>,
    parent_ctx: &Arc<TurnContext>,
    event: RequestPermissionsEvent,
    cancel_token: &CancellationToken,
) {
    let call_id = event.call_id;
    let args = RequestPermissionsArgs {
        reason: event.reason,
        permissions: event.permissions,
    };
    let response_fut = parent_session.request_permissions(parent_ctx, call_id.clone(), args);
    let response =
        await_request_permissions_with_cancel(response_fut, parent_session, &call_id, cancel_token)
            .await;
    let _ = codex
        .submit(Op::RequestPermissionsResponse {
            id: call_id,
            response,
        })
        .await;
}

async fn await_user_input_with_cancel<F>(
    fut: F,
    parent_session: &Session,
    sub_id: &str,
    cancel_token: &CancellationToken,
) -> RequestUserInputResponse
where
    F: core::future::Future<Output = Option<RequestUserInputResponse>>,
{
    tokio::select! {
        biased;
        _ = cancel_token.cancelled() => {
            let empty = RequestUserInputResponse {
                answers: HashMap::new(),
            };
            parent_session
                .notify_user_input_response(sub_id, empty.clone())
                .await;
            empty
        }
        response = fut => response.unwrap_or_else(|| RequestUserInputResponse {
            answers: HashMap::new(),
        }),
    }
}

async fn await_request_permissions_with_cancel<F>(
    fut: F,
    parent_session: &Session,
    call_id: &str,
    cancel_token: &CancellationToken,
) -> RequestPermissionsResponse
where
    F: core::future::Future<Output = Option<RequestPermissionsResponse>>,
{
    tokio::select! {
        biased;
        _ = cancel_token.cancelled() => {
            let empty = RequestPermissionsResponse {
                permissions: Default::default(),
                scope: PermissionGrantScope::Turn,
            };
            parent_session
                .notify_request_permissions_response(call_id, empty.clone())
                .await;
            empty
        }
        response = fut => response.unwrap_or_else(|| RequestPermissionsResponse {
            permissions: Default::default(),
            scope: PermissionGrantScope::Turn,
        }),
    }
}

/// Await an approval decision, aborting on cancellation.
async fn await_approval_with_cancel<F>(
    fut: F,
    parent_session: &Session,
    approval_id: &str,
    cancel_token: &CancellationToken,
    review_cancel_token: Option<&CancellationToken>,
) -> codex_protocol::protocol::ReviewDecision
where
    F: core::future::Future<Output = codex_protocol::protocol::ReviewDecision>,
{
    tokio::select! {
        biased;
        _ = cancel_token.cancelled() => {
            if let Some(review_cancel_token) = review_cancel_token {
                review_cancel_token.cancel();
            }
            parent_session
                .notify_approval(approval_id, codex_protocol::protocol::ReviewDecision::Abort)
                .await;
            codex_protocol::protocol::ReviewDecision::Abort
        }
        decision = fut => {
            decision
        }
    }
}

#[cfg(test)]
#[path = "codex_delegate_tests.rs"]
mod tests;
