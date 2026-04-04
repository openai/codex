use super::*;
use crate::agent::status::is_final;
use crate::error::CodexErr;
use futures::FutureExt;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::Receiver;
use tokio::time::Instant;
use tokio::time::timeout_at;

pub(crate) struct Handler;

#[async_trait]
impl ToolHandler for Handler {
    type Output = WaitAgentResult;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            call_id,
            ..
        } = invocation;
        let arguments = function_arguments(payload)?;
        let args: WaitArgs = parse_arguments(&arguments)?;

        if let Some(owner_thread_id) = session
            .services
            .agent_control
            .watchdog_owner_for_active_helper(session.conversation_id)
            .await
        {
            return Err(FunctionCallError::RespondToModel(format!(
                "wait_agent is not available to watchdog check-in agents. This thread is a one-shot watchdog check-in for owner {owner_thread_id}. Send the result to the parent/root agent with `send_input`. If you finish without `send_input`, runtime will forward your conclusory message to the owner as the mandatory fallback wake-up path. Exiting without either `send_input` or a final message is a bug; every watchdog check-in must wake the owner thread."
            )));
        }

        let receiver_thread_ids = args
            .targets
            .iter()
            .map(|target| parse_agent_id_target(target))
            .collect::<Result<Vec<_>, _>>()?;
        let mut receiver_agents = Vec::with_capacity(receiver_thread_ids.len());
        let mut target_by_thread_id = HashMap::with_capacity(receiver_thread_ids.len());
        for receiver_thread_id in &receiver_thread_ids {
            let agent_metadata = session
                .services
                .agent_control
                .get_agent_metadata(*receiver_thread_id)
                .unwrap_or_default();
            target_by_thread_id.insert(
                *receiver_thread_id,
                agent_metadata
                    .agent_path
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| receiver_thread_id.to_string()),
            );
            receiver_agents.push(CollabAgentRef {
                thread_id: *receiver_thread_id,
                agent_nickname: agent_metadata.agent_nickname,
                agent_role: agent_metadata.agent_role,
            });
        }

        let watchdog_target_ids = session
            .services
            .agent_control
            .watchdog_targets(&receiver_thread_ids)
            .await;
        let mut waited_thread_ids = Vec::new();
        let mut watchdog_statuses = Vec::new();
        split_wait_ids(
            &session,
            receiver_thread_ids,
            &watchdog_target_ids,
            &mut waited_thread_ids,
            &mut watchdog_statuses,
        )
        .await;

        let timeout_ms = args.timeout_ms.unwrap_or(DEFAULT_WAIT_TIMEOUT_MS);
        let timeout_ms = match timeout_ms {
            ms if ms <= 0 => {
                return Err(FunctionCallError::RespondToModel(
                    "timeout_ms must be greater than zero".to_owned(),
                ));
            }
            ms => ms.clamp(MIN_WAIT_TIMEOUT_MS, MAX_WAIT_TIMEOUT_MS),
        };

        session
            .send_event(
                &turn,
                CollabWaitingBeginEvent {
                    sender_thread_id: session.conversation_id,
                    receiver_thread_ids: waited_thread_ids.clone(),
                    receiver_agents: receiver_agents.clone(),
                    call_id: call_id.clone(),
                }
                .into(),
            )
            .await;

        if waited_thread_ids.is_empty() {
            let statuses_map = watchdog_statuses.iter().cloned().collect::<HashMap<_, _>>();
            let content = serde_json::to_string(&statuses_map).map_err(|err| {
                FunctionCallError::Fatal(format!("failed to serialize wait_agent status: {err}"))
            })?;
            session
                .send_event(
                    &turn,
                    CollabWaitingEndEvent {
                        sender_thread_id: session.conversation_id,
                        call_id,
                        agent_statuses: Vec::new(),
                        statuses: statuses_map,
                    }
                    .into(),
                )
                .await;
            return Err(FunctionCallError::RespondToModel(format!(
                "wait_agent cannot be used to wait for watchdog check-ins. You passed only watchdog handle ids. Watchdog check-ins only happen after the current turn ends and the owner thread is idle for at least watchdog_interval_s. `wait_agent` on a watchdog handle is status-only and cannot confirm a new check-in. Do not poll with `wait_agent`, `list_agents`, or shell `sleep`: the owner thread is still active during this turn, so those calls cannot make the watchdog fire. Current watchdog handle statuses: {content}"
            )));
        }

        let mut status_rxs = Vec::with_capacity(waited_thread_ids.len());
        let mut initial_final_statuses = Vec::new();
        for id in &waited_thread_ids {
            match session.services.agent_control.subscribe_status(*id).await {
                Ok(rx) => {
                    let status = rx.borrow().clone();
                    if is_final(&status) {
                        initial_final_statuses.push((*id, status));
                    }
                    status_rxs.push((*id, rx));
                }
                Err(CodexErr::ThreadNotFound(_)) => {
                    initial_final_statuses.push((*id, AgentStatus::NotFound));
                }
                Err(err) => {
                    let mut statuses = HashMap::with_capacity(1 + watchdog_statuses.len());
                    statuses.insert(*id, session.services.agent_control.get_status(*id).await);
                    statuses.extend(watchdog_statuses.iter().cloned());
                    session
                        .send_event(
                            &turn,
                            CollabWaitingEndEvent {
                                sender_thread_id: session.conversation_id,
                                call_id: call_id.clone(),
                                agent_statuses: build_wait_agent_statuses(
                                    &statuses,
                                    &receiver_agents,
                                ),
                                statuses,
                            }
                            .into(),
                        )
                        .await;
                    return Err(collab_agent_error(*id, err));
                }
            }
        }

        let statuses = if !initial_final_statuses.is_empty() {
            initial_final_statuses
        } else {
            let mut futures = FuturesUnordered::new();
            for (id, rx) in status_rxs {
                let session = session.clone();
                futures.push(wait_for_final_status(session, id, rx));
            }
            let mut results = Vec::new();
            let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
            loop {
                match timeout_at(deadline, futures.next()).await {
                    Ok(Some(Some(result))) => {
                        results.push(result);
                        break;
                    }
                    Ok(Some(None)) => continue,
                    Ok(None) | Err(_) => break,
                }
            }
            if !results.is_empty() {
                loop {
                    match futures.next().now_or_never() {
                        Some(Some(Some(result))) => results.push(result),
                        Some(Some(None)) => continue,
                        Some(None) | None => break,
                    }
                }
            }
            results
        };

        let timed_out = statuses.is_empty();
        let mut statuses_by_id = statuses.clone().into_iter().collect::<HashMap<_, _>>();
        statuses_by_id.extend(watchdog_statuses);
        let agent_statuses = build_wait_agent_statuses(&statuses_by_id, &receiver_agents);
        let result = WaitAgentResult {
            status: statuses_by_id
                .iter()
                .filter_map(|(thread_id, status)| {
                    target_by_thread_id
                        .get(thread_id)
                        .cloned()
                        .map(|target| (target, status.clone()))
                })
                .collect(),
            timed_out,
        };

        session
            .send_event(
                &turn,
                CollabWaitingEndEvent {
                    sender_thread_id: session.conversation_id,
                    call_id,
                    agent_statuses,
                    statuses: statuses_by_id,
                }
                .into(),
            )
            .await;

        Ok(result)
    }
}

#[derive(Debug, Deserialize)]
struct WaitArgs {
    #[serde(default)]
    targets: Vec<String>,
    timeout_ms: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct WaitAgentResult {
    pub(crate) status: HashMap<String, AgentStatus>,
    pub(crate) timed_out: bool,
}

impl ToolOutput for WaitAgentResult {
    fn log_preview(&self) -> String {
        tool_output_json_text(self, "wait_agent")
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        tool_output_response_item(call_id, payload, self, /*success*/ None, "wait_agent")
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        tool_output_code_mode_result(self, "wait_agent")
    }
}

async fn wait_for_final_status(
    session: Arc<Session>,
    thread_id: ThreadId,
    mut status_rx: Receiver<AgentStatus>,
) -> Option<(ThreadId, AgentStatus)> {
    let mut status = status_rx.borrow().clone();
    if is_final(&status) {
        return Some((thread_id, status));
    }

    loop {
        if status_rx.changed().await.is_err() {
            let latest = session.services.agent_control.get_status(thread_id).await;
            return is_final(&latest).then_some((thread_id, latest));
        }
        status = status_rx.borrow().clone();
        if is_final(&status) {
            return Some((thread_id, status));
        }
    }
}

async fn split_wait_ids(
    session: &Arc<Session>,
    requested_thread_ids: Vec<ThreadId>,
    watchdog_target_ids: &HashSet<ThreadId>,
    waited_thread_ids: &mut Vec<ThreadId>,
    watchdog_statuses: &mut Vec<(ThreadId, AgentStatus)>,
) {
    for thread_id in requested_thread_ids {
        if watchdog_target_ids.contains(&thread_id) {
            let status = session.services.agent_control.get_status(thread_id).await;
            watchdog_statuses.push((thread_id, status));
        } else {
            waited_thread_ids.push(thread_id);
        }
    }
}
