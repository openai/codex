use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_app_server_protocol::JSONRPCErrorError;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::rpc::RpcNotificationSender;
use crate::rpc::invalid_request;
use crate::server::process_handler::ProcessHandler;

#[cfg(test)]
const DETACHED_SESSION_TTL: Duration = Duration::from_millis(200);
#[cfg(not(test))]
const DETACHED_SESSION_TTL: Duration = Duration::from_secs(10);

#[derive(Clone, Default)]
pub(crate) struct SessionRegistry {
    inner: Arc<SessionRegistryInner>,
}

#[derive(Default)]
struct SessionRegistryInner {
    sessions: Mutex<HashMap<String, Arc<SessionEntry>>>,
    next_attachment_id: AtomicU64,
}

struct SessionEntry {
    session_id: String,
    process: ProcessHandler,
    current_attachment_id: AtomicU64,
    detached_attachment_id: AtomicU64,
    detached_expires_at: StdMutex<Option<tokio::time::Instant>>,
}

#[derive(Clone)]
pub(crate) struct SessionHandle {
    registry: SessionRegistry,
    entry: Arc<SessionEntry>,
    attachment_id: u64,
}

impl SessionRegistry {
    pub(crate) async fn attach(
        &self,
        resume_session_id: Option<String>,
        notifications: RpcNotificationSender,
    ) -> Result<SessionHandle, JSONRPCErrorError> {
        let attachment_id = self.inner.next_attachment_id.fetch_add(1, Ordering::SeqCst) + 1;
        let mut expired_entry = None;
        let mut expired_error = None;
        let entry = {
            let mut sessions = self.inner.sessions.lock().await;
            let entry = if let Some(session_id) = resume_session_id {
                let entry = sessions
                    .get(&session_id)
                    .cloned()
                    .ok_or_else(|| invalid_request(format!("unknown session id {session_id}")))?;
                let expired = entry
                    .detached_expires_at
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .is_some_and(|deadline| tokio::time::Instant::now() >= deadline);
                if expired {
                    expired_error =
                        Some(invalid_request(format!("unknown session id {session_id}")));
                    expired_entry = sessions.remove(&session_id);
                    None
                } else {
                    Some(entry)
                }
            } else {
                let session_id = Uuid::new_v4().to_string();
                let entry = Arc::new(SessionEntry {
                    session_id: session_id.clone(),
                    process: ProcessHandler::new(notifications.clone()),
                    current_attachment_id: AtomicU64::new(0),
                    detached_attachment_id: AtomicU64::new(0),
                    detached_expires_at: StdMutex::new(None),
                });
                sessions.insert(session_id, Arc::clone(&entry));
                Some(entry)
            };

            if let Some(entry) = entry.as_ref() {
                entry.process.set_notification_sender(Some(notifications));
                entry
                    .current_attachment_id
                    .store(attachment_id, Ordering::SeqCst);
                entry.detached_attachment_id.store(0, Ordering::SeqCst);
                *entry
                    .detached_expires_at
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            }

            entry
        };

        if let Some(entry) = expired_entry {
            entry.process.shutdown().await;
            return Err(
                expired_error.unwrap_or_else(|| invalid_request("unknown session id".to_string()))
            );
        }
        let Some(entry) = entry else {
            return Err(invalid_request("unknown session id".to_string()));
        };

        Ok(SessionHandle {
            registry: self.clone(),
            entry,
            attachment_id,
        })
    }

    async fn expire_if_detached(&self, session_id: String, attachment_id: u64) {
        tokio::time::sleep(DETACHED_SESSION_TTL).await;

        let removed = {
            let mut sessions = self.inner.sessions.lock().await;
            let Some(entry) = sessions.get(&session_id) else {
                return;
            };
            if entry.current_attachment_id.load(Ordering::SeqCst) != 0
                || entry.detached_attachment_id.load(Ordering::SeqCst) != attachment_id
            {
                return;
            }
            if entry
                .detached_expires_at
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some_and(|deadline| tokio::time::Instant::now() < deadline)
            {
                return;
            }
            sessions.remove(&session_id)
        };

        if let Some(entry) = removed {
            entry.process.shutdown().await;
        }
    }
}

impl SessionHandle {
    pub(crate) fn session_id(&self) -> &str {
        &self.entry.session_id
    }

    pub(crate) fn is_current_attachment(&self) -> bool {
        self.entry.current_attachment_id.load(Ordering::SeqCst) == self.attachment_id
    }

    pub(crate) fn process(&self) -> &ProcessHandler {
        &self.entry.process
    }

    pub(crate) async fn detach(&self) {
        if self
            .entry
            .current_attachment_id
            .compare_exchange(self.attachment_id, 0, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        self.entry
            .detached_attachment_id
            .store(self.attachment_id, Ordering::SeqCst);
        *self
            .entry
            .detached_expires_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(tokio::time::Instant::now() + DETACHED_SESSION_TTL);
        self.entry
            .process
            .set_notification_sender(/*notifications*/ None);

        let registry = self.registry.clone();
        let session_id = self.entry.session_id.clone();
        let attachment_id = self.attachment_id;
        tokio::spawn(async move {
            registry.expire_if_detached(session_id, attachment_id).await;
        });
    }
}
