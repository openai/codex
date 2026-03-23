use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::CollabAgentToolCallStatus;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::PatchChangeKind;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadTokenUsage;
use codex_app_server_protocol::TurnStatus;
use codex_core::config::Config;
use codex_protocol::models::WebSearchAction;
use codex_protocol::protocol::SessionConfiguredEvent;
use serde_json::json;

pub use crate::event_processor::CodexStatus;
use crate::event_processor::EventProcessor;
use crate::event_processor::handle_last_message;
use crate::exec_events::AgentMessageItem;
use crate::exec_events::CollabAgentState;
use crate::exec_events::CollabAgentStatus;
use crate::exec_events::CollabTool;
use crate::exec_events::CollabToolCallItem;
use crate::exec_events::CollabToolCallStatus;
use crate::exec_events::CommandExecutionItem;
use crate::exec_events::CommandExecutionStatus as ExecCommandExecutionStatus;
use crate::exec_events::ErrorItem;
use crate::exec_events::FileChangeItem;
use crate::exec_events::FileUpdateChange;
use crate::exec_events::ItemCompletedEvent;
use crate::exec_events::ItemStartedEvent;
use crate::exec_events::ItemUpdatedEvent;
use crate::exec_events::McpToolCallItem;
use crate::exec_events::McpToolCallItemError;
use crate::exec_events::McpToolCallItemResult;
use crate::exec_events::McpToolCallStatus as ExecMcpToolCallStatus;
use crate::exec_events::PatchApplyStatus as ExecPatchApplyStatus;
use crate::exec_events::PatchChangeKind as ExecPatchChangeKind;
use crate::exec_events::ReasoningItem;
use crate::exec_events::ThreadErrorEvent;
use crate::exec_events::ThreadEvent;
use crate::exec_events::ThreadItem as ExecThreadItem;
use crate::exec_events::ThreadItemDetails;
use crate::exec_events::ThreadStartedEvent;
use crate::exec_events::TodoItem;
use crate::exec_events::TodoListItem;
use crate::exec_events::TurnCompletedEvent;
use crate::exec_events::TurnFailedEvent;
use crate::exec_events::TurnStartedEvent;
use crate::exec_events::Usage;
use crate::exec_events::WebSearchItem;

pub struct EventProcessorWithJsonOutput {
    last_message_path: Option<PathBuf>,
    next_item_id: AtomicU64,
    raw_to_exec_item_id: HashMap<String, String>,
    running_todo_list: Option<RunningTodoList>,
    last_total_token_usage: Option<ThreadTokenUsage>,
    last_critical_error: Option<ThreadErrorEvent>,
    final_message: Option<String>,
}

#[derive(Debug, Clone)]
struct RunningTodoList {
    item_id: String,
    items: Vec<TodoItem>,
}

#[derive(Debug, PartialEq)]
pub struct CollectedThreadEvents {
    pub events: Vec<ThreadEvent>,
    pub status: CodexStatus,
}

impl EventProcessorWithJsonOutput {
    pub fn new(last_message_path: Option<PathBuf>) -> Self {
        Self {
            last_message_path,
            next_item_id: AtomicU64::new(0),
            raw_to_exec_item_id: HashMap::new(),
            running_todo_list: None,
            last_total_token_usage: None,
            last_critical_error: None,
            final_message: None,
        }
    }

    pub fn final_message(&self) -> Option<&str> {
        self.final_message.as_deref()
    }

    fn next_item_id(&self) -> String {
        format!("item_{}", self.next_item_id.fetch_add(1, Ordering::SeqCst))
    }

    #[allow(clippy::print_stdout)]
    fn emit(&self, event: ThreadEvent) {
        println!(
            "{}",
            serde_json::to_string(&event).unwrap_or_else(|err| {
                json!({
                    "type": "error",
                    "message": format!("failed to serialize exec json event: {err}"),
                })
                .to_string()
            })
        );
    }

    fn usage_from_last_total(&self) -> Usage {
        let Some(usage) = self.last_total_token_usage.as_ref() else {
            return Usage::default();
        };
        Usage {
            input_tokens: usage.total.input_tokens,
            cached_input_tokens: usage.total.cached_input_tokens,
            output_tokens: usage.total.output_tokens,
        }
    }

    pub fn map_todo_items(plan: &[codex_app_server_protocol::TurnPlanStep]) -> Vec<TodoItem> {
        plan.iter()
            .map(|step| TodoItem {
                text: step.step.clone(),
                completed: matches!(
                    step.status,
                    codex_app_server_protocol::TurnPlanStepStatus::Completed
                ),
            })
            .collect()
    }

    fn map_item_with_id(
        item: ThreadItem,
        make_id: impl FnOnce() -> String,
    ) -> Option<ExecThreadItem> {
        match item {
            ThreadItem::AgentMessage { text, .. } => Some(ExecThreadItem {
                id: make_id(),
                details: ThreadItemDetails::AgentMessage(AgentMessageItem { text }),
            }),
            ThreadItem::Reasoning { summary, .. } => {
                let text = summary.join("\n");
                if text.trim().is_empty() {
                    return None;
                }
                Some(ExecThreadItem {
                    id: make_id(),
                    details: ThreadItemDetails::Reasoning(ReasoningItem { text }),
                })
            }
            ThreadItem::CommandExecution {
                command,
                aggregated_output,
                exit_code,
                status,
                ..
            } => Some(ExecThreadItem {
                id: make_id(),
                details: ThreadItemDetails::CommandExecution(CommandExecutionItem {
                    command,
                    aggregated_output: aggregated_output.unwrap_or_default(),
                    exit_code,
                    status: match status {
                        CommandExecutionStatus::InProgress => ExecCommandExecutionStatus::InProgress,
                        CommandExecutionStatus::Completed => ExecCommandExecutionStatus::Completed,
                        CommandExecutionStatus::Failed => ExecCommandExecutionStatus::Failed,
                        CommandExecutionStatus::Declined => ExecCommandExecutionStatus::Declined,
                    },
                }),
            }),
            ThreadItem::FileChange {
                changes, status, ..
            } => Some(ExecThreadItem {
                id: make_id(),
                details: ThreadItemDetails::FileChange(FileChangeItem {
                    changes: changes
                        .into_iter()
                        .map(|change| FileUpdateChange {
                            path: change.path,
                            kind: match change.kind {
                                PatchChangeKind::Add => ExecPatchChangeKind::Add,
                                PatchChangeKind::Delete => ExecPatchChangeKind::Delete,
                                PatchChangeKind::Update { .. } => ExecPatchChangeKind::Update,
                            },
                        })
                        .collect(),
                    status: match status {
                        PatchApplyStatus::InProgress => ExecPatchApplyStatus::InProgress,
                        PatchApplyStatus::Completed => ExecPatchApplyStatus::Completed,
                        PatchApplyStatus::Failed | PatchApplyStatus::Declined => {
                            ExecPatchApplyStatus::Failed
                        }
                    },
                }),
            }),
            ThreadItem::McpToolCall {
                server,
                tool,
                status,
                arguments,
                result,
                error,
                ..
            } => Some(ExecThreadItem {
                id: make_id(),
                details: ThreadItemDetails::McpToolCall(McpToolCallItem {
                    server,
                    tool,
                    status: match status {
                        McpToolCallStatus::InProgress => ExecMcpToolCallStatus::InProgress,
                        McpToolCallStatus::Completed => ExecMcpToolCallStatus::Completed,
                        McpToolCallStatus::Failed => ExecMcpToolCallStatus::Failed,
                    },
                    arguments,
                    result: result.map(|result| McpToolCallItemResult {
                        content: result.content,
                        structured_content: result.structured_content,
                    }),
                    error: error.map(|error| McpToolCallItemError {
                        message: error.message,
                    }),
                }),
            }),
            ThreadItem::CollabAgentToolCall {
                tool,
                sender_thread_id,
                receiver_thread_ids,
                prompt,
                agents_states,
                status,
                ..
            } => Some(ExecThreadItem {
                id: make_id(),
                details: ThreadItemDetails::CollabToolCall(CollabToolCallItem {
                    tool: match tool {
                        CollabAgentTool::SpawnAgent => CollabTool::SpawnAgent,
                        CollabAgentTool::SendInput => CollabTool::SendInput,
                        CollabAgentTool::ResumeAgent => CollabTool::Wait,
                        CollabAgentTool::Wait => CollabTool::Wait,
                        CollabAgentTool::CloseAgent => CollabTool::CloseAgent,
                    },
                    sender_thread_id,
                    receiver_thread_ids,
                    prompt,
                    agents_states: agents_states
                        .into_iter()
                        .map(|(thread_id, state)| {
                            (
                                thread_id,
                                CollabAgentState {
                                    status: match state.status {
                                        codex_app_server_protocol::CollabAgentStatus::PendingInit => {
                                            CollabAgentStatus::PendingInit
                                        }
                                        codex_app_server_protocol::CollabAgentStatus::Running => {
                                            CollabAgentStatus::Running
                                        }
                                        codex_app_server_protocol::CollabAgentStatus::Interrupted => {
                                            CollabAgentStatus::Interrupted
                                        }
                                        codex_app_server_protocol::CollabAgentStatus::Completed => {
                                            CollabAgentStatus::Completed
                                        }
                                        codex_app_server_protocol::CollabAgentStatus::Errored => {
                                            CollabAgentStatus::Errored
                                        }
                                        codex_app_server_protocol::CollabAgentStatus::Shutdown => {
                                            CollabAgentStatus::Shutdown
                                        }
                                        codex_app_server_protocol::CollabAgentStatus::NotFound => {
                                            CollabAgentStatus::NotFound
                                        }
                                    },
                                    message: state.message,
                                },
                            )
                        })
                        .collect(),
                    status: match status {
                        CollabAgentToolCallStatus::InProgress => CollabToolCallStatus::InProgress,
                        CollabAgentToolCallStatus::Completed => CollabToolCallStatus::Completed,
                        CollabAgentToolCallStatus::Failed => CollabToolCallStatus::Failed,
                    },
                }),
            }),
            ThreadItem::WebSearch {
                id: raw_id,
                query,
                action,
            } => Some(ExecThreadItem {
                id: make_id(),
                details: ThreadItemDetails::WebSearch(WebSearchItem {
                    id: raw_id,
                    query,
                    action: match action {
                        Some(action) => serde_json::from_value(
                            serde_json::to_value(action).unwrap_or_else(|_| json!("other")),
                        )
                        .unwrap_or(WebSearchAction::Other),
                        None => WebSearchAction::Other,
                    },
                }),
            }),
            _ => None,
        }
    }

    fn started_item_id(&mut self, raw_id: &str) -> String {
        if let Some(existing) = self.raw_to_exec_item_id.get(raw_id) {
            return existing.clone();
        }
        let exec_id = self.next_item_id();
        self.raw_to_exec_item_id
            .insert(raw_id.to_string(), exec_id.clone());
        exec_id
    }

    fn completed_item_id(&mut self, raw_id: &str) -> String {
        self.raw_to_exec_item_id
            .remove(raw_id)
            .unwrap_or_else(|| self.next_item_id())
    }

    fn map_started_item(&mut self, item: ThreadItem) -> Option<ExecThreadItem> {
        match item {
            ThreadItem::AgentMessage { .. } | ThreadItem::Reasoning { .. } => None,
            other => {
                let raw_id = other.id().to_string();
                Self::map_item_with_id(other, || self.started_item_id(&raw_id))
            }
        }
    }

    fn map_completed_item_mut(&mut self, item: ThreadItem) -> Option<ExecThreadItem> {
        if let ThreadItem::Reasoning { summary, .. } = &item
            && summary.join("\n").trim().is_empty()
        {
            return None;
        }
        match &item {
            ThreadItem::AgentMessage { .. } | ThreadItem::Reasoning { .. } => {
                Self::map_item_with_id(item, || self.next_item_id())
            }
            other => {
                let raw_id = other.id().to_string();
                Self::map_item_with_id(item, || self.completed_item_id(&raw_id))
            }
        }
    }

    fn final_message_from_turn_items(items: &[ThreadItem]) -> Option<String> {
        items
            .iter()
            .rev()
            .find_map(|item| match item {
                ThreadItem::AgentMessage { text, .. } => Some(text.clone()),
                _ => None,
            })
            .or_else(|| {
                items.iter().rev().find_map(|item| match item {
                    ThreadItem::Plan { text, .. } => Some(text.clone()),
                    _ => None,
                })
            })
    }

    pub fn thread_started_event(session_configured: &SessionConfiguredEvent) -> ThreadEvent {
        ThreadEvent::ThreadStarted(ThreadStartedEvent {
            thread_id: session_configured.session_id.to_string(),
        })
    }

    pub fn collect_warning(&mut self, message: String) -> CollectedThreadEvents {
        CollectedThreadEvents {
            events: vec![ThreadEvent::ItemCompleted(ItemCompletedEvent {
                item: ExecThreadItem {
                    id: self.next_item_id(),
                    details: ThreadItemDetails::Error(ErrorItem { message }),
                },
            })],
            status: CodexStatus::Running,
        }
    }

    pub fn collect_thread_events(
        &mut self,
        notification: ServerNotification,
    ) -> CollectedThreadEvents {
        let mut events = Vec::new();
        let status = match notification {
            ServerNotification::ConfigWarning(notification) => {
                let message = match notification.details {
                    Some(details) if !details.is_empty() => {
                        format!("{} ({details})", notification.summary)
                    }
                    _ => notification.summary,
                };
                events.push(ThreadEvent::ItemCompleted(ItemCompletedEvent {
                    item: ExecThreadItem {
                        id: self.next_item_id(),
                        details: ThreadItemDetails::Error(ErrorItem { message }),
                    },
                }));
                CodexStatus::Running
            }
            ServerNotification::Error(notification) => {
                let message = match notification.error.additional_details {
                    Some(details) if !details.is_empty() => {
                        format!("{} ({details})", notification.error.message)
                    }
                    _ => notification.error.message,
                };
                let error = ThreadErrorEvent { message };
                self.last_critical_error = Some(error.clone());
                events.push(ThreadEvent::Error(error));
                CodexStatus::Running
            }
            ServerNotification::DeprecationNotice(notification) => {
                let message = match notification.details {
                    Some(details) if !details.is_empty() => {
                        format!("{} ({details})", notification.summary)
                    }
                    _ => notification.summary,
                };
                events.push(ThreadEvent::ItemCompleted(ItemCompletedEvent {
                    item: ExecThreadItem {
                        id: self.next_item_id(),
                        details: ThreadItemDetails::Error(ErrorItem { message }),
                    },
                }));
                CodexStatus::Running
            }
            ServerNotification::HookStarted(_) | ServerNotification::HookCompleted(_) => {
                CodexStatus::Running
            }
            ServerNotification::ItemStarted(notification) => {
                if let Some(item) = self.map_started_item(notification.item) {
                    events.push(ThreadEvent::ItemStarted(ItemStartedEvent { item }));
                }
                CodexStatus::Running
            }
            ServerNotification::ItemCompleted(notification) => {
                if let Some(item) = self.map_completed_item_mut(notification.item) {
                    if let ThreadItemDetails::AgentMessage(AgentMessageItem { text }) =
                        &item.details
                    {
                        self.final_message = Some(text.clone());
                    }
                    events.push(ThreadEvent::ItemCompleted(ItemCompletedEvent { item }));
                }
                CodexStatus::Running
            }
            ServerNotification::ModelRerouted(notification) => {
                events.push(ThreadEvent::ItemCompleted(ItemCompletedEvent {
                    item: ExecThreadItem {
                        id: self.next_item_id(),
                        details: ThreadItemDetails::Error(ErrorItem {
                            message: format!(
                                "model rerouted: {} -> {} ({:?})",
                                notification.from_model, notification.to_model, notification.reason
                            ),
                        }),
                    },
                }));
                CodexStatus::Running
            }
            ServerNotification::ThreadTokenUsageUpdated(notification) => {
                self.last_total_token_usage = Some(notification.token_usage);
                CodexStatus::Running
            }
            ServerNotification::TurnCompleted(notification) => {
                if let Some(running) = self.running_todo_list.take() {
                    events.push(ThreadEvent::ItemCompleted(ItemCompletedEvent {
                        item: ExecThreadItem {
                            id: running.item_id,
                            details: ThreadItemDetails::TodoList(TodoListItem {
                                items: running.items,
                            }),
                        },
                    }));
                }
                match notification.turn.status {
                    TurnStatus::Completed => {
                        if let Some(final_message) =
                            Self::final_message_from_turn_items(notification.turn.items.as_slice())
                        {
                            self.final_message = Some(final_message);
                        }
                        events.push(ThreadEvent::TurnCompleted(TurnCompletedEvent {
                            usage: self.usage_from_last_total(),
                        }));
                        CodexStatus::InitiateShutdown
                    }
                    TurnStatus::Failed => {
                        let error = notification
                            .turn
                            .error
                            .map(|error| ThreadErrorEvent {
                                message: match error.additional_details {
                                    Some(details) if !details.is_empty() => {
                                        format!("{} ({details})", error.message)
                                    }
                                    _ => error.message,
                                },
                            })
                            .or_else(|| self.last_critical_error.clone())
                            .unwrap_or_else(|| ThreadErrorEvent {
                                message: "turn failed".to_string(),
                            });
                        events.push(ThreadEvent::TurnFailed(TurnFailedEvent { error }));
                        CodexStatus::InitiateShutdown
                    }
                    TurnStatus::Interrupted => CodexStatus::InitiateShutdown,
                    TurnStatus::InProgress => CodexStatus::Running,
                }
            }
            ServerNotification::TurnDiffUpdated(_) => CodexStatus::Running,
            ServerNotification::TurnPlanUpdated(notification) => {
                let items = Self::map_todo_items(&notification.plan);
                if let Some(running) = self.running_todo_list.as_mut() {
                    running.items = items.clone();
                    let item_id = running.item_id.clone();
                    events.push(ThreadEvent::ItemUpdated(ItemUpdatedEvent {
                        item: ExecThreadItem {
                            id: item_id,
                            details: ThreadItemDetails::TodoList(TodoListItem { items }),
                        },
                    }));
                } else {
                    let item_id = self.next_item_id();
                    self.running_todo_list = Some(RunningTodoList {
                        item_id: item_id.clone(),
                        items: items.clone(),
                    });
                    events.push(ThreadEvent::ItemStarted(ItemStartedEvent {
                        item: ExecThreadItem {
                            id: item_id,
                            details: ThreadItemDetails::TodoList(TodoListItem { items }),
                        },
                    }));
                }
                CodexStatus::Running
            }
            ServerNotification::TurnStarted(_) => {
                events.push(ThreadEvent::TurnStarted(TurnStartedEvent {}));
                CodexStatus::Running
            }
            _ => CodexStatus::Running,
        };

        CollectedThreadEvents { events, status }
    }
}

impl EventProcessor for EventProcessorWithJsonOutput {
    fn print_config_summary(
        &mut self,
        _: &Config,
        _: &str,
        session_configured: &SessionConfiguredEvent,
    ) {
        self.emit(Self::thread_started_event(session_configured));
    }

    fn process_server_notification(&mut self, notification: ServerNotification) -> CodexStatus {
        let collected = self.collect_thread_events(notification);
        for event in collected.events {
            self.emit(event);
        }
        collected.status
    }

    fn process_warning(&mut self, message: String) -> CodexStatus {
        let collected = self.collect_warning(message);
        for event in collected.events {
            self.emit(event);
        }
        collected.status
    }

    fn print_final_output(&mut self) {
        if let Some(path) = self.last_message_path.as_deref() {
            handle_last_message(self.final_message.as_deref(), path);
        }
    }
}
