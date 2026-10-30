//! Owns backend sessions independently of the displayed conversation's UI state.
//! Polling is fair and cancellation-safe; closing one stream never closes another.

use super::resume_preparation::BackendSettings;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::AppServerSessionEvent;
use crate::backend_id::BackendId;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ServerNotification;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::future::Future;
use std::task::Poll;

/// The initial conventional session and any additional sessions registered at launch.
pub(crate) struct InitialBackendSessions {
    initial: AppServerSession,
    others: Vec<AppServerSession>,
}

impl InitialBackendSessions {
    pub(crate) fn singleton(initial: AppServerSession) -> Self {
        Self {
            initial,
            others: Vec::new(),
        }
    }

    #[allow(
        dead_code,
        reason = "used by the under-development backend registration path"
    )]
    pub(crate) fn multiple(initial: AppServerSession, others: Vec<AppServerSession>) -> Self {
        assert!(
            others
                .iter()
                .all(|session| session.backend_id() != initial.backend_id())
        );
        Self { initial, others }
    }

    pub(super) fn into_parts(self) -> (AppServerSession, Vec<AppServerSession>) {
        (self.initial, self.others)
    }
}

struct SessionEntry {
    session: AppServerSession,
    listening: bool,
    pending_requests: VecDeque<AppServerSessionEvent>,
}

/// A session's stable position in an append-only backend collection.
#[derive(Clone, Copy)]
pub(super) struct SessionSlot(usize);

pub(super) struct BackendSessions {
    entries: Vec<SessionEntry>,
    pub(super) displayed: SessionSlot,
    pub(super) settings: HashMap<BackendId, BackendSettings>,
    next_source: usize,
}

impl BackendSessions {
    pub(super) fn from_initial(initial: AppServerSession, others: Vec<AppServerSession>) -> Self {
        let mut sessions = Self::new(initial);
        for session in others {
            sessions.insert(session);
        }
        sessions
    }

    pub(super) fn new(session: AppServerSession) -> Self {
        let mut sessions = Self {
            entries: Vec::new(),
            displayed: SessionSlot(0),
            settings: HashMap::new(),
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
            pending_requests: VecDeque::new(),
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

    pub(super) fn displayed_owner(&self) -> BackendId {
        self.session_at(self.displayed).backend_id()
    }

    pub(super) fn picker_views(&self) -> Vec<AppServerSession> {
        let displayed_owner = self.displayed_owner();
        self.entries
            .iter()
            .filter(|entry| entry.listening || entry.session.backend_id() == displayed_owner)
            .map(|entry| entry.session.picker_view())
            .collect()
    }

    pub(super) fn session_at_mut(&mut self, slot: SessionSlot) -> &mut AppServerSession {
        &mut self.entries[slot.0].session
    }

    pub(super) fn listening_slot(&self, owner: BackendId) -> Option<SessionSlot> {
        self.slot(owner)
            .filter(|slot| self.entries[slot.0].listening)
    }

    pub(super) fn is_listening(&self, owner: BackendId) -> bool {
        self.listening_slot(owner).is_some()
    }

    pub(super) fn resume_events(&mut self, owner: BackendId) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.session.backend_id() == owner)
        {
            entry.listening = true;
            entry.pending_requests.clear();
        }
    }

    pub(super) async fn next_event(
        &mut self,
        paused_backend: Option<BackendId>,
    ) -> (BackendId, Option<AppServerSessionEvent>) {
        let request_owner = self.displayed_owner();
        std::future::poll_fn(|cx| {
            for offset in 0..self.entries.len() {
                let index = (self.next_source + offset) % self.entries.len();
                let entry = &mut self.entries[index];
                let owner = entry.session.backend_id();
                if !entry.listening || paused_backend == Some(owner) {
                    continue;
                }
                let event = if owner == request_owner
                    && let Some(event) = entry.pending_requests.pop_front()
                {
                    Some(event)
                } else {
                    // Both transports have cancellation-safe event queues.
                    let Poll::Ready(event) = std::pin::pin!(entry.session.next_event()).poll(cx)
                    else {
                        continue;
                    };
                    event
                };
                let event = match event {
                    Some(event)
                        if owner != request_owner
                            && matches!(&event.event, AppServerEvent::ServerRequest(_)) =>
                    {
                        entry.pending_requests.push_back(event);
                        cx.waker().wake_by_ref();
                        continue;
                    }
                    event => event,
                };
                if let Some(AppServerSessionEvent { event: AppServerEvent::ServerNotification(notification), .. }) = &event
                    && let ServerNotification::ServerRequestResolved(resolved) = notification.as_ref()
                {
                    entry.pending_requests.retain(|pending| !matches!(
                        &pending.event, AppServerEvent::ServerRequest(request) if request.id() == &resolved.request_id
                    ));
                }
                if event
                    .as_ref()
                    .is_none_or(|event| matches!(event.event, AppServerEvent::Disconnected { .. }))
                {
                    entry.pending_requests.clear();
                }
                entry.listening = event.is_some();
                self.next_source = (index + 1) % self.entries.len();
                return Poll::Ready((owner, event));
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
