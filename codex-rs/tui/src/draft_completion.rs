use std::sync::Arc;
use std::time::Duration;

use codex_core::AuthManager;
use codex_core::ModelClient;
use codex_core::Prompt;
use codex_core::ResponseEvent;
use codex_core::config::Config;
use codex_core::default_client::originator;
use codex_core::models_manager::manager::ModelsManager;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use tokio::time::timeout;
use tokio_stream::StreamExt;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

const DRAFT_COMPLETION_MODEL: &str = "galapagos-nano-alpha";
const DRAFT_COMPLETION_TIMEOUT: Duration = Duration::from_secs(6);
const DRAFT_COMPLETION_AGENT_LABEL: &str = "tui_draft_completion";
const MAX_SUGGESTIONS: usize = 3;

#[derive(Debug, Clone)]
pub(crate) struct DraftCompletionRequest {
    pub(crate) request_id: u64,
    pub(crate) draft: String,
    pub(crate) last_user_turn: Option<String>,
    pub(crate) last_assistant_turn: Option<String>,
}

pub(crate) struct DraftCompletionManager {
    auth_manager: Arc<AuthManager>,
    models_manager: Arc<ModelsManager>,
    app_tx: AppEventSender,
    config: Config,
}

impl DraftCompletionManager {
    pub(crate) fn new(
        auth_manager: Arc<AuthManager>,
        models_manager: Arc<ModelsManager>,
        app_tx: AppEventSender,
        config: Config,
    ) -> Self {
        Self {
            auth_manager,
            models_manager,
            app_tx,
            config,
        }
    }

    pub(crate) fn start_request(&self, request: DraftCompletionRequest) {
        let auth_manager = Arc::clone(&self.auth_manager);
        let models_manager = Arc::clone(&self.models_manager);
        let app_tx = self.app_tx.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let result = timeout(
                DRAFT_COMPLETION_TIMEOUT,
                complete_draft(
                    auth_manager,
                    models_manager,
                    config,
                    &request.draft,
                    request.last_user_turn.as_deref(),
                    request.last_assistant_turn.as_deref(),
                ),
            )
            .await;

            let result = match result {
                Ok(Ok(suggestions)) => Ok(suggestions),
                Ok(Err(err)) => {
                    tracing::warn!("draft completion failed: {err}");
                    Err(err)
                }
                Err(_) => {
                    tracing::warn!("draft completion timed out");
                    Err("draft completion timed out".to_string())
                }
            };

            app_tx.send(AppEvent::DraftCompletionResult {
                request_id: request.request_id,
                result,
            });
        });
    }
}

async fn complete_draft(
    auth_manager: Arc<AuthManager>,
    models_manager: Arc<ModelsManager>,
    config: Config,
    draft: &str,
    last_user_turn: Option<&str>,
    last_assistant_turn: Option<&str>,
) -> Result<Vec<String>, String> {
    let model_info = models_manager
        .get_model_info(DRAFT_COMPLETION_MODEL, &config)
        .await;
    let auth = auth_manager.auth_cached();
    let auth_mode = auth
        .as_ref()
        .map(codex_core::CodexAuth::auth_mode)
        .map(codex_otel::TelemetryAuthMode::from);
    let account_id = auth
        .as_ref()
        .and_then(codex_core::CodexAuth::get_account_id);
    let account_email = auth
        .as_ref()
        .and_then(codex_core::CodexAuth::get_account_email);
    let telemetry = SessionTelemetry::new(
        ThreadId::new(),
        DRAFT_COMPLETION_MODEL,
        DRAFT_COMPLETION_MODEL,
        account_id,
        account_email,
        auth_mode,
        originator().value,
        config.otel.log_user_prompt,
        codex_core::terminal::user_agent(),
        SessionSource::SubAgent(codex_protocol::protocol::SubAgentSource::Other(
            DRAFT_COMPLETION_AGENT_LABEL.to_string(),
        )),
    );
    let model_client = ModelClient::new(
        Some(auth_manager),
        ThreadId::new(),
        config.model_provider.clone(),
        SessionSource::SubAgent(codex_protocol::protocol::SubAgentSource::Other(
            DRAFT_COMPLETION_AGENT_LABEL.to_string(),
        )),
        config.model_verbosity,
        false,
        false,
        false,
        None,
    );
    let mut session = model_client.new_session();
    let prompt = PromptBuilder::new(draft, last_user_turn, last_assistant_turn).build();
    let mut stream = session
        .stream(
            &prompt,
            &model_info,
            &telemetry,
            None,
            model_info.default_reasoning_summary,
            None,
            None,
        )
        .await
        .map_err(|err| err.to_string())?;

    let mut output = String::new();
    while let Some(event) = stream.next().await {
        match event.map_err(|err| err.to_string())? {
            ResponseEvent::OutputTextDelta(delta) => output.push_str(&delta),
            ResponseEvent::OutputItemDone(item) if output.is_empty() => {
                output.push_str(&response_item_text(&item));
            }
            ResponseEvent::OutputItemDone(_) => {}
            ResponseEvent::Completed { .. } => break,
            ResponseEvent::Created
            | ResponseEvent::OutputItemAdded(_)
            | ResponseEvent::ServerModel(_)
            | ResponseEvent::ServerReasoningIncluded(_)
            | ResponseEvent::ReasoningSummaryDelta { .. }
            | ResponseEvent::ReasoningContentDelta { .. }
            | ResponseEvent::ReasoningSummaryPartAdded { .. }
            | ResponseEvent::RateLimits(_)
            | ResponseEvent::ModelsEtag(_) => {}
        }
    }

    parse_suggestions(draft, &output)
}

fn response_item_text(item: &ResponseItem) -> String {
    match item {
        ResponseItem::Message { content, .. } => content
            .iter()
            .map(|content_item| match content_item {
                ContentItem::InputText { text } | ContentItem::OutputText { text } => text.as_str(),
                ContentItem::InputImage { .. } => "",
            })
            .collect(),
        _ => String::new(),
    }
}

struct PromptBuilder<'a> {
    draft: &'a str,
    last_user_turn: Option<&'a str>,
    last_assistant_turn: Option<&'a str>,
}

impl<'a> PromptBuilder<'a> {
    fn new(
        draft: &'a str,
        last_user_turn: Option<&'a str>,
        last_assistant_turn: Option<&'a str>,
    ) -> Self {
        Self {
            draft,
            last_user_turn,
            last_assistant_turn,
        }
    }

    fn build(self) -> Prompt {
        let request = format!(
            "You are helping complete a draft in Codex CLI.\n\
             Return exactly three candidate continuations for the current draft.\n\
             Rules:\n\
             - Each suggestion must be only the text to append to the draft.\n\
             - Do not repeat the existing draft.\n\
             - Prefer concise, plausible continuations.\n\
             - Preserve the apparent tone and topic of the conversation.\n\
             - Return strict JSON: an array of exactly 3 strings and nothing else.\n\n\
             <last_user_turn>\n{}\n</last_user_turn>\n\n\
             <last_assistant_turn>\n{}\n</last_assistant_turn>\n\n\
             <current_draft>\n{}\n</current_draft>\n",
            self.last_user_turn.unwrap_or(""),
            self.last_assistant_turn.unwrap_or(""),
            self.draft,
        );
        let mut prompt = Prompt::default();
        prompt.input = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: request }],
            end_turn: None,
            phase: None,
        }];
        prompt.base_instructions = BaseInstructions::default();
        prompt
    }
}

fn parse_suggestions(draft: &str, output: &str) -> Result<Vec<String>, String> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err("draft completion returned an empty response".to_string());
    }

    let suggestions = if let Ok(parsed) = serde_json::from_str::<Vec<String>>(trimmed) {
        parsed
    } else {
        trimmed
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| {
                line.trim_start_matches(|ch: char| {
                    ch.is_ascii_digit() || matches!(ch, '.' | ')' | '-' | '*' | ' ')
                })
            })
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    };

    let mut sanitized = Vec::new();
    for suggestion in suggestions {
        let suggestion = sanitize_suggestion(draft, &suggestion);
        if suggestion.is_empty() || sanitized.contains(&suggestion) {
            continue;
        }
        sanitized.push(suggestion);
        if sanitized.len() == MAX_SUGGESTIONS {
            break;
        }
    }

    if sanitized.is_empty() {
        Err("draft completion returned no usable suggestions".to_string())
    } else {
        Ok(sanitized)
    }
}

fn sanitize_suggestion(draft: &str, suggestion: &str) -> String {
    let trimmed = suggestion
        .trim_matches(|ch: char| matches!(ch, '\n' | '\r' | '\t' | '`'))
        .trim_end();
    let suffix = trimmed.strip_prefix(draft).unwrap_or(trimmed);
    suffix.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::parse_suggestions;

    #[test]
    fn parse_suggestions_strips_repeated_draft_prefix() {
        let suggestions =
            parse_suggestions("hello", "[\"hello world\", \" there\", \" everyone\"]")
                .expect("parse suggestions");

        assert_eq!(
            suggestions,
            vec![
                " world".to_string(),
                " there".to_string(),
                " everyone".to_string()
            ]
        );
    }

    #[test]
    fn parse_suggestions_falls_back_to_plain_lines() {
        let suggestions = parse_suggestions(
            "draft",
            "1. first suggestion\n2. second suggestion\n3. third suggestion",
        )
        .expect("parse suggestions");

        assert_eq!(
            suggestions,
            vec![
                "first suggestion".to_string(),
                "second suggestion".to_string(),
                "third suggestion".to_string()
            ]
        );
    }
}
