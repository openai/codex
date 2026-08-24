//! Structured-output schema and normalization for generated TUI thread titles.

use super::App;
use crate::app_event::AppEvent;
use crate::app_event::ThreadTitleDestination;
use crate::app_server_session::AppServerSession;
use crate::temporary_structured_request::TemporaryStructuredThreadOptions;
use crate::temporary_structured_request::run_temporary_structured_turn;
use crate::temporary_structured_request::start_temporary_thread;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::ReasoningEffort;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;

pub(super) const THREAD_TITLE_MAX_CHARS: usize = 36;
const THREAD_TITLE_MODEL: &str = "gpt-5.6-luna";
pub(super) const THREAD_TITLE_PROMPT_MAX_BYTES: usize = 960;

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
                return;
            }
        };

        let Ok(temporary_thread_id) = ThreadId::from_string(&temporary_thread_id_text) else {
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
