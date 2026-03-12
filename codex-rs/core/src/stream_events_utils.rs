use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_protocol::config_types::ModeKind;
use codex_protocol::items::TurnItem;
use codex_utils_stream_parser::strip_citations;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::error::CodexErr;
use crate::error::Result;
use crate::function_tool::FunctionCallError;
use crate::memories::citations::get_thread_id_from_citations;
use crate::parse_turn_item;
use crate::state_db;
use crate::tools::parallel::ToolCallRuntime;
use crate::tools::router::ToolRouter;
use codex_protocol::models::DeveloperInstructions;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_utils_stream_parser::strip_proposed_plan_blocks;
use futures::Future;
use tracing::debug;
use tracing::instrument;

fn strip_hidden_assistant_markup(text: &str, plan_mode: bool) -> String {
    let (without_citations, _) = strip_citations(text);
    if plan_mode {
        strip_proposed_plan_blocks(&without_citations)
    } else {
        without_citations
    }
}

pub(crate) fn raw_assistant_output_text_from_item(item: &ResponseItem) -> Option<String> {
    if let ResponseItem::Message { role, content, .. } = item
        && role == "assistant"
    {
        let combined = content
            .iter()
            .filter_map(|ci| match ci {
                codex_protocol::models::ContentItem::OutputText { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();
        return Some(combined);
    }
    None
}

async fn save_image_generation_result(call_id: &str, result: &str) -> Result<PathBuf> {
    let bytes = BASE64_STANDARD
        .decode(result.trim().as_bytes())
        .map_err(|err| {
            CodexErr::InvalidRequest(format!("invalid image generation payload: {err}"))
        })?;
    let mut file_stem: String = call_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if file_stem.is_empty() {
        file_stem = "generated_image".to_string();
    }
    let path = default_image_generation_output_dir().join(format!("{file_stem}.png"));
    tokio::fs::write(&path, bytes).await?;
    Ok(path)
}

pub(crate) fn default_image_generation_output_dir() -> PathBuf {
    std::env::temp_dir()
}

/// Persist a completed model response item and record any cited memory usage.
pub(crate) async fn record_completed_response_item(
    sess: &Session,
    turn_context: &TurnContext,
    item: &ResponseItem,
) {
    sess.record_conversation_items(turn_context, std::slice::from_ref(item))
        .await;
    maybe_mark_thread_memory_mode_polluted_from_web_search(sess, turn_context, item).await;
    record_stage1_output_usage_for_completed_item(turn_context, item).await;
}

async fn maybe_mark_thread_memory_mode_polluted_from_web_search(
    sess: &Session,
    turn_context: &TurnContext,
    item: &ResponseItem,
) {
    if !turn_context
        .config
        .memories
        .no_memories_if_mcp_or_web_search
        || !matches!(item, ResponseItem::WebSearchCall { .. })
    {
        return;
    }
    state_db::mark_thread_memory_mode_polluted(
        sess.services.state_db.as_deref(),
        sess.conversation_id,
        "record_completed_response_item",
    )
    .await;
}

async fn record_stage1_output_usage_for_completed_item(
    turn_context: &TurnContext,
    item: &ResponseItem,
) {
    let Some(raw_text) = raw_assistant_output_text_from_item(item) else {
        return;
    };

    let (_, citations) = strip_citations(&raw_text);
    let thread_ids = get_thread_id_from_citations(citations);
    if thread_ids.is_empty() {
        return;
    }

    if let Some(db) = state_db::get_state_db(turn_context.config.as_ref()).await {
        let _ = db.record_stage1_output_usage(&thread_ids).await;
    }
}

/// Handle a completed output item from the model stream, recording it and
/// queuing any tool execution futures. This records items immediately so
/// history and rollout stay in sync even if the turn is later cancelled.
pub(crate) type InFlightFuture<'f> =
    Pin<Box<dyn Future<Output = Result<ResponseInputItem>> + Send + 'f>>;

#[derive(Default)]
pub(crate) struct OutputItemResult {
    pub last_agent_message: Option<String>,
    pub needs_follow_up: bool,
    pub tool_future: Option<InFlightFuture<'static>>,
}

pub(crate) struct HandleOutputCtx {
    pub sess: Arc<Session>,
    pub turn_context: Arc<TurnContext>,
    pub tool_runtime: ToolCallRuntime,
    pub cancellation_token: CancellationToken,
}

#[instrument(level = "trace", skip_all)]
pub(crate) async fn handle_output_item_done(
    ctx: &mut HandleOutputCtx,
    item: ResponseItem,
    previously_active_item: Option<TurnItem>,
) -> Result<OutputItemResult> {
    let mut output = OutputItemResult::default();
    let plan_mode = ctx.turn_context.collaboration_mode.mode == ModeKind::Plan;

    match ToolRouter::build_tool_call(ctx.sess.as_ref(), item.clone()).await {
        // The model emitted a tool call; log it, persist the item immediately, and queue the tool execution.
        Ok(Some(call)) => {
            let payload_preview = call.payload.log_payload().into_owned();
            tracing::info!(
                thread_id = %ctx.sess.conversation_id,
                "ToolCall: {} {}",
                call.tool_name,
                payload_preview
            );

            record_completed_response_item(ctx.sess.as_ref(), ctx.turn_context.as_ref(), &item)
                .await;

            let cancellation_token = ctx.cancellation_token.child_token();
            let tool_future: InFlightFuture<'static> = Box::pin(
                ctx.tool_runtime
                    .clone()
                    .handle_tool_call(call, cancellation_token),
            );

            output.needs_follow_up = true;
            output.tool_future = Some(tool_future);
        }
        // No tool call: convert messages/reasoning into turn items and mark them as complete.
        Ok(None) => {
            if let Some(turn_item) = handle_non_tool_response_item(
                ctx.sess.as_ref(),
                ctx.turn_context.as_ref(),
                &item,
                plan_mode,
            )
            .await
            {
                if previously_active_item.is_none() {
                    let mut started_item = turn_item.clone();
                    if let TurnItem::ImageGeneration(item) = &mut started_item {
                        item.status = "in_progress".to_string();
                        item.revised_prompt = None;
                        item.result.clear();
                        item.saved_path = None;
                    }
                    ctx.sess
                        .emit_turn_item_started(&ctx.turn_context, &started_item)
                        .await;
                }

                ctx.sess
                    .emit_turn_item_completed(&ctx.turn_context, turn_item)
                    .await;
            }

            record_completed_response_item(ctx.sess.as_ref(), ctx.turn_context.as_ref(), &item)
                .await;
            let last_agent_message = last_assistant_message_from_item(&item, plan_mode);

            output.last_agent_message = last_agent_message;
        }
        // Guardrail: the model issued a LocalShellCall without an id; surface the error back into history.
        Err(FunctionCallError::MissingLocalShellCallId) => {
            let msg = "LocalShellCall without call_id or id";
            ctx.turn_context
                .session_telemetry
                .log_tool_failed("local_shell", msg);
            tracing::error!(msg);

            let response = ResponseInputItem::FunctionCallOutput {
                call_id: String::new(),
                output: FunctionCallOutputPayload {
                    body: FunctionCallOutputBody::Text(msg.to_string()),
                    ..Default::default()
                },
            };
            record_completed_response_item(ctx.sess.as_ref(), ctx.turn_context.as_ref(), &item)
                .await;
            if let Some(response_item) = response_input_to_response_item(&response) {
                ctx.sess
                    .record_conversation_items(
                        &ctx.turn_context,
                        std::slice::from_ref(&response_item),
                    )
                    .await;
            }

            output.needs_follow_up = true;
        }
        // The tool request should be answered directly (or was denied); push that response into the transcript.
        Err(FunctionCallError::RespondToModel(message)) => {
            let response = ResponseInputItem::FunctionCallOutput {
                call_id: String::new(),
                output: FunctionCallOutputPayload {
                    body: FunctionCallOutputBody::Text(message),
                    ..Default::default()
                },
            };
            record_completed_response_item(ctx.sess.as_ref(), ctx.turn_context.as_ref(), &item)
                .await;
            if let Some(response_item) = response_input_to_response_item(&response) {
                ctx.sess
                    .record_conversation_items(
                        &ctx.turn_context,
                        std::slice::from_ref(&response_item),
                    )
                    .await;
            }

            output.needs_follow_up = true;
        }
        // A fatal error occurred; surface it back into history.
        Err(FunctionCallError::Fatal(message)) => {
            return Err(CodexErr::Fatal(message));
        }
    }

    Ok(output)
}

pub(crate) async fn handle_non_tool_response_item(
    sess: &Session,
    turn_context: &TurnContext,
    item: &ResponseItem,
    plan_mode: bool,
) -> Option<TurnItem> {
    debug!(?item, "Output item");

    match item {
        ResponseItem::Message { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. } => {
            let mut turn_item = parse_turn_item(item)?;
            if let TurnItem::AgentMessage(agent_message) = &mut turn_item {
                let combined = agent_message
                    .content
                    .iter()
                    .map(|entry| match entry {
                        codex_protocol::items::AgentMessageContent::Text { text } => text.as_str(),
                    })
                    .collect::<String>();
                let stripped = strip_hidden_assistant_markup(&combined, plan_mode);
                agent_message.content =
                    vec![codex_protocol::items::AgentMessageContent::Text { text: stripped }];
            }
            if let TurnItem::ImageGeneration(image_item) = &mut turn_item {
                match save_image_generation_result(&image_item.id, &image_item.result).await {
                    Ok(path) => {
                        image_item.saved_path = Some(path.to_string_lossy().into_owned());
                        let image_output_dir = default_image_generation_output_dir();
                        let message: ResponseItem = DeveloperInstructions::new(format!(
                            "Generated images are saved to {} as {} by default.",
                            image_output_dir.display(),
                            image_output_dir.join("<image_id>.png").display(),
                        ))
                        .into();
                        sess.record_conversation_items(
                            turn_context,
                            std::slice::from_ref(&message),
                        )
                        .await;
                    }
                    Err(err) => {
                        let output_dir = default_image_generation_output_dir();
                        tracing::warn!(
                            call_id = %image_item.id,
                            output_dir = %output_dir.display(),
                            "failed to save generated image: {err}"
                        );
                    }
                }
            }
            Some(turn_item)
        }
        ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::ToolSearchOutput { .. } => {
            debug!("unexpected tool output from stream");
            None
        }
        _ => None,
    }
}

pub(crate) fn last_assistant_message_from_item(
    item: &ResponseItem,
    plan_mode: bool,
) -> Option<String> {
    if let Some(combined) = raw_assistant_output_text_from_item(item) {
        if combined.is_empty() {
            return None;
        }
        let stripped = strip_hidden_assistant_markup(&combined, plan_mode);
        if stripped.trim().is_empty() {
            return None;
        }
        return Some(stripped);
    }
    None
}

pub(crate) fn response_input_to_response_item(input: &ResponseInputItem) -> Option<ResponseItem> {
    match input {
        ResponseInputItem::FunctionCallOutput { call_id, output } => {
            Some(ResponseItem::FunctionCallOutput {
                call_id: call_id.clone(),
                output: output.clone(),
            })
        }
        ResponseInputItem::CustomToolCallOutput { call_id, output } => {
            Some(ResponseItem::CustomToolCallOutput {
                call_id: call_id.clone(),
                output: output.clone(),
            })
        }
        ResponseInputItem::McpToolCallOutput { call_id, output } => {
            let output = output.as_function_call_output_payload();
            Some(ResponseItem::FunctionCallOutput {
                call_id: call_id.clone(),
                output,
            })
        }
        ResponseInputItem::ToolSearchOutput {
            call_id,
            status,
            execution,
            tools,
        } => Some(ResponseItem::ToolSearchOutput {
            call_id: Some(call_id.clone()),
            status: status.clone(),
            execution: execution.clone(),
            tools: tools.clone(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::default_image_generation_output_dir;
    use super::handle_non_tool_response_item;
    use super::last_assistant_message_from_item;
    use super::save_image_generation_result;
    use crate::codex::make_session_and_context;
    use crate::error::CodexErr;
    use codex_protocol::items::TurnItem;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use pretty_assertions::assert_eq;

    fn assistant_output_text(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: Some("msg-1".to_string()),
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
            end_turn: Some(true),
            phase: None,
        }
    }

    #[tokio::test]
    async fn handle_non_tool_response_item_strips_citations_from_assistant_message() {
        let (session, turn_context) = make_session_and_context().await;
        let item = assistant_output_text("hello<oai-mem-citation>doc1</oai-mem-citation> world");

        let turn_item = handle_non_tool_response_item(&session, &turn_context, &item, false)
            .await
            .expect("assistant message should parse");

        let TurnItem::AgentMessage(agent_message) = turn_item else {
            panic!("expected agent message");
        };
        let text = agent_message
            .content
            .iter()
            .map(|entry| match entry {
                codex_protocol::items::AgentMessageContent::Text { text } => text.as_str(),
            })
            .collect::<String>();
        assert_eq!(text, "hello world");
    }

    #[test]
    fn last_assistant_message_from_item_strips_citations_and_plan_blocks() {
        let item = assistant_output_text(
            "before<oai-mem-citation>doc1</oai-mem-citation>\n<proposed_plan>\n- x\n</proposed_plan>\nafter",
        );

        let message = last_assistant_message_from_item(&item, true)
            .expect("assistant text should remain after stripping");

        assert_eq!(message, "before\nafter");
    }

    #[test]
    fn last_assistant_message_from_item_returns_none_for_citation_only_message() {
        let item = assistant_output_text("<oai-mem-citation>doc1</oai-mem-citation>");

        assert_eq!(last_assistant_message_from_item(&item, false), None);
    }

    #[test]
    fn last_assistant_message_from_item_returns_none_for_plan_only_hidden_message() {
        let item = assistant_output_text("<proposed_plan>\n- x\n</proposed_plan>");

        assert_eq!(last_assistant_message_from_item(&item, true), None);
    }

    #[tokio::test]
    async fn save_image_generation_result_saves_base64_to_png_in_temp_dir() {
        let expected_path = default_image_generation_output_dir().join("ig_save_base64.png");
        let _ = std::fs::remove_file(&expected_path);

        let saved_path = save_image_generation_result("ig_save_base64", "Zm9v")
            .await
            .expect("image should be saved");

        assert_eq!(saved_path, expected_path);
        assert_eq!(std::fs::read(&saved_path).expect("saved file"), b"foo");
        let _ = std::fs::remove_file(&saved_path);
    }

    #[tokio::test]
    async fn save_image_generation_result_rejects_data_url_payload() {
        let result = "data:image/jpeg;base64,Zm9v";

        let err = save_image_generation_result("ig_456", result)
            .await
            .expect_err("data url payload should error");
        assert!(matches!(err, CodexErr::InvalidRequest(_)));
    }

    #[tokio::test]
    async fn save_image_generation_result_overwrites_existing_file() {
        let existing_path = default_image_generation_output_dir().join("ig_overwrite.png");
        std::fs::write(&existing_path, b"existing").expect("seed existing image");

        let saved_path = save_image_generation_result("ig_overwrite", "Zm9v")
            .await
            .expect("image should be saved");

        assert_eq!(saved_path, existing_path);
        assert_eq!(std::fs::read(&saved_path).expect("saved file"), b"foo");
        let _ = std::fs::remove_file(&saved_path);
    }

    #[tokio::test]
    async fn save_image_generation_result_sanitizes_call_id_for_temp_dir_output_path() {
        let expected_path = default_image_generation_output_dir().join("___ig___.png");
        let _ = std::fs::remove_file(&expected_path);

        let saved_path = save_image_generation_result("../ig/..", "Zm9v")
            .await
            .expect("image should be saved");

        assert_eq!(saved_path, expected_path);
        assert_eq!(std::fs::read(&saved_path).expect("saved file"), b"foo");
        let _ = std::fs::remove_file(&saved_path);
    }

    #[tokio::test]
    async fn save_image_generation_result_rejects_non_standard_base64() {
        let err = save_image_generation_result("ig_urlsafe", "_-8")
            .await
            .expect_err("non-standard base64 should error");
        assert!(matches!(err, CodexErr::InvalidRequest(_)));
    }

    #[tokio::test]
    async fn save_image_generation_result_rejects_non_base64_data_urls() {
        let err = save_image_generation_result("ig_svg", "data:image/svg+xml,<svg/>")
            .await
            .expect_err("non-base64 data url should error");
        assert!(matches!(err, CodexErr::InvalidRequest(_)));
    }
}
