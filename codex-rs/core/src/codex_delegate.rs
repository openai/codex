use std::sync::Arc;

use async_channel::Receiver;
use async_channel::Sender;
use codex_async_utils::OrCancelExt;
use codex_extension_api::LoadedUserInstructions;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::Submission;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::user_input::UserInput;
use serde_json::Value;
use std::time::Duration;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::config::Constrained;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::ForkPersistence;
use crate::session::GitEnrichmentPolicy;
use crate::session::SUBMISSION_CHANNEL_CAPACITY;
use crate::session::SessionIo;
use crate::session::SessionSpawnArgs;
use crate::session::emit_subagent_session_started;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_history::InitialHistory;
use codex_login::AuthManager;
use codex_models_manager::manager::SharedModelsManager;
use codex_protocol::error::CodexErr;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::turn_input::TurnInputMode;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnInputSubmission;
use codex_protocol::turn_input::TurnStartOptions;

#[cfg(test)]
use crate::session::completed_session_loop_termination;

/// Start an interactive sub-Codex thread and return its runtime and IO channels.
///
/// Delegates never request approvals, and the returned IO yields their public events.
/// Its submission channel accepts additional `Op`s for the sub-agent.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_codex_thread_interactive(
    mut config: Config,
    auth_manager: Arc<AuthManager>,
    models_manager: SharedModelsManager,
    parent_session: Arc<Session>,
    parent_ctx: Arc<TurnContext>,
    parent_environments: TurnEnvironmentSnapshot,
    cancel_token: CancellationToken,
    subagent_source: SubAgentSource,
    initial_history: Option<InitialHistory>,
    git_enrichment_policy: GitEnrichmentPolicy,
    windows_sandbox_proxy_settings_mode: codex_sandboxing::WindowsSandboxProxySettingsMode,
) -> Result<(Arc<Session>, SessionIo), CodexErr> {
    if config.permissions.approval_policy.value() != AskForApproval::Never {
        return Err(CodexErr::InvalidRequest(
            "Codex delegates require approval policy `never`".to_string(),
        ));
    }
    config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
    config.model_provider.supports_websockets &= parent_session
        .services
        .model_client
        .responses_websocket_enabled();

    let (tx_sub, rx_sub) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);
    let (tx_ops, rx_ops) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);
    let conversation_history = initial_history.unwrap_or(InitialHistory::New);
    let forked_from_thread_id = conversation_history.forked_from_id();
    let user_instructions = LoadedUserInstructions {
        instructions: parent_session.user_instructions().await,
        warnings: Vec::new(),
    };
    let session_source = SessionSource::SubAgent(subagent_source.clone());
    let is_guardian_reviewer = crate::guardian::is_guardian_reviewer_source(&session_source);
    let extensions = if is_guardian_reviewer {
        codex_extension_api::empty_extension_registry()
    } else {
        Arc::clone(&parent_session.services.extensions)
    };
    let (session, io) = Box::pin(Session::spawn(SessionSpawnArgs {
        config,
        allow_provider_model_fallback: false,
        user_instructions,
        installation_id: parent_session.installation_id.clone(),
        auth_manager,
        models_manager,
        environment_manager: parent_session
            .services
            .turn_environments
            .environment_manager(),
        skills_service: Arc::clone(&parent_session.services.skills_service),
        plugins_manager: Arc::clone(&parent_session.services.plugins_manager),
        mcp_manager: Arc::clone(&parent_session.services.mcp_manager),
        code_mode_session_provider: parent_session.services.code_mode_service.session_provider(),
        extensions,
        conversation_history,
        requested_history_mode: None,
        fork_persistence: ForkPersistence::Copied,
        session_source,
        forked_from_thread_id,
        parent_thread_id: Some(parent_session.thread_id),
        thread_source: Some(if is_guardian_reviewer {
            ThreadSource::GuardianReview
        } else {
            ThreadSource::Subagent
        }),
        originator: parent_ctx.originator.clone(),
        agent_control: parent_session.services.agent_control.clone(),
        dynamic_tools: Vec::new(),
        metrics_service_name: None,
        user_shell_override: None,
        inherited_environments: Some(parent_environments.clone()),
        inherited_exec_policy: Some(Arc::clone(&parent_session.services.exec_policy)),
        parent_rollout_thread_trace: codex_rollout_trace::ThreadTraceContext::disabled(),
        parent_trace: None,
        environment_selections: parent_environments.to_selections(),
        thread_extension_init: codex_extension_api::ExtensionDataInit::default(),
        client_mcp_extensions: parent_session.services.client_mcp_extensions.clone(),
        reserved_thread_id: None,
        analytics_events_client: Some(parent_session.services.analytics_events_client.clone()),
        thread_store: Arc::clone(&parent_session.services.thread_store),
        attestation_provider: parent_session.services.attestation_provider.clone(),
        external_time_provider: Some(Arc::clone(&parent_session.services.time_provider)),
        inherited_multi_agent_version: Some(MultiAgentVersion::Disabled),
        git_enrichment_policy,
        windows_sandbox_proxy_settings_mode,
    }))
    .or_cancel(&cancel_token)
    .await??;
    let thread_config = session.thread_config_snapshot().await;
    let client_metadata = parent_session.app_server_client_metadata().await;
    emit_subagent_session_started(
        &parent_session.services.analytics_events_client,
        client_metadata,
        session.session_id(),
        session.thread_id(),
        Some(parent_session.thread_id),
        thread_config,
        subagent_source,
    );
    // Use a child token so parent cancel cascades but we can scope it to this task
    let cancel_token_events = cancel_token.child_token();
    let cancel_token_ops = cancel_token.child_token();

    // Forward public events from the sub-agent to the consumer.
    let io = Arc::new(io);
    let caller_io = SessionIo {
        tx_sub: tx_ops,
        rx_event: rx_sub,
        agent_status: io.agent_status.clone(),
        session_loop_termination: io.session_loop_termination.clone(),
    };
    let io_for_events = Arc::clone(&io);
    tokio::spawn(async move {
        forward_events(io_for_events, tx_sub, cancel_token_events).await;
    });

    // Forward ops from the caller to the sub-agent.
    tokio::spawn(async move {
        forward_ops(io, rx_ops, cancel_token_ops).await;
    });

    Ok((session, caller_io))
}

/// Convenience wrapper for one-time use with an initial prompt.
///
/// Internally calls the interactive variant, then immediately submits the provided input.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_codex_thread_one_shot(
    config: Config,
    auth_manager: Arc<AuthManager>,
    models_manager: SharedModelsManager,
    input: Vec<UserInput>,
    parent_session: Arc<Session>,
    parent_ctx: Arc<TurnContext>,
    cancel_token: CancellationToken,
    subagent_source: SubAgentSource,
    final_output_json_schema: Option<Value>,
    initial_history: Option<InitialHistory>,
) -> Result<(Arc<Session>, SessionIo), CodexErr> {
    // Use a child token so we can stop the delegate after completion without
    // requiring the caller to cancel the parent token.
    let child_cancel = cancel_token.child_token();
    let parent_turn_id = parent_ctx.sub_id.clone();
    let parent_environments = parent_ctx.environments.clone();
    let root_turn_id = parent_ctx.turn_metadata_state.root_turn_id();
    let (session, io) = Box::pin(run_codex_thread_interactive(
        config,
        auth_manager,
        models_manager,
        parent_session,
        parent_ctx,
        parent_environments,
        child_cancel.clone(),
        subagent_source,
        initial_history,
        GitEnrichmentPolicy::Fresh,
        codex_sandboxing::WindowsSandboxProxySettingsMode::Reconcile,
    ))
    .await?;

    // Send the initial input to kick off the one-shot turn.
    let submission = io
        .submit_turn_input(
            TurnInputRequest::user_input(input).on_start(TurnStartOptions {
                final_output_json_schema,
                parent_turn_id: Some(parent_turn_id),
                root_turn_id,
            }),
            TurnInputMode::StartIfIdle,
        )
        .await?;
    match submission {
        TurnInputSubmission::Started { .. } => {}
        submission => {
            return Err(CodexErr::InvalidRequest(format!(
                "delegate turn input was not started: {submission:?}"
            )));
        }
    }

    // Bridge events so we can observe completion and shut down automatically.
    let (tx_bridge, rx_bridge) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);
    let ops_tx = io.tx_sub.clone();
    let agent_status = io.agent_status.clone();
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
                        parent_turn_id: None,
                        root_turn_id: None,
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

    Ok((
        session,
        SessionIo {
            rx_event: rx_bridge,
            tx_sub: tx_closed,
            agent_status,
            session_loop_termination,
        },
    ))
}

async fn forward_events(
    io: Arc<SessionIo>,
    tx_sub: Sender<Event>,
    cancel_token: CancellationToken,
) {
    let cancelled = cancel_token.cancelled();
    tokio::pin!(cancelled);

    loop {
        tokio::select! {
            _ = &mut cancelled => {
                shutdown_delegate(&io).await;
                break;
            }
            event = io.next_event() => {
                let event = match event {
                    Ok(event) => event,
                    Err(_) => break,
                };
                match event {
                    Event {
                        id: _,
                        msg:
                            EventMsg::TokenCount(_)
                            | EventMsg::SessionConfigured(_)
                            | EventMsg::McpStartupUpdate(_)
                            | EventMsg::McpStartupComplete(_),
                    } => {}
                    other => {
                        if !forward_event_or_shutdown(&io, &tx_sub, &cancel_token, other).await
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
async fn shutdown_delegate(io: &SessionIo) {
    let _ = io.submit(Op::Interrupt).await;
    let _ = io.submit(Op::Shutdown {}).await;

    let _ = timeout(Duration::from_millis(500), async {
        while let Ok(event) = io.next_event().await {
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
    io: &SessionIo,
    tx_sub: &Sender<Event>,
    cancel_token: &CancellationToken,
    event: Event,
) -> bool {
    match tx_sub.send(event).or_cancel(cancel_token).await {
        Ok(Ok(())) => true,
        _ => {
            shutdown_delegate(io).await;
            false
        }
    }
}

/// Forward ops from a caller to a sub-agent, respecting cancellation.
async fn forward_ops(
    io: Arc<SessionIo>,
    rx_ops: Receiver<Submission>,
    cancel_token_ops: CancellationToken,
) {
    loop {
        let submission = match rx_ops.recv().or_cancel(&cancel_token_ops).await {
            Ok(Ok(submission)) => submission,
            Ok(Err(_)) | Err(_) => break,
        };
        let _ = io.submit_with_id(submission).await;
    }
}

#[cfg(test)]
#[path = "codex_delegate_tests.rs"]
mod tests;
