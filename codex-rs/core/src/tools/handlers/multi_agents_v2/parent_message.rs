use super::message_tool::message_content;
use super::*;
use crate::tools::context::FunctionToolOutput;
use codex_protocol::ThreadId;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;

struct ParentMessageSource {
    parent_thread_id: ThreadId,
    parent_agent_path: AgentPath,
    child_agent_path: AgentPath,
}

pub(crate) struct Handler;

impl ToolHandler for Handler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let arguments = function_arguments(invocation.payload.clone())?;
        let args: ParentMessageArgs = parse_arguments(&arguments)?;
        let delivery = args.delivery_options();
        handle_parent_message(invocation, message_content(args.message)?, delivery).await
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParentMessageArgs {
    message: String,
    #[serde(default)]
    mode: ParentMessageMode,
    trigger_turn: Option<bool>,
}

impl ParentMessageArgs {
    fn delivery_options(&self) -> ParentMessageDelivery {
        ParentMessageDelivery {
            mode: self.mode,
            trigger_turn: self
                .trigger_turn
                .unwrap_or(self.mode == ParentMessageMode::Interrupt),
        }
    }
}

#[derive(Debug, Default, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ParentMessageMode {
    #[default]
    #[serde(alias = "enqueue")]
    Queue,
    Interrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParentMessageDelivery {
    mode: ParentMessageMode,
    trigger_turn: bool,
}

async fn handle_parent_message(
    invocation: ToolInvocation,
    prompt: String,
    delivery: ParentMessageDelivery,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let ToolInvocation {
        session,
        turn,
        call_id,
        ..
    } = invocation;
    let session_source = session.session_source().await;
    let source_result = parent_message_source(&turn.session_source)
        .or_else(|| parent_message_source(&session_source))
        .or_else(|| parent_message_source_from_agent_registry(session.as_ref()))
        .or_else(|| {
            parent_message_source_from_thread_spawn_context(
                session.as_ref(),
                &turn.session_source,
                &session_source,
            )
        });
    let source = match source_result {
        Some(Ok(source)) => Some(source),
        Some(Err(err)) => return Err(err),
        None => {
            parent_message_source_from_parent_thread(
                session.as_ref(),
                &turn.session_source,
                &session_source,
            )
            .await
        }
    };
    let source = source.ok_or_else(|| {
        FunctionCallError::RespondToModel(
            "send_parent_message is only available from a spawned sub-agent".to_string(),
        )
    })?;

    session
        .send_event(
            &turn,
            CollabAgentInteractionBeginEvent {
                call_id: call_id.clone(),
                sender_thread_id: session.conversation_id,
                receiver_thread_id: source.parent_thread_id,
                prompt: prompt.clone(),
            }
            .into(),
        )
        .await;

    let parent_waiting = if delivery.trigger_turn && delivery.mode == ParentMessageMode::Queue {
        session
            .services
            .agent_control
            .has_mailbox_waiters(source.parent_thread_id)
            .await
            .map_err(|err| collab_agent_error(source.parent_thread_id, err))?
    } else {
        false
    };

    let communication = InterAgentCommunication::new(
        source.child_agent_path,
        source.parent_agent_path,
        Vec::new(),
        prompt.clone(),
        delivery.trigger_turn,
    );
    let parent_status_before_wake = session
        .services
        .agent_control
        .get_status(source.parent_thread_id)
        .await;
    let parent_running = matches!(parent_status_before_wake, AgentStatus::Running);
    let defer_delivery_until_parent_is_free = parent_running
        && (delivery.mode == ParentMessageMode::Interrupt
            || (delivery.trigger_turn && !parent_waiting));
    let send_result = if defer_delivery_until_parent_is_free {
        if delivery.mode == ParentMessageMode::Interrupt {
            session
                .services
                .agent_control
                .interrupt_agent(source.parent_thread_id)
                .await
                .map_err(|err| collab_agent_error(source.parent_thread_id, err))?;
        }
        deliver_parent_message_when_parent_is_free(
            session.services.agent_control.clone(),
            source.parent_thread_id,
            communication,
            delivery.trigger_turn,
        );
        Ok(())
    } else {
        session
            .services
            .agent_control
            .enqueue_inter_agent_communication(source.parent_thread_id, communication)
            .await
            .map_err(|err| collab_agent_error(source.parent_thread_id, err))
    };

    if send_result.is_ok()
        && delivery.trigger_turn
        && !parent_waiting
        && !defer_delivery_until_parent_is_free
    {
        session
            .services
            .agent_control
            .maybe_start_turn_for_pending_work(source.parent_thread_id)
            .await
            .map_err(|err| collab_agent_error(source.parent_thread_id, err))?;
    }

    let status = session
        .services
        .agent_control
        .get_status(source.parent_thread_id)
        .await;
    session
        .send_event(
            &turn,
            CollabAgentInteractionEndEvent {
                call_id,
                sender_thread_id: session.conversation_id,
                receiver_thread_id: source.parent_thread_id,
                receiver_agent_nickname: None,
                receiver_agent_role: None,
                prompt,
                status,
            }
            .into(),
        )
        .await;

    send_result?;
    Ok(FunctionToolOutput::from_text(String::new(), Some(true)))
}

fn deliver_parent_message_when_parent_is_free(
    agent_control: crate::agent::control::AgentControl,
    parent_thread_id: ThreadId,
    communication: InterAgentCommunication,
    trigger_turn: bool,
) {
    tokio::spawn(async move {
        let Ok(mut status_rx) = agent_control.subscribe_status(parent_thread_id).await else {
            return;
        };
        while matches!(*status_rx.borrow(), AgentStatus::Running) {
            if status_rx.changed().await.is_err() {
                return;
            }
        }
        if agent_control
            .enqueue_inter_agent_communication(parent_thread_id, communication)
            .await
            .is_err()
        {
            return;
        }
        if trigger_turn {
            let _ = agent_control
                .maybe_start_turn_for_pending_work(parent_thread_id)
                .await;
        }
    });
}

fn parent_message_source(
    session_source: &SessionSource,
) -> Option<Result<ParentMessageSource, FunctionCallError>> {
    match session_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            agent_path: Some(agent_path),
            ..
        }) => Some(
            parent_agent_path(agent_path).map(|parent_agent_path| ParentMessageSource {
                parent_thread_id: *parent_thread_id,
                parent_agent_path,
                child_agent_path: agent_path.clone(),
            }),
        ),
        _ => None,
    }
}

fn parent_message_source_from_agent_registry(
    session: &crate::session::session::Session,
) -> Option<Result<ParentMessageSource, FunctionCallError>> {
    let child_agent_path = session
        .services
        .agent_control
        .get_agent_metadata(session.conversation_id)?
        .agent_path?;
    let parent_agent_path = match parent_agent_path(&child_agent_path) {
        Ok(parent_agent_path) => parent_agent_path,
        Err(err) => return Some(Err(err)),
    };
    let Some(parent_thread_id) = session
        .services
        .agent_control
        .agent_id_for_path(&parent_agent_path)
    else {
        return Some(Err(FunctionCallError::RespondToModel(
            "Could not resolve parent thread for this sub-agent".to_string(),
        )));
    };
    Some(Ok(ParentMessageSource {
        parent_thread_id,
        parent_agent_path,
        child_agent_path,
    }))
}

fn parent_message_source_from_thread_spawn_context(
    session: &crate::session::session::Session,
    turn_session_source: &SessionSource,
    session_source: &SessionSource,
) -> Option<Result<ParentMessageSource, FunctionCallError>> {
    let parent_thread_id = thread_spawn_parent_thread_id(turn_session_source)
        .or_else(|| thread_spawn_parent_thread_id(session_source))?;
    let child_agent_path = session
        .services
        .agent_control
        .get_agent_metadata(session.conversation_id)
        .and_then(|metadata| metadata.agent_path);
    child_agent_path.map(|child_agent_path| {
        let parent_agent_path = parent_agent_path(&child_agent_path)?;
        Ok(ParentMessageSource {
            parent_thread_id,
            parent_agent_path,
            child_agent_path,
        })
    })
}

async fn parent_message_source_from_parent_thread(
    session: &crate::session::session::Session,
    turn_session_source: &SessionSource,
    session_source: &SessionSource,
) -> Option<ParentMessageSource> {
    let parent_thread_id = thread_spawn_parent_thread_id(turn_session_source)
        .or_else(|| thread_spawn_parent_thread_id(session_source))?;
    let parent_agent_path = session
        .services
        .agent_control
        .get_agent_config_snapshot(parent_thread_id)
        .await
        .and_then(|snapshot| snapshot.session_source.get_agent_path())
        .unwrap_or_else(AgentPath::root);
    let synthetic_name = format!(
        "agent_{}",
        session.conversation_id.to_string().replace('-', "_")
    );
    let child_agent_path = parent_agent_path.join(&synthetic_name).ok()?;
    Some(ParentMessageSource {
        parent_thread_id,
        parent_agent_path,
        child_agent_path,
    })
}

fn thread_spawn_parent_thread_id(session_source: &SessionSource) -> Option<ThreadId> {
    let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id, ..
    }) = session_source
    else {
        return None;
    };
    Some(*parent_thread_id)
}

fn parent_agent_path(child_agent_path: &AgentPath) -> Result<AgentPath, FunctionCallError> {
    child_agent_path
        .as_str()
        .rsplit_once('/')
        .and_then(|(parent, _)| AgentPath::try_from(parent).ok())
        .ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "Could not resolve parent agent path for this sub-agent".to_string(),
            )
        })
}
