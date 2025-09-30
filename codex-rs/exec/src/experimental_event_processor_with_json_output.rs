use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;

use crate::event_processor::CodexStatus;
use crate::event_processor::EventProcessor;
use crate::event_processor::handle_last_message;
use crate::exec_events::AssistantMessageItem;
use crate::exec_events::CommandExecutionItem;
use crate::exec_events::CommandExecutionStatus;
use crate::exec_events::ConversationErrorEvent;
use crate::exec_events::ConversationEvent;
use crate::exec_events::ConversationItem;
use crate::exec_events::ConversationItemDetails;
use crate::exec_events::FileChangeItem;
use crate::exec_events::FileUpdateChange;
use crate::exec_events::ItemCompletedEvent;
use crate::exec_events::ItemStartedEvent;
use crate::exec_events::ItemUpdatedEvent;
use crate::exec_events::McpToolCallItem;
use crate::exec_events::McpToolCallStatus;
use crate::exec_events::PatchApplyStatus;
use crate::exec_events::PatchChangeKind;
use crate::exec_events::ReasoningItem;
use crate::exec_events::ThreadStartedEvent;
use crate::exec_events::TodoItem;
use crate::exec_events::TodoListItem;
use crate::exec_events::TurnCompletedEvent;
use crate::exec_events::TurnFailedEvent;
use crate::exec_events::TurnStartedEvent;
use crate::exec_events::Usage;
use codex_core::config::Config;
use codex_core::plan_tool::StepStatus;
use codex_core::plan_tool::UpdatePlanArgs;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::AgentReasoningEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandBeginEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::FileChange;
use codex_core::protocol::McpToolCallBeginEvent;
use codex_core::protocol::McpToolCallEndEvent;
use codex_core::protocol::PatchApplyBeginEvent;
use codex_core::protocol::PatchApplyEndEvent;
use codex_core::protocol::SessionConfiguredEvent;
use codex_core::protocol::TaskCompleteEvent;
use codex_core::protocol::TaskStartedEvent;
use tracing::error;
use tracing::warn;

pub struct ExperimentalEventProcessorWithJsonOutput {
    last_message_path: Option<PathBuf>,
    next_event_id: AtomicU64,
    // Tracks running commands by call_id, including the associated item id.
    running_commands: HashMap<String, RunningCommand>,
    running_patch_applies: HashMap<String, PatchApplyBeginEvent>,
    // Tracks the todo list for the current turn (at most one per turn).
    running_todo_list: Option<RunningTodoList>,
    last_total_token_usage: Option<codex_core::protocol::TokenUsage>,
    running_mcp_tool_calls: HashMap<String, RunningMcpToolCall>,
    last_critical_error: Option<ConversationErrorEvent>,
}

#[derive(Debug, Clone)]
struct RunningCommand {
    command: String,
    item_id: String,
}

#[derive(Debug, Clone)]
struct RunningTodoList {
    item_id: String,
    items: Vec<TodoItem>,
}

#[derive(Debug, Clone)]
struct RunningMcpToolCall {
    server: String,
    tool: String,
    item_id: String,
}

impl ExperimentalEventProcessorWithJsonOutput {
    pub fn new(last_message_path: Option<PathBuf>) -> Self {
        Self {
            last_message_path,
            next_event_id: AtomicU64::new(0),
            running_commands: HashMap::new(),
            running_patch_applies: HashMap::new(),
            running_todo_list: None,
            last_total_token_usage: None,
            running_mcp_tool_calls: HashMap::new(),
            last_critical_error: None,
        }
    }

    pub fn collect_conversation_events(&mut self, event: &Event) -> Vec<ConversationEvent> {
        match &event.msg {
            EventMsg::SessionConfigured(ev) => self.handle_session_configured(ev),
            EventMsg::AgentMessage(ev) => self.handle_agent_message(ev),
            EventMsg::AgentReasoning(ev) => self.handle_reasoning_event(ev),
            EventMsg::ExecCommandBegin(ev) => self.handle_exec_command_begin(ev),
            EventMsg::ExecCommandEnd(ev) => self.handle_exec_command_end(ev),
            EventMsg::McpToolCallBegin(ev) => self.handle_mcp_tool_call_begin(ev),
            EventMsg::McpToolCallEnd(ev) => self.handle_mcp_tool_call_end(ev),
            EventMsg::PatchApplyBegin(ev) => self.handle_patch_apply_begin(ev),
            EventMsg::PatchApplyEnd(ev) => self.handle_patch_apply_end(ev),
            EventMsg::TokenCount(ev) => {
                if let Some(info) = &ev.info {
                    self.last_total_token_usage = Some(info.total_token_usage.clone());
                }
                Vec::new()
            }
            EventMsg::TaskStarted(ev) => self.handle_task_started(ev),
            EventMsg::TaskComplete(_) => self.handle_task_complete(),
            EventMsg::Error(ev) => {
                let error = ConversationErrorEvent {
                    message: ev.message.clone(),
                };
                self.last_critical_error = Some(error.clone());
                vec![ConversationEvent::Error(error)]
            }
            EventMsg::StreamError(ev) => vec![ConversationEvent::Error(ConversationErrorEvent {
                message: ev.message.clone(),
            })],
            EventMsg::PlanUpdate(ev) => self.handle_plan_update(ev),
            _ => Vec::new(),
        }
    }

    fn get_next_item_id(&self) -> String {
        format!(
            "item_{}",
            self.next_event_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        )
    }

    fn handle_session_configured(
        &self,
        payload: &SessionConfiguredEvent,
    ) -> Vec<ConversationEvent> {
        vec![ConversationEvent::ThreadStarted(ThreadStartedEvent {
            thread_id: payload.session_id.to_string(),
        })]
    }

    fn handle_agent_message(&self, payload: &AgentMessageEvent) -> Vec<ConversationEvent> {
        let item = ConversationItem {
            id: self.get_next_item_id(),

            details: ConversationItemDetails::AssistantMessage(AssistantMessageItem {
                text: payload.message.clone(),
            }),
        };

        vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
            item,
        })]
    }

    fn handle_reasoning_event(&self, ev: &AgentReasoningEvent) -> Vec<ConversationEvent> {
        let item = ConversationItem {
            id: self.get_next_item_id(),

            details: ConversationItemDetails::Reasoning(ReasoningItem {
                text: ev.text.clone(),
            }),
        };

        vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
            item,
        })]
    }
    fn handle_exec_command_begin(&mut self, ev: &ExecCommandBeginEvent) -> Vec<ConversationEvent> {
        let item_id = self.get_next_item_id();

        let command_string = match shlex::try_join(ev.command.iter().map(String::as_str)) {
            Ok(command_string) => command_string,
            Err(e) => {
                warn!(
                    call_id = ev.call_id,
                    "Failed to stringify command: {e:?}; skipping item.started"
                );
                ev.command.join(" ")
            }
        };

        self.running_commands.insert(
            ev.call_id.clone(),
            RunningCommand {
                command: command_string.clone(),
                item_id: item_id.clone(),
            },
        );

        let item = ConversationItem {
            id: item_id,
            details: ConversationItemDetails::CommandExecution(CommandExecutionItem {
                command: command_string,
                aggregated_output: String::new(),
                exit_code: None,
                status: CommandExecutionStatus::InProgress,
            }),
        };

        vec![ConversationEvent::ItemStarted(ItemStartedEvent { item })]
    }

    fn handle_mcp_tool_call_begin(&mut self, ev: &McpToolCallBeginEvent) -> Vec<ConversationEvent> {
        let item_id = self.get_next_item_id();
        let server = ev.invocation.server.clone();
        let tool = ev.invocation.tool.clone();

        self.running_mcp_tool_calls.insert(
            ev.call_id.clone(),
            RunningMcpToolCall {
                server: server.clone(),
                tool: tool.clone(),
                item_id: item_id.clone(),
            },
        );

        let item = ConversationItem {
            id: item_id,
            details: ConversationItemDetails::McpToolCall(McpToolCallItem {
                server,
                tool,
                status: McpToolCallStatus::InProgress,
            }),
        };

        vec![ConversationEvent::ItemStarted(ItemStartedEvent { item })]
    }

    fn handle_mcp_tool_call_end(&mut self, ev: &McpToolCallEndEvent) -> Vec<ConversationEvent> {
        let status = if ev.is_success() {
            McpToolCallStatus::Completed
        } else {
            McpToolCallStatus::Failed
        };

        let (server, tool, item_id) = match self.running_mcp_tool_calls.remove(&ev.call_id) {
            Some(running) => (running.server, running.tool, running.item_id),
            None => {
                warn!(
                    call_id = ev.call_id,
                    "Received McpToolCallEnd without begin; synthesizing new item"
                );
                (
                    ev.invocation.server.clone(),
                    ev.invocation.tool.clone(),
                    self.get_next_item_id(),
                )
            }
        };

        let item = ConversationItem {
            id: item_id,
            details: ConversationItemDetails::McpToolCall(McpToolCallItem {
                server,
                tool,
                status,
            }),
        };

        vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
            item,
        })]
    }

    fn handle_patch_apply_begin(&mut self, ev: &PatchApplyBeginEvent) -> Vec<ConversationEvent> {
        self.running_patch_applies
            .insert(ev.call_id.clone(), ev.clone());

        Vec::new()
    }

    fn map_change_kind(&self, kind: &FileChange) -> PatchChangeKind {
        match kind {
            FileChange::Add { .. } => PatchChangeKind::Add,
            FileChange::Delete { .. } => PatchChangeKind::Delete,
            FileChange::Update { .. } => PatchChangeKind::Update,
        }
    }

    fn handle_patch_apply_end(&mut self, ev: &PatchApplyEndEvent) -> Vec<ConversationEvent> {
        if let Some(running_patch_apply) = self.running_patch_applies.remove(&ev.call_id) {
            let status = if ev.success {
                PatchApplyStatus::Completed
            } else {
                PatchApplyStatus::Failed
            };
            let item = ConversationItem {
                id: self.get_next_item_id(),

                details: ConversationItemDetails::FileChange(FileChangeItem {
                    changes: running_patch_apply
                        .changes
                        .iter()
                        .map(|(path, change)| FileUpdateChange {
                            path: path.to_str().unwrap_or("").to_string(),
                            kind: self.map_change_kind(change),
                        })
                        .collect(),
                    status,
                }),
            };

            return vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
                item,
            })];
        }

        Vec::new()
    }

    fn handle_exec_command_end(&mut self, ev: &ExecCommandEndEvent) -> Vec<ConversationEvent> {
        let Some(RunningCommand { command, item_id }) = self.running_commands.remove(&ev.call_id)
        else {
            warn!(
                call_id = ev.call_id,
                "ExecCommandEnd without matching ExecCommandBegin; skipping item.completed"
            );
            return Vec::new();
        };
        let status = if ev.exit_code == 0 {
            CommandExecutionStatus::Completed
        } else {
            CommandExecutionStatus::Failed
        };
        let item = ConversationItem {
            id: item_id,

            details: ConversationItemDetails::CommandExecution(CommandExecutionItem {
                command,
                aggregated_output: ev.aggregated_output.clone(),
                exit_code: Some(ev.exit_code),
                status,
            }),
        };

        vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
            item,
        })]
    }

    fn todo_items_from_plan(&self, args: &UpdatePlanArgs) -> Vec<TodoItem> {
        args.plan
            .iter()
            .map(|p| TodoItem {
                text: p.step.clone(),
                completed: matches!(p.status, StepStatus::Completed),
            })
            .collect()
    }

    fn handle_plan_update(&mut self, args: &UpdatePlanArgs) -> Vec<ConversationEvent> {
        let items = self.todo_items_from_plan(args);

        if let Some(running) = &mut self.running_todo_list {
            running.items = items.clone();
            let item = ConversationItem {
                id: running.item_id.clone(),
                details: ConversationItemDetails::TodoList(TodoListItem { items }),
            };
            return vec![ConversationEvent::ItemUpdated(ItemUpdatedEvent { item })];
        }

        let item_id = self.get_next_item_id();
        self.running_todo_list = Some(RunningTodoList {
            item_id: item_id.clone(),
            items: items.clone(),
        });
        let item = ConversationItem {
            id: item_id,
            details: ConversationItemDetails::TodoList(TodoListItem { items }),
        };
        vec![ConversationEvent::ItemStarted(ItemStartedEvent { item })]
    }

    fn handle_task_started(&mut self, _: &TaskStartedEvent) -> Vec<ConversationEvent> {
        self.last_critical_error = None;
        vec![ConversationEvent::TurnStarted(TurnStartedEvent {})]
    }

    fn handle_task_complete(&mut self) -> Vec<ConversationEvent> {
        let usage = if let Some(u) = &self.last_total_token_usage {
            Usage {
                input_tokens: u.input_tokens,
                cached_input_tokens: u.cached_input_tokens,
                output_tokens: u.output_tokens,
            }
        } else {
            Usage::default()
        };

        let mut items = Vec::new();

        if let Some(running) = self.running_todo_list.take() {
            let item = ConversationItem {
                id: running.item_id,
                details: ConversationItemDetails::TodoList(TodoListItem {
                    items: running.items,
                }),
            };
            items.push(ConversationEvent::ItemCompleted(ItemCompletedEvent {
                item,
            }));
        }

        if let Some(error) = self.last_critical_error.take() {
            items.push(ConversationEvent::TurnFailed(TurnFailedEvent { error }));
        } else {
            items.push(ConversationEvent::TurnCompleted(TurnCompletedEvent {
                usage,
            }));
        }

        items
    }
}

impl EventProcessor for ExperimentalEventProcessorWithJsonOutput {
    fn print_config_summary(&mut self, _: &Config, _: &str, ev: &SessionConfiguredEvent) {
        self.process_event(Event {
            id: "".to_string(),
            msg: EventMsg::SessionConfigured(ev.clone()),
        });
    }

    fn process_event(&mut self, event: Event) -> CodexStatus {
        let aggregated = self.collect_conversation_events(&event);
        for conv_event in aggregated {
            match serde_json::to_string(&conv_event) {
                Ok(line) => {
                    println!("{line}");
                }
                Err(e) => {
                    error!("Failed to serialize event: {e:?}");
                }
            }
        }

        let Event { msg, .. } = event;

        if let EventMsg::TaskComplete(TaskCompleteEvent { last_agent_message }) = msg {
            if let Some(output_file) = self.last_message_path.as_deref() {
                handle_last_message(last_agent_message.as_deref(), output_file);
            }
            CodexStatus::InitiateShutdown
        } else {
            CodexStatus::Running
        }
    }
}
