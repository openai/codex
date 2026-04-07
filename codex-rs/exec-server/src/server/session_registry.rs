use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
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

pub(crate) struct SessionRegistry {
    sessions: Mutex<HashMap<String, Arc<SessionEntry>>>,
}

struct SessionEntry {
    session_id: String,
    process: ProcessHandler,
    attachment: StdMutex<AttachmentState>,
}

struct AttachmentState {
    current_connection_id: Option<ConnectionId>,
    detached_connection_id: Option<ConnectionId>,
    detached_expires_at: Option<tokio::time::Instant>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConnectionId(Uuid);

impl std::fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone)]
pub(crate) struct SessionHandle {
    registry: Arc<SessionRegistry>,
    entry: Arc<SessionEntry>,
    connection_id: ConnectionId,
}

impl SessionRegistry {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            sessions: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) async fn attach(
        self: &Arc<Self>,
        resume_session_id: Option<String>,
        notifications: RpcNotificationSender,
    ) -> Result<SessionHandle, JSONRPCErrorError> {
        let connection_id = ConnectionId(Uuid::new_v4());
        let mut expired_entry = None;
        let mut expired_error = None;
        let entry = {
            let mut sessions = self.sessions.lock().await;
            let entry = if let Some(session_id) = resume_session_id {
                let entry = sessions
                    .get(&session_id)
                    .cloned()
                    .ok_or_else(|| invalid_request(format!("unknown session id {session_id}")))?;
                if entry.is_expired(tokio::time::Instant::now()) {
                    expired_error =
                        Some(invalid_request(format!("unknown session id {session_id}")));
                    expired_entry = sessions.remove(&session_id);
                    None
                } else if entry.has_active_connection() {
                    return Err(invalid_request(format!(
                        "session {session_id} is already attached to another connection"
                    )));
                } else {
                    Some(entry)
                }
            } else {
                let session_id = Uuid::new_v4().to_string();
                let entry = Arc::new(SessionEntry {
                    session_id: session_id.clone(),
                    process: ProcessHandler::new(notifications.clone()),
                    attachment: StdMutex::new(AttachmentState {
                        current_connection_id: Some(connection_id),
                        detached_connection_id: None,
                        detached_expires_at: None,
                    }),
                });
                sessions.insert(session_id, Arc::clone(&entry));
                Some(entry)
            };

            if let Some(entry) = entry.as_ref() {
                entry.process.set_notification_sender(Some(notifications));
                entry.attach(connection_id);
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
            registry: Arc::clone(self),
            entry,
            connection_id,
        })
    }

    async fn expire_if_detached(&self, session_id: String, connection_id: ConnectionId) {
        tokio::time::sleep(DETACHED_SESSION_TTL).await;

        let removed = {
            let mut sessions = self.sessions.lock().await;
            let Some(entry) = sessions.get(&session_id) else {
                return;
            };
            if !entry.is_detached_connection_expired(connection_id, tokio::time::Instant::now()) {
                return;
            }
            sessions.remove(&session_id)
        };

        if let Some(entry) = removed {
            entry.process.shutdown().await;
        }
    }
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

impl SessionEntry {
    fn attach(&self, connection_id: ConnectionId) {
        let mut attachment = self
            .attachment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        attachment.current_connection_id = Some(connection_id);
        attachment.detached_connection_id = None;
        attachment.detached_expires_at = None;
    }

    fn detach(&self, connection_id: ConnectionId) -> bool {
        let mut attachment = self
            .attachment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if attachment.current_connection_id != Some(connection_id) {
            return false;
        }

        attachment.current_connection_id = None;
        attachment.detached_connection_id = Some(connection_id);
        attachment.detached_expires_at = Some(tokio::time::Instant::now() + DETACHED_SESSION_TTL);
        true
    }

    fn has_active_connection(&self) -> bool {
        self.attachment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .current_connection_id
            .is_some()
    }

    fn is_attached_to(&self, connection_id: ConnectionId) -> bool {
        self.attachment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .current_connection_id
            == Some(connection_id)
    }

    fn is_expired(&self, now: tokio::time::Instant) -> bool {
        self.attachment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .detached_expires_at
            .is_some_and(|deadline| now >= deadline)
    }

    fn is_detached_connection_expired(
        &self,
        connection_id: ConnectionId,
        now: tokio::time::Instant,
    ) -> bool {
        let attachment = self
            .attachment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        attachment.current_connection_id.is_none()
            && attachment.detached_connection_id == Some(connection_id)
            && attachment
                .detached_expires_at
                .is_some_and(|deadline| now >= deadline)
    }
}

impl SessionHandle {
    pub(crate) fn session_id(&self) -> &str {
        &self.entry.session_id
    }

    pub(crate) fn connection_id(&self) -> String {
        self.connection_id.to_string()
    }

    pub(crate) fn is_session_attached(&self) -> bool {
        self.entry.is_attached_to(self.connection_id)
    }

    pub(crate) fn process(&self) -> &ProcessHandler {
        &self.entry.process
    }

    pub(crate) async fn detach(&self) {
        if !self.entry.detach(self.connection_id) {
            return;
        }

        self.entry
            .process
            .set_notification_sender(/*notifications*/ None);

        let registry = Arc::clone(&self.registry);
        let session_id = self.entry.session_id.clone();
        let connection_id = self.connection_id;
        tokio::spawn(async move {
            registry.expire_if_detached(session_id, connection_id).await;
        });
    }
}
