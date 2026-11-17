//! Coordinates independent root resumes while keeping list mutations and uncertain sources exclusive.
//!
//! The request queue serializes canonical thread IDs across listener replacement. UUID aliases,
//! paths, and uncertain sources use exclusive list ordering before resolving their identity.

use super::*;
use crate::request_processors::thread_list_state::ThreadListResumePermit;
use codex_protocol::protocol::SessionSource;

pub(super) struct ResumePermit<'a> {
    list: ThreadListResumePermit<'a>,
    thread_id: Option<ThreadId>,
}

impl<'a> ResumePermit<'a> {
    /// Only canonical IDs can share: all requests for that spelling use the same queue.
    /// Callers check source ownership under the permit and retry exclusively when needed.
    pub(super) async fn acquire(
        processor: &'a ThreadRequestProcessor,
        params: &ThreadResumeParams,
        force_exclusive: bool,
    ) -> Result<Self, JSONRPCErrorError> {
        let thread_id = if !force_exclusive && params.history.is_none() && params.path.is_none() {
            ThreadId::from_string(&params.thread_id).ok()
        } else {
            None
        };
        let shared = thread_id.is_some_and(|id| id.to_string() == params.thread_id);
        let list = processor
            .thread_list_state_permit
            .acquire_resume(shared)
            .await?;
        Ok(Self { list, thread_id })
    }

    pub(super) fn is_shared(&self) -> bool {
        self.list.is_shared()
    }

    pub(super) fn finish_probe(&mut self) {
        self.list.finish_probe();
    }

    pub(super) async fn upgrade(self) -> Result<Self, JSONRPCErrorError> {
        let Self { list, thread_id } = self;
        Ok(Self {
            list: list.upgrade().await?,
            thread_id,
        })
    }

    /// Never use one thread's shared permit for another.
    pub(super) fn check_id(&self, actual: ThreadId) -> Result<(), JSONRPCErrorError> {
        if self.thread_id.is_some_and(|requested| requested != actual) {
            return Err(invalid_request(
                "thread source changed during resume; retry with the current thread ID or path",
            ));
        }
        Ok(())
    }
}

pub(super) fn source_can_share(source: &StoredThread) -> bool {
    source.attached_to_thread_id.is_none()
        && source_kind_can_share(&source.source, source.parent_thread_id)
}

/// Agent-owned, unknown and parent-linked sources retain the original exclusive ordering.
pub(super) fn source_kind_can_share(
    source: &SessionSource,
    parent_thread_id: Option<ThreadId>,
) -> bool {
    parent_thread_id.is_none()
        && !source.is_non_root_agent()
        && !matches!(source, SessionSource::Unknown)
}
