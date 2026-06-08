use super::AgentControl;
use crate::config::Config;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionSource;
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
