use super::AgentControl;
use crate::config::Config;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[derive(Default)]
pub(super) struct AgentExecutionLimiter {
    active: AtomicUsize,
}

pub(crate) struct AgentExecutionGuard {
    limiter: Arc<AgentExecutionLimiter>,
}

impl Drop for AgentExecutionGuard {
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
        self.ensure_execution_capacity(config.as_ref(), multi_agent_version, &thread.session_source)
    }

    pub(crate) fn ensure_execution_capacity(
        &self,
        config: &Config,
        multi_agent_version: MultiAgentVersion,
        session_source: &SessionSource,
    ) -> CodexResult<()> {
        let Some(max_threads) = execution_limit(config, multi_agent_version, session_source) else {
            return Ok(());
        };
        if self.agent_execution_limiter.has_capacity(max_threads) {
            Ok(())
        } else {
            Err(CodexErr::AgentLimitReached { max_threads })
        }
    }

    pub(crate) fn execution_guard(
        &self,
        config: &Config,
        multi_agent_version: MultiAgentVersion,
        session_source: &SessionSource,
    ) -> Option<AgentExecutionGuard> {
        if execution_limit(config, multi_agent_version, session_source).is_none() {
            return None;
        };
        Some(Arc::clone(&self.agent_execution_limiter).guard())
    }
}

impl AgentExecutionLimiter {
    fn has_capacity(&self, max_threads: usize) -> bool {
        self.active.load(Ordering::Acquire) < max_threads
    }

    fn guard(self: Arc<Self>) -> AgentExecutionGuard {
        self.active.fetch_add(1, Ordering::AcqRel);
        AgentExecutionGuard { limiter: self }
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
