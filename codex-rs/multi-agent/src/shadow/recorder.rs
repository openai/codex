use std::time::SystemTime;

use codex_core::protocol::AgentMessageDeltaEvent;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::InputMessageKind;
use codex_core::protocol::UserMessageEvent;
use serde::Deserialize;
use serde::Serialize;

use crate::AgentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ShadowHistoryKind {
    #[default]
    Agent,
    User,
    Info,
    Warning,
    Error,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowHistoryEntry {
    pub kind: ShadowHistoryKind,
    pub lines: Vec<String>,
    pub is_stream_continuation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShadowTranscriptCapture {
    pub user_inputs: Vec<InputItem>,
    pub agent_outputs: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct ShadowSessionMetrics {
    pub session_count: usize,
    pub events: usize,
    pub user_inputs: usize,
    pub agent_outputs: usize,
    pub turns: usize,
    pub total_bytes: usize,
    pub total_compressed_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowSnapshot {
    pub conversation_id: String,
    pub agent_id: AgentId,
    pub history: Vec<ShadowHistoryEntry>,
    pub capture: ShadowTranscriptCapture,
    pub metrics: ShadowSessionMetrics,
    pub events: Vec<Event>,
    pub recorded_at: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ShadowRecorder {
    conversation_id: String,
    agent_id: AgentId,
    history: Vec<ShadowHistoryEntry>,
    capture: ShadowTranscriptCapture,
    metrics: ShadowSessionMetrics,
    current_stream: Option<String>,
    last_updated: SystemTime,
    synthetic_event_counter: usize,
}

impl ShadowRecorder {
    pub fn new(conversation_id: String, agent_id: AgentId) -> Self {
        Self {
            conversation_id,
            agent_id,
            history: Vec::new(),
            capture: ShadowTranscriptCapture::default(),
            metrics: ShadowSessionMetrics::default(),
            current_stream: None,
            last_updated: SystemTime::now(),
            synthetic_event_counter: 0,
        }
    }

    fn next_synthetic_event_id(&mut self) -> String {
        let id = format!(
            "shadow-{}-{}",
            self.agent_id.as_str(),
            self.synthetic_event_counter
        );
        self.synthetic_event_counter = self.synthetic_event_counter.wrapping_add(1);
        id
    }

    pub fn make_user_event(&mut self, message: String) -> Event {
        Event {
            id: self.next_synthetic_event_id(),
            msg: EventMsg::UserMessage(UserMessageEvent {
                message,
                kind: Some(InputMessageKind::Plain),
                images: None,
            }),
        }
    }

    pub fn record_event(&mut self, event: &Event) {
        self.metrics.events += 1;
        self.metrics.total_bytes += approximate_event_size(event);
        self.last_updated = SystemTime::now();

        match &event.msg {
            EventMsg::AgentMessage(AgentMessageEvent { message }) => {
                self.finish_stream();
                self.history.push(ShadowHistoryEntry {
                    kind: ShadowHistoryKind::Agent,
                    lines: message
                        .lines()
                        .map(std::string::ToString::to_string)
                        .collect(),
                    is_stream_continuation: false,
                });
            }
            EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta }) => {
                let stream = self.current_stream.get_or_insert_with(String::new);
                stream.push_str(delta);
            }
            EventMsg::UserMessage(user) => {
                self.finish_stream();
                let mut lines = Vec::new();
                lines.extend(user.message.lines().map(std::string::ToString::to_string));
                self.history.push(ShadowHistoryEntry {
                    kind: ShadowHistoryKind::User,
                    lines: if lines.is_empty() {
                        vec![String::new()]
                    } else {
                        lines
                    },
                    is_stream_continuation: false,
                });
            }
            EventMsg::TaskComplete(complete) => {
                self.finish_stream();
                if let Some(last) = &complete.last_agent_message {
                    self.history.push(ShadowHistoryEntry {
                        kind: ShadowHistoryKind::Agent,
                        lines: last.lines().map(std::string::ToString::to_string).collect(),
                        is_stream_continuation: false,
                    });
                }
                self.metrics.turns += 1;
            }
            EventMsg::Error(err) => {
                self.finish_stream();
                self.history.push(ShadowHistoryEntry {
                    kind: ShadowHistoryKind::Error,
                    lines: vec![err.message.clone()],
                    is_stream_continuation: false,
                });
            }
            EventMsg::StreamError(err) => {
                self.finish_stream();
                self.history.push(ShadowHistoryEntry {
                    kind: ShadowHistoryKind::Warning,
                    lines: vec![err.message.clone()],
                    is_stream_continuation: false,
                });
            }
            EventMsg::BackgroundEvent(ev) => {
                self.finish_stream();
                self.history.push(ShadowHistoryEntry {
                    kind: ShadowHistoryKind::Info,
                    lines: vec![ev.message.clone()],
                    is_stream_continuation: false,
                });
            }
            EventMsg::PlanUpdate(update) => {
                self.finish_stream();
                let mut lines = Vec::new();
                for item in &update.plan {
                    lines.push(format!("{} [{:?}]", item.step, item.status));
                }
                if let Some(explanation) = update.explanation.as_ref()
                    && !explanation.is_empty()
                {
                    lines.push(format!("Explanation: {explanation}"));
                }
                if lines.is_empty() {
                    lines.push("Plan updated.".to_string());
                }
                self.history.push(ShadowHistoryEntry {
                    kind: ShadowHistoryKind::Info,
                    lines,
                    is_stream_continuation: false,
                });
            }
            EventMsg::ExecCommandBegin(ev) => {
                self.finish_stream();
                self.history.push(ShadowHistoryEntry {
                    kind: ShadowHistoryKind::Info,
                    lines: vec![format!("Command started: {}", ev.command.join(" "))],
                    is_stream_continuation: false,
                });
            }
            EventMsg::ExecCommandOutputDelta(delta) => {
                let stream = self.current_stream.get_or_insert_with(String::new);
                let text = String::from_utf8_lossy(&delta.chunk);
                stream.push_str(&text);
            }
            EventMsg::ExecCommandEnd(ev) => {
                self.finish_stream();
                self.history.push(ShadowHistoryEntry {
                    kind: ShadowHistoryKind::Info,
                    lines: vec![format!("Command exited with code {}", ev.exit_code)],
                    is_stream_continuation: false,
                });
                if !ev.stdout.is_empty() {
                    self.history.push(ShadowHistoryEntry {
                        kind: ShadowHistoryKind::Agent,
                        lines: ev
                            .stdout
                            .lines()
                            .map(std::string::ToString::to_string)
                            .collect(),
                        is_stream_continuation: false,
                    });
                }
                if !ev.stderr.is_empty() {
                    self.history.push(ShadowHistoryEntry {
                        kind: ShadowHistoryKind::Warning,
                        lines: ev
                            .stderr
                            .lines()
                            .map(std::string::ToString::to_string)
                            .collect(),
                        is_stream_continuation: false,
                    });
                }
            }
            EventMsg::McpToolCallBegin(ev) => {
                self.finish_stream();
                self.history.push(ShadowHistoryEntry {
                    kind: ShadowHistoryKind::Info,
                    lines: vec![format!("MCP tool call started: {:?}", ev.invocation)],
                    is_stream_continuation: false,
                });
            }
            EventMsg::McpToolCallEnd(ev) => {
                self.finish_stream();
                let mut lines = Vec::new();
                lines.push(format!("Invocation: {:?}", ev.invocation));
                lines.push(format!("Duration: {:?}", ev.duration));
                lines.push(format!("Result: {:?}", ev.result));
                self.history.push(ShadowHistoryEntry {
                    kind: ShadowHistoryKind::Info,
                    lines,
                    is_stream_continuation: false,
                });
            }
            EventMsg::ShutdownComplete => {
                self.finish_stream();
            }
            _ => {
                self.finish_stream();
                self.history.push(ShadowHistoryEntry {
                    kind: ShadowHistoryKind::System,
                    lines: vec![format!("{:?}", event.msg)],
                    is_stream_continuation: false,
                });
            }
        }
    }

    pub fn record_user_inputs(&mut self, items: &[InputItem]) {
        if items.is_empty() {
            return;
        }
        self.capture.user_inputs.extend_from_slice(items);
        self.metrics.user_inputs += items.len();
    }

    pub fn record_agent_outputs(&mut self, outputs: &[String]) {
        if outputs.is_empty() {
            return;
        }
        self.capture.agent_outputs.extend(outputs.to_owned());
        self.metrics.agent_outputs += outputs.len();
    }

    pub fn snapshot(&self, events: &[Event]) -> ShadowSnapshot {
        let mut metrics = self.metrics;
        metrics.session_count = 1;
        metrics.total_compressed_bytes = 0;
        ShadowSnapshot {
            conversation_id: self.conversation_id.clone(),
            agent_id: self.agent_id.clone(),
            history: self.history.clone(),
            capture: self.capture.clone(),
            metrics,
            events: events.to_vec(),
            recorded_at: self.last_updated,
        }
    }

    pub fn metrics(&self) -> ShadowSessionMetrics {
        self.metrics
    }

    pub fn raw_bytes(&self) -> usize {
        self.metrics.total_bytes
    }

    fn finish_stream(&mut self) {
        if let Some(stream) = self.current_stream.take()
            && !stream.is_empty()
        {
            self.history.push(ShadowHistoryEntry {
                kind: ShadowHistoryKind::Agent,
                lines: stream
                    .lines()
                    .map(std::string::ToString::to_string)
                    .collect(),
                is_stream_continuation: true,
            });
        }
    }
}

fn approximate_event_size(event: &Event) -> usize {
    serde_json::to_string(event).map(|s| s.len()).unwrap_or(0)
}
