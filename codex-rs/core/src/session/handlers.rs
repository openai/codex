use crate::realtime_conversation::handle_audio as handle_realtime_conversation_audio;
use crate::realtime_conversation::handle_close as handle_realtime_conversation_close;
use crate::realtime_conversation::handle_start as handle_realtime_conversation_start;
use crate::realtime_conversation::handle_text as handle_realtime_conversation_text;
use async_channel::Receiver;
use codex_otel::set_parent_from_w3c_trace_context;
use codex_protocol::protocol::Submission;
use tracing::Instrument;
use tracing::debug_span;
use tracing::info_span;

use crate::session::SteerInputError;
use crate::session::session::Session;
use crate::session::session::SessionSettingsUpdate;

use crate::config::Config;
use crate::config_loader::CloudRequirementsLoader;
use crate::config_loader::LoaderOverrides;
use crate::config_loader::load_config_layers_state;
use crate::realtime_context::REALTIME_TURN_TOKEN_BUDGET;
use crate::realtime_context::truncate_realtime_text_to_token_budget;
use crate::realtime_conversation::REALTIME_USER_TEXT_PREFIX;
use crate::realtime_conversation::prefix_realtime_v2_text;
use crate::session::spawn_review_thread;
use codex_exec_server::LOCAL_FS;
use codex_features::Feature;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::review_prompts::resolve_review_request;
use crate::rollout::RolloutRecorder;
use crate::rollout::read_session_meta_line;
use crate::tasks::CompactTask;
use crate::tasks::UndoTask;
use crate::tasks::UserShellCommandMode;
use crate::tasks::UserShellCommandTask;
use crate::tasks::execute_user_shell_command;
use codex_mcp::collect_mcp_snapshot_from_manager;
use codex_mcp::compute_auth_statuses;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::ListSkillsResponseEvent;
use codex_protocol::protocol::McpServerRefreshConfig;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RealtimeConversationListVoicesResponseEvent;
use codex_protocol::protocol::RealtimeVoicesList;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SkillErrorInfo;
use codex_protocol::protocol::SkillsListEntry;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_protocol::protocol::ThreadNameUpdatedEvent;
use codex_protocol::protocol::ThreadRolledBackEvent;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::WarningEvent;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::request_user_input::RequestUserInputResponse;

use crate::context_manager::is_user_turn_boundary;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::items::UserMessageItem;
use codex_protocol::mcp::RequestId as ProtocolRequestId;
use codex_protocol::user_input::UserInput;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::debug;
use tracing::info;
use tracing::warn;

pub async fn interrupt(sess: &Arc<Session>) {
    sess.interrupt_task().await;
}

pub async fn clean_background_terminals(sess: &Arc<Session>) {
    sess.close_unified_exec_processes().await;
}

pub async fn realtime_conversation_list_voices(sess: &Session, sub_id: String) {
    sess.send_event_raw(Event {
        id: sub_id,
        msg: EventMsg::RealtimeConversationListVoicesResponse(
            RealtimeConversationListVoicesResponseEvent {
                voices: RealtimeVoicesList::builtin(),
            },
        ),
    })
    .await;
}

pub async fn override_turn_context(sess: &Session, sub_id: String, updates: SessionSettingsUpdate) {
    let environments_update = updates.environments.clone();
    if let Some(environments) = updates.environments.as_ref()
        && let Err(err) = Session::validate_turn_environment_selections(
            &sess.services.environment_manager,
            environments,
        )
    {
        sess.send_event_raw(Event {
            id: sub_id,
            msg: EventMsg::Error(ErrorEvent {
                message: err.to_string(),
                codex_error_info: Some(CodexErrorInfo::BadRequest),
            }),
        })
        .await;
        return;
    }

    if let Err(err) = sess.update_settings(updates).await {
        sess.send_event_raw(Event {
            id: sub_id,
            msg: EventMsg::Error(ErrorEvent {
                message: err.to_string(),
                codex_error_info: Some(CodexErrorInfo::BadRequest),
            }),
        })
        .await;
    } else if let Some(environments) = environments_update {
        sess.persist_sticky_environment_update(environments).await;
    }
}

pub async fn user_input_or_turn(sess: &Arc<Session>, sub_id: String, op: Op) {
    user_input_or_turn_inner(
        sess,
        sub_id,
        op,
        /*mirror_user_text_to_realtime*/ Some(()),
    )
    .await;
}

pub(super) async fn user_input_or_turn_inner(
    sess: &Arc<Session>,
    sub_id: String,
    op: Op,
    mirror_user_text_to_realtime: Option<()>,
) {
    let (items, updates, responsesapi_client_metadata, environments) = match op {
        Op::UserTurn {
            cwd,
            approval_policy,
            approvals_reviewer,
            sandbox_policy,
            model,
            effort,
            summary,
            service_tier,
            final_output_json_schema,
            items,
            collaboration_mode,
            personality,
            environments,
        } => {
            let collaboration_mode = collaboration_mode.or_else(|| {
                Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: model.clone(),
                        reasoning_effort: effort,
                        developer_instructions: None,
                    },
                })
            });
            (
                items,
                SessionSettingsUpdate {
                    cwd: Some(cwd),
                    approval_policy: Some(approval_policy),
                    approvals_reviewer,
                    sandbox_policy: Some(sandbox_policy),
                    windows_sandbox_level: None,
                    collaboration_mode,
                    reasoning_summary: summary,
                    service_tier,
                    final_output_json_schema: Some(final_output_json_schema),
                    personality,
                    app_server_client_name: None,
                    app_server_client_version: None,
                    environments: None,
                },
                None,
                environments,
            )
        }
        Op::UserInput {
            items,
            environments,
            final_output_json_schema,
            responsesapi_client_metadata,
        } => (
            items,
            SessionSettingsUpdate {
                final_output_json_schema: Some(final_output_json_schema),
                ..Default::default()
            },
            responsesapi_client_metadata,
            environments,
        ),
        _ => unreachable!(),
    };

    let Ok(current_context) = sess
        .new_turn_with_sub_id(sub_id.clone(), updates, environments)
        .await
    else {
        // new_turn_with_sub_id already emits the error event.
        return;
    };
    sess.maybe_emit_unknown_model_warning_for_turn(current_context.as_ref())
        .await;
    let accepted_items = match sess
        .steer_input(
            items.clone(),
            /*expected_turn_id*/ None,
            responsesapi_client_metadata.clone(),
        )
        .await
    {
        Ok(_) => {
            current_context.session_telemetry.user_prompt(&items);
            Some(items)
        }
        Err(SteerInputError::NoActiveTurn(items)) => {
            if let Some(responsesapi_client_metadata) = responsesapi_client_metadata {
                current_context
                    .turn_metadata_state
                    .set_responsesapi_client_metadata(responsesapi_client_metadata);
            }
            current_context.session_telemetry.user_prompt(&items);
            sess.refresh_mcp_servers_if_requested(&current_context)
                .await;
            let accepted_items = items.clone();
            sess.spawn_task(
                Arc::clone(&current_context),
                items,
                crate::tasks::RegularTask::new(),
            )
            .await;
            Some(accepted_items)
        }
        Err(err) => {
            sess.send_event_raw(Event {
                id: sub_id,
                msg: EventMsg::Error(err.to_error_event()),
            })
            .await;
            None
        }
    };
    if let (Some(items), Some(())) = (accepted_items, mirror_user_text_to_realtime) {
        self::mirror_user_text_to_realtime(sess, &items).await;
    }
}

async fn mirror_user_text_to_realtime(sess: &Arc<Session>, items: &[UserInput]) {
    let text = UserMessageItem::new(items).message();
    if text.is_empty() {
        return;
    }
    let text = if sess.conversation.is_running_v2().await {
        prefix_realtime_v2_text(text, REALTIME_USER_TEXT_PREFIX)
    } else {
        text
    };
    let text = truncate_realtime_text_to_token_budget(&text, REALTIME_TURN_TOKEN_BUDGET);
    if text.is_empty() {
        return;
    }
    if sess.conversation.running_state().await.is_none() {
        return;
    }
    if let Err(err) = sess.conversation.text_in(text).await {
        debug!("failed to mirror user text to realtime conversation: {err}");
    }
}

/// Records an inter-agent assistant envelope, then lets the shared pending-work scheduler
/// decide whether an idle session should start a regular turn.
pub async fn inter_agent_communication(
    sess: &Arc<Session>,
    sub_id: String,
    communication: InterAgentCommunication,
) {
    let trigger_turn = communication.trigger_turn;
    sess.enqueue_mailbox_communication(communication);
    if trigger_turn {
        sess.maybe_start_turn_for_pending_work_with_sub_id(sub_id)
            .await;
    }
}

pub async fn run_user_shell_command(sess: &Arc<Session>, sub_id: String, command: String) {
    if let Some((turn_context, cancellation_token)) =
        sess.active_turn_context_and_cancellation_token().await
    {
        let session = Arc::clone(sess);
        tokio::spawn(async move {
            execute_user_shell_command(
                session,
                turn_context,
                command,
                cancellation_token,
                UserShellCommandMode::ActiveTurnAuxiliary,
            )
            .await;
        });
        return;
    }

    let turn_context = sess.new_default_turn_with_sub_id(sub_id).await;
    sess.spawn_task(
        Arc::clone(&turn_context),
        Vec::new(),
        UserShellCommandTask::new(command),
    )
    .await;
}

pub async fn resolve_elicitation(
    sess: &Arc<Session>,
    server_name: String,
    request_id: ProtocolRequestId,
    decision: codex_protocol::approvals::ElicitationAction,
    content: Option<Value>,
    meta: Option<Value>,
) {
    let action = match decision {
        codex_protocol::approvals::ElicitationAction::Accept => ElicitationAction::Accept,
        codex_protocol::approvals::ElicitationAction::Decline => ElicitationAction::Decline,
        codex_protocol::approvals::ElicitationAction::Cancel => ElicitationAction::Cancel,
    };
    let content = match action {
        // Preserve the legacy fallback for clients that only send an action.
        ElicitationAction::Accept => Some(content.unwrap_or_else(|| serde_json::json!({}))),
        ElicitationAction::Decline | ElicitationAction::Cancel => None,
    };
    let response = ElicitationResponse {
        action,
        content,
        meta,
    };
    let request_id = match request_id {
        ProtocolRequestId::String(value) => {
            rmcp::model::NumberOrString::String(std::sync::Arc::from(value))
        }
        ProtocolRequestId::Integer(value) => rmcp::model::NumberOrString::Number(value),
    };
    if let Err(err) = sess
        .resolve_elicitation(server_name, request_id, response)
        .await
    {
        warn!(
            error = %err,
            "failed to resolve elicitation request in session"
        );
    }
}

/// Propagate a user's exec approval decision to the session.
/// Also optionally applies an execpolicy amendment.
pub async fn exec_approval(
    sess: &Arc<Session>,
    approval_id: String,
    turn_id: Option<String>,
    decision: ReviewDecision,
) {
    let event_turn_id = turn_id.unwrap_or_else(|| approval_id.clone());
    if let ReviewDecision::ApprovedExecpolicyAmendment {
        proposed_execpolicy_amendment,
    } = &decision
    {
        match sess
            .persist_execpolicy_amendment(proposed_execpolicy_amendment)
            .await
        {
            Ok(()) => {
                sess.record_execpolicy_amendment_message(
                    &event_turn_id,
                    proposed_execpolicy_amendment,
                )
                .await;
            }
            Err(err) => {
                let message = format!("Failed to apply execpolicy amendment: {err}");
                tracing::warn!("{message}");
                let warning = EventMsg::Warning(WarningEvent { message });
                sess.send_event_raw(Event {
                    id: event_turn_id.clone(),
                    msg: warning,
                })
                .await;
            }
        }
    }
    match decision {
        ReviewDecision::Abort => {
            sess.interrupt_task().await;
        }
        other => sess.notify_approval(&approval_id, other).await,
    }
}

pub async fn patch_approval(sess: &Arc<Session>, id: String, decision: ReviewDecision) {
    match decision {
        ReviewDecision::Abort => {
            sess.interrupt_task().await;
        }
        other => sess.notify_approval(&id, other).await,
    }
}

pub async fn request_user_input_response(
    sess: &Arc<Session>,
    id: String,
    response: RequestUserInputResponse,
) {
    sess.notify_user_input_response(&id, response).await;
}

pub async fn request_permissions_response(
    sess: &Arc<Session>,
    id: String,
    response: RequestPermissionsResponse,
) {
    sess.notify_request_permissions_response(&id, response)
        .await;
}

pub async fn dynamic_tool_response(sess: &Arc<Session>, id: String, response: DynamicToolResponse) {
    sess.notify_dynamic_tool_response(&id, response).await;
}

pub async fn add_to_history(sess: &Arc<Session>, config: &Arc<Config>, text: String) {
    let id = sess.conversation_id;
    let config = Arc::clone(config);
    tokio::spawn(async move {
        if let Err(e) = crate::message_history::append_entry(&text, &id, &config).await {
            warn!("failed to append to message history: {e}");
        }
    });
}

pub async fn get_history_entry_request(
    sess: &Arc<Session>,
    config: &Arc<Config>,
    sub_id: String,
    offset: usize,
    log_id: u64,
) {
    let config = Arc::clone(config);
    let sess_clone = Arc::clone(sess);

    tokio::spawn(async move {
        // Run lookup in blocking thread because it does file IO + locking.
        let entry_opt = tokio::task::spawn_blocking(move || {
            crate::message_history::lookup(log_id, offset, &config)
        })
        .await
        .unwrap_or(None);

        let event = Event {
            id: sub_id,
            msg: EventMsg::GetHistoryEntryResponse(
                codex_protocol::protocol::GetHistoryEntryResponseEvent {
                    offset,
                    log_id,
                    entry: entry_opt.map(|e| codex_protocol::message_history::HistoryEntry {
                        conversation_id: e.session_id,
                        ts: e.ts,
                        text: e.text,
                    }),
                },
            ),
        };

        sess_clone.send_event_raw(event).await;
    });
}

pub async fn refresh_mcp_servers(sess: &Arc<Session>, refresh_config: McpServerRefreshConfig) {
    let mut guard = sess.pending_mcp_server_refresh_config.lock().await;
    *guard = Some(refresh_config);
}

pub async fn reload_user_config(sess: &Arc<Session>) {
    sess.reload_user_config_layer().await;
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "MCP tool listing reads through the session-owned manager guard"
)]
pub async fn list_mcp_tools(sess: &Session, config: &Arc<Config>, sub_id: String) {
    let mcp_connection_manager = sess.services.mcp_connection_manager.read().await;
    let auth = sess.services.auth_manager.auth().await;
    let background_authorization_header_value = if let Some(auth) = auth.as_ref() {
        sess.services
            .auth_manager
            .chatgpt_authorization_header_for_auth(auth)
            .await
    } else {
        None
    };
    let mcp_servers = sess
        .services
        .mcp_manager
        .effective_servers_with_authorization_header(
            config,
            auth.as_ref(),
            background_authorization_header_value.as_deref(),
        )
        .await;
    let snapshot = collect_mcp_snapshot_from_manager(
        &mcp_connection_manager,
        compute_auth_statuses(mcp_servers.iter(), config.mcp_oauth_credentials_store_mode).await,
    )
    .await;
    let event = Event {
        id: sub_id,
        msg: EventMsg::McpListToolsResponse(snapshot),
    };
    sess.send_event_raw(event).await;
}

pub async fn list_skills(sess: &Session, sub_id: String, cwds: Vec<PathBuf>, force_reload: bool) {
    let default_cwd = {
        let state = sess.state.lock().await;
        state.session_configuration.cwd.to_path_buf()
    };
    let cwds = if cwds.is_empty() {
        vec![default_cwd]
    } else {
        cwds
    };

    let skills_manager = &sess.services.skills_manager;
    let plugins_manager = &sess.services.plugins_manager;
    let fs = sess
        .services
        .environment_manager
        .default_environment()
        .map(|environment| environment.get_filesystem());
    let config = sess.get_config().await;
    let codex_home = sess.codex_home().await;
    let mut skills = Vec::new();
    let empty_cli_overrides: &[(String, toml::Value)] = &[];
    for cwd in cwds {
        let cwd_abs = match AbsolutePathBuf::relative_to_current_dir(cwd.as_path()) {
            Ok(path) => path,
            Err(err) => {
                let error_path = cwd.clone();
                skills.push(SkillsListEntry {
                    cwd,
                    skills: Vec::new(),
                    errors: vec![SkillErrorInfo {
                        path: error_path,
                        message: err.to_string(),
                    }],
                });
                continue;
            }
        };
        let config_layer_stack = match load_config_layers_state(
            LOCAL_FS.as_ref(),
            &codex_home,
            Some(cwd_abs.clone()),
            empty_cli_overrides,
            LoaderOverrides::default(),
            CloudRequirementsLoader::default(),
            &codex_config::NoopThreadConfigLoader,
            /*host_name*/ None,
        )
        .await
        {
            Ok(config_layer_stack) => config_layer_stack,
            Err(err) => {
                let error_path = cwd.clone();
                skills.push(SkillsListEntry {
                    cwd,
                    skills: Vec::new(),
                    errors: vec![SkillErrorInfo {
                        path: error_path,
                        message: err.to_string(),
                    }],
                });
                continue;
            }
        };
        let effective_skill_roots = plugins_manager
            .effective_skill_roots_for_layer_stack(
                &config_layer_stack,
                config.features.enabled(Feature::Plugins),
            )
            .await;
        let skills_input = crate::SkillsLoadInput::new(
            cwd_abs.clone(),
            effective_skill_roots,
            config_layer_stack,
            config.bundled_skills_enabled(),
        );
        let outcome = skills_manager
            .skills_for_cwd(&skills_input, force_reload, fs.clone())
            .await;
        let errors = super::errors_to_info(&outcome.errors);
        let skills_metadata = super::skills_to_info(&outcome.skills, &outcome.disabled_paths);
        skills.push(SkillsListEntry {
            cwd,
            skills: skills_metadata,
            errors,
        });
    }

    let event = Event {
        id: sub_id,
        msg: EventMsg::ListSkillsResponse(ListSkillsResponseEvent { skills }),
    };
    sess.send_event_raw(event).await;
}

pub async fn undo(sess: &Arc<Session>, sub_id: String) {
    let turn_context = sess.new_default_turn_with_sub_id(sub_id).await;
    sess.spawn_task(turn_context, Vec::new(), UndoTask::new())
        .await;
}

pub async fn compact(sess: &Arc<Session>, sub_id: String) {
    let turn_context = sess.new_default_turn_with_sub_id(sub_id).await;

    sess.spawn_task(
        Arc::clone(&turn_context),
        vec![UserInput::Text {
            text: turn_context.compact_prompt().to_string(),
            // Compaction prompt is synthesized; no UI element ranges to preserve.
            text_elements: Vec::new(),
        }],
        CompactTask,
    )
    .await;
}

pub async fn drop_memories(sess: &Arc<Session>, config: &Arc<Config>, sub_id: String) {
    let mut errors = Vec::new();

    if let Some(state_db) = sess.services.state_db.as_deref() {
        if let Err(err) = state_db.clear_memory_data().await {
            errors.push(format!("failed clearing memory rows from state db: {err}"));
        }
    } else {
        errors.push("state db unavailable; memory rows were not cleared".to_string());
    }

    if let Err(err) = crate::memories::clear_memory_roots_contents(&config.codex_home).await {
        errors.push(format!(
            "failed clearing memory directories under {}: {err}",
            config.codex_home.display()
        ));
    }

    if errors.is_empty() {
        let memory_root = crate::memories::memory_root(&config.codex_home);
        sess.send_event_raw(Event {
            id: sub_id,
            msg: EventMsg::Warning(WarningEvent {
                message: format!(
                    "Dropped memories at {} and cleared memory rows from state db.",
                    memory_root.display()
                ),
            }),
        })
        .await;
        return;
    }

    sess.send_event_raw(Event {
        id: sub_id,
        msg: EventMsg::Error(ErrorEvent {
            message: format!("Memory drop completed with errors: {}", errors.join("; ")),
            codex_error_info: Some(CodexErrorInfo::Other),
        }),
    })
    .await;
}

pub async fn update_memories(sess: &Arc<Session>, config: &Arc<Config>, sub_id: String) {
    let session_source = {
        let state = sess.state.lock().await;
        state.session_configuration.session_source.clone()
    };

    crate::memories::start_memories_startup_task(sess, Arc::clone(config), &session_source);

    sess.send_event_raw(Event {
        id: sub_id.clone(),
        msg: EventMsg::Warning(WarningEvent {
            message: "Memory update triggered.".to_string(),
        }),
    })
    .await;
}

pub async fn thread_rollback(sess: &Arc<Session>, sub_id: String, num_turns: u32) {
    if num_turns == 0 {
        sess.send_event_raw(Event {
            id: sub_id,
            msg: EventMsg::Error(ErrorEvent {
                message: "num_turns must be >= 1".to_string(),
                codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
            }),
        })
        .await;
        return;
    }

    let has_active_turn = { sess.active_turn.lock().await.is_some() };
    if has_active_turn {
        sess.send_event_raw(Event {
            id: sub_id,
            msg: EventMsg::Error(ErrorEvent {
                message: "Cannot rollback while a turn is in progress.".to_string(),
                codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
            }),
        })
        .await;
        return;
    }

    let turn_context = sess.new_default_turn_with_sub_id(sub_id).await;
    let rollout_path = {
        let recorder = {
            let guard = sess.services.rollout.lock().await;
            guard.clone()
        };
        let Some(recorder) = recorder else {
            sess.send_event_raw(Event {
                id: turn_context.sub_id.clone(),
                msg: EventMsg::Error(ErrorEvent {
                    message: "thread rollback requires a persisted rollout path".to_string(),
                    codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
                }),
            })
            .await;
            return;
        };
        recorder.rollout_path().to_path_buf()
    };
    if let Some(recorder) = {
        let guard = sess.services.rollout.lock().await;
        guard.clone()
    } && let Err(err) = recorder.flush().await
    {
        sess.send_event_raw(Event {
            id: turn_context.sub_id.clone(),
            msg: EventMsg::Error(ErrorEvent {
                message: format!(
                    "failed to flush rollout `{}` for rollback replay: {err}",
                    rollout_path.display()
                ),
                codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
            }),
        })
        .await;
        return;
    }

    let initial_history = match RolloutRecorder::get_rollout_history(rollout_path.as_path()).await {
        Ok(history) => history,
        Err(err) => {
            sess.send_event_raw(Event {
                id: turn_context.sub_id.clone(),
                msg: EventMsg::Error(ErrorEvent {
                    message: format!(
                        "failed to load rollout `{}` for rollback replay: {err}",
                        rollout_path.display()
                    ),
                    codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
                }),
            })
            .await;
            return;
        }
    };

    let rollback_event = ThreadRolledBackEvent { num_turns };
    let rollback_msg = EventMsg::ThreadRolledBack(rollback_event.clone());
    let replay_items = initial_history
        .get_rollout_items()
        .into_iter()
        .chain(std::iter::once(RolloutItem::EventMsg(rollback_msg.clone())))
        .collect::<Vec<_>>();
    sess.apply_rollout_reconstruction(turn_context.as_ref(), replay_items.as_slice())
        .await;
    sess.recompute_token_usage(turn_context.as_ref()).await;

    sess.persist_rollout_items(&[RolloutItem::EventMsg(rollback_msg.clone())])
        .await;
    if let Err(err) = sess.flush_rollout().await {
        sess.send_event(
            turn_context.as_ref(),
            EventMsg::Warning(WarningEvent {
                message: format!(
                    "Rolled the thread back, but failed to save the rollback marker. Codex will continue retrying. Error: {err}"
                ),
            }),
        )
        .await;
    }

    sess.deliver_event_raw(Event {
        id: turn_context.sub_id.clone(),
        msg: rollback_msg,
    })
    .await;
}

async fn persist_thread_name_update(
    sess: &Arc<Session>,
    event: ThreadNameUpdatedEvent,
) -> anyhow::Result<EventMsg> {
    let msg = EventMsg::ThreadNameUpdated(event);
    let item = RolloutItem::EventMsg(msg.clone());
    let recorder = {
        let guard = sess.services.rollout.lock().await;
        guard.clone()
    }
    .ok_or_else(|| anyhow::anyhow!("Session persistence is disabled; cannot rename thread."))?;
    recorder.persist().await?;
    recorder.record_items(std::slice::from_ref(&item)).await?;
    recorder.flush().await?;
    Ok(msg)
}

pub(super) async fn persist_thread_memory_mode_update(
    sess: &Arc<Session>,
    mode: ThreadMemoryMode,
) -> anyhow::Result<()> {
    let recorder = {
        let guard = sess.services.rollout.lock().await;
        guard.clone()
    }
    .ok_or_else(|| {
        anyhow::anyhow!("Session persistence is disabled; cannot update thread memory mode.")
    })?;
    recorder.persist().await?;
    recorder.flush().await?;

    let rollout_path = recorder.rollout_path().to_path_buf();
    let mut session_meta = read_session_meta_line(rollout_path.as_path()).await?;
    if session_meta.meta.id != sess.conversation_id {
        anyhow::bail!(
            "rollout session metadata id mismatch: expected {}, found {}",
            sess.conversation_id,
            session_meta.meta.id
        );
    }
    session_meta.meta.memory_mode = Some(
        match mode {
            ThreadMemoryMode::Enabled => "enabled",
            ThreadMemoryMode::Disabled => "disabled",
        }
        .to_string(),
    );

    let item = RolloutItem::SessionMeta(session_meta);
    recorder.record_items(std::slice::from_ref(&item)).await?;
    recorder.flush().await?;
    Ok(())
}

/// Persists the thread name in the rollout and state database, updates in-memory state, and
/// emits a `ThreadNameUpdated` event on success.
pub async fn set_thread_name(sess: &Arc<Session>, sub_id: String, name: String) {
    let Some(name) = crate::util::normalize_thread_name(&name) else {
        let event = Event {
            id: sub_id,
            msg: EventMsg::Error(ErrorEvent {
                message: "Thread name cannot be empty.".to_string(),
                codex_error_info: Some(CodexErrorInfo::BadRequest),
            }),
        };
        sess.send_event_raw(event).await;
        return;
    };

    let updated = ThreadNameUpdatedEvent {
        thread_id: sess.conversation_id,
        thread_name: Some(name.clone()),
    };

    let msg = match persist_thread_name_update(sess, updated).await {
        Ok(msg) => msg,
        Err(err) => {
            warn!("Failed to persist thread name update to rollout: {err}");
            let event = Event {
                id: sub_id,
                msg: EventMsg::Error(ErrorEvent {
                    message: err.to_string(),
                    codex_error_info: Some(CodexErrorInfo::Other),
                }),
            };
            sess.send_event_raw(event).await;
            return;
        }
    };

    if let Some(state_db) = sess.services.state_db.as_deref()
        && let Err(err) = state_db
            .update_thread_title(sess.conversation_id, &name)
            .await
    {
        warn!("Failed to update thread title in state db: {err}");
    }

    {
        let mut state = sess.state.lock().await;
        state.session_configuration.thread_name = Some(name.clone());
    }

    let codex_home = sess.codex_home().await;
    if let Err(err) =
        crate::rollout::append_thread_name(&codex_home, sess.conversation_id, &name).await
    {
        warn!("Failed to update legacy thread name index: {err}");
    }

    sess.deliver_event_raw(Event { id: sub_id, msg }).await;
}

/// Persists thread-level memory mode metadata for the active session.
///
/// This does not involve the model and only affects whether the thread is
/// eligible for future memory generation.
pub async fn set_thread_memory_mode(sess: &Arc<Session>, sub_id: String, mode: ThreadMemoryMode) {
    if let Err(err) = persist_thread_memory_mode_update(sess, mode).await {
        warn!("Failed to persist thread memory mode update to rollout: {err}");
        let event = Event {
            id: sub_id,
            msg: EventMsg::Error(ErrorEvent {
                message: err.to_string(),
                codex_error_info: Some(CodexErrorInfo::Other),
            }),
        };
        sess.send_event_raw(event).await;
    }
}

pub async fn shutdown(sess: &Arc<Session>, sub_id: String) -> bool {
    sess.abort_all_tasks(TurnAbortReason::Interrupted).await;
    let _ = sess.conversation.shutdown().await;
    sess.services
        .unified_exec_manager
        .terminate_all_processes()
        .await;
    sess.guardian_review_session.shutdown().await;
    info!("Shutting down Codex instance");
    let history = sess.clone_history().await;
    let turn_count = history
        .raw_items()
        .iter()
        .filter(|item| is_user_turn_boundary(item))
        .count();
    sess.services.session_telemetry.counter(
        "codex.conversation.turn.count",
        i64::try_from(turn_count).unwrap_or(0),
        &[],
    );

    // Gracefully flush and shutdown rollout recorder on session end so tests
    // that inspect the rollout file do not race with the background writer.
    let recorder_opt = {
        let mut guard = sess.services.rollout.lock().await;
        guard.take()
    };
    if let Some(rec) = recorder_opt
        && let Err(e) = rec.shutdown().await
    {
        warn!("failed to shutdown rollout recorder: {e}");
        let event = Event {
            id: sub_id.clone(),
            msg: EventMsg::Error(ErrorEvent {
                message: "Failed to shutdown rollout recorder".to_string(),
                codex_error_info: Some(CodexErrorInfo::Other),
            }),
        };
        sess.send_event_raw(event).await;
    }

    let event = Event {
        id: sub_id,
        msg: EventMsg::ShutdownComplete,
    };
    sess.send_event_raw(event).await;
    true
}

pub async fn review(
    sess: &Arc<Session>,
    config: &Arc<Config>,
    sub_id: String,
    review_request: ReviewRequest,
) {
    let turn_context = sess.new_default_turn_with_sub_id(sub_id.clone()).await;
    sess.maybe_emit_unknown_model_warning_for_turn(turn_context.as_ref())
        .await;
    sess.refresh_mcp_servers_if_requested(&turn_context).await;
    match resolve_review_request(review_request, turn_context.cwd()) {
        Ok(resolved) => {
            spawn_review_thread(
                Arc::clone(sess),
                Arc::clone(config),
                turn_context.clone(),
                sub_id,
                resolved,
            )
            .await;
        }
        Err(err) => {
            let event = Event {
                id: sub_id,
                msg: EventMsg::Error(ErrorEvent {
                    message: err.to_string(),
                    codex_error_info: Some(CodexErrorInfo::Other),
                }),
            };
            sess.send_event(&turn_context, event.msg).await;
        }
    }
}

pub(super) async fn submission_loop(
    sess: Arc<Session>,
    config: Arc<Config>,
    rx_sub: Receiver<Submission>,
) {
    // To break out of this loop, send Op::Shutdown.
    while let Ok(sub) = rx_sub.recv().await {
        debug!(?sub, "Submission");
        let dispatch_span = submission_dispatch_span(&sub);
        let should_exit = async {
            match sub.op.clone() {
                Op::Interrupt => {
                    interrupt(&sess).await;
                    false
                }
                Op::CleanBackgroundTerminals => {
                    clean_background_terminals(&sess).await;
                    false
                }
                Op::RealtimeConversationStart(params) => {
                    if let Err(err) =
                        handle_realtime_conversation_start(&sess, sub.id.clone(), params).await
                    {
                        sess.send_event_raw(Event {
                            id: sub.id.clone(),
                            msg: EventMsg::Error(ErrorEvent {
                                message: err.to_string(),
                                codex_error_info: Some(CodexErrorInfo::Other),
                            }),
                        })
                        .await;
                    }
                    false
                }
                Op::RealtimeConversationAudio(params) => {
                    handle_realtime_conversation_audio(&sess, sub.id.clone(), params).await;
                    false
                }
                Op::RealtimeConversationText(params) => {
                    handle_realtime_conversation_text(&sess, sub.id.clone(), params).await;
                    false
                }
                Op::RealtimeConversationClose => {
                    handle_realtime_conversation_close(&sess, sub.id.clone()).await;
                    false
                }
                Op::RealtimeConversationListVoices => {
                    realtime_conversation_list_voices(&sess, sub.id.clone()).await;
                    false
                }
                Op::OverrideTurnContext {
                    cwd,
                    approval_policy,
                    approvals_reviewer,
                    sandbox_policy,
                    windows_sandbox_level,
                    model,
                    effort,
                    summary,
                    service_tier,
                    collaboration_mode,
                    personality,
                    environments,
                } => {
                    let collaboration_mode = if let Some(collab_mode) = collaboration_mode {
                        collab_mode
                    } else {
                        let state = sess.state.lock().await;
                        state.session_configuration.collaboration_mode.with_updates(
                            model.clone(),
                            effort,
                            /*developer_instructions*/ None,
                        )
                    };
                    override_turn_context(
                        &sess,
                        sub.id.clone(),
                        SessionSettingsUpdate {
                            cwd,
                            approval_policy,
                            approvals_reviewer,
                            sandbox_policy,
                            windows_sandbox_level,
                            collaboration_mode: Some(collaboration_mode),
                            reasoning_summary: summary,
                            service_tier,
                            personality,
                            environments,
                            ..Default::default()
                        },
                    )
                    .await;
                    false
                }
                Op::UserInput { .. } | Op::UserTurn { .. } => {
                    user_input_or_turn(&sess, sub.id.clone(), sub.op).await;
                    false
                }
                Op::InterAgentCommunication { communication } => {
                    inter_agent_communication(&sess, sub.id.clone(), communication).await;
                    false
                }
                Op::ExecApproval {
                    id: approval_id,
                    turn_id,
                    decision,
                } => {
                    exec_approval(&sess, approval_id, turn_id, decision).await;
                    false
                }
                Op::PatchApproval { id, decision } => {
                    patch_approval(&sess, id, decision).await;
                    false
                }
                Op::UserInputAnswer { id, response } => {
                    request_user_input_response(&sess, id, response).await;
                    false
                }
                Op::RequestPermissionsResponse { id, response } => {
                    request_permissions_response(&sess, id, response).await;
                    false
                }
                Op::DynamicToolResponse { id, response } => {
                    dynamic_tool_response(&sess, id, response).await;
                    false
                }
                Op::AddToHistory { text } => {
                    add_to_history(&sess, &config, text).await;
                    false
                }
                Op::GetHistoryEntryRequest { offset, log_id } => {
                    get_history_entry_request(&sess, &config, sub.id.clone(), offset, log_id).await;
                    false
                }
                Op::ListMcpTools => {
                    list_mcp_tools(&sess, &config, sub.id.clone()).await;
                    false
                }
                Op::RefreshMcpServers { config } => {
                    refresh_mcp_servers(&sess, config).await;
                    false
                }
                Op::ReloadUserConfig => {
                    reload_user_config(&sess).await;
                    false
                }
                Op::ListSkills { cwds, force_reload } => {
                    list_skills(&sess, sub.id.clone(), cwds, force_reload).await;
                    false
                }
                Op::Undo => {
                    undo(&sess, sub.id.clone()).await;
                    false
                }
                Op::Compact => {
                    compact(&sess, sub.id.clone()).await;
                    false
                }
                Op::DropMemories => {
                    drop_memories(&sess, &config, sub.id.clone()).await;
                    false
                }
                Op::UpdateMemories => {
                    update_memories(&sess, &config, sub.id.clone()).await;
                    false
                }
                Op::ThreadRollback { num_turns } => {
                    thread_rollback(&sess, sub.id.clone(), num_turns).await;
                    false
                }
                Op::SetThreadName { name } => {
                    set_thread_name(&sess, sub.id.clone(), name).await;
                    false
                }
                Op::SetThreadMemoryMode { mode } => {
                    set_thread_memory_mode(&sess, sub.id.clone(), mode).await;
                    false
                }
                Op::RunUserShellCommand { command } => {
                    run_user_shell_command(&sess, sub.id.clone(), command).await;
                    false
                }
                Op::ResolveElicitation {
                    server_name,
                    request_id,
                    decision,
                    content,
                    meta,
                } => {
                    resolve_elicitation(&sess, server_name, request_id, decision, content, meta)
                        .await;
                    false
                }
                Op::Shutdown => shutdown(&sess, sub.id.clone()).await,
                Op::Review { review_request } => {
                    review(&sess, &config, sub.id.clone(), review_request).await;
                    false
                }
                _ => false, // Ignore unknown ops; enum is non_exhaustive to allow extensions.
            }
        }
        .instrument(dispatch_span)
        .await;
        if should_exit {
            break;
        }
    }
    // Also drain cached guardian state if the submission loop exits because
    // the channel closed without receiving an explicit shutdown op.
    sess.guardian_review_session.shutdown().await;
    debug!("Agent loop exited");
}

pub(super) fn submission_dispatch_span(sub: &Submission) -> tracing::Span {
    let op_name = sub.op.kind();
    let span_name = format!("op.dispatch.{op_name}");
    let dispatch_span = match &sub.op {
        Op::RealtimeConversationAudio(_) => {
            debug_span!(
                "submission_dispatch",
                otel.name = span_name.as_str(),
                submission.id = sub.id.as_str(),
                codex.op = op_name
            )
        }
        _ => info_span!(
            "submission_dispatch",
            otel.name = span_name.as_str(),
            submission.id = sub.id.as_str(),
            codex.op = op_name
        ),
    };
    if let Some(trace) = sub.trace.as_ref()
        && !set_parent_from_w3c_trace_context(&dispatch_span, trace)
    {
        warn!(
            submission.id = sub.id.as_str(),
            "ignoring invalid submission trace carrier"
        );
    }
    dispatch_span
}
