use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex::run_collaboration_turn;
use crate::state::AgentId;
use crate::state::AgentLifecycleState;
use crate::state::TaskKind;
use crate::tools::context::SharedTurnDiffTracker;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_protocol::user_input::UserInput;

use super::SessionTask;
use super::SessionTaskContext;

/// An async supervisor that drives child collaboration agents.
///
/// Each agent gets a runner task that executes turns on demand and emits
/// `AgentRunResult` events on a broadcast channel. History swaps are serialized
/// via a shared lock so runners can progress while the main agent continues.
#[derive(Clone)]
pub(crate) struct CollaborationSupervisor {
    tx: mpsc::Sender<SupervisorCommand>,
    events: broadcast::Sender<AgentRunResult>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentRunResult {
    pub(crate) agent: AgentId,
    pub(crate) status: AgentLifecycleState,
}

#[derive(Debug)]
enum AgentCommand {
    Run { max_duration: i32 },
    Close,
}

enum SupervisorCommand {
    RunAgents {
        targets: Vec<AgentId>,
        max_duration: i32,
    },
    CloseAgents {
        targets: Vec<AgentId>,
    },
}

impl CollaborationSupervisor {
    pub(crate) fn spawn(session: Arc<Session>) -> Self {
        let (tx, mut rx) = mpsc::channel::<SupervisorCommand>(8);
        let (events, _rx) = broadcast::channel::<AgentRunResult>(64);
        let mut runners: HashMap<AgentId, mpsc::Sender<AgentCommand>> = HashMap::new();
        let events_tx = events.clone();

        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    SupervisorCommand::RunAgents {
                        targets,
                        max_duration,
                    } => {
                        for agent in &targets {
                            ensure_runner(
                                *agent,
                                &mut runners,
                                Arc::clone(&session),
                                events_tx.clone(),
                            );
                        }
                        for target in targets {
                            let tx = runners.get(&target).cloned();
                            if let Some(tx) = tx {
                                match tx.try_send(AgentCommand::Run { max_duration }) {
                                    Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => {}
                                    Err(mpsc::error::TrySendError::Closed(_)) => {
                                        runners.remove(&target);
                                    }
                                }
                            }
                        }
                    }
                    SupervisorCommand::CloseAgents { targets } => {
                        for agent in targets {
                            if let Some(tx) = runners.remove(&agent) {
                                let _ = tx.send(AgentCommand::Close).await;
                            }
                        }
                    }
                }
            }
        });

        Self { tx, events }
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<AgentRunResult> {
        self.events.subscribe()
    }

    pub(crate) async fn start_agents(
        &self,
        targets: Vec<AgentId>,
        max_duration: i32,
    ) -> Result<(), String> {
        let cmd = SupervisorCommand::RunAgents {
            targets,
            max_duration,
        };
        self.tx
            .send(cmd)
            .await
            .map_err(|err| format!("collaboration supervisor unavailable: {err}"))
    }

    pub(crate) async fn close_agents(&self, targets: Vec<AgentId>) {
        let _ = self
            .tx
            .send(SupervisorCommand::CloseAgents { targets })
            .await;
    }
}

fn ensure_runner(
    agent: AgentId,
    runners: &mut HashMap<AgentId, mpsc::Sender<AgentCommand>>,
    session: Arc<Session>,
    events: broadcast::Sender<AgentRunResult>,
) {
    if runners.contains_key(&agent) {
        return;
    }

    let (tx, mut rx) = mpsc::channel::<AgentCommand>(1);
    runners.insert(agent, tx);

    tokio::spawn(async move {
        let mut pending_run = false;
        let mut next_budget = 0;
        loop {
            if !pending_run {
                match rx.recv().await {
                    Some(AgentCommand::Run { max_duration }) => {
                        pending_run = true;
                        next_budget = max_duration;
                    }
                    Some(AgentCommand::Close) | None => break,
                }
            }

            if !pending_run {
                continue;
            }

            let budget = next_budget;
            pending_run = false;
            next_budget = i32::MAX;

            match run_agent_turns(Arc::clone(&session), agent, budget).await {
                Ok((results, keep_running)) => {
                    for result in results {
                        let _ = events.send(result);
                    }
                    pending_run = keep_running;
                }
                Err(err) => {
                    let _ = events.send(AgentRunResult {
                        agent,
                        status: AgentLifecycleState::Error { error: err },
                    });
                }
            }
        }
    });
}

async fn run_agent_turns(
    session: Arc<Session>,
    target: AgentId,
    max_duration: i32,
) -> Result<(Vec<AgentRunResult>, bool), String> {
    let mut remaining_budget = max_duration;
    let mut results = Vec::new();
    let mut keep_running = false;

    while remaining_budget > 0 {
        let agent_snapshot = {
            let collab = session.collaboration_state().lock().await;
            collab.agent(target).cloned()
        };
        let Some(agent_snapshot) = agent_snapshot else {
            return Err(format!("unknown agent {}", target.0));
        };
        if matches!(
            agent_snapshot.status,
            AgentLifecycleState::Closed
                | AgentLifecycleState::Exhausted
                | AgentLifecycleState::Error { .. }
        ) {
            results.push(AgentRunResult {
                agent: target,
                status: agent_snapshot.status,
            });
            break;
        }
        {
            let mut collab = session.collaboration_state().lock().await;
            if let Some(agent) = collab.agent_mut(target) {
                agent.status = AgentLifecycleState::Running;
            }
        }

        let mut agent_history = agent_snapshot.history.clone();
        let sub_id = {
            let mut collab = session.collaboration_state().lock().await;

            collab.next_sub_id(target)
        };
        let turn_context = session
            .make_collaboration_turn_context(&agent_snapshot, sub_id.clone())
            .await;
        session.register_sub_id(target, sub_id.clone()).await;
        let tracker: SharedTurnDiffTracker =
            Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
        let mut agent_status = AgentLifecycleState::Running;
        let before_tokens = agent_history.get_total_token_usage();
        let run_result = run_collaboration_turn(
            Arc::clone(&session),
            Arc::clone(&turn_context),
            tracker,
            agent_history.get_history_for_prompt(),
            CancellationToken::new(),
        )
        .await;

        let (delta_tokens, continue_running) = match run_result {
            Ok((needs_follow_up, last)) => {
                let new_history = session.clone_history_for_agent(target).await;
                let after_tokens = new_history.get_total_token_usage();
                let delta_tokens = after_tokens
                    .saturating_sub(before_tokens)
                    .clamp(0, i32::MAX as i64) as i32;
                {
                    let mut collab = session.collaboration_state().lock().await;
                    if let Some(agent) = collab.agent_mut(target) {
                        if needs_follow_up {
                            agent_status = AgentLifecycleState::Running;
                        } else {
                            agent_status = AgentLifecycleState::Idle {
                                last_agent_message: last.clone(),
                            };
                        }
                        agent.status = agent_status.clone();
                        agent.history = new_history.clone();
                    }
                }
                (delta_tokens, needs_follow_up)
            }
            Err(err) => {
                {
                    let mut collab = session.collaboration_state().lock().await;
                    if let Some(agent) = collab.agent_mut(target) {
                        agent_status = AgentLifecycleState::Error {
                            error: err.to_string(),
                        };
                        agent.status = agent_status.clone();
                    }
                }
                (0, false)
            }
        };

        remaining_budget = remaining_budget.saturating_sub(delta_tokens);

        let final_status = {
            let mut collab = session.collaboration_state().lock().await;
            if continue_running
                && remaining_budget <= 0
                && let Some(agent) = collab.agent_mut(target)
            {
                agent.status = AgentLifecycleState::Exhausted;
            }
            collab.agent(target).map(|a| a.status.clone())
        }
        .unwrap_or(agent_status.clone());

        results.push(AgentRunResult {
            agent: target,
            status: final_status,
        });

        keep_running |= continue_running && remaining_budget > 0;
        if !continue_running || remaining_budget <= 0 {
            break;
        }
    }

    Ok((results, keep_running))
}

/// Collaboration task wrapper.
#[allow(dead_code)]
#[derive(Clone, Copy, Default)]
pub(crate) struct CollaborationTask;

#[async_trait]
impl SessionTask for CollaborationTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        let sess = session.clone_session();
        crate::codex::run_task(sess, ctx, input, cancellation_token).await
    }
}
