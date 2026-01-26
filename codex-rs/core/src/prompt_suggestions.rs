use std::sync::Arc;

use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::PromptSuggestionEvent;
use codex_protocol::protocol::SessionSource;
use futures::StreamExt;
use rand::Rng;
use serde::Deserialize;
use serde_json::json;
use tracing::warn;

use crate::WireApi;
use crate::client_common::Prompt;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::content_items_to_text;
use crate::error::CodexErr;
use crate::error::Result;
use crate::features::Feature;
use crate::truncate::TruncationPolicy;
use crate::truncate::truncate_text;

const PROMPT_SUGGESTION_SAMPLE_RATE: f64 = 0.3;
const PROMPT_SUGGESTION_INPUT_TOKENS: usize = 700;
const PROMPT_SUGGESTION_OUTPUT_BYTES: usize = 400;
const PROMPT_SUGGESTION_INSTRUCTIONS: &str = "Generate a single follow-up user prompt suggestion based on the last assistant response. \
Return a JSON object with one key: prompt. The prompt must be concise, actionable, and not repeat \
the assistant response. No extra keys or commentary.";
const PROMPT_SUGGESTION_USER_MESSAGE: &str = "Suggest the next user prompt to continue the work.";

#[derive(Deserialize)]
struct PromptSuggestionOutput {
    prompt: String,
}

pub(crate) fn maybe_spawn_prompt_suggestion(
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
    last_agent_message: Option<String>,
) {
    if !session.enabled(Feature::PromptSuggestions) {
        return;
    }
    if !matches!(turn_context.client.get_session_source(), SessionSource::Cli) {
        return;
    }
    let Some(last_agent_message) = last_agent_message
        .map(|message| message.trim().to_string())
        .filter(|message| !message.is_empty())
    else {
        return;
    };
    let mut rng = rand::rng();
    if !rng.random_bool(PROMPT_SUGGESTION_SAMPLE_RATE) {
        return;
    }

    tokio::spawn(async move {
        if let Err(err) =
            generate_and_emit_prompt_suggestion(session, turn_context, last_agent_message).await
        {
            warn!("prompt suggestion generation failed: {err}");
        }
    });
}

async fn generate_and_emit_prompt_suggestion(
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
    last_agent_message: String,
) -> Result<()> {
    let mut input = if let Some(history_depth) = turn_context.history_depth {
        let mut history = session.clone_history().await;
        history.retain_last_n_user_turns(history_depth);
        history.for_prompt()
    } else {
        let truncated = truncate_text(
            &last_agent_message,
            TruncationPolicy::Tokens(PROMPT_SUGGESTION_INPUT_TOKENS),
        );
        vec![ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText { text: truncated }],
            end_turn: None,
        }]
    };
    input.push(ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: PROMPT_SUGGESTION_USER_MESSAGE.to_string(),
        }],
        end_turn: None,
    });

    let output_schema = match turn_context.client.get_provider().wire_api {
        WireApi::Chat => None,
        WireApi::Responses | WireApi::ResponsesWebsocket => Some(prompt_suggestion_schema()),
    };

    let prompt = Prompt {
        input,
        tools: Vec::new(),
        parallel_tool_calls: false,
        base_instructions: BaseInstructions {
            text: PROMPT_SUGGESTION_INSTRUCTIONS.to_string(),
        },
        personality: None,
        max_output_tokens: turn_context.max_output_tokens,
        output_schema,
    };

    let mut client_session = turn_context.client.new_session();
    let mut stream = client_session.stream(&prompt).await?;
    let mut output_text = String::new();
    let mut last_message: Option<String> = None;

    loop {
        let Some(event) = stream.next().await else {
            return Err(CodexErr::Stream(
                "stream closed before response.completed".into(),
                None,
            ));
        };
        match event {
            Ok(codex_api::common::ResponseEvent::OutputTextDelta(delta)) => {
                output_text.push_str(&delta);
            }
            Ok(codex_api::common::ResponseEvent::OutputItemDone(item)) => {
                if let ResponseItem::Message { role, content, .. } = &item
                    && role == "assistant"
                {
                    last_message = content_items_to_text(content);
                }
            }
            Ok(codex_api::common::ResponseEvent::RateLimits(snapshot)) => {
                session
                    .update_rate_limits(turn_context.as_ref(), snapshot)
                    .await;
            }
            Ok(codex_api::common::ResponseEvent::Completed { token_usage, .. }) => {
                session
                    .update_token_usage_info(turn_context.as_ref(), token_usage.as_ref())
                    .await;
                break;
            }
            Ok(_) => {}
            Err(err) => return Err(err),
        }
    }

    let raw = if output_text.trim().is_empty() {
        last_message.unwrap_or_default()
    } else {
        output_text
    };

    let Some(suggestion) = normalize_prompt_suggestion(&raw) else {
        return Ok(());
    };

    if session.active_turn.lock().await.is_some() {
        return Ok(());
    }

    session
        .send_event(
            turn_context.as_ref(),
            EventMsg::PromptSuggestion(PromptSuggestionEvent { suggestion }),
        )
        .await;
    Ok(())
}

fn prompt_suggestion_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "prompt": { "type": "string" }
        },
        "required": ["prompt"],
        "additionalProperties": false
    })
}

fn normalize_prompt_suggestion(raw: &str) -> Option<String> {
    let candidate = extract_prompt_candidate(raw).unwrap_or_else(|| raw.to_string());
    let trimmed = candidate.trim().trim_matches('"').trim_matches('`').trim();
    if trimmed.is_empty() {
        return None;
    }
    let truncated = truncate_text(
        trimmed,
        TruncationPolicy::Bytes(PROMPT_SUGGESTION_OUTPUT_BYTES),
    );
    let cleaned = truncated.trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

fn extract_prompt_candidate(raw: &str) -> Option<String> {
    if let Ok(parsed) = serde_json::from_str::<PromptSuggestionOutput>(raw) {
        return Some(parsed.prompt);
    }
    if let (Some(start), Some(end)) = (raw.find('{'), raw.rfind('}'))
        && start < end
        && let Some(slice) = raw.get(start..=end)
        && let Ok(parsed) = serde_json::from_str::<PromptSuggestionOutput>(slice)
    {
        return Some(parsed.prompt);
    }
    None
}
