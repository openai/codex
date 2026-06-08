use super::AgentControl;
use crate::config::Config;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use futures::future::BoxFuture;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[derive(Default)]
pub(super) struct AgentExecutionLimiter {
    active: AtomicUsize,
}

pub(crate) struct AgentExecutionPermit {
    limiter: Arc<AgentExecutionLimiter>,
}

impl Drop for AgentExecutionPermit {
    fn drop(&mut self) {
        self.limiter.active.fetch_sub(1, Ordering::AcqRel);
    }
}

impl AgentControl {
    pub(super) async fn ensure_execution_capacity_for_op(
        &self,
        thread_id: ThreadId,
        op: &Op,
    ) -> CodexResult<()> {
        if !op_starts_turn(op) {
            return Ok(());
        }
        let state = self.upgrade()?;
        let thread = state.get_thread(thread_id).await?;
        if thread.codex.session.active_turn.lock().await.is_some() {
            return Ok(());
        }
        let config = thread.codex.session.get_config().await;
        let multi_agent_version = thread
            .multi_agent_version()
            .unwrap_or_else(|| config.multi_agent_version_from_features());
        let Some(max_threads) =
            execution_limit(config.as_ref(), multi_agent_version, &thread.session_source)
        else {
            return Ok(());
        };
        Arc::clone(&self.agent_execution_limiter)
            .try_acquire(max_threads)
            .map(drop)
    }

    pub(crate) fn try_acquire_execution_permit(
        &self,
        config: &Config,
        multi_agent_version: MultiAgentVersion,
        session_source: &SessionSource,
    ) -> CodexResult<Option<AgentExecutionPermit>> {
        let Some(max_threads) = execution_limit(config, multi_agent_version, session_source) else {
            return Ok(None);
        };
        Arc::clone(&self.agent_execution_limiter)
            .try_acquire(max_threads)
            .map(Some)
    }

    pub(crate) fn maybe_start_v2_pending_work(&self) -> BoxFuture<'_, ()> {
        // Erase this future's concrete type to break the recursive Send proof through
        // task completion -> pending work -> task start -> task completion.
        Box::pin(async move {
            let Ok(state) = self.upgrade() else {
                return;
            };
            for metadata in self.state.live_agents() {
                let Some(thread_id) = metadata.agent_id else {
                    continue;
                };
                let Ok(thread) = state.get_thread(thread_id).await else {
                    continue;
                };
                let config = thread.codex.session.get_config().await;
                let multi_agent_version = thread
                    .multi_agent_version()
                    .unwrap_or_else(|| config.multi_agent_version_from_features());
                if execution_limit(config.as_ref(), multi_agent_version, &thread.session_source)
                    .is_none()
                {
                    continue;
                }
                if !thread
                    .codex
                    .session
                    .input_queue
                    .has_trigger_turn_mailbox_items()
                    .await
                {
                    continue;
                }
                thread
                    .codex
                    .session
                    .maybe_start_turn_for_pending_work()
                    .await;
            }
        })
    }
}

impl AgentExecutionLimiter {
    fn try_acquire(self: Arc<Self>, max_threads: usize) -> CodexResult<AgentExecutionPermit> {
        let mut current = self.active.load(Ordering::Acquire);
        loop {
            if current >= max_threads {
                return Err(CodexErr::AgentLimitReached { max_threads });
            }
            match self.active.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(AgentExecutionPermit { limiter: self }),
                Err(updated) => current = updated,
            }
        }
    }
}

fn op_starts_turn(op: &Op) -> bool {
    matches!(op, Op::UserInput { .. })
        || matches!(op, Op::InterAgentCommunication { communication } if communication.trigger_turn)
}

fn execution_limit(
    config: &Config,
    multi_agent_version: MultiAgentVersion,
    session_source: &SessionSource,
) -> Option<usize> {
    if multi_agent_version != MultiAgentVersion::V2
        || !matches!(session_source, SessionSource::SubAgent(_))
    {
        return None;
    }
    Some(
        config
            .effective_agent_max_threads(MultiAgentVersion::V2)
            .unwrap_or(usize::MAX),
    )
}

#[cfg(test)]
#[path = "execution_tests.rs"]
mod tests;
