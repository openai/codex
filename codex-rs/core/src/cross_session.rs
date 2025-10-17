use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::RwLock;
use std::sync::RwLockReadGuard;
use std::sync::RwLockWriteGuard;
use std::time::Duration;

use futures::Stream;
use serde_json::Value;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::broadcast;
use tokio::sync::oneshot;
use tokio::time;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tracing::debug;
use tracing::error;

use crate::codex_conversation::CodexConversation;
use crate::config::Config;
use crate::error::CodexErr;
use crate::protocol::AgentMessageEvent;
use crate::protocol::AskForApproval;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::InputItem;
use crate::protocol::Op;
use crate::protocol::SandboxPolicy;
use crate::protocol_config_types::ReasoningEffort as ReasoningEffortConfig;
use crate::protocol_config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::ConversationId;

/// Default capacity for broadcast channels that fan out session events.
const EVENT_BUFFER_LEN: usize = 256;

/// Encapsulates the defaults needed to submit a new `Op::UserTurn`.
#[derive(Debug, Clone)]
pub struct SessionDefaults {
    pub cwd: PathBuf,
    pub approval_policy: AskForApproval,
    pub sandbox_policy: SandboxPolicy,
    pub model: String,
    pub effort: Option<ReasoningEffortConfig>,
    pub summary: ReasoningSummaryConfig,
}

impl SessionDefaults {
    pub fn from_config(config: &Config) -> Self {
        Self {
            cwd: config.cwd.clone(),
            approval_policy: config.approval_policy,
            sandbox_policy: config.sandbox_policy.clone(),
            model: config.model.clone(),
            effort: config.model_reasoning_effort,
            summary: config.model_reasoning_summary,
        }
    }
}

/// Request payload for posting a user turn to a session.
#[derive(Debug, Clone)]
pub struct PostUserTurnRequest {
    pub target: RoleOrId,
    pub text: String,
    pub final_output_json_schema: Option<Value>,
}

/// Identifier used when targeting sessions for cross-session routing.
#[derive(Debug, Clone)]
pub enum RoleOrId {
    Session(ConversationId),
    RunRole { run_id: String, role: String },
}

/// Handle returned by [`CrossSessionHub::post_user_turn`].
pub struct TurnHandle {
    conversation_id: ConversationId,
    submission_id: String,
    receiver: TokioMutex<Option<oneshot::Receiver<AssistantMessage>>>,
}

impl TurnHandle {
    pub fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }

    pub fn submission_id(&self) -> &str {
        &self.submission_id
    }
}

impl fmt::Debug for TurnHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurnHandle")
            .field("conversation_id", &self.conversation_id)
            .field("submission_id", &self.submission_id)
            .finish()
    }
}

/// First assistant message emitted for a bridged turn.
#[derive(Debug, Clone)]
pub struct AssistantMessage {
    pub conversation_id: ConversationId,
    pub submission_id: String,
    pub message: AgentMessageEvent,
}

/// Wrapper around a session event tagged with its conversation id.
#[derive(Debug, Clone)]
pub struct SessionEvent {
    pub conversation_id: ConversationId,
    pub event: Event,
}

/// Stream of [`SessionEvent`] instances for a particular session.
pub struct SessionEventStream {
    inner: BroadcastStream<SessionEvent>,
}

impl SessionEventStream {
    fn new(receiver: broadcast::Receiver<SessionEvent>) -> Self {
        Self {
            inner: BroadcastStream::new(receiver),
        }
    }
}

impl Stream for SessionEventStream {
    type Item = SessionEvent;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            match Pin::new(&mut self.inner).poll_next(cx) {
                std::task::Poll::Ready(Some(Ok(event))) => {
                    return std::task::Poll::Ready(Some(event));
                }
                std::task::Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(_)))) => continue,
                std::task::Poll::Ready(None) => return std::task::Poll::Ready(None),
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }
    }
}

#[derive(Clone)]
struct RoleKey {
    run_id: Arc<str>,
    role: Arc<str>,
}

impl RoleKey {
    fn new(run_id: String, role: String) -> Self {
        Self {
            run_id: Arc::<str>::from(run_id),
            role: Arc::<str>::from(role),
        }
    }
}

impl PartialEq for RoleKey {
    fn eq(&self, other: &Self) -> bool {
        self.run_id.as_ref() == other.run_id.as_ref() && self.role.as_ref() == other.role.as_ref()
    }
}

impl Eq for RoleKey {}

impl std::hash::Hash for RoleKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(self.run_id.as_ref(), state);
        std::hash::Hash::hash(self.role.as_ref(), state);
    }
}

struct SessionEntry {
    conversation_id: ConversationId,
    conversation: Arc<CodexConversation>,
    defaults: SessionDefaults,
    role_key: Option<RoleKey>,
    event_tx: broadcast::Sender<SessionEvent>,
    turn_watchers: TokioMutex<HashMap<String, oneshot::Sender<AssistantMessage>>>,
    pending_messages: TokioMutex<HashMap<String, AssistantMessage>>,
    shutdown_tx: StdMutex<Option<oneshot::Sender<()>>>,
}

impl SessionEntry {
    fn new(
        conversation_id: ConversationId,
        conversation: Arc<CodexConversation>,
        defaults: SessionDefaults,
        role_key: Option<RoleKey>,
        event_tx: broadcast::Sender<SessionEvent>,
        shutdown_tx: oneshot::Sender<()>,
    ) -> Self {
        Self {
            conversation_id,
            conversation,
            defaults,
            role_key,
            event_tx,
            turn_watchers: TokioMutex::new(HashMap::new()),
            pending_messages: TokioMutex::new(HashMap::new()),
            shutdown_tx: StdMutex::new(Some(shutdown_tx)),
        }
    }

    async fn register_waiter(
        &self,
        submission_id: String,
        sender: oneshot::Sender<AssistantMessage>,
    ) {
        {
            let mut watchers = self.turn_watchers.lock().await;
            if let Some(message) = {
                let mut pending = self.pending_messages.lock().await;
                pending.remove(&submission_id)
            } {
                drop(watchers);
                let _ = sender.send(message);
                return;
            }
            watchers.insert(submission_id, sender);
        }
    }

    async fn notify_assistant_message(&self, message: AssistantMessage) {
        let submission_id = message.submission_id.clone();
        let sender_opt = {
            let mut watchers = self.turn_watchers.lock().await;
            watchers.remove(&submission_id)
        };

        if let Some(sender) = sender_opt {
            let _ = sender.send(message);
        } else {
            let mut pending = self.pending_messages.lock().await;
            pending.entry(submission_id).or_insert(message);
        }
    }

    fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.event_tx.subscribe()
    }

    fn close(&self) {
        if let Ok(mut guard) = self.shutdown_tx.lock()
            && let Some(tx) = guard.take()
        {
            let _ = tx.send(());
        }
    }

    fn role_key(&self) -> Option<RoleKey> {
        self.role_key.clone()
    }
}

/// Input for registering a session with the hub.
pub struct SessionRegistration {
    pub conversation_id: ConversationId,
    pub conversation: Arc<CodexConversation>,
    pub defaults: SessionDefaults,
    pub run_id: Option<String>,
    pub role: Option<String>,
}

/// Guard that unregisters the session on drop.
pub struct RegisteredSession {
    inner: Arc<Inner>,
    conversation_id: ConversationId,
}

impl RegisteredSession {
    pub fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }
}

impl Drop for RegisteredSession {
    fn drop(&mut self) {
        self.inner.unregister(self.conversation_id);
    }
}

#[derive(Default)]
struct Inner {
    sessions: RwLock<HashMap<ConversationId, Arc<SessionEntry>>>,
    roles: RwLock<HashMap<RoleKey, ConversationId>>,
}

impl Inner {
    fn sessions_read(
        &self,
    ) -> Result<RwLockReadGuard<'_, HashMap<ConversationId, Arc<SessionEntry>>>, CrossSessionError>
    {
        self.sessions
            .read()
            .map_err(|_| CrossSessionError::LockPoisoned("sessions"))
    }

    fn sessions_write(
        &self,
    ) -> Result<RwLockWriteGuard<'_, HashMap<ConversationId, Arc<SessionEntry>>>, CrossSessionError>
    {
        self.sessions
            .write()
            .map_err(|_| CrossSessionError::LockPoisoned("sessions"))
    }

    fn roles_read(
        &self,
    ) -> Result<RwLockReadGuard<'_, HashMap<RoleKey, ConversationId>>, CrossSessionError> {
        self.roles
            .read()
            .map_err(|_| CrossSessionError::LockPoisoned("roles"))
    }

    fn roles_write(
        &self,
    ) -> Result<RwLockWriteGuard<'_, HashMap<RoleKey, ConversationId>>, CrossSessionError> {
        self.roles
            .write()
            .map_err(|_| CrossSessionError::LockPoisoned("roles"))
    }

    fn insert(&self, entry: Arc<SessionEntry>) -> Result<(), CrossSessionError> {
        {
            let mut sessions = self.sessions_write()?;
            if sessions
                .insert(entry.conversation_id, entry.clone())
                .is_some()
            {
                return Err(CrossSessionError::SessionAlreadyRegistered(
                    entry.conversation_id,
                ));
            }
        }

        if let Some(role_key) = entry.role_key() {
            let mut roles = self.roles_write()?;
            if roles.contains_key(&role_key) {
                self.sessions_write()?.remove(&entry.conversation_id);
                return Err(CrossSessionError::RoleAlreadyRegistered {
                    run_id: role_key.run_id.to_string(),
                    role: role_key.role.to_string(),
                });
            }
            roles.insert(role_key, entry.conversation_id);
        }

        Ok(())
    }

    fn unregister(&self, conversation_id: ConversationId) {
        if let Some(entry) = self.remove_internal(conversation_id) {
            entry.close();
        }
    }

    fn remove_internal(&self, conversation_id: ConversationId) -> Option<Arc<SessionEntry>> {
        let (entry, role_key) = {
            let mut sessions = self.sessions.write().ok()?;
            let entry = sessions.remove(&conversation_id)?;
            let role_key = entry.role_key();
            (entry, role_key)
        };

        if let Some(role_key) = role_key
            && let Ok(mut roles) = self.roles.write()
        {
            roles.remove(&role_key);
        }

        Some(entry)
    }

    fn resolve_session(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Arc<SessionEntry>, CrossSessionError> {
        self.sessions_read()?
            .get(&conversation_id)
            .cloned()
            .ok_or(CrossSessionError::SessionNotFound(conversation_id))
    }

    fn resolve_target(&self, target: &RoleOrId) -> Result<Arc<SessionEntry>, CrossSessionError> {
        match target {
            RoleOrId::Session(id) => self.resolve_session(*id),
            RoleOrId::RunRole { run_id, role } => {
                let conversation_id = {
                    let roles = self.roles_read()?;
                    let key = RoleKey::new(run_id.clone(), role.clone());
                    roles
                        .get(&key)
                        .copied()
                        .ok_or_else(|| CrossSessionError::RoleNotFound {
                            run_id: run_id.clone(),
                            role: role.clone(),
                        })?
                };
                self.resolve_session(conversation_id)
            }
        }
    }
}

/// Cross-session coordination hub.
#[derive(Default, Clone)]
pub struct CrossSessionHub {
    inner: Arc<Inner>,
}

impl CrossSessionHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_session(
        &self,
        registration: SessionRegistration,
    ) -> Result<RegisteredSession, CrossSessionError> {
        let SessionRegistration {
            conversation_id,
            conversation,
            defaults,
            run_id,
            role,
        } = registration;

        let role_key = match (run_id, role) {
            (Some(run_id), Some(role)) => Some(RoleKey::new(run_id, role)),
            (None, None) => None,
            _ => {
                return Err(CrossSessionError::IncompleteRoleRegistration);
            }
        };

        let (event_tx, _) = broadcast::channel(EVENT_BUFFER_LEN);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let entry = Arc::new(SessionEntry::new(
            conversation_id,
            Arc::clone(&conversation),
            defaults,
            role_key,
            event_tx,
            shutdown_tx,
        ));

        self.inner.insert(entry.clone())?;

        self.spawn_event_forwarder(entry, conversation, shutdown_rx);

        Ok(RegisteredSession {
            inner: Arc::clone(&self.inner),
            conversation_id,
        })
    }

    pub async fn post_user_turn(
        &self,
        request: PostUserTurnRequest,
    ) -> Result<TurnHandle, CrossSessionError> {
        let entry = self.inner.resolve_target(&request.target)?;

        let items = vec![InputItem::Text { text: request.text }];

        let defaults = &entry.defaults;
        let submission_id = entry
            .conversation
            .submit(Op::UserTurn {
                items,
                cwd: defaults.cwd.clone(),
                approval_policy: defaults.approval_policy,
                sandbox_policy: defaults.sandbox_policy.clone(),
                model: defaults.model.clone(),
                effort: defaults.effort,
                summary: defaults.summary,
                final_output_json_schema: request.final_output_json_schema,
            })
            .await
            .map_err(CrossSessionError::from)?;

        let (tx, rx) = oneshot::channel();

        entry.register_waiter(submission_id.clone(), tx).await;

        Ok(TurnHandle {
            conversation_id: entry.conversation_id,
            submission_id,
            receiver: TokioMutex::new(Some(rx)),
        })
    }

    pub async fn await_first_assistant(
        &self,
        handle: &TurnHandle,
        timeout: Duration,
    ) -> Result<AssistantMessage, CrossSessionError> {
        let receiver = {
            let mut guard = handle.receiver.lock().await;
            guard.take().ok_or(CrossSessionError::TurnHandleConsumed)?
        };

        match time::timeout(timeout, receiver).await {
            Ok(Ok(message)) => Ok(message),
            Ok(Err(_)) => Err(CrossSessionError::SessionClosed),
            Err(_) => Err(CrossSessionError::AwaitTimeout(timeout)),
        }
    }

    pub fn stream_events(
        &self,
        conversation_id: ConversationId,
    ) -> Result<SessionEventStream, CrossSessionError> {
        let entry = self.inner.resolve_session(conversation_id)?;
        Ok(SessionEventStream::new(entry.subscribe()))
    }

    fn spawn_event_forwarder(
        &self,
        entry: Arc<SessionEntry>,
        conversation: Arc<CodexConversation>,
        mut shutdown_rx: oneshot::Receiver<()>,
    ) {
        let conversation_id = entry.conversation_id;
        let event_tx = entry.event_tx.clone();
        let inner = Arc::clone(&self.inner);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        debug!("CrossSessionHub received shutdown for session {conversation_id}");
                        break;
                    }
                    event = conversation.next_event() => {
                        match event {
                            Ok(event) => {
                                if let EventMsg::AgentMessage(agent_message) = &event.msg {
                                    let message = AssistantMessage {
                                        conversation_id,
                                        submission_id: event.id.clone(),
                                        message: agent_message.clone(),
                                    };
                                    entry.notify_assistant_message(message).await;
                                }

                                if let Err(err) = event_tx.send(SessionEvent {
                                    conversation_id,
                                    event: event.clone(),
                                }) {
                                    debug!(
                                        "CrossSessionHub dropped event for session {conversation_id}: {err}"
                                    );
                                }

                                if matches!(event.msg, EventMsg::ShutdownComplete) {
                                    break;
                                }
                            }
                            Err(err) => {
                                error!("CrossSessionHub event loop error for session {conversation_id}: {err:#?}");
                                break;
                            }
                        }
                    }
                }
            }

            inner.unregister(conversation_id);
        });
    }
}

/// Errors surfaced by cross-session orchestration.
#[derive(thiserror::Error, Debug)]
pub enum CrossSessionError {
    #[error("session {0} is already registered with the hub")]
    SessionAlreadyRegistered(ConversationId),
    #[error("run {run_id} already has a {role} session registered")]
    RoleAlreadyRegistered { run_id: String, role: String },
    #[error("session {0} does not exist")]
    SessionNotFound(ConversationId),
    #[error("no session registered for run {run_id} role {role}")]
    RoleNotFound { run_id: String, role: String },
    #[error("session role registration must set both run_id and role")]
    IncompleteRoleRegistration,
    #[error("turn handle has already been awaited")]
    TurnHandleConsumed,
    #[error("session closed before an assistant message was emitted")]
    SessionClosed,
    #[error("timed out waiting {0:?} for assistant response")]
    AwaitTimeout(Duration),
    #[error("internal lock poisoned: {0}")]
    LockPoisoned(&'static str),
    #[error("submit failed: {0}")]
    SubmitFailed(#[from] CodexErr),
}
