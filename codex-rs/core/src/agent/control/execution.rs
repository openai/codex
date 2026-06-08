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
pub(super) struct V2ExecutionSlots {
    active: AtomicUsize,
}

pub(crate) struct V2ExecutionSlot {
    slots: Arc<V2ExecutionSlots>,
}

impl Drop for V2ExecutionSlot {
    fn drop(&mut self) {
        self.slots.active.fetch_sub(1, Ordering::AcqRel);
    }
}

impl AgentControl {
    pub(super) async fn ensure_v2_execution_capacity_for_op(
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
        self.ensure_v2_execution_capacity(
            config.as_ref(),
            multi_agent_version,
            &thread.session_source,
        )
    }

    pub(crate) fn reserve_v2_execution_slot(
        &self,
        config: &Config,
        multi_agent_version: MultiAgentVersion,
        session_source: &SessionSource,
    ) -> CodexResult<Option<V2ExecutionSlot>> {
        if !uses_v2_execution_slot(multi_agent_version, session_source) {
            return Ok(None);
        }
        let max_threads = config
            .effective_agent_max_threads(MultiAgentVersion::V2)
            .unwrap_or(usize::MAX);
        Arc::clone(&self.v2_execution_slots)
            .reserve(max_threads)
            .map(Some)
    }

    pub(super) fn ensure_v2_execution_capacity(
        &self,
        config: &Config,
        multi_agent_version: MultiAgentVersion,
        session_source: &SessionSource,
    ) -> CodexResult<()> {
        if !uses_v2_execution_slot(multi_agent_version, session_source) {
            return Ok(());
        }
        let max_threads = config
            .effective_agent_max_threads(MultiAgentVersion::V2)
            .unwrap_or(usize::MAX);
        if self.v2_execution_slots.has_capacity(max_threads) {
            Ok(())
        } else {
            Err(CodexErr::AgentLimitReached { max_threads })
        }
    }
}

impl V2ExecutionSlots {
    fn reserve(self: Arc<Self>, max_threads: usize) -> CodexResult<V2ExecutionSlot> {
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
                Ok(_) => return Ok(V2ExecutionSlot { slots: self }),
                Err(updated) => current = updated,
            }
        }
    }

    fn has_capacity(&self, max_threads: usize) -> bool {
        self.active.load(Ordering::Acquire) < max_threads
    }
}

fn op_starts_turn(op: &Op) -> bool {
    matches!(op, Op::UserInput { .. })
        || matches!(op, Op::InterAgentCommunication { communication } if communication.trigger_turn)
}

fn uses_v2_execution_slot(
    multi_agent_version: MultiAgentVersion,
    session_source: &SessionSource,
) -> bool {
    multi_agent_version == MultiAgentVersion::V2
        && matches!(session_source, SessionSource::SubAgent(_))
}

#[cfg(test)]
#[path = "execution_tests.rs"]
mod tests;
