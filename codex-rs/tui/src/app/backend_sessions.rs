//! Owns backend sessions independently of the displayed conversation's UI state.
//! Polling is fair and cancellation-safe; closing one stream never closes another.

use crate::app_server_session::AppServerSession;
use crate::app_server_session::AppServerSessionEvent;
use crate::backend_id::BackendId;
use std::future::Future;
use std::task::Poll;

struct SessionEntry {
    session: AppServerSession,
    listening: bool,
}

/// A session's stable position in an append-only backend collection.
#[derive(Clone, Copy)]
pub(super) struct SessionSlot(usize);

pub(super) struct BackendSessions {
    entries: Vec<SessionEntry>,
    next_source: usize,
}

impl BackendSessions {
    pub(super) fn new(session: AppServerSession) -> Self {
        let mut sessions = Self {
            entries: Vec::new(),
            next_source: 0,
        };
        sessions.insert(session);
        sessions
    }

    pub(super) fn insert(&mut self, session: AppServerSession) {
        assert!(self.session(session.backend_id()).is_none());
        self.entries.push(SessionEntry {
            session,
            listening: true,
        });
    }

    pub(super) fn session(&self, owner: BackendId) -> Option<&AppServerSession> {
        self.slot(owner).map(|slot| self.session_at(slot))
    }

    pub(super) fn slot(&self, owner: BackendId) -> Option<SessionSlot> {
        self.entries
            .iter()
            .position(|entry| entry.session.backend_id() == owner)
            .map(SessionSlot)
    }

    pub(super) fn session_at(&self, slot: SessionSlot) -> &AppServerSession {
        &self.entries[slot.0].session
    }

    pub(super) fn session_at_mut(&mut self, slot: SessionSlot) -> &mut AppServerSession {
        &mut self.entries[slot.0].session
    }

    pub(super) fn is_listening(&self, owner: BackendId) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.session.backend_id() == owner && entry.listening)
    }

    pub(super) fn resume_events(&mut self, owner: BackendId) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.session.backend_id() == owner)
        {
            entry.listening = true;
        }
    }

    pub(super) async fn next_event(
        &mut self,
        paused_backend: Option<BackendId>,
    ) -> (BackendId, Option<AppServerSessionEvent>) {
        std::future::poll_fn(|cx| {
            for offset in 0..self.entries.len() {
                let index = (self.next_source + offset) % self.entries.len();
                let entry = &mut self.entries[index];
                let owner = entry.session.backend_id();
                if !entry.listening || paused_backend == Some(owner) {
                    continue;
                }
                // Both client transports receive from cancellation-safe queues. Dropping a
                // pending receive leaves its event available for the next main-loop poll.
                let event = std::pin::pin!(entry.session.next_event()).poll(cx);
                if let Poll::Ready(event) = event {
                    entry.listening = event.is_some();
                    self.next_source = (index + 1) % self.entries.len();
                    return Poll::Ready((owner, event));
                }
            }
            Poll::Pending
        })
        .await
    }

    pub(super) async fn shutdown(self) -> std::io::Result<()> {
        let mut first_error = None;
        for entry in self.entries {
            if let Err(error) = entry.session.shutdown().await {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}
