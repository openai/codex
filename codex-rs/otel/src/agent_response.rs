//! Exports opted-in live final responses, with turn-local deduplication and bounded text.
//! Hosts supply completed raw messages and attribution; stored history is never changed.

use crate::SessionTelemetry;
use crate::events::shared::log_event;
use codex_protocol::AgentPath;
use codex_protocol::ResponseItemId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_utils_stream_parser::strip_citations;
use codex_utils_string::take_bytes_at_char_boundary;
use std::collections::HashSet;
use std::sync::Mutex;

const MAX_RESPONSE_BYTES: usize = 65_536;

/// Host-owned attribution for one live completed response item.
pub struct AgentResponseContext<'a> {
    pub turn_id: &'a str,
    pub session_source: &'a SessionSource,
    pub parent_turn_id: Option<String>,
    pub root_turn_id: Option<String>,
    pub initiating_agent_path: Option<&'a AgentPath>,
}

/// One turn's response-log state. Create a fresh instance for each turn; never replay history.
/// Only construct when response logging is opted in and an OTLP log exporter is configured.
#[derive(Default)]
pub struct AgentResponseLogger {
    logged: Mutex<HashSet<ResponseItemId>>,
}

impl AgentResponseLogger {
    /// Record a live final message once, using the telemetry of its actual sampling step.
    pub fn record(
        &self,
        telemetry: &SessionTelemetry,
        item: &ResponseItem,
        context: AgentResponseContext<'_>,
    ) {
        let ResponseItem::Message {
            id: Some(item_id),
            role,
            phase: Some(MessagePhase::FinalAnswer),
            content,
            ..
        } = item
        else {
            return;
        };
        if role != "assistant" {
            return;
        }
        let (agent_type, parent_conversation_id) = match context.session_source {
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id, ..
            }) => ("subagent", Some(parent_thread_id.to_string())),
            SessionSource::Cli
            | SessionSource::VSCode
            | SessionSource::Exec
            | SessionSource::Mcp
            | SessionSource::Custom(_)
            | SessionSource::Unknown => ("main", None),
            SessionSource::Internal(_)
            | SessionSource::SubAgent(
                SubAgentSource::Review
                | SubAgentSource::Compact
                | SubAgentSource::MemoryConsolidation
                | SubAgentSource::Other(_),
            ) => return,
        };
        let text = content
            .iter()
            .filter_map(|content| match content {
                ContentItem::OutputText { text } => Some(text.as_str()),
                ContentItem::InputText { .. }
                | ContentItem::InputImage { .. }
                | ContentItem::InputAudio { .. } => None,
            })
            .collect::<String>();
        // Keep plan content, stripping memory citations only from the exported copy.
        let (text, _) = strip_citations(&text);
        if text.trim().is_empty()
            || !self
                .logged
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(item_id.clone())
        {
            return;
        }
        let response = take_bytes_at_char_boundary(&text, MAX_RESPONSE_BYTES);
        log_event!(
            telemetry,
            event.name = "codex.agent_response",
            agent.type = agent_type,
            turn.id = context.turn_id,
            item.id = item_id.as_str(),
            parent.conversation.id = parent_conversation_id.as_deref(),
            parent.turn.id = context.parent_turn_id.as_deref(),
            root.turn.id = context.root_turn_id.as_deref(),
            initiating.agent.path = context.initiating_agent_path.map(AgentPath::as_str),
            response = response,
            response_length = text.len() as u64,
            response_truncated = response.len() < text.len(),
        );
    }
}
