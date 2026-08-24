//! Structured-output schema and normalization for generated TUI thread titles.

use super::App;
use super::thread_events::ThreadBufferedEvent;
use crate::app_event::AppEvent;
use crate::app_event::ThreadTitleDestination;
use crate::app_server_session::AppServerSession;
use crate::temporary_structured_request::TemporaryStructuredThreadOptions;
use crate::temporary_structured_request::run_temporary_structured_turn;
use crate::temporary_structured_request::start_temporary_thread;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::UserInput;
use codex_protocol::ThreadId;
use codex_protocol::models::MessagePhase;
use codex_protocol::openai_models::ReasoningEffort;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;

pub(super) const THREAD_TITLE_MAX_CHARS: usize = 36;
const THREAD_TITLE_MODEL: &str = "gpt-5.6-luna";
pub(super) const THREAD_TITLE_PROMPT_MAX_BYTES: usize = 960;
const THREAD_TITLE_RECENT_MESSAGES: usize = 8;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedThreadTitle {
    title: String,
}

impl App {
    /// Start a hidden title-generation thread without blocking the UI loop.
    pub(super) fn generate_thread_title(
        &mut self,
        app_server: &AppServerSession,
        thread_id: ThreadId,
        destination: ThreadTitleDestination,
        prompt: String,
    ) {
        let request_handle = app_server.request_handle();
        let model = if self.chat_widget.config_ref().model_provider_id == "openai"
            && self.chat_widget.has_chatgpt_account()
            && self
                .chat_widget
                .model_catalog()
                .try_list_models()
                .is_ok_and(|models| models.iter().any(|model| model.model == THREAD_TITLE_MODEL))
        {
            THREAD_TITLE_MODEL.to_string()
        } else {
            self.chat_widget.current_model().to_string()
        };
        let effort = (model == THREAD_TITLE_MODEL).then_some(ReasoningEffort::Low);
        let config = self.chat_widget.config_ref();
        let options = TemporaryStructuredThreadOptions {
            model,
            model_provider: config.model_provider_id.clone(),
            cwd: config.cwd.display().to_string(),
            active_permission_profile: config
                .permissions
                .active_permission_profile()
                .map(|profile| profile.id),
            mcp_server_names: config.mcp_servers.get().keys().cloned().collect(),
        };

        let event_sender = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = start_temporary_thread(&request_handle, options)
                .await
                .map(|thread| thread.thread.id)
                .map_err(|error| error.to_string());

            event_sender.send(AppEvent::ThreadTitleStarted {
                thread_id,
                destination,
                prompt,
                effort,
                result,
            });
        });
    }

    /// Register a started hidden thread and generate its structured title.
    pub(super) fn on_thread_title_started(
        &mut self,
        app_server: &AppServerSession,
        thread_id: ThreadId,
        destination: ThreadTitleDestination,
        prompt: String,
        effort: Option<ReasoningEffort>,
        result: Result<String, String>,
    ) {
        let temporary_thread_id_text = match result {
            Ok(thread_id) => thread_id,
            Err(error) => {
                tracing::debug!(%error, "failed to start title-generation thread");
                if let ThreadTitleDestination::RenameSuggestion { request_id } = destination {
                    self.chat_widget.apply_thread_name_suggestion(
                        thread_id, request_id, /*suggestion*/ None,
                    );
                }
                return;
            }
        };

        let Ok(temporary_thread_id) = ThreadId::from_string(&temporary_thread_id_text) else {
            if let ThreadTitleDestination::RenameSuggestion { request_id } = destination {
                self.chat_widget
                    .apply_thread_name_suggestion(thread_id, request_id, /*suggestion*/ None);
            }
            return;
        };

        let (sender, receiver) = mpsc::unbounded_channel();
        self.temporary_structured_requests
            .insert(temporary_thread_id, sender);

        let request_handle = app_server.request_handle();
        let event_sender = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = run_temporary_structured_turn(
                request_handle,
                temporary_thread_id_text,
                prompt,
                thread_title_output_schema(),
                effort,
                receiver,
            )
            .await
            .map_err(|error| error.to_string());

            event_sender.send(AppEvent::GeneratedThreadTitle {
                thread_id,
                temporary_thread_id,
                destination,
                result,
            });
        });
    }

    /// Suggest a title from stored and live conversation items without persisting it.
    pub(super) async fn suggest_thread_name(
        &mut self,
        app_server: &AppServerSession,
        thread_id: ThreadId,
        request_id: uuid::Uuid,
    ) {
        if self.chat_widget.thread_id() != Some(thread_id) {
            return;
        }

        let Some(channel) = self.thread_event_channels.get(&thread_id) else {
            self.chat_widget
                .apply_thread_name_suggestion(thread_id, request_id, /*suggestion*/ None);
            return;
        };

        let conversation = {
            let store = channel.store.lock().await;
            let mut seen = std::collections::HashSet::new();

            recent_conversation_messages(
                store
                    .turns
                    .iter()
                    .flat_map(|turn| turn.items.iter())
                    .chain(store.buffer.iter().filter_map(|event| {
                        let ThreadBufferedEvent::Notification(notification) = event else {
                            return None;
                        };

                        let ServerNotification::ItemCompleted(notification) = notification.as_ref()
                        else {
                            return None;
                        };

                        Some(&notification.item)
                    }))
                    .filter(|item| seen.insert(item.id().to_string())),
            )
        };

        let Some(conversation) = conversation else {
            self.chat_widget
                .apply_thread_name_suggestion(thread_id, request_id, /*suggestion*/ None);
            return;
        };

        self.generate_thread_title(
            app_server,
            thread_id,
            ThreadTitleDestination::RenameSuggestion { request_id },
            recent_conversation_thread_title_prompt(&conversation),
        );
    }
}

/// Constrain generated metadata to one nonempty title within the display limit.
pub(super) fn thread_title_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "title": {
                "type": "string",
                "minLength": 1,
                "maxLength": THREAD_TITLE_MAX_CHARS,
            },
        },
        "required": ["title"],
        "additionalProperties": false,
    })
}

fn thread_title_instructions() -> String {
    format!(
        "Generate a concise, single-line task title of at most \
  {THREAD_TITLE_MAX_CHARS} characters and under five words where possible. \
  Start with an imperative verb. Capitalize only the first word unless the \
  user's language, proper nouns, acronyms, or code terms require otherwise. \
  Preserve ticket references exactly. Write in the user's language. \
  Do not use quotes, markdown, or trailing punctuation. \
  Do not answer the request."
    )
}

/// Build a bounded title request without truncating a Unicode character.
pub(super) fn thread_title_prompt(user_message: &str) -> String {
    let instructions = thread_title_instructions();
    let prefix = format!("{instructions}\n\nUser prompt:\n");
    let remaining_bytes = THREAD_TITLE_PROMPT_MAX_BYTES.saturating_sub(prefix.len());
    let user_message = user_message
        .trim()
        .char_indices()
        .take_while(|(index, character)| index + character.len_utf8() <= remaining_bytes)
        .map(|(_, character)| character)
        .collect::<String>();

    format!("{prefix}{user_message}")
}

/// Format recent substantive messages chronologically without trusting their markup.
pub(super) fn recent_conversation_messages<'a, I>(items: I) -> Option<String>
where
    I: IntoIterator<Item = &'a ThreadItem>,
    I::IntoIter: DoubleEndedIterator,
{
    let mut messages = items
        .into_iter()
        .rev()
        .filter_map(|item| match item {
            ThreadItem::UserMessage { content, .. } => {
                let text = content
                    .iter()
                    .filter_map(|input| match input {
                        UserInput::Text { text, .. } => {
                            Some(crate::ide_context::extract_prompt_request_with_offset(text).0)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                (!text.trim().is_empty()).then_some(("user", text))
            }
            ThreadItem::AgentMessage { text, phase, .. }
                if !matches!(phase, Some(MessagePhase::Commentary)) && !text.trim().is_empty() =>
            {
                Some(("assistant", text.clone()))
            }
            _ => None,
        })
        .take(THREAD_TITLE_RECENT_MESSAGES)
        .map(|(role, text)| {
            let escaped = text
                .trim()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");

            (role, escaped)
        })
        .collect::<Vec<_>>();

    if messages.is_empty() {
        return None;
    }

    messages.reverse();

    let conversation_bytes = THREAD_TITLE_PROMPT_MAX_BYTES
        .saturating_sub(recent_conversation_thread_title_prompt("").len());
    let markup_bytes = "<conversation>\n".len()
        + "\n</conversation>".len()
        + messages.len().saturating_sub(/*rhs*/ 1)
        + messages
            .iter()
            .map(|(role, _)| "<message role=\"\"></message>".len() + role.len())
            .sum::<usize>();
    let message_bytes = conversation_bytes.saturating_sub(markup_bytes) / messages.len();
    let content_bytes = messages.iter().map(|(_, text)| text.len()).sum::<usize>();
    let should_truncate = content_bytes > conversation_bytes.saturating_sub(markup_bytes);
    let messages = messages
        .into_iter()
        .map(|(role, mut text)| {
            if should_truncate && text.len() > message_bytes {
                let mut end = message_bytes;
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                if let Some(entity_start) = text[..end].rfind('&')
                    && !text[entity_start..end].contains(';')
                {
                    end = entity_start;
                }
                text.truncate(end);
            }

            format!("<message role=\"{role}\">{text}</message>")
        })
        .collect::<Vec<_>>();

    Some(format!(
        "<conversation>\n{}\n</conversation>",
        messages.join("\n")
    ))
}

/// Bound the entire suggestion prompt while preserving complete Unicode characters.
pub(super) fn recent_conversation_thread_title_prompt(conversation: &str) -> String {
    let instructions = thread_title_instructions();
    let prefix = format!(
        "{instructions}\n\
Prioritize the current task and latest substantive user request.\n\n\
Recent conversation messages:\n"
    );
    let conversation = conversation.trim();
    let remaining_bytes = THREAD_TITLE_PROMPT_MAX_BYTES.saturating_sub(prefix.len());
    let mut start = conversation.len().saturating_sub(remaining_bytes);
    while !conversation.is_char_boundary(start) {
        start += 1;
    }
    let conversation = &conversation[start..];

    format!("{prefix}{conversation}")
}

/// Normalize a generated title and truncate it without splitting Unicode characters.
pub(super) fn parse_thread_title(response: &str) -> Option<String> {
    if !response.trim_start().starts_with('{') {
        return None;
    }

    let title = serde_json::from_str::<GeneratedThreadTitle>(response)
        .ok()?
        .title;

    let normalized = title
        .trim()
        .trim_matches(|character| matches!(character, '"' | '\'' | '`' | '“' | '”' | '‘' | '’'))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches(['.', '?', '!'])
        .trim_end()
        .to_string();

    if normalized.is_empty() {
        return None;
    }

    Some(normalized.chars().take(THREAD_TITLE_MAX_CHARS).collect())
}

#[cfg(test)]
#[path = "thread_title_tests.rs"]
mod tests;
