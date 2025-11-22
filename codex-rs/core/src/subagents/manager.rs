use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::config::Config as CodexConfig;
use codex_protocol::AgentId;
use codex_protocol::ConversationId;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentInboxEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::SubagentCreatedEvent;
use codex_protocol::protocol::SubagentLifecycleEvent;
use codex_protocol::protocol::SubagentLifecycleOrigin;
use codex_protocol::protocol::SubagentLifecycleStatus;
use codex_protocol::protocol::SubagentReasoningHeaderEvent;
use codex_protocol::protocol::SubagentRemovedEvent;
use codex_protocol::protocol::SubagentStatusEvent;
use codex_protocol::protocol::SubagentSummary;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::RwLock;
use tokio::sync::Semaphore;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::warn;

use crate::codex::Codex;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex_delegate::run_codex_conversation_interactive;
use crate::error::CodexErr;
use crate::model_family::derive_default_model_family;
use crate::model_family::find_family_for_model;
use crate::protocol::SandboxPolicy;
use crate::subagents::SubagentMetadata;
use crate::subagents::SubagentOrigin;
use crate::subagents::SubagentRegistry;
use crate::subagents::SubagentStatus;
use codex_protocol::config_types::ReasoningEffort;

const LOG_CAPACITY: usize = 200;
const ROOT_AGENT_ID: AgentId = 0;

#[derive(Clone)]
struct RootInboxItem {
    sender_agent_id: AgentId,
    timestamp_ms: i64,
    payload: RootInboxPayload,
    metadata: Option<SubagentMetadata>,
}

#[derive(Clone)]
enum RootInboxPayload {
    Message(InboxMessage),
    Completion(SubagentCompletion),
}

#[derive(Clone)]
pub struct SubagentManager {
    registry: Arc<SubagentRegistry>,
    runs: Arc<RwLock<HashMap<ConversationId, Arc<ManagedSubagent>>>>,
    completions: Arc<RwLock<HashMap<ConversationId, SubagentCompletion>>>,
    completed_logs: Arc<RwLock<HashMap<ConversationId, Vec<LoggedEvent>>>>,
    completed_inbox: Arc<RwLock<HashMap<ConversationId, Vec<InboxMessage>>>>,
    root_inbox: Arc<RwLock<HashMap<ConversationId, Vec<RootInboxItem>>>>,
    emitters: Arc<RwLock<HashMap<ConversationId, SubagentEmitter>>>,
    max_active_subagents: usize,
    permits: Arc<Semaphore>,
    next_agent_id: Arc<AtomicU64>,
    root_agent_uses_user_messages: bool,
    root_inbox_autosubmit: bool,
    subagent_inbox_inject_before_tools: bool,
    watchdogs: Arc<RwLock<HashMap<(ConversationId, AgentId), WatchdogHandle>>>,
}

struct WatchdogHandle {
    cancel: tokio_util::sync::CancellationToken,
    #[allow(dead_code)]
    task: tokio::task::JoinHandle<()>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchdogAction {
    Started,
    Replaced,
    Canceled,
    NotFound,
}

fn clamp_reasoning_effort_for_model(config: &mut CodexConfig) {
    if let Some(effort) = config.model_reasoning_effort {
        let family = find_family_for_model(&config.model)
            .unwrap_or_else(|| derive_default_model_family(&config.model));
        if !family.supports_reasoning_summaries {
            config.model_reasoning_effort = None;
            return;
        }
        if effort == ReasoningEffort::XHigh && !config.model.starts_with("gpt-5.1-codex-max") {
            config.model_reasoning_effort = Some(ReasoningEffort::High);
        }
    }
}

fn normalize_prompt(prompt: Option<String>) -> Option<String> {
    prompt
        .as_ref()
        .map(|text| text.trim())
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

impl SubagentManager {
    fn allocate_agent_id(&self) -> AgentId {
        self.next_agent_id.fetch_add(1, Ordering::Relaxed)
    }

    async fn enqueue_root_inbox_completion(
        &self,
        root_session_id: &ConversationId,
        source_session_id: &ConversationId,
        completion: SubagentCompletion,
        metadata: SubagentMetadata,
    ) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|dur| dur.as_millis() as i64)
            .unwrap_or(0);
        let _ = self
            .push_root_inbox_entry(
                root_session_id,
                source_session_id,
                RootInboxItem {
                    sender_agent_id: metadata.agent_id,
                    timestamp_ms: now_ms,
                    payload: RootInboxPayload::Completion(completion),
                    metadata: Some(metadata),
                },
            )
            .await;
    }

    async fn parent_agent_id(&self, session_id: &ConversationId) -> AgentId {
        self.registry
            .get(session_id)
            .await
            .map(|m| m.agent_id)
            .unwrap_or(ROOT_AGENT_ID)
    }

    pub fn new(
        registry: Arc<SubagentRegistry>,
        max_active_subagents: usize,
        root_agent_uses_user_messages: bool,
        root_inbox_autosubmit: bool,
        subagent_inbox_inject_before_tools: bool,
    ) -> Self {
        Self {
            registry,
            runs: Arc::new(RwLock::new(HashMap::new())),
            completions: Arc::new(RwLock::new(HashMap::new())),
            completed_logs: Arc::new(RwLock::new(HashMap::new())),
            completed_inbox: Arc::new(RwLock::new(HashMap::new())),
            root_inbox: Arc::new(RwLock::new(HashMap::new())),
            emitters: Arc::new(RwLock::new(HashMap::new())),
            max_active_subagents,
            permits: Arc::new(Semaphore::new(max_active_subagents)),
            next_agent_id: Arc::new(AtomicU64::new(1)),
            root_agent_uses_user_messages,
            root_inbox_autosubmit,
            subagent_inbox_inject_before_tools,
            watchdogs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn root_inbox_autosubmit_enabled(&self) -> bool {
        self.root_inbox_autosubmit
    }

    pub fn inbox_inject_before_tools(&self) -> bool {
        self.subagent_inbox_inject_before_tools
    }

    /// Walk parent links to find the root session for a caller.
    async fn find_root_session_id(&self, session_id: ConversationId) -> Option<ConversationId> {
        let entries = self.registry.list().await;
        let mut current = session_id;
        loop {
            let meta = entries.iter().find(|m| m.session_id == current)?;
            if let Some(parent) = meta.parent_session_id {
                current = parent;
                continue;
            }
            return Some(current);
        }
    }

    /// Import subagent metadata from persisted rollout events (resume).
    pub async fn import_from_rollout(
        &self,
        rollout_items: &[RolloutItem],
        parent_session_id: ConversationId,
    ) {
        let by_session = Self::collect_lifecycle_from_rollout(rollout_items);
        let watchdogs = Self::collect_watchdogs_from_rollout(rollout_items);

        for meta in by_session.values() {
            self.registry.register_imported(meta.clone()).await;
        }

        for (agent_id, (interval, message)) in watchdogs {
            let _ = self
                .watchdog_action(
                    parent_session_id,
                    ROOT_AGENT_ID,
                    agent_id,
                    interval,
                    message,
                    false,
                )
                .await;
        }
    }

    fn collect_lifecycle_from_rollout(
        rollout_items: &[RolloutItem],
    ) -> HashMap<ConversationId, SubagentMetadata> {
        use codex_protocol::protocol::SubagentLifecycleEvent;

        let mut by_session: HashMap<ConversationId, SubagentMetadata> = HashMap::new();

        for item in rollout_items {
            match item {
                RolloutItem::EventMsg(EventMsg::SubagentLifecycle(event)) => match event {
                    SubagentLifecycleEvent::Created(ev) => {
                        let meta = SubagentMetadata::from_summary(&ev.subagent);
                        by_session.insert(ev.subagent.session_id, meta);
                    }
                    SubagentLifecycleEvent::Status(ev) => {
                        if let Some(meta) = by_session.get_mut(&ev.session_id) {
                            meta.status = ev.status.into();
                        }
                    }
                    SubagentLifecycleEvent::ReasoningHeader(ev) => {
                        if let Some(meta) = by_session.get_mut(&ev.session_id) {
                            meta.reasoning_header = Some(ev.reasoning_header.clone());
                        }
                    }
                    SubagentLifecycleEvent::Deleted(ev) => {
                        by_session.remove(&ev.session_id);
                    }
                },
                RolloutItem::EventMsg(EventMsg::AgentInbox(ev)) => {
                    if let Some(meta) = by_session.get_mut(&ev.session_id) {
                        meta.pending_messages = ev.pending_messages;
                        meta.pending_interrupts = ev.pending_interrupts;
                    }
                }
                _ => {}
            }
        }

        by_session
    }

    fn collect_watchdogs_from_rollout(
        rollout_items: &[RolloutItem],
    ) -> HashMap<AgentId, (u64, String)> {
        let mut watchdogs: HashMap<AgentId, (u64, String)> = HashMap::new();
        let mut call_args: HashMap<String, serde_json::Value> = HashMap::new();

        for item in rollout_items {
            match item {
                RolloutItem::ResponseItem(ResponseItem::FunctionCall {
                    name,
                    call_id,
                    arguments,
                    ..
                }) if name == "subagent_watchdog" => {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(arguments) {
                        call_args.insert(call_id.clone(), val);
                    }
                }
                RolloutItem::ResponseItem(ResponseItem::FunctionCallOutput { call_id, output }) => {
                    if let Some(_args) = call_args.remove(call_id)
                        && let Ok(val) = serde_json::from_str::<serde_json::Value>(&output.content)
                        && let Some(agent_id) =
                            val.get("agent_id").and_then(serde_json::Value::as_u64)
                        && let Some(action) = val.get("action").and_then(|v| v.as_str())
                    {
                        match action {
                            "started" | "replaced" => {
                                let interval = val
                                    .get("interval_s")
                                    .and_then(serde_json::Value::as_u64)
                                    .unwrap_or(300);
                                let message = val.get("message").and_then(|v| v.as_str()).unwrap_or("Watchdog ping — report current status, next step, and PLAN.md progress.").to_string();
                                watchdogs.insert(agent_id as AgentId, (interval, message));
                            }
                            "canceled" => {
                                watchdogs.remove(&(agent_id as AgentId));
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        watchdogs
    }

    async fn register_emitter(
        &self,
        session_id: ConversationId,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
    ) {
        let emitter = SubagentEmitter { session, turn };
        self.emitters.write().await.insert(session_id, emitter);
    }

    async fn remove_emitter(&self, session_id: &ConversationId) {
        self.emitters.write().await.remove(session_id);
    }

    async fn prepare_child_session(
        &self,
        metadata: &SubagentMetadata,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
    ) {
        self.register_emitter(metadata.session_id, Arc::clone(&session), Arc::clone(&turn))
            .await;
        self.emit_created(metadata).await;
        self.emit_inbox(metadata).await;
    }

    async fn rollback_child(&self, session_id: &ConversationId) {
        self.emit_deleted(session_id).await;
        self.remove_emitter(session_id).await;
        self.registry.remove(session_id).await;
    }

    async fn send_event_to_parent(&self, session_id: &ConversationId, msg: EventMsg) {
        if let Some(emitter) = self.emitters.read().await.get(session_id).cloned() {
            emitter.session.send_event(&emitter.turn, msg).await;
        }
    }

    async fn enqueue_root_inbox_message(
        &self,
        root_session_id: &ConversationId,
        source_session_id: &ConversationId,
        message: InboxMessage,
    ) {
        let _ = self
            .push_root_inbox_entry(
                root_session_id,
                source_session_id,
                RootInboxItem {
                    sender_agent_id: message.sender_agent_id,
                    timestamp_ms: message.timestamp_ms,
                    payload: RootInboxPayload::Message(message),
                    metadata: None,
                },
            )
            .await;
    }

    async fn push_root_inbox_entry(
        &self,
        root_session_id: &ConversationId,
        source_session_id: &ConversationId,
        item: RootInboxItem,
    ) -> usize {
        let pending_messages = {
            let mut guard = self.root_inbox.write().await;
            let entry = guard.entry(*root_session_id).or_insert_with(Vec::new);
            entry.push(item);
            entry.len()
        };

        let event = EventMsg::AgentInbox(AgentInboxEvent {
            agent_id: ROOT_AGENT_ID,
            session_id: *root_session_id,
            pending_messages,
            pending_interrupts: 0,
        });
        self.send_event_to_parent(source_session_id, event).await;

        if self.root_inbox_autosubmit
            && let Some(root_session) = crate::session_index::get(root_session_id)
            && !root_session.has_active_turn().await
        {
            let items = self.drain_root_inbox_to_items(root_session_id).await;
            if !items.is_empty() {
                root_session.autosubmit_inbox_task(items).await;
            }
        }

        pending_messages
    }

    /// Drain and clear the root inbox for `root_session_id`, returning a
    /// sequence of synthetic `subagent_await` call/output pairs that surface
    /// all messages grouped by sending subagent. Callers are responsible for
    /// recording these items into the root session's history in the desired
    /// order relative to other turn items.
    pub async fn drain_root_inbox_to_items(
        &self,
        root_session_id: &ConversationId,
    ) -> Vec<ResponseItem> {
        let drained: Vec<RootInboxItem> = {
            let mut guard = self.root_inbox.write().await;
            guard.remove(root_session_id).unwrap_or_default()
        };

        if drained.is_empty() {
            return Vec::new();
        }

        // Reset root inbox counts to zero for UIs.
        let event = EventMsg::AgentInbox(AgentInboxEvent {
            agent_id: ROOT_AGENT_ID,
            session_id: *root_session_id,
            pending_messages: 0,
            pending_interrupts: 0,
        });
        self.send_event_to_parent(root_session_id, event).await;

        #[derive(Default)]
        struct RootAggregate {
            messages: Vec<InboxMessage>,
            completion: Option<SubagentCompletion>,
            earliest_timestamp: i64,
            metadata: Option<SubagentMetadata>,
        }

        let mut aggregates: HashMap<AgentId, RootAggregate> = HashMap::new();
        for entry in drained {
            let aggregate =
                aggregates
                    .entry(entry.sender_agent_id)
                    .or_insert_with(|| RootAggregate {
                        earliest_timestamp: entry.timestamp_ms,
                        metadata: entry.metadata.clone(),
                        ..RootAggregate::default()
                    });
            aggregate.earliest_timestamp = aggregate.earliest_timestamp.min(entry.timestamp_ms);
            if aggregate.metadata.is_none() {
                aggregate.metadata = entry.metadata.clone();
            }
            match entry.payload {
                RootInboxPayload::Message(msg) => aggregate.messages.push(msg),
                RootInboxPayload::Completion(completion) => aggregate.completion = Some(completion),
            }
        }

        let registry_entries = self.registry.list().await;
        let mut items = Vec::new();
        let mut aggregates_vec: Vec<(AgentId, RootAggregate)> = aggregates.into_iter().collect();
        aggregates_vec.sort_by_key(|(_, aggregate)| aggregate.earliest_timestamp);

        for (sender_agent_id, mut aggregate) in aggregates_vec {
            if aggregate.messages.is_empty() && aggregate.completion.is_none() {
                continue;
            }

            // Prefer metadata captured at enqueue time, but fall back to the registry.
            let metadata = aggregate
                .metadata
                .or_else(|| {
                    registry_entries
                        .iter()
                        .find(|m| {
                            m.agent_id == sender_agent_id
                                && m.parent_session_id == Some(*root_session_id)
                        })
                        .cloned()
                })
                .or_else(|| {
                    if sender_agent_id == ROOT_AGENT_ID {
                        let now = SystemTime::now();
                        let now_ms = now
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_millis() as i64)
                            .unwrap_or(0);
                        Some(SubagentMetadata {
                            agent_id: ROOT_AGENT_ID,
                            parent_agent_id: None,
                            session_id: *root_session_id,
                            parent_session_id: None,
                            origin: SubagentOrigin::Spawn,
                            initial_message_count: 0,
                            status: SubagentStatus::Ready,
                            created_at: now,
                            created_at_ms: now_ms,
                            session_key: root_session_id.to_string(),
                            label: Some("root".to_string()),
                            summary: None,
                            reasoning_header: None,
                            pending_messages: 0,
                            pending_interrupts: 0,
                        })
                    } else {
                        None
                    }
                });
            let Some(metadata) = metadata else {
                continue;
            };

            aggregate.messages.sort_by_key(|m| m.timestamp_ms);

            let call_id = format!("await-{sender_agent_id}");
            let arguments = serde_json::json!({
                "timeout_s": 0,
            })
            .to_string();

            let call_item = ResponseItem::FunctionCall {
                id: None,
                name: "subagent_await".to_string(),
                arguments,
                call_id: call_id.clone(),
            };

            let lifecycle_status = metadata.status;
            let started_at_ms = metadata.created_at_ms;
            let completion_status = aggregate
                .completion
                .as_ref()
                .map(crate::tools::handlers::subagent::completion_status);
            let response_body = serde_json::json!({
                "session_id": metadata.session_id,
                "completion_status": completion_status,
                "lifecycle_status": lifecycle_status,
                "started_at_ms": started_at_ms,
                "timed_out": false,
                "messages": aggregate.messages,
                "completion": aggregate.completion,
                "metadata": metadata,
                "injected": true,
            })
            .to_string();

            let output_item = ResponseItem::FunctionCallOutput {
                call_id: call_id.clone(),
                output: FunctionCallOutputPayload {
                    content: response_body,
                    content_items: None,
                    success: Some(true),
                },
            };

            items.push(call_item);
            items.push(output_item);
        }

        items
    }

    async fn update_inbox_counts_and_emit(
        &self,
        session_id: &ConversationId,
        pending_messages: usize,
        pending_interrupts: usize,
    ) {
        if let Some(metadata) = self
            .registry
            .update_inbox_counts(session_id, pending_messages, pending_interrupts)
            .await
        {
            self.emit_inbox(&metadata).await;
        }
    }

    async fn agent_id_for(&self, session_id: &ConversationId) -> AgentId {
        self.registry
            .get(session_id)
            .await
            .map(|m| m.agent_id)
            .unwrap_or(ROOT_AGENT_ID)
    }

    async fn emit_inbox(&self, metadata: &SubagentMetadata) {
        let msg = EventMsg::AgentInbox(AgentInboxEvent {
            agent_id: metadata.agent_id,
            session_id: metadata.session_id,
            pending_messages: metadata.pending_messages,
            pending_interrupts: metadata.pending_interrupts,
        });
        self.send_event_to_parent(&metadata.session_id, msg).await;
    }

    async fn emit_created(&self, metadata: &SubagentMetadata) {
        let summary = to_subagent_summary(metadata);
        let event =
            EventMsg::SubagentLifecycle(SubagentLifecycleEvent::Created(SubagentCreatedEvent {
                subagent: summary,
            }));
        self.send_event_to_parent(&metadata.session_id, event).await;
    }

    /// Helper to synthesize a `subagent_await` tool call and corresponding
    /// output into the given session's history. Callers are responsible for
    /// choosing the appropriate target session (parent vs. child) and for
    /// deciding which subset of messages should be delivered.
    async fn inject_synthetic_await_into_session(
        &self,
        target_session_id: &ConversationId,
        agent_id: AgentId,
        metadata: &SubagentMetadata,
        messages: &[InboxMessage],
        completion: Option<&SubagentCompletion>,
    ) {
        if messages.is_empty() && completion.is_none() {
            return;
        }

        let Some(session) = crate::session_index::get(target_session_id) else {
            return;
        };
        let turn = session
            .new_turn(crate::codex::SessionSettingsUpdate::default())
            .await;

        // Synthesize a tool call + output pair that matches the
        // `subagent_await` tool schema. Use a stable call id for ease of
        // debugging; uniqueness is not required for tests.
        let call_id = format!("await-{agent_id}");
        let arguments = serde_json::json!({
            "timeout_s": 0,
        })
        .to_string();

        let call_item = ResponseItem::FunctionCall {
            id: None,
            name: "subagent_await".to_string(),
            arguments,
            call_id: call_id.clone(),
        };

        let completion_status = completion.map(crate::tools::handlers::subagent::completion_status);
        let lifecycle_status = metadata.status;
        let started_at_ms = metadata.created_at_ms;
        let response_body = serde_json::json!({
            "session_id": metadata.session_id,
            "completion_status": completion_status,
            "lifecycle_status": lifecycle_status,
            "started_at_ms": started_at_ms,
            "timed_out": false,
            "messages": messages,
            "completion": completion,
            "metadata": metadata,
            "injected": true,
        })
        .to_string();

        let output_item = ResponseItem::FunctionCallOutput {
            call_id: call_id.clone(),
            output: FunctionCallOutputPayload {
                content: response_body,
                content_items: None,
                success: Some(true),
            },
        };

        session
            .record_conversation_items(&turn, &[call_item, output_item])
            .await;
    }

    /// Deliver a batch of inbox messages and optional completion into the
    /// child session's history so the subagent itself can see messages in its
    /// own thread, honoring the `root_agent_uses_user_messages` toggle.
    pub(crate) async fn deliver_inbox_to_threads_at_yield(&self, result: &AwaitInboxResult) {
        let metadata = &result.metadata;
        let messages = &result.messages;
        let completion = result.completion.as_ref();

        if messages.is_empty() && completion.is_none() {
            return;
        }

        // For the child session, either deliver all messages via a synthetic
        // subagent_await (tool-only mode) or split them so that root-origin
        // messages become user turns while messages from other agents are
        // surfaced via subagent_await.
        let child_session_id = metadata.session_id;

        if self.root_agent_uses_user_messages && !messages.is_empty() {
            let mut from_root = Vec::new();
            let mut from_others = Vec::new();
            for msg in messages {
                if msg.sender_agent_id == ROOT_AGENT_ID {
                    from_root.push(msg.clone());
                } else {
                    from_others.push(msg.clone());
                }
            }

            if !from_others.is_empty() || completion.is_some() {
                self.inject_synthetic_await_into_session(
                    &child_session_id,
                    metadata.agent_id,
                    metadata,
                    &from_others,
                    completion,
                )
                .await;
            }

            if !from_root.is_empty()
                && let Some(child_session) = crate::session_index::get(&child_session_id)
            {
                let turn = child_session
                    .new_turn(crate::codex::SessionSettingsUpdate::default())
                    .await;
                let mut items = Vec::new();

                for msg in from_root {
                    if let Some(prompt) = msg.prompt.clone() {
                        let trimmed = prompt.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        items.push(ResponseItem::Message {
                            id: None,
                            role: "user".to_string(),
                            content: vec![ContentItem::InputText {
                                text: trimmed.to_string(),
                            }],
                        });
                    }
                }

                if !items.is_empty() {
                    child_session.record_conversation_items(&turn, &items).await;
                }
            }
        } else {
            // Tool-only mode: all messages are delivered via a synthetic
            // subagent_await call in the child history.
            self.inject_synthetic_await_into_session(
                &child_session_id,
                metadata.agent_id,
                metadata,
                messages,
                completion,
            )
            .await;
        }
    }

    async fn emit_status(&self, session_id: &ConversationId, status: SubagentStatus) {
        let agent_id = self.agent_id_for(session_id).await;
        let event =
            EventMsg::SubagentLifecycle(SubagentLifecycleEvent::Status(SubagentStatusEvent {
                agent_id,
                session_id: *session_id,
                status: status.into(),
            }));
        self.send_event_to_parent(session_id, event).await;
    }

    async fn emit_reasoning_header(&self, session_id: &ConversationId, header: String) {
        let agent_id = self.agent_id_for(session_id).await;
        let event = EventMsg::SubagentLifecycle(SubagentLifecycleEvent::ReasoningHeader(
            SubagentReasoningHeaderEvent {
                agent_id,
                session_id: *session_id,
                reasoning_header: header,
            },
        ));
        self.send_event_to_parent(session_id, event).await;
    }

    async fn emit_deleted(&self, session_id: &ConversationId) {
        let agent_id = self.agent_id_for(session_id).await;
        let event =
            EventMsg::SubagentLifecycle(SubagentLifecycleEvent::Deleted(SubagentRemovedEvent {
                agent_id,
                session_id: *session_id,
            }));
        self.send_event_to_parent(session_id, event).await;
    }

    async fn update_status_and_emit(&self, session_id: &ConversationId, status: SubagentStatus) {
        let _ = self.registry.update_status(session_id, status).await;
        self.emit_status(session_id, status).await;
    }

    async fn update_reasoning_header_and_emit(&self, session_id: &ConversationId, header: String) {
        let header_clone = header.clone();
        if self
            .registry
            .update_reasoning_header(session_id, header)
            .await
            .is_some()
        {
            self.emit_reasoning_header(session_id, header_clone).await;
        }
    }

    pub(crate) async fn spawn(
        &self,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        request: SpawnRequest,
    ) -> Result<SubagentMetadata, SubagentManagerError> {
        let session_id = ConversationId::new();
        let parent_agent_id = self.parent_agent_id(&session.conversation_id()).await;
        let agent_id = self.allocate_agent_id();
        let metadata = self
            .registry
            .register_spawn(
                session_id,
                Some(session.conversation_id()),
                Some(parent_agent_id),
                agent_id,
                0,
                request.label.clone(),
                request.summary.clone(),
            )
            .await;

        self.prepare_child_session(&metadata, Arc::clone(&session), Arc::clone(&turn))
            .await;

        let launch = self
            .launch_child(
                session_id,
                Arc::clone(&session),
                Arc::clone(&turn),
                None,
                request.sandbox_mode,
                request.model,
            )
            .await;
        let runtime = match launch {
            Ok(runtime) => runtime,
            Err(err) => {
                self.rollback_child(&session_id).await;
                return Err(err);
            }
        };

        match runtime.submit_prompt(&request.prompt).await {
            Ok(true) => {
                self.update_status_and_emit(&session_id, SubagentStatus::Running)
                    .await;
            }
            Ok(false) => {
                self.update_status_and_emit(&session_id, SubagentStatus::Ready)
                    .await;
            }
            Err(err) => {
                self.handle_launch_failure(session_id, runtime.clone(), err)
                    .await;
                return Err(SubagentManagerError::LaunchFailed(
                    "failed to submit initial prompt".to_string(),
                ));
            }
        }

        let final_metadata = self.registry.get(&session_id).await.unwrap_or(metadata);
        Ok(final_metadata)
    }

    pub(crate) async fn fork(
        &self,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        request: ForkRequest,
    ) -> Result<SubagentMetadata, SubagentManagerError> {
        let session_id = ConversationId::new();
        let metadata = self
            .registry
            .register_fork(
                session_id,
                request.parent_session_id,
                Some(self.parent_agent_id(&request.parent_session_id).await),
                self.allocate_agent_id(),
                request.initial_message_count,
                request.label.clone(),
                request.summary.clone(),
            )
            .await;

        self.prepare_child_session(&metadata, Arc::clone(&session), Arc::clone(&turn))
            .await;

        let mut fork_items = build_fork_history(Arc::clone(&session))
            .await
            .get_rollout_items();
        // Remove the parent's fork call/output so the child history only contains the synthetic entry.
        remove_call_items(&mut fork_items, &request.call_id);
        // Append synthetic fork call/return so the child sees the fork result.
        let child_payload_json = json!({
            "role": "child",
            "parent_session_id": request.parent_session_id,
            "child_session_id": session_id,
            "label": request.label,
            "summary": request.summary,
            "prompt": request.prompt,
        })
        .to_string();
        let synthetic_call = ResponseItem::FunctionCall {
            id: None,
            name: "subagent_fork".to_string(),
            arguments: request.arguments.clone(),
            call_id: request.call_id.clone(),
        };
        let synthetic_return = ResponseItem::FunctionCallOutput {
            call_id: request.call_id.clone(),
            output: FunctionCallOutputPayload {
                content: child_payload_json,
                content_items: None,
                success: Some(true),
            },
        };
        fork_items.push(RolloutItem::ResponseItem(synthetic_call));
        fork_items.push(RolloutItem::ResponseItem(synthetic_return));
        let launch = self
            .launch_child(
                session_id,
                Arc::clone(&session),
                Arc::clone(&turn),
                Some(InitialHistory::Forked(fork_items)),
                request.sandbox_mode,
                request.model,
            )
            .await;
        let runtime = match launch {
            Ok(runtime) => runtime,
            Err(err) => {
                self.rollback_child(&session_id).await;
                return Err(err);
            }
        };

        let trimmed_prompt = normalize_prompt(request.prompt.clone());

        match trimmed_prompt {
            Some(prompt) => match runtime.submit_prompt(&prompt).await {
                Ok(true) => {
                    self.update_status_and_emit(&session_id, SubagentStatus::Running)
                        .await;
                }
                Ok(false) => {
                    self.update_status_and_emit(&session_id, SubagentStatus::Ready)
                        .await;
                }
                Err(err) => {
                    self.handle_launch_failure(session_id, runtime.clone(), err)
                        .await;
                    return Err(SubagentManagerError::LaunchFailed(
                        "failed to submit initial prompt".to_string(),
                    ));
                }
            },
            None => {
                self.update_status_and_emit(&session_id, SubagentStatus::Ready)
                    .await;
            }
        }

        let final_metadata = self.registry.get(&session_id).await.unwrap_or(metadata);
        Ok(final_metadata)
    }

    pub async fn send_message(
        &self,
        request: SendMessageRequest,
    ) -> Result<SubagentMetadata, SubagentManagerError> {
        let SendMessageRequest {
            session_id,
            label,
            summary,
            prompt,
            agent_id,
            sender_agent_id,
            interrupt,
        } = request;
        let runtime = {
            let runs = self.runs.read().await;
            runs.get(&session_id).cloned()
        }
        .ok_or(SubagentManagerError::NotFound)?;

        let existing = self
            .registry
            .get(&session_id)
            .await
            .ok_or(SubagentManagerError::NotFound)?;

        if existing.agent_id != agent_id {
            return Err(SubagentManagerError::AgentIdMismatch {
                session_id,
                agent_id,
            });
        }

        let updated_label = label.or(existing.label.clone());
        let updated_summary = summary.or(existing.summary.clone());

        let metadata = self
            .registry
            .update_label_and_summary(&session_id, updated_label, updated_summary)
            .await
            .ok_or(SubagentManagerError::NotFound)?;

        // Keep prior completions/logs intact so history reflects previous turns.
        // Only clear the runtime's in-flight completion latch so new work can proceed.
        runtime.clear_completion();
        let trimmed_prompt = normalize_prompt(prompt);

        if interrupt || trimmed_prompt.is_some() {
            let counts = runtime
                .enqueue_message(PendingMessage {
                    prompt: trimmed_prompt.clone(),
                    interrupt,
                })
                .await;
            self.update_inbox_counts_and_emit(&session_id, counts.0, counts.1)
                .await;
        }

        // Record a logical inbox message so callers of subagent_await can see
        // who sent what, even though the underlying runtime is still driven by
        // UserInput events via the mailbox.
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|dur| dur.as_millis() as i64)
            .unwrap_or(0);
        let inbox_message = InboxMessage {
            sender_agent_id,
            recipient_agent_id: agent_id,
            interrupt,
            prompt: trimmed_prompt.clone(),
            timestamp_ms: now_ms,
        };
        let _ = runtime.enqueue_inbox_message(inbox_message).await;

        if interrupt {
            runtime.interrupt().await;
        }

        if trimmed_prompt.is_some() {
            self.update_status_and_emit(&session_id, SubagentStatus::Running)
                .await;
        } else {
            self.update_status_and_emit(&session_id, SubagentStatus::Ready)
                .await;
        }

        let final_metadata = self.registry.get(&session_id).await.unwrap_or(metadata);
        Ok(final_metadata)
    }

    pub async fn watchdog_action(
        &self,
        caller_session_id: ConversationId,
        caller_agent_id: AgentId,
        target_agent_id: AgentId,
        interval_s: u64,
        message: String,
        cancel: bool,
    ) -> Result<WatchdogAction, SubagentManagerError> {
        if cancel {
            return Ok(self
                .cancel_watchdog(&caller_session_id, target_agent_id)
                .await);
        }

        let root_session_id = if target_agent_id == ROOT_AGENT_ID {
            self.find_root_session_id(caller_session_id)
                .await
                .unwrap_or(caller_session_id)
        } else {
            caller_session_id
        };

        let target_session_id = if target_agent_id == ROOT_AGENT_ID {
            root_session_id
        } else {
            self.registry
                .list()
                .await
                .into_iter()
                .find(|m| m.agent_id == target_agent_id)
                .map(|m| m.session_id)
                .ok_or(SubagentManagerError::NotFound)?
        };

        let action = self
            .start_watchdog(
                caller_session_id,
                root_session_id,
                caller_agent_id,
                target_session_id,
                target_agent_id,
                interval_s,
                message,
            )
            .await?;
        Ok(action)
    }

    async fn start_watchdog(
        &self,
        caller_session_id: ConversationId,
        root_session_id: ConversationId,
        caller_agent_id: AgentId,
        target_session_id: ConversationId,
        target_agent_id: AgentId,
        interval_s: u64,
        message: String,
    ) -> Result<WatchdogAction, SubagentManagerError> {
        let key = (caller_session_id, target_agent_id);

        if let Some(existing) = self.watchdogs.write().await.remove(&key) {
            existing.cancel.cancel();
        }

        let cancel = tokio_util::sync::CancellationToken::new();
        let cancel_child = cancel.clone();
        let manager = self.clone();
        let msg_clone = message.clone();
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_child.cancelled() => break,
                    _ = tokio::time::sleep(Duration::from_secs(interval_s)) => {
                        if cancel_child.is_cancelled() {
                            break;
                        }
                        if target_agent_id == ROOT_AGENT_ID {
                            if caller_agent_id == ROOT_AGENT_ID {
                                let now_ms = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .map(|d| d.as_millis() as i64)
                                    .unwrap_or(0);
                                let inbox = InboxMessage {
                                    sender_agent_id: ROOT_AGENT_ID,
                                    recipient_agent_id: ROOT_AGENT_ID,
                                    interrupt: false,
                                    prompt: Some(msg_clone.clone()),
                                    timestamp_ms: now_ms,
                                };
                                manager
                                    .enqueue_root_inbox_message(
                                        &root_session_id,
                                        &caller_session_id,
                                        inbox,
                                    )
                                    .await;
                            } else if let Some(sender_meta) = manager.registry.get(&caller_session_id).await {
                                let _ = manager
                                    .send_message_to_root(
                                        root_session_id,
                                        caller_session_id,
                                        Some(msg_clone.clone()),
                                        sender_meta,
                                    )
                                    .await;
                            } else {
                                warn!(caller_session_id = %caller_session_id, "watchdog could not find sender metadata; stopping watchdog");
                                break;
                            }
                        } else {
                            let request = SendMessageRequest {
                                session_id: target_session_id,
                                label: None,
                                summary: None,
                                prompt: Some(msg_clone.clone()),
                                agent_id: target_agent_id,
                                sender_agent_id: caller_agent_id,
                                interrupt: false,
                            };
                            if let Err(err) = manager.send_message(request).await {
                                warn!(?err, "watchdog send_message failed; stopping watchdog");
                                break;
                            }
                        }
                    }
                }
            }
        });

        let replaced = self
            .watchdogs
            .write()
            .await
            .insert(key, WatchdogHandle { cancel, task })
            .is_some();
        Ok(if replaced {
            WatchdogAction::Replaced
        } else {
            WatchdogAction::Started
        })
    }

    async fn cancel_watchdog(
        &self,
        parent_session_id: &ConversationId,
        target_agent_id: AgentId,
    ) -> WatchdogAction {
        let key = (*parent_session_id, target_agent_id);
        if let Some(handle) = self.watchdogs.write().await.remove(&key) {
            handle.cancel.cancel();
            WatchdogAction::Canceled
        } else {
            WatchdogAction::NotFound
        }
    }

    pub async fn send_message_to_root(
        &self,
        root_session_id: ConversationId,
        source_session_id: ConversationId,
        prompt: Option<String>,
        sender_metadata: SubagentMetadata,
    ) -> Result<(), SubagentManagerError> {
        let trimmed_prompt = normalize_prompt(prompt);

        if trimmed_prompt.is_none() {
            return Ok(());
        }

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|dur| dur.as_millis() as i64)
            .unwrap_or(0);
        let inbox_message = InboxMessage {
            sender_agent_id: sender_metadata.agent_id,
            recipient_agent_id: ROOT_AGENT_ID,
            interrupt: false,
            prompt: trimmed_prompt,
            timestamp_ms: now_ms,
        };

        self.enqueue_root_inbox_message(&root_session_id, &source_session_id, inbox_message)
            .await;

        Ok(())
    }

    pub async fn cancel(
        &self,
        session_id: ConversationId,
    ) -> Result<SubagentMetadata, SubagentManagerError> {
        let metadata = self
            .registry
            .get(&session_id)
            .await
            .ok_or(SubagentManagerError::NotFound)?;

        if is_terminal_status(metadata.status) {
            return Ok(metadata);
        }

        if let Some(runtime) = {
            let runs = self.runs.read().await;
            runs.get(&session_id).cloned()
        } {
            self.finalize_terminal(
                &session_id,
                runtime,
                SubagentCompletion::Canceled {
                    reason: TurnAbortReason::Interrupted,
                },
                SubagentStatus::Canceled,
            )
            .await;
        } else {
            let completion = SubagentCompletion::Canceled {
                reason: TurnAbortReason::Interrupted,
            };
            self.update_status_and_emit(&session_id, SubagentStatus::Canceled)
                .await;
            {
                let mut completions = self.completions.write().await;
                completions.insert(session_id, completion.clone());
            }
            {
                let mut logs = self.completed_logs.write().await;
                logs.entry(session_id).or_insert_with(Vec::new);
            }
            {
                let mut inbox = self.completed_inbox.write().await;
                inbox.entry(session_id).or_insert_with(Vec::new);
            }
            self.remove_runtime_entry(&session_id).await;
        }

        let updated = self
            .registry
            .get(&session_id)
            .await
            .ok_or(SubagentManagerError::NotFound)?;
        Ok(updated)
    }

    pub async fn metadata(&self, session_id: &ConversationId) -> Option<SubagentMetadata> {
        self.registry.get(session_id).await
    }

    /// Returns a snapshot of the in-memory log buffer for a running subagent.
    ///
    /// Entries are ordered from oldest to newest. The buffer already only
    /// contains events since the child was spawned/forked.
    pub async fn snapshot_logs(
        &self,
        session_id: &ConversationId,
    ) -> Result<Vec<LogEntry>, SubagentManagerError> {
        if let Some(runtime) = {
            let runs = self.runs.read().await;
            runs.get(session_id).cloned()
        } {
            let guard = runtime.logs.lock().await;
            let entries = guard.iter().map(LogEntry::from_logged).collect::<Vec<_>>();
            Ok(entries)
        } else {
            let logs = self.completed_logs.read().await;
            let snapshot = logs
                .get(session_id)
                .cloned()
                .ok_or(SubagentManagerError::NotFound)?;
            let entries = snapshot
                .into_iter()
                .map(|logged| LogEntry::from_logged(&logged))
                .collect();
            Ok(entries)
        }
    }

    fn remaining_timeout(
        start: tokio::time::Instant,
        timeout_total: Option<Duration>,
        session_id: &ConversationId,
        agent_id: AgentId,
    ) -> Result<Option<Duration>, SubagentManagerError> {
        if let Some(total) = timeout_total {
            let elapsed = start.elapsed();
            if elapsed >= total {
                let timeout_ms = total.as_millis().try_into().unwrap_or(u64::MAX);
                return Err(SubagentManagerError::AwaitTimedOut {
                    session_id: *session_id,
                    agent_id,
                    timeout_ms,
                });
            }
            Ok(Some(total - elapsed))
        } else {
            Ok(None)
        }
    }

    pub async fn await_completion(
        &self,
        session_id: &ConversationId,
        timeout: Option<Duration>,
    ) -> Result<AwaitResult, SubagentManagerError> {
        if let Some(completion) = {
            let completions = self.completions.read().await;
            completions.get(session_id).cloned()
        } {
            let desired_status = status_from_completion(&completion);
            let metadata = self
                .registry
                .get(session_id)
                .await
                .ok_or(SubagentManagerError::NotFound)?;
            if metadata.status != desired_status {
                self.update_status_and_emit(session_id, desired_status)
                    .await;
                let metadata = self
                    .registry
                    .get(session_id)
                    .await
                    .ok_or(SubagentManagerError::NotFound)?;
                return Ok(AwaitResult {
                    metadata,
                    completion,
                });
            }
            return Ok(AwaitResult {
                metadata,
                completion,
            });
        }

        let runtime = {
            let runs = self.runs.read().await;
            runs.get(session_id).cloned()
        };
        let runtime = match runtime {
            Some(runtime) => runtime,
            None => {
                if let Some(completion) = {
                    let completions = self.completions.read().await;
                    completions.get(session_id).cloned()
                } {
                    let metadata = self
                        .registry
                        .get(session_id)
                        .await
                        .ok_or(SubagentManagerError::NotFound)?;
                    return Ok(AwaitResult {
                        metadata,
                        completion,
                    });
                }
                return Err(SubagentManagerError::NotFound);
            }
        };
        let mut receiver = runtime.completion_receiver();
        let agent_id = self.agent_id_for(session_id).await;

        if let Some(completion) = current_completion(&receiver) {
            let metadata = self
                .registry
                .get(session_id)
                .await
                .ok_or(SubagentManagerError::NotFound)?;
            return Ok(AwaitResult {
                metadata,
                completion,
            });
        }

        let timeout_total = timeout;
        let start = tokio::time::Instant::now();
        let mut completion_opt = None;
        loop {
            let changed = if let Some(remaining) =
                Self::remaining_timeout(start, timeout_total, session_id, agent_id)?
            {
                match tokio::time::timeout(remaining, receiver.changed()).await {
                    Ok(result) => result,
                    Err(_) => {
                        let timeout_ms = timeout_total
                            .map(|d| d.as_millis().try_into().unwrap_or(u64::MAX))
                            .unwrap_or(0);
                        return Err(SubagentManagerError::AwaitTimedOut {
                            session_id: *session_id,
                            agent_id,
                            timeout_ms,
                        });
                    }
                }
            } else {
                receiver.changed().await
            };

            match changed {
                Ok(()) => {
                    if let Some(completion) = current_completion(&receiver) {
                        completion_opt = Some(completion);
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        let completion = if let Some(completion) = completion_opt {
            completion
        } else {
            let completions = self.completions.read().await;
            completions
                .get(session_id)
                .cloned()
                .ok_or(SubagentManagerError::NotFound)?
        };

        {
            let mut completions = self.completions.write().await;
            completions.insert(*session_id, completion.clone());
        }

        let desired_status = status_from_completion(&completion);
        if let Some(current) = self.registry.get(session_id).await
            && current.status != desired_status
        {
            self.update_status_and_emit(session_id, desired_status)
                .await;
        }

        let metadata = self
            .registry
            .get(session_id)
            .await
            .ok_or(SubagentManagerError::NotFound)?;
        Ok(AwaitResult {
            metadata,
            completion,
        })
    }

    /// Wait for either new inbox messages or a completion for the given
    /// session. This is the backing implementation for the `subagent_await`
    /// tool and is intentionally conservative: it mirrors the existing
    /// completion semantics while layering inbox delivery on top.
    pub async fn await_inbox_and_completion(
        &self,
        session_id: &ConversationId,
        timeout: Option<Duration>,
    ) -> Result<AwaitInboxResult, SubagentManagerError> {
        // If we already have a stored completion and no runtime, there is
        // nothing left to wait for; return the terminal state with an empty
        // message list.
        if let Some(completion) = {
            let completions = self.completions.read().await;
            completions.get(session_id).cloned()
        } {
            let metadata = self
                .registry
                .get(session_id)
                .await
                .ok_or(SubagentManagerError::NotFound)?;
            return Ok(AwaitInboxResult {
                metadata,
                completion: Some(completion),
                messages: Vec::new(),
            });
        }

        let runtime = {
            let runs = self.runs.read().await;
            runs.get(session_id).cloned()
        };
        let runtime = match runtime {
            Some(runtime) => runtime,
            None => {
                if let Some(completion) = {
                    let completions = self.completions.read().await;
                    completions.get(session_id).cloned()
                } {
                    let metadata = self
                        .registry
                        .get(session_id)
                        .await
                        .ok_or(SubagentManagerError::NotFound)?;
                    return Ok(AwaitInboxResult {
                        metadata,
                        completion: Some(completion),
                        messages: Vec::new(),
                    });
                }
                return Err(SubagentManagerError::NotFound);
            }
        };

        let mut receiver = runtime.completion_receiver();
        let inbox_notify = runtime.inbox_notifier();
        let agent_id = self.agent_id_for(session_id).await;

        // Fast path: if there are already inbox messages or a completion,
        // return immediately without waiting.
        let mut messages = runtime.drain_inbox().await;
        let mut completion_opt = current_completion(&receiver);
        if completion_opt.is_some() || !messages.is_empty() {
            if let Some(ref completion) = completion_opt {
                let desired_status = status_from_completion(completion);
                self.update_status_and_emit(session_id, desired_status)
                    .await;
                {
                    let mut completions = self.completions.write().await;
                    completions.insert(*session_id, completion.clone());
                }
            }
            let metadata = self
                .registry
                .get(session_id)
                .await
                .ok_or(SubagentManagerError::NotFound)?;
            return Ok(AwaitInboxResult {
                metadata,
                completion: completion_opt,
                messages,
            });
        }

        let timeout_total = timeout;
        let start = tokio::time::Instant::now();

        loop {
            let remaining = Self::remaining_timeout(start, timeout_total, session_id, agent_id)?;

            if let Some(rem) = remaining {
                tokio::select! {
                    _ = tokio::time::sleep(rem) => {
                        let timeout_ms = timeout_total
                            .map(|d| d.as_millis().try_into().unwrap_or(u64::MAX))
                            .unwrap_or(0);
                        return Err(SubagentManagerError::AwaitTimedOut {
                            session_id: *session_id,
                            agent_id,
                            timeout_ms,
                        });
                    }
                    _ = inbox_notify.notified() => {
                        let new_messages = runtime.drain_inbox().await;
                        if !new_messages.is_empty() {
                            messages.extend(new_messages);
                            break;
                        }
                        // Spurious wakeup or inbox drained by another waiter;
                        // loop and wait again.
                    }
                    changed = receiver.changed() => {
                        match changed {
                            Ok(()) => {
                                if let Some(completion) = current_completion(&receiver) {
                                    completion_opt = Some(completion);
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
            } else {
                tokio::select! {
                    _ = inbox_notify.notified() => {
                        let new_messages = runtime.drain_inbox().await;
                        if !new_messages.is_empty() {
                            messages.extend(new_messages);
                            break;
                        }
                    }
                    changed = receiver.changed() => {
                        match changed {
                            Ok(()) => {
                                if let Some(completion) = current_completion(&receiver) {
                                    completion_opt = Some(completion);
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
        }

        // Finalize completion state if we observed a terminal result.
        if let Some(ref completion) = completion_opt {
            let desired_status = status_from_completion(completion);
            self.update_status_and_emit(session_id, desired_status)
                .await;
            {
                let mut completions = self.completions.write().await;
                completions.insert(*session_id, completion.clone());
            }
        }

        let metadata = self
            .registry
            .get(session_id)
            .await
            .ok_or(SubagentManagerError::NotFound)?;

        Ok(AwaitInboxResult {
            metadata,
            completion: completion_opt,
            messages,
        })
    }

    pub async fn prune(&self, request: PruneRequest) -> Result<PruneReport, SubagentManagerError> {
        if !request.all {
            if let Some(ids) = &request.session_ids {
                if ids.is_empty() {
                    return Err(SubagentManagerError::InvalidPruneRequest(
                        "session_ids list cannot be empty".to_string(),
                    ));
                }
            } else {
                return Err(SubagentManagerError::InvalidPruneRequest(
                    "specify session_ids or set all=true".to_string(),
                ));
            }
        }

        let mut report = PruneReport::default();

        if let Some(ids) = &request.session_ids {
            for id in ids {
                self.prune_single(*id, request.completed_only, &mut report)
                    .await;
            }
        } else {
            let entries = self.registry.list().await;
            for metadata in entries {
                self.prune_with_metadata(metadata, request.completed_only, &mut report)
                    .await;
            }
        }

        Ok(report)
    }

    async fn prune_single(
        &self,
        session_id: ConversationId,
        completed_only: bool,
        report: &mut PruneReport,
    ) {
        if let Some(metadata) = self.registry.get(&session_id).await {
            self.prune_with_metadata(metadata, completed_only, report)
                .await;
        } else {
            if let Some(runtime) = {
                let mut runs = self.runs.write().await;
                runs.remove(&session_id)
            } {
                runtime.shutdown().await;
            }
            {
                let mut completions = self.completions.write().await;
                completions.remove(&session_id);
            }
            {
                let mut logs = self.completed_logs.write().await;
                logs.remove(&session_id);
            }
            {
                let mut inbox = self.completed_inbox.write().await;
                inbox.remove(&session_id);
            }
            report.unknown.push(session_id);
        }
    }

    async fn prune_with_metadata(
        &self,
        metadata: SubagentMetadata,
        completed_only: bool,
        report: &mut PruneReport,
    ) {
        let session_id = metadata.session_id;
        if completed_only && !is_terminal_status(metadata.status) {
            report.skipped_active.push(session_id);
            return;
        }

        if let Some(runtime) = {
            let mut runs = self.runs.write().await;
            runs.remove(&session_id)
        } {
            runtime.shutdown().await;
        }

        {
            let mut completions = self.completions.write().await;
            completions.remove(&session_id);
        }
        {
            let mut logs = self.completed_logs.write().await;
            logs.remove(&session_id);
        }
        {
            let mut inbox = self.completed_inbox.write().await;
            inbox.remove(&session_id);
        }

        self.emit_deleted(&session_id).await;
        self.remove_emitter(&session_id).await;

        if self.registry.remove(&session_id).await.is_some() {
            report.pruned.push(session_id);
        } else {
            report.unknown.push(session_id);
        }
    }

    async fn launch_child(
        &self,
        session_id: ConversationId,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        initial_history: Option<InitialHistory>,
        sandbox_override: Option<SandboxMode>,
        model_override: Option<String>,
    ) -> Result<Arc<ManagedSubagent>, SubagentManagerError> {
        let permit = self.permits.clone().try_acquire_owned().map_err(|_| {
            SubagentManagerError::LimitReached {
                limit: self.max_active_subagents,
            }
        })?;
        let mut config: CodexConfig = turn.client.config().as_ref().clone();
        if let Some(model) = model_override.as_ref() {
            config = config.clone_with_model_override(model).map_err(|err| {
                SubagentManagerError::LaunchFailed(format!(
                    "invalid model `{model}` for subagent: {err}"
                ))
            })?;
        }
        // Ensure the subagent's reasoning settings are valid for its model.
        clamp_reasoning_effort_for_model(&mut config);
        let parent_policy = turn.sandbox_policy.clone();
        if let Some(requested) = sandbox_override {
            let parent_mode = sandbox_mode_from_policy(&parent_policy);
            if sandbox_mode_rank(requested) > sandbox_mode_rank(parent_mode) {
                drop(permit);
                return Err(SubagentManagerError::SandboxOverrideForbidden {
                    requested,
                    parent: parent_mode,
                });
            }
            let policy = sandbox_policy_for_mode(requested, &parent_policy);
            config.sandbox_policy = policy;
        }
        let cancel_token = CancellationToken::new();
        let child = match run_codex_conversation_interactive(
            config,
            Arc::clone(&session.services.auth_manager),
            Arc::clone(&session),
            Arc::clone(&turn),
            cancel_token.clone(),
            Some(session_id),
            initial_history,
            SubAgentSource::Other("subagent_orchestrator".into()),
        )
        .await
        {
            Ok(child) => child,
            Err(err) => {
                drop(permit);
                return Err(SubagentManagerError::LaunchFailed(err.to_string()));
            }
        };

        let entry = Arc::new(ManagedSubagent::new(child, cancel_token, permit));

        {
            let mut runs = self.runs.write().await;
            runs.insert(session_id, Arc::clone(&entry));
        }

        let manager = self.clone();
        let runtime = Arc::clone(&entry);
        tokio::spawn(async move {
            manager.pump_events(session_id, runtime).await;
        });

        let pending_ops_manager = self.clone();
        let pending_ops_runtime = Arc::clone(&entry);
        tokio::spawn(async move {
            pending_ops_manager
                .run_pending_ops(session_id, pending_ops_runtime)
                .await;
        });

        Ok(entry)
    }

    async fn pump_events(&self, session_id: ConversationId, runtime: Arc<ManagedSubagent>) {
        let cancel = runtime.cancellation_token();
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    // Cancellation is triggered only after finalize_terminal has already
                    // updated registry state and shut the runtime down.
                    break;
                }
                next = runtime.codex.next_event() => {
                    match next {
                        Ok(event) => {
                            runtime.record_event(&event).await;
                            match &event.msg {
                                EventMsg::AgentReasoningDelta(delta) => {
                                    if !runtime.append_reasoning_delta(&delta.delta).await {
                                        continue;
                                    }
                                    if let Some(header) = runtime.reasoning_header().await {
                                        self
                                            .update_reasoning_header_and_emit(
                                                &session_id,
                                                header,
                                            )
                                            .await;
                                    }
                                }
                                EventMsg::AgentReasoning(reasoning) => {
                                    if runtime.reasoning_header().await.is_none()
                                        && let Some(header) =
                                            extract_first_bold(&reasoning.text)
                                    {
                                        runtime.set_reasoning_header(header.clone()).await;
                                        self
                                            .update_reasoning_header_and_emit(
                                                &session_id,
                                                header,
                                            )
                                            .await;
                                    }
                                }
                                EventMsg::TaskComplete(ev) => {
                                    runtime
                                        .set_completion(SubagentCompletion::Completed {
                                            last_message: ev.last_agent_message.clone(),
                                        })
                                        .await;

                                    // Treat normal completion as an implicit
                                    // subagent_await: drain any pending inbox
                                    // messages, update terminal status, and
                                    // inject synthetic tool calls / user
                                    // messages into the parent and child
                                    // histories so callers do not need to
                                    // issue an explicit await.
                                    if let Ok(result) = self
                                        .await_inbox_and_completion(
                                            &session_id,
                                            Some(Duration::from_millis(0)),
                                        )
                                        .await
                                    {
                                        self
                                            .deliver_inbox_to_threads_at_yield(&result)
                                            .await;

                                        if let Some(completion) = result.completion.clone() {
                                            let status = status_from_completion(&completion);
                                            self
                                                .finalize_terminal(
                                                    &session_id,
                                                    Arc::clone(&runtime),
                                                    completion,
                                                    status,
                                                )
                                                .await;
                                        }
                                    }
                                }
                                EventMsg::TurnAborted(ev) => {
                                    // Interrupted turn: treat as a
                                    // cancellation, surface any pending inbox
                                    // messages via a synthetic await, then
                                    // finalize the runtime.
                                    runtime
                                        .set_completion(SubagentCompletion::Canceled {
                                            reason: ev.reason.clone(),
                                        })
                                        .await;
                                    if let Ok(result) = self
                                        .await_inbox_and_completion(
                                            &session_id,
                                            Some(Duration::from_millis(0)),
                                        )
                                        .await
                                    {
                                        self
                                            .deliver_inbox_to_threads_at_yield(&result)
                                            .await;
                                    }
                                    self
                                        .finalize_terminal(
                                            &session_id,
                                            Arc::clone(&runtime),
                                            SubagentCompletion::Canceled {
                                                reason: ev.reason.clone(),
                                            },
                                            SubagentStatus::Canceled,
                                        )
                                        .await;
                                    break;
                                }
                                EventMsg::StreamError(ev) => {
                                    runtime
                                        .set_completion(SubagentCompletion::Failed {
                                            message: ev.message.clone(),
                                        })
                                        .await;
                                    if let Ok(result) = self
                                        .await_inbox_and_completion(
                                            &session_id,
                                            Some(Duration::from_millis(0)),
                                        )
                                        .await
                                    {
                                        self
                                            .deliver_inbox_to_threads_at_yield(&result)
                                            .await;
                                    }
                                    self
                                        .finalize_terminal(
                                            &session_id,
                                            Arc::clone(&runtime),
                                            SubagentCompletion::Failed {
                                                message: ev.message.clone(),
                                            },
                                            SubagentStatus::Failed,
                                        )
                                        .await;
                                    break;
                                }
                                EventMsg::Error(ev) => {
                                    runtime
                                        .set_completion(SubagentCompletion::Failed {
                                            message: ev.message.clone(),
                                        })
                                        .await;
                                    if let Ok(result) = self
                                        .await_inbox_and_completion(
                                            &session_id,
                                            Some(Duration::from_millis(0)),
                                        )
                                        .await
                                    {
                                        self
                                            .deliver_inbox_to_threads_at_yield(&result)
                                            .await;
                                    }
                                    self
                                        .finalize_terminal(
                                            &session_id,
                                            Arc::clone(&runtime),
                                            SubagentCompletion::Failed {
                                                message: ev.message.clone(),
                                            },
                                            SubagentStatus::Failed,
                                        )
                                        .await;
                                    break;
                                }
                                _ => {}
                            }
                        }
                        Err(err) => {
                            self
                                .finalize_terminal(
                                    &session_id,
                                    Arc::clone(&runtime),
                                    SubagentCompletion::Failed {
                                        message: err.to_string(),
                                    },
                                    SubagentStatus::Failed,
                                )
                                .await;
                            break;
                        }
                    }
                }
            }
        }
        // Runtime is kept alive after completion so messages can resume; no auto-removal here.
    }

    async fn run_pending_ops(&self, session_id: ConversationId, runtime: Arc<ManagedSubagent>) {
        let cancel = runtime.cancellation_token();
        let notify = runtime.pending_ops_notifier();
        loop {
            while let Some((message, counts)) = runtime.dequeue_message().await {
                let PendingMessage { prompt, interrupt } = message;
                self.update_inbox_counts_and_emit(&session_id, counts.0, counts.1)
                    .await;
                if interrupt {
                    runtime.interrupt().await;
                }
                if let Some(prompt) = prompt {
                    match runtime.submit_prompt(&prompt).await {
                        Ok(true) => {
                            self.update_status_and_emit(&session_id, SubagentStatus::Running)
                                .await;
                        }
                        Ok(false) => {
                            self.update_status_and_emit(&session_id, SubagentStatus::Ready)
                                .await;
                        }
                        Err(err) => {
                            self.finalize_terminal(
                                &session_id,
                                Arc::clone(&runtime),
                                SubagentCompletion::Failed {
                                    message: err.to_string(),
                                },
                                SubagentStatus::Failed,
                            )
                            .await;
                            return;
                        }
                    }
                } else if interrupt {
                    self.update_status_and_emit(&session_id, SubagentStatus::Ready)
                        .await;
                }
            }

            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = notify.notified() => continue,
            }
        }
    }

    async fn finalize_terminal(
        &self,
        session_id: &ConversationId,
        runtime: Arc<ManagedSubagent>,
        completion: SubagentCompletion,
        status: SubagentStatus,
    ) {
        let completion_clone = completion.clone();
        runtime.set_completion(completion.clone()).await;
        self.update_status_and_emit(session_id, status).await;
        {
            let mut completions = self.completions.write().await;
            completions.insert(*session_id, completion_clone);
        }
        let log_snapshot = runtime.snapshot_logs().await;
        {
            let mut logs = self.completed_logs.write().await;
            logs.insert(*session_id, log_snapshot);
        }
        let inbox_snapshot = runtime.snapshot_inbox().await;
        {
            let mut inbox = self.completed_inbox.write().await;
            inbox.insert(*session_id, inbox_snapshot);
        }
        // Do NOT shut down or remove the runtime; keep it alive so further messages
        // can be processed unless explicitly pruned or canceled.

        // Surface completion to the parent as a logical inbox message, so from
        // the parent's perspective a completion is equivalent to a
        // subagent_send_message targeting the parent agent id.
        self.send_completion_to_parent(session_id, completion).await;
    }

    async fn remove_runtime_entry(
        &self,
        session_id: &ConversationId,
    ) -> Option<Arc<ManagedSubagent>> {
        let mut runs = self.runs.write().await;
        runs.remove(session_id)
    }

    async fn handle_launch_failure(
        &self,
        session_id: ConversationId,
        runtime: Arc<ManagedSubagent>,
        err: CodexErr,
    ) {
        self.finalize_terminal(
            &session_id,
            runtime,
            SubagentCompletion::Failed {
                message: err.to_string(),
            },
            SubagentStatus::Failed,
        )
        .await;
    }

    async fn send_completion_to_parent(
        &self,
        session_id: &ConversationId,
        completion: SubagentCompletion,
    ) {
        let Some(metadata) = self.registry.get(session_id).await else {
            return;
        };

        let Some(parent_session_id) = metadata.parent_session_id else {
            return;
        };

        // If the parent is the root agent (id 0), enqueue into the root inbox.
        if metadata.parent_agent_id == Some(ROOT_AGENT_ID) {
            self.enqueue_root_inbox_completion(
                &parent_session_id,
                session_id,
                completion,
                metadata.clone(),
            )
            .await;
        } else if let Some(parent_agent_id) = metadata.parent_agent_id {
            let completion_prompt = match &completion {
                SubagentCompletion::Completed { last_message } => last_message
                    .clone()
                    .unwrap_or_else(|| "subagent completed".to_string()),
                SubagentCompletion::Failed { message } => {
                    format!("subagent failed: {message}")
                }
                SubagentCompletion::Canceled { reason } => {
                    format!("subagent canceled: {reason:?}")
                }
            };

            // Otherwise, treat the parent as another subagent and enqueue a
            // logical message into its inbox.
            let _ = self
                .send_message(SendMessageRequest {
                    session_id: parent_session_id,
                    label: None,
                    summary: None,
                    prompt: Some(completion_prompt),
                    agent_id: parent_agent_id,
                    sender_agent_id: metadata.agent_id,
                    interrupt: false,
                })
                .await;
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SubagentManagerError {
    #[error("subagent session not found")]
    NotFound,
    #[error("agent id mismatch for session {session_id} (agent_id {agent_id})")]
    AgentIdMismatch {
        session_id: ConversationId,
        agent_id: AgentId,
    },
    #[error("failed to launch subagent: {0}")]
    LaunchFailed(String),
    #[error("active subagent limit ({limit}) reached")]
    LimitReached { limit: usize },
    #[error("timed out waiting {timeout_ms} ms for agent_id {agent_id}")]
    AwaitTimedOut {
        session_id: ConversationId,
        agent_id: AgentId,
        timeout_ms: u64,
    },
    #[error("invalid prune request: {0}")]
    InvalidPruneRequest(String),
    #[error(
        "sandbox override {requested:?} not permitted because parent session runs in {parent:?}"
    )]
    SandboxOverrideForbidden {
        requested: SandboxMode,
        parent: SandboxMode,
    },
}

#[derive(Debug, Clone)]
pub struct SpawnRequest {
    pub prompt: String,
    pub label: Option<String>,
    pub summary: Option<String>,
    pub sandbox_mode: Option<SandboxMode>,
    pub model: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ForkRequest {
    pub parent_session_id: ConversationId,
    pub initial_message_count: usize,
    pub label: Option<String>,
    pub summary: Option<String>,
    pub call_id: String,
    pub arguments: String,
    pub prompt: Option<String>,
    pub sandbox_mode: Option<SandboxMode>,
    pub model: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SendMessageRequest {
    pub session_id: ConversationId,
    pub label: Option<String>,
    pub summary: Option<String>,
    pub prompt: Option<String>,
    pub agent_id: AgentId,
    /// Agent id of the sender; this is used for inbox
    /// attribution so awaiters can see who sent each
    /// message.
    pub sender_agent_id: AgentId,
    pub interrupt: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct InboxMessage {
    pub sender_agent_id: AgentId,
    pub recipient_agent_id: AgentId,
    pub interrupt: bool,
    pub prompt: Option<String>,
    pub timestamp_ms: i64,
}

#[derive(Clone, Debug)]
struct PendingMessage {
    prompt: Option<String>,
    interrupt: bool,
}

#[derive(Default)]
struct PendingOpsQueues {
    regular: VecDeque<PendingMessage>,
    interrupts: VecDeque<PendingMessage>,
}

impl PendingOpsQueues {
    fn counts(&self) -> (usize, usize) {
        (self.regular.len(), self.interrupts.len())
    }

    fn push(&mut self, message: PendingMessage) {
        if message.interrupt {
            self.interrupts.push_back(message);
        } else {
            self.regular.push_back(message);
        }
    }

    fn pop(&mut self) -> Option<PendingMessage> {
        if let Some(msg) = self.interrupts.pop_front() {
            return Some(msg);
        }
        self.regular.pop_front()
    }
}

#[derive(Clone)]
struct SubagentEmitter {
    session: Arc<Session>,
    turn: Arc<TurnContext>,
}

struct ManagedSubagent {
    codex: Codex,
    cancel_token: CancellationToken,
    logs: Mutex<VecDeque<LoggedEvent>>,
    completion_tx: watch::Sender<Option<SubagentCompletion>>,
    completion_rx: watch::Receiver<Option<SubagentCompletion>>,
    reasoning_buffer: Mutex<String>,
    reasoning_header: Mutex<Option<String>>,
    inbox: Mutex<VecDeque<InboxMessage>>,
    inbox_notify: Arc<Notify>,
    pending_ops: Mutex<PendingOpsQueues>,
    pending_ops_notify: Arc<Notify>,
    _permit: Mutex<Option<OwnedSemaphorePermit>>,
}

impl ManagedSubagent {
    fn new(codex: Codex, cancel_token: CancellationToken, permit: OwnedSemaphorePermit) -> Self {
        let (completion_tx, completion_rx) = watch::channel(None);
        Self {
            codex,
            cancel_token,
            logs: Mutex::new(VecDeque::with_capacity(LOG_CAPACITY)),
            completion_tx,
            completion_rx,
            reasoning_buffer: Mutex::new(String::new()),
            reasoning_header: Mutex::new(None),
            inbox: Mutex::new(VecDeque::new()),
            inbox_notify: Arc::new(Notify::new()),
            pending_ops: Mutex::new(PendingOpsQueues::default()),
            pending_ops_notify: Arc::new(Notify::new()),
            _permit: Mutex::new(Some(permit)),
        }
    }

    async fn submit_prompt(&self, prompt: &str) -> Result<bool, CodexErr> {
        if prompt.trim().is_empty() {
            return Ok(false);
        }
        let items = vec![UserInput::Text {
            text: prompt.to_string(),
        }];
        self.codex
            .submit(codex_protocol::protocol::Op::UserInput { items })
            .await?;
        Ok(true)
    }

    async fn enqueue_message(&self, message: PendingMessage) -> (usize, usize) {
        let mut guard = self.pending_ops.lock().await;
        guard.push(message);
        let counts = guard.counts();
        self.pending_ops_notify.notify_one();
        counts
    }

    async fn dequeue_message(&self) -> Option<(PendingMessage, (usize, usize))> {
        let mut guard = self.pending_ops.lock().await;
        guard.pop().map(|msg| (msg, guard.counts()))
    }

    fn pending_ops_notifier(&self) -> Arc<Notify> {
        Arc::clone(&self.pending_ops_notify)
    }

    async fn enqueue_inbox_message(&self, message: InboxMessage) -> usize {
        let mut guard = self.inbox.lock().await;
        guard.push_back(message);
        let len = guard.len();
        self.inbox_notify.notify_one();
        len
    }

    async fn drain_inbox(&self) -> Vec<InboxMessage> {
        let mut guard = self.inbox.lock().await;
        guard.drain(..).collect()
    }

    async fn snapshot_inbox(&self) -> Vec<InboxMessage> {
        let guard = self.inbox.lock().await;
        guard.iter().cloned().collect()
    }

    fn inbox_notifier(&self) -> Arc<Notify> {
        Arc::clone(&self.inbox_notify)
    }

    async fn interrupt(&self) {
        let _ = self
            .codex
            .submit(codex_protocol::protocol::Op::Interrupt)
            .await;
    }

    fn cancellation_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    async fn shutdown(&self) {
        let _ = self
            .codex
            .submit(codex_protocol::protocol::Op::Shutdown {})
            .await;
        self.cancel_token.cancel();
    }

    async fn record_event(&self, event: &Event) {
        let mut logs = self.logs.lock().await;
        if logs.len() >= LOG_CAPACITY {
            logs.pop_front();
        }
        logs.push_back(LoggedEvent::new(event.clone()));
    }

    async fn set_completion(&self, completion: SubagentCompletion) {
        let _ = self.completion_tx.send_replace(Some(completion));
    }

    fn clear_completion(&self) {
        let _ = self.completion_tx.send_replace(None);
    }

    fn completion_receiver(&self) -> watch::Receiver<Option<SubagentCompletion>> {
        self.completion_rx.clone()
    }

    async fn snapshot_logs(&self) -> Vec<LoggedEvent> {
        let logs = self.logs.lock().await;
        logs.iter().cloned().collect()
    }

    async fn reasoning_header(&self) -> Option<String> {
        self.reasoning_header.lock().await.clone()
    }

    async fn set_reasoning_header(&self, header: String) {
        let mut guard = self.reasoning_header.lock().await;
        *guard = Some(header);
    }

    async fn append_reasoning_delta(&self, delta: &str) -> bool {
        {
            let guard = self.reasoning_header.lock().await;
            if guard.is_some() {
                return true;
            }
        }
        let mut buf = self.reasoning_buffer.lock().await;
        buf.push_str(delta);
        if let Some(header) = extract_first_bold(&buf) {
            drop(buf);
            self.set_reasoning_header(header).await;
            return true;
        }
        false
    }
}

#[derive(Clone)]
struct LoggedEvent {
    timestamp: SystemTime,
    event: Event,
}

impl LoggedEvent {
    fn new(event: Event) -> Self {
        Self {
            timestamp: SystemTime::now(),
            event,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp_ms: i64,
    pub event: Event,
}

impl LogEntry {
    fn from_logged(src: &LoggedEvent) -> Self {
        Self {
            timestamp_ms: unix_time_millis(src.timestamp),
            event: src.event.clone(),
        }
    }
}

fn unix_time_millis(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as i64,
        Err(err) => -(err.duration().as_millis() as i64),
    }
}

fn extract_first_bold(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] == b'*' && bytes[i + 1] == b'*' {
            let start = i + 2;
            let mut j = start;
            while j + 1 < bytes.len() {
                if bytes[j] == b'*' && bytes[j + 1] == b'*' {
                    let inner = &s[start..j];
                    let trimmed = inner.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    } else {
                        return None;
                    }
                }
                j += 1;
            }
            break;
        }
        i += 1;
    }
    None
}

fn sandbox_mode_from_policy(policy: &SandboxPolicy) -> SandboxMode {
    match policy {
        SandboxPolicy::DangerFullAccess => SandboxMode::DangerFullAccess,
        SandboxPolicy::WorkspaceWrite { .. } => SandboxMode::WorkspaceWrite,
        SandboxPolicy::ReadOnly => SandboxMode::ReadOnly,
    }
}

fn sandbox_mode_rank(mode: SandboxMode) -> u8 {
    match mode {
        SandboxMode::DangerFullAccess => 3,
        SandboxMode::WorkspaceWrite => 2,
        SandboxMode::ReadOnly => 1,
    }
}

fn sandbox_policy_for_mode(requested: SandboxMode, parent_policy: &SandboxPolicy) -> SandboxPolicy {
    match requested {
        SandboxMode::ReadOnly => SandboxPolicy::new_read_only_policy(),
        SandboxMode::WorkspaceWrite => match parent_policy {
            SandboxPolicy::WorkspaceWrite { .. } => parent_policy.clone(),
            _ => SandboxPolicy::new_workspace_write_policy(),
        },
        SandboxMode::DangerFullAccess => SandboxPolicy::DangerFullAccess,
    }
}

fn to_subagent_summary(metadata: &SubagentMetadata) -> SubagentSummary {
    SubagentSummary {
        agent_id: metadata.agent_id,
        parent_agent_id: metadata.parent_agent_id,
        session_id: metadata.session_id,
        parent_session_id: metadata.parent_session_id,
        origin: metadata.origin.into(),
        status: metadata.status.into(),
        label: metadata.label.clone(),
        summary: metadata.summary.clone(),
        reasoning_header: metadata.reasoning_header.clone(),
        started_at_ms: metadata.created_at_ms,
        pending_messages: metadata.pending_messages,
        pending_interrupts: metadata.pending_interrupts,
    }
}

impl From<SubagentOrigin> for SubagentLifecycleOrigin {
    fn from(value: SubagentOrigin) -> Self {
        match value {
            SubagentOrigin::Spawn => SubagentLifecycleOrigin::Spawn,
            SubagentOrigin::Fork => SubagentLifecycleOrigin::Fork,
            SubagentOrigin::SendMessage => SubagentLifecycleOrigin::SendMessage,
        }
    }
}

impl From<SubagentStatus> for SubagentLifecycleStatus {
    fn from(value: SubagentStatus) -> Self {
        match value {
            SubagentStatus::Queued => SubagentLifecycleStatus::Queued,
            SubagentStatus::Running => SubagentLifecycleStatus::Running,
            SubagentStatus::Ready => SubagentLifecycleStatus::Ready,
            SubagentStatus::Idle => SubagentLifecycleStatus::Idle,
            SubagentStatus::Failed => SubagentLifecycleStatus::Failed,
            SubagentStatus::Canceled => SubagentLifecycleStatus::Canceled,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub enum SubagentCompletion {
    Completed { last_message: Option<String> },
    Canceled { reason: TurnAbortReason },
    Failed { message: String },
}

#[derive(Clone, Debug, Serialize)]
pub struct AwaitResult {
    pub metadata: SubagentMetadata,
    pub completion: SubagentCompletion,
}

#[derive(Clone, Debug, Serialize)]
pub struct AwaitInboxResult {
    pub metadata: SubagentMetadata,
    pub completion: Option<SubagentCompletion>,
    pub messages: Vec<InboxMessage>,
}

#[derive(Clone, Debug)]
pub struct PruneRequest {
    pub session_ids: Option<Vec<ConversationId>>,
    pub all: bool,
    pub completed_only: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PruneErrorEntry {
    pub session_id: ConversationId,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PruneReport {
    pub pruned: Vec<ConversationId>,
    pub skipped_active: Vec<ConversationId>,
    pub unknown: Vec<ConversationId>,
    pub errors: Vec<PruneErrorEntry>,
}

async fn build_fork_history(session: Arc<Session>) -> InitialHistory {
    let mut history = session.clone_history().await;
    let items = history.get_history_for_prompt();
    if items.is_empty() {
        InitialHistory::New
    } else {
        let rollout = items
            .into_iter()
            .map(RolloutItem::ResponseItem)
            .collect::<Vec<_>>();
        InitialHistory::Forked(rollout)
    }
}

fn remove_call_items(items: &mut Vec<RolloutItem>, call_id: &str) {
    items.retain(|item| {
        !matches!(
            item,
            RolloutItem::ResponseItem(ResponseItem::FunctionCall { call_id: cid, .. })
                if cid == call_id
        ) && !matches!(
            item,
            RolloutItem::ResponseItem(ResponseItem::FunctionCallOutput { call_id: cid, .. })
                if cid == call_id
        )
    });
}

fn current_completion(
    receiver: &watch::Receiver<Option<SubagentCompletion>>,
) -> Option<SubagentCompletion> {
    receiver.borrow().clone()
}

fn status_from_completion(completion: &SubagentCompletion) -> SubagentStatus {
    match completion {
        SubagentCompletion::Completed { .. } => SubagentStatus::Idle,
        SubagentCompletion::Failed { .. } => SubagentStatus::Failed,
        SubagentCompletion::Canceled { .. } => SubagentStatus::Canceled,
    }
}

fn is_terminal_status(status: SubagentStatus) -> bool {
    matches!(
        status,
        SubagentStatus::Idle | SubagentStatus::Failed | SubagentStatus::Canceled
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::SUBMISSION_CHANNEL_CAPACITY;
    use async_channel::bounded;
    use codex_protocol::protocol::TurnAbortReason;

    use std::time::Duration;

    async fn setup_await_fixture() -> (Arc<SubagentManager>, ConversationId, Arc<ManagedSubagent>) {
        let registry = Arc::new(SubagentRegistry::new());
        let manager = Arc::new(SubagentManager::new(
            Arc::clone(&registry),
            2,
            false,
            false,
            false,
        ));
        let session_id = ConversationId::new();
        registry
            .register_spawn(
                session_id,
                None,
                Some(ROOT_AGENT_ID),
                1,
                0,
                Some("child".to_string()),
                None,
            )
            .await;

        let permit = manager
            .permits
            .clone()
            .acquire_owned()
            .await
            .expect("permit");
        let (tx_sub, _rx_sub) = bounded(SUBMISSION_CHANNEL_CAPACITY);
        let (_tx_event, rx_event) = bounded(SUBMISSION_CHANNEL_CAPACITY);
        let codex = Codex {
            next_id: AtomicU64::new(1),
            tx_sub,
            rx_event,
            conversation_id: session_id,
        };
        let runtime = Arc::new(ManagedSubagent::new(
            codex,
            CancellationToken::new(),
            permit,
        ));
        manager
            .runs
            .write()
            .await
            .insert(session_id, Arc::clone(&runtime));

        (manager, session_id, runtime)
    }

    #[tokio::test]
    async fn await_inbox_unblocks_on_message() {
        let (manager, session_id, runtime) = setup_await_fixture().await;
        let waiter = tokio::spawn({
            let manager = Arc::clone(&manager);
            async move {
                manager
                    .await_inbox_and_completion(&session_id, Some(Duration::from_secs(30)))
                    .await
                    .expect("await result")
            }
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        runtime
            .enqueue_inbox_message(InboxMessage {
                sender_agent_id: 1,
                recipient_agent_id: ROOT_AGENT_ID,
                interrupt: false,
                prompt: Some("hello".to_string()),
                timestamp_ms: 1,
            })
            .await;

        let result = waiter.await.expect("join awaiter");
        assert!(result.completion.is_none());
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].prompt.as_deref(), Some("hello"));
    }

    #[tokio::test]
    async fn await_inbox_unblocks_on_completion() {
        let (manager, session_id, runtime) = setup_await_fixture().await;
        let waiter = tokio::spawn({
            let manager = Arc::clone(&manager);
            async move {
                manager
                    .await_inbox_and_completion(&session_id, None)
                    .await
                    .expect("await result")
            }
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        runtime
            .completion_tx
            .send_replace(Some(SubagentCompletion::Completed {
                last_message: Some("done".to_string()),
            }));

        let result = waiter.await.expect("join awaiter");
        assert!(result.messages.is_empty());
        assert!(matches!(
            result.completion,
            Some(SubagentCompletion::Completed { .. })
        ));
    }

    #[tokio::test]
    async fn await_completion_marks_status_completed() {
        let registry = Arc::new(SubagentRegistry::new());
        let manager = SubagentManager::new(Arc::clone(&registry), 1, false, false, false);
        let session_id = ConversationId::new();
        registry
            .register_spawn(session_id, None, None, 1, 0, None, None)
            .await;

        {
            let mut completions = manager.completions.write().await;
            completions.insert(
                session_id,
                SubagentCompletion::Completed {
                    last_message: Some("done".to_string()),
                },
            );
        }

        let result = manager
            .await_completion(&session_id, None)
            .await
            .expect("await result");
        assert_eq!(result.metadata.status, SubagentStatus::Idle);
    }

    #[tokio::test]
    async fn await_completion_preserves_failed_and_canceled() {
        let registry = Arc::new(SubagentRegistry::new());
        let manager = SubagentManager::new(Arc::clone(&registry), 1, false, false, false);
        let session_id = ConversationId::new();
        registry
            .register_spawn(session_id, None, None, 1, 0, None, None)
            .await;

        for completion in [
            SubagentCompletion::Failed {
                message: "boom".to_string(),
            },
            SubagentCompletion::Canceled {
                reason: TurnAbortReason::Interrupted,
            },
        ] {
            {
                let mut completions = manager.completions.write().await;
                completions.insert(session_id, completion.clone());
            }

            let result = manager
                .await_completion(&session_id, None)
                .await
                .expect("await result");

            let expected_status = status_from_completion(&completion);
            assert_eq!(result.metadata.status, expected_status);
            assert_eq!(status_from_completion(&completion), expected_status);
        }
    }

    #[tokio::test]
    async fn cancel_without_runtime_marks_status_canceled() {
        let registry = Arc::new(SubagentRegistry::new());
        let manager = SubagentManager::new(Arc::clone(&registry), 1, false, false, false);
        let session_id = ConversationId::new();
        registry
            .register_spawn(
                session_id,
                Some(ConversationId::new()),
                Some(1),
                2,
                0,
                None,
                None,
            )
            .await;

        let metadata = manager.cancel(session_id).await.expect("cancel result");
        assert_eq!(metadata.status, SubagentStatus::Canceled);

        let stored = registry.get(&session_id).await.expect("metadata");
        assert_eq!(stored.status, SubagentStatus::Canceled);

        let completions = manager.completions.read().await;
        assert!(matches!(
            completions.get(&session_id),
            Some(SubagentCompletion::Canceled { .. })
        ));
    }

    #[tokio::test]
    async fn root_inbox_carries_completion_payload() {
        let registry = Arc::new(SubagentRegistry::new());
        let manager = SubagentManager::new(Arc::clone(&registry), 4, false, true, false);
        let root_session = ConversationId::new();
        let child_session = ConversationId::new();

        registry
            .register_spawn(
                child_session,
                Some(root_session),
                Some(ROOT_AGENT_ID),
                1,
                0,
                Some("child".to_string()),
                None,
            )
            .await;

        manager
            .send_completion_to_parent(
                &child_session,
                SubagentCompletion::Completed {
                    last_message: Some("done".to_string()),
                },
            )
            .await;

        let items = manager.drain_root_inbox_to_items(&root_session).await;
        assert_eq!(items.len(), 2);

        let content = items
            .iter()
            .find_map(|item| match item {
                ResponseItem::FunctionCallOutput { output, .. } => Some(output.content.clone()),
                _ => None,
            })
            .expect("function call output");
        let payload: serde_json::Value = serde_json::from_str(&content).expect("valid json");
        assert_eq!(payload["completion_status"], "completed");
        assert_eq!(
            payload["completion"]["Completed"]["last_message"],
            serde_json::Value::String("done".to_string())
        );
        assert_eq!(payload["messages"], serde_json::Value::Array(Vec::new()));
    }

    #[tokio::test]
    async fn root_inbox_preserves_fifo_across_senders() {
        let registry = Arc::new(SubagentRegistry::new());
        let manager = SubagentManager::new(Arc::clone(&registry), 4, false, true, false);
        let root_session = ConversationId::new();
        let child_a = ConversationId::new();
        let child_b = ConversationId::new();

        registry
            .register_spawn(
                child_a,
                Some(root_session),
                Some(ROOT_AGENT_ID),
                1,
                0,
                Some("alpha".to_string()),
                None,
            )
            .await;
        registry
            .register_spawn(
                child_b,
                Some(root_session),
                Some(ROOT_AGENT_ID),
                2,
                0,
                Some("beta".to_string()),
                None,
            )
            .await;

        manager
            .enqueue_root_inbox_message(
                &root_session,
                &child_b,
                InboxMessage {
                    sender_agent_id: 2,
                    recipient_agent_id: ROOT_AGENT_ID,
                    interrupt: false,
                    prompt: Some("m2-first".to_string()),
                    timestamp_ms: 5,
                },
            )
            .await;
        manager
            .enqueue_root_inbox_message(
                &root_session,
                &child_a,
                InboxMessage {
                    sender_agent_id: 1,
                    recipient_agent_id: ROOT_AGENT_ID,
                    interrupt: false,
                    prompt: Some("m1-only".to_string()),
                    timestamp_ms: 10,
                },
            )
            .await;
        manager
            .enqueue_root_inbox_message(
                &root_session,
                &child_b,
                InboxMessage {
                    sender_agent_id: 2,
                    recipient_agent_id: ROOT_AGENT_ID,
                    interrupt: false,
                    prompt: Some("m2-late".to_string()),
                    timestamp_ms: 15,
                },
            )
            .await;

        let items = manager.drain_root_inbox_to_items(&root_session).await;
        assert_eq!(items.len(), 4);

        let mut order = Vec::new();
        for chunk in items.chunks(2) {
            if let [
                ResponseItem::FunctionCall { arguments: _, .. },
                ResponseItem::FunctionCallOutput { output, .. },
            ] = chunk
            {
                let payload: serde_json::Value =
                    serde_json::from_str(&output.content).expect("payload json");
                let agent_id = payload["metadata"]["agent_id"].as_i64().expect("agent id");
                order.push(agent_id);
                let prompts = payload["messages"]
                    .as_array()
                    .expect("messages array")
                    .iter()
                    .map(|entry| entry["prompt"].as_str().unwrap().to_string())
                    .collect::<Vec<_>>();
                if agent_id == 2 {
                    assert_eq!(prompts, vec!["m2-first".to_string(), "m2-late".to_string()]);
                } else {
                    assert_eq!(prompts, vec!["m1-only".to_string()]);
                }
            } else {
                panic!("unexpected chunk layout");
            }
        }

        assert_eq!(order, vec![2, 1]);
    }
}
