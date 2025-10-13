use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use codex_protocol::ConversationId;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InputItem;
use codex_protocol::protocol::TaskCompleteEvent;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::codex::Codex;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::model_family::find_family_for_model;
use crate::protocol::AskForApproval;
use crate::protocol::InitialHistory;
use crate::protocol::Op;
use crate::protocol::SessionSource;
use crate::subsessions::error::SubsessionError;
use crate::subsessions::profile::SessionType;
use crate::subsessions::profile::SubsessionProfile;

pub(crate) type ChildResult = Result<Option<String>, SubsessionError>;

#[derive(Debug)]
enum ChildStatus {
    Pending,
    Done(Option<String>),
    Failed(SubsessionError),
    Cancelled,
}

struct ChildRecord {
    status: Mutex<ChildStatus>,
    notify: Notify,
    cancel_tx: watch::Sender<bool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl ChildRecord {
    fn new(cancel_tx: watch::Sender<bool>) -> Self {
        Self {
            status: Mutex::new(ChildStatus::Pending),
            notify: Notify::new(),
            cancel_tx,
            handle: Mutex::new(None),
        }
    }

    async fn update(&self, status: ChildStatus) {
        let mut guard = self.status.lock().await;
        *guard = status;
        self.notify.notify_waiters();
    }

    async fn status(&self) -> ChildStatus {
        let guard = self.status.lock().await;
        match &*guard {
            ChildStatus::Pending => ChildStatus::Pending,
            ChildStatus::Done(value) => ChildStatus::Done(value.clone()),
            ChildStatus::Failed(err) => ChildStatus::Failed(err.clone()),
            ChildStatus::Cancelled => ChildStatus::Cancelled,
        }
    }

    async fn set_handle(&self, handle: JoinHandle<()>) {
        let mut guard = self.handle.lock().await;
        *guard = Some(handle);
    }

    async fn send_cancel(&self) {
        if self.cancel_tx.send(true).is_err() {
            warn!("subsession cancellation receiver already dropped");
        }
        let mut guard = self.handle.lock().await;
        let _ = guard.take();
        // Drop the handle so the task can observe the cancellation signal and shut down cleanly.
    }
}

pub(crate) struct SubsessionManager {
    children: Mutex<HashMap<ConversationId, Arc<ChildRecord>>>,
}

impl SubsessionManager {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            children: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) async fn spawn_child(
        self: &Arc<Self>,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        session_type: SessionType,
        prompt: String,
    ) -> Result<ConversationId, SubsessionError> {
        let profile = SubsessionProfile::for_session_type(session_type);
        let parent_config = turn.client.get_config();
        let child_config = build_child_config(parent_config.as_ref(), turn.as_ref(), &profile);
        let model_name = child_config.model.clone();

        let auth_manager = turn
            .client
            .get_auth_manager()
            .ok_or(SubsessionError::MissingAuthManager)?;

        let (cancel_tx, cancel_rx) = watch::channel(false);
        let manager = Arc::clone(self);

        let spawn_result = Codex::spawn(
            child_config,
            auth_manager,
            InitialHistory::New,
            SessionSource::Exec,
        )
        .await
        .map_err(|err| SubsessionError::SpawnFailed {
            message: format!("{err:#}"),
        })?;

        let conversation_id = spawn_result.conversation_id;
        let codex = spawn_result.codex;
        let record = Arc::new(ChildRecord::new(cancel_tx));

        {
            let mut guard = manager.children.lock().await;
            guard.insert(conversation_id, Arc::clone(&record));
        }

        session
            .notify_background_event(
                "subsessions",
                format!("spawned child session {conversation_id} with model {model_name}"),
            )
            .await;

        let driver_session = Arc::clone(&session);
        let driver_conversation_id = conversation_id;
        let handle = tokio::spawn(async move {
            let result = run_child_conversation(
                driver_session.clone(),
                codex,
                driver_conversation_id,
                prompt,
                cancel_rx,
            )
            .await;

            let status = match result {
                Ok(value) => ChildStatus::Done(value),
                Err(err) => match err {
                    SubsessionError::Cancelled { .. } => ChildStatus::Cancelled,
                    other => ChildStatus::Failed(other),
                },
            };

            manager.finish_child(conversation_id, status).await;
        });

        record.set_handle(handle).await;

        Ok(conversation_id)
    }

    pub(crate) async fn wait_child(
        &self,
        conversation_id: &ConversationId,
        timeout: Option<Duration>,
    ) -> Result<Option<String>, SubsessionError> {
        let record = self
            .lookup(conversation_id)
            .await
            .ok_or_else(|| SubsessionError::unknown(conversation_id))?;

        let mut status = record.status().await;
        if matches!(status, ChildStatus::Pending) {
            match timeout {
                Some(duration) if duration == Duration::ZERO => {
                    return Err(SubsessionError::pending(conversation_id));
                }
                Some(duration) => {
                    let notified = tokio::time::timeout(duration, record.notify.notified()).await;
                    if notified.is_err() {
                        return Err(SubsessionError::timeout(
                            conversation_id,
                            duration.as_millis() as u64,
                        ));
                    }
                }
                None => record.notify.notified().await,
            }
            status = record.status().await;
        }

        match status {
            ChildStatus::Pending => Err(SubsessionError::pending(conversation_id)),
            ChildStatus::Done(result) => Ok(result),
            ChildStatus::Cancelled => Err(SubsessionError::cancelled(conversation_id)),
            ChildStatus::Failed(err) => Err(err),
        }
    }

    pub(crate) async fn cancel_child(
        &self,
        conversation_id: &ConversationId,
        session: Arc<Session>,
    ) -> Result<bool, SubsessionError> {
        let record = match self.lookup(conversation_id).await {
            Some(rec) => rec,
            None => return Err(SubsessionError::unknown(conversation_id)),
        };

        match record.status().await {
            ChildStatus::Pending => {
                record.send_cancel().await;
                record.update(ChildStatus::Cancelled).await;
                session
                    .notify_background_event(
                        "subsessions",
                        format!("child session {conversation_id} cancelled"),
                    )
                    .await;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub(crate) async fn abort_all_children(&self, session: Arc<Session>) {
        if self.cancel_pending_children().await {
            session
                .notify_background_event("subsessions", "aborted all child sessions")
                .await;
        }
    }

    async fn finish_child(&self, conversation_id: ConversationId, status: ChildStatus) {
        if let Some(record) = self.lookup(&conversation_id).await {
            record.update(status).await;
        } else {
            warn!(%conversation_id, "dropping result for unknown child session");
        }
    }

    async fn lookup(&self, conversation_id: &ConversationId) -> Option<Arc<ChildRecord>> {
        let guard = self.children.lock().await;
        guard.get(conversation_id).cloned()
    }

    async fn cancel_pending_children(&self) -> bool {
        let children = {
            let guard = self.children.lock().await;
            guard.values().cloned().collect::<Vec<_>>()
        };
        let mut cancelled_any = false;
        for child in children {
            if matches!(child.status().await, ChildStatus::Pending) {
                child.send_cancel().await;
                child.update(ChildStatus::Cancelled).await;
                cancelled_any = true;
            }
        }
        cancelled_any
    }
}

async fn run_child_conversation(
    session: Arc<Session>,
    mut codex: Codex,
    conversation_id: ConversationId,
    prompt: String,
    mut cancel_rx: watch::Receiver<bool>,
) -> ChildResult {
    let submit_input = Op::UserInput {
        items: vec![InputItem::Text {
            text: prompt.clone(),
        }],
    };

    if let Err(err) = codex.submit(submit_input).await {
        return Err(SubsessionError::SpawnFailed {
            message: format!("failed to submit child input: {err:#}"),
        });
    }

    let mut last_agent_message: Option<String> = None;
    loop {
        tokio::select! {
            changed = cancel_rx.changed() => {
                if changed.is_ok() && *cancel_rx.borrow() {
                    let _ = codex.submit(Op::Shutdown).await;
                    return Err(SubsessionError::cancelled(&conversation_id));
                }
            }
            event = codex.next_event() => {
                let event = event.map_err(|err| SubsessionError::SpawnFailed {
                    message: format!("child session stream error: {err:#}"),
                })?;
                match handle_child_event(
                    &session,
                    &mut codex,
                    &conversation_id,
                    event,
                    &mut last_agent_message,
                )
                .await? {
                    EventProgress::Continue => continue,
                    EventProgress::Completed => return Ok(last_agent_message.clone()),
                }
            }
        }
    }
}

enum EventProgress {
    Continue,
    Completed,
}

async fn handle_child_event(
    session: &Arc<Session>,
    codex: &mut Codex,
    conversation_id: &ConversationId,
    event: Event,
    last_agent_message: &mut Option<String>,
) -> Result<EventProgress, SubsessionError> {
    match event.msg {
        EventMsg::AgentMessage(AgentMessageEvent { message }) => {
            *last_agent_message = Some(message);
            Ok(EventProgress::Continue)
        }
        EventMsg::TaskComplete(TaskCompleteEvent {
            last_agent_message: msg,
        }) => {
            if msg.is_some() {
                *last_agent_message = msg;
            }
            let _ = codex.submit(Op::Shutdown).await;
            session
                .notify_background_event(
                    "subsessions",
                    format!("child session {conversation_id} completed"),
                )
                .await;
            Ok(EventProgress::Continue)
        }
        EventMsg::ShutdownComplete => Ok(EventProgress::Completed),
        EventMsg::Error(err) => Err(SubsessionError::SpawnFailed {
            message: format!("child session error: {}", err.message),
        }),
        _ => Ok(EventProgress::Continue),
    }
}

fn build_child_config(
    parent_config: &Config,
    turn: &TurnContext,
    profile: &SubsessionProfile,
) -> Config {
    let mut config = parent_config.clone();
    config.cwd = turn.cwd.clone();
    config.approval_policy = AskForApproval::Never;
    config.sandbox_policy = turn.sandbox_policy.clone();
    config.shell_environment_policy = turn.shell_environment_policy.clone();
    config.base_instructions = Some(profile.developer_instructions.to_string());
    config.model = profile
        .model_name
        .map(std::string::ToString::to_string)
        .unwrap_or_else(|| parent_config.model.clone());

    if let Some(family) = find_family_for_model(&config.model) {
        config.model_family = family;
    }

    config.model_reasoning_effort = turn.client.get_reasoning_effort();
    config.model_reasoning_summary = turn.client.get_reasoning_summary();
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tokio::sync::oneshot;
    use tokio::sync::watch;
    use tokio::time::timeout;

    #[tokio::test]
    async fn cancel_pending_children_only_updates_pending_records() {
        let manager = SubsessionManager::new();
        let (pending_tx, _pending_rx) = watch::channel(false);
        let pending_record = Arc::new(ChildRecord::new(pending_tx));
        let pending_id = ConversationId::default();
        {
            let mut guard = manager.children.lock().await;
            guard.insert(pending_id, Arc::clone(&pending_record));
        }

        let (done_tx, _done_rx) = watch::channel(false);
        let done_record = Arc::new(ChildRecord::new(done_tx));
        done_record
            .update(ChildStatus::Done(Some("final".to_string())))
            .await;
        let done_id = ConversationId::new();
        {
            let mut guard = manager.children.lock().await;
            guard.insert(done_id, Arc::clone(&done_record));
        }

        let cancelled = manager.cancel_pending_children().await;
        assert!(cancelled, "pending record should be cancelled");
        assert!(matches!(
            pending_record.status().await,
            ChildStatus::Cancelled
        ));
        match done_record.status().await {
            ChildStatus::Done(value) => assert_eq!(value.as_deref(), Some("final")),
            status => panic!("expected done status, got {status:?}"),
        }
    }

    #[tokio::test]
    async fn send_cancel_allows_task_to_observe_shutdown() {
        let (cancel_tx, mut cancel_rx) = watch::channel(false);
        let record = Arc::new(ChildRecord::new(cancel_tx));
        let (observed_tx, observed_rx) = oneshot::channel();

        let handle = tokio::spawn(async move {
            loop {
                if *cancel_rx.borrow() {
                    let _ = observed_tx.send(());
                    break;
                }
                if cancel_rx.changed().await.is_err() {
                    break;
                }
            }
        });

        record.set_handle(handle).await;
        record.send_cancel().await;

        let observed = timeout(Duration::from_millis(200), observed_rx).await;
        assert!(
            observed.is_ok(),
            "task should observe cancellation before the handle is dropped"
        );
        assert!(
            observed.unwrap().is_ok(),
            "task should report cancellation observation"
        );
    }
}
