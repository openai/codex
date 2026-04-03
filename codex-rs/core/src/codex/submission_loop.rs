use super::*;

pub(super) async fn submission_loop(
    sess: Arc<Session>,
    config: Arc<Config>,
    rx_sub: Receiver<Submission>,
) {
    while let Ok(sub) = rx_sub.recv().await {
        debug!(?sub, "Submission");
        let dispatch_span = submission_dispatch_span(&sub);
        let should_exit = dispatch_submission(&sess, &config, sub)
            .instrument(dispatch_span)
            .await;
        if should_exit {
            break;
        }
    }
    sess.guardian_review_session.shutdown().await;
    debug!("Agent loop exited");
}

async fn dispatch_submission(sess: &Arc<Session>, config: &Arc<Config>, sub: Submission) -> bool {
    let Submission { id, op, .. } = sub;
    match op {
        op @ (Op::Interrupt
        | Op::CleanBackgroundTerminals
        | Op::RealtimeConversationStart(_)
        | Op::RealtimeConversationAudio(_)
        | Op::RealtimeConversationText(_)
        | Op::RealtimeConversationClose
        | Op::OverrideTurnContext { .. }
        | Op::UserInput { .. }
        | Op::UserTurn { .. }
        | Op::InterAgentCommunication { .. }) => dispatch_interactive_op(sess, id, op).await,
        op @ (Op::ExecApproval { .. }
        | Op::PatchApproval { .. }
        | Op::UserInputAnswer { .. }
        | Op::RequestPermissionsResponse { .. }
        | Op::DynamicToolResponse { .. }
        | Op::ResolveElicitation { .. }) => dispatch_response_op(sess, id, op).await,
        op @ (Op::AddToHistory { .. }
        | Op::GetHistoryEntryRequest { .. }
        | Op::ListMcpTools
        | Op::RefreshMcpServers { .. }
        | Op::ReloadUserConfig
        | Op::ListSkills { .. }
        | Op::Undo
        | Op::Compact
        | Op::DropMemories
        | Op::UpdateMemories
        | Op::ThreadRollback { .. }
        | Op::SetThreadName { .. }
        | Op::RunUserShellCommand { .. }
        | Op::Shutdown
        | Op::Review { .. }) => dispatch_maintenance_op(sess, config, id, op).await,
        _ => false,
    }
}

async fn dispatch_interactive_op(sess: &Arc<Session>, submission_id: String, op: Op) -> bool {
    match op {
        Op::Interrupt => {
            handlers::interrupt(sess).await;
            false
        }
        Op::CleanBackgroundTerminals => {
            handlers::clean_background_terminals(sess).await;
            false
        }
        Op::RealtimeConversationStart(params) => {
            if let Err(err) =
                handle_realtime_conversation_start(sess, submission_id.clone(), params).await
            {
                sess.send_event_raw(Event {
                    id: submission_id,
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
            handle_realtime_conversation_audio(sess, submission_id, params).await;
            false
        }
        Op::RealtimeConversationText(params) => {
            handle_realtime_conversation_text(sess, submission_id, params).await;
            false
        }
        Op::RealtimeConversationClose => {
            handle_realtime_conversation_close(sess, submission_id).await;
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
        } => {
            let collaboration_mode = if let Some(collab_mode) = collaboration_mode {
                collab_mode
            } else {
                let state = sess.state.lock().await;
                state
                    .session_configuration
                    .collaboration_mode
                    .with_updates(model, effort, /*developer_instructions*/ None)
            };
            handlers::override_turn_context(
                sess,
                submission_id,
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
                    ..Default::default()
                },
            )
            .await;
            false
        }
        Op::UserInput { .. } | Op::UserTurn { .. } => {
            handlers::user_input_or_turn(sess, submission_id, op).await;
            false
        }
        Op::InterAgentCommunication { communication } => {
            handlers::inter_agent_communication(sess, submission_id, communication).await;
            false
        }
        _ => unreachable!("interactive dispatcher received unsupported op"),
    }
}

async fn dispatch_response_op(sess: &Arc<Session>, submission_id: String, op: Op) -> bool {
    match op {
        Op::ExecApproval {
            id: approval_id,
            turn_id,
            decision,
        } => {
            handlers::exec_approval(sess, approval_id, turn_id, decision).await;
            false
        }
        Op::PatchApproval { id, decision } => {
            handlers::patch_approval(sess, id, decision).await;
            false
        }
        Op::UserInputAnswer { id, response } => {
            handlers::request_user_input_response(sess, id, response).await;
            false
        }
        Op::RequestPermissionsResponse { id, response } => {
            handlers::request_permissions_response(sess, id, response).await;
            false
        }
        Op::DynamicToolResponse { id, response } => {
            handlers::dynamic_tool_response(sess, id, response).await;
            false
        }
        Op::ResolveElicitation {
            server_name,
            request_id,
            decision,
            content,
            meta,
        } => {
            handlers::resolve_elicitation(sess, server_name, request_id, decision, content, meta)
                .await;
            false
        }
        _ => {
            let _ = submission_id;
            unreachable!("response dispatcher received unsupported op")
        }
    }
}

async fn dispatch_maintenance_op(
    sess: &Arc<Session>,
    config: &Arc<Config>,
    submission_id: String,
    op: Op,
) -> bool {
    match op {
        Op::AddToHistory { text } => {
            handlers::add_to_history(sess, config, text).await;
            false
        }
        Op::GetHistoryEntryRequest { offset, log_id } => {
            handlers::get_history_entry_request(sess, config, submission_id, offset, log_id).await;
            false
        }
        Op::ListMcpTools => {
            handlers::list_mcp_tools(sess, config, submission_id).await;
            false
        }
        Op::RefreshMcpServers { config } => {
            handlers::refresh_mcp_servers(sess, config).await;
            false
        }
        Op::ReloadUserConfig => {
            handlers::reload_user_config(sess).await;
            false
        }
        Op::ListSkills { cwds, force_reload } => {
            handlers::list_skills(sess, submission_id, cwds, force_reload).await;
            false
        }
        Op::Undo => {
            handlers::undo(sess, submission_id).await;
            false
        }
        Op::Compact => {
            handlers::compact(sess, submission_id).await;
            false
        }
        Op::DropMemories => {
            handlers::drop_memories(sess, config, submission_id).await;
            false
        }
        Op::UpdateMemories => {
            handlers::update_memories(sess, config, submission_id).await;
            false
        }
        Op::ThreadRollback { num_turns } => {
            handlers::thread_rollback(sess, submission_id, num_turns).await;
            false
        }
        Op::SetThreadName { name } => {
            handlers::set_thread_name(sess, submission_id, name).await;
            false
        }
        Op::RunUserShellCommand { command } => {
            handlers::run_user_shell_command(sess, submission_id, command).await;
            false
        }
        Op::Shutdown => handlers::shutdown(sess, submission_id).await,
        Op::Review { review_request } => {
            handlers::review(sess, config, submission_id, review_request).await;
            false
        }
        _ => unreachable!("maintenance dispatcher received unsupported op"),
    }
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
