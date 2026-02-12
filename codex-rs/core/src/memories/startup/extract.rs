use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::client_common::ResponseStream;
use crate::codex::Session;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::rollout::RolloutRecorder;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use futures::StreamExt;
use tracing::warn;

use super::StageOneRequestContext;
use crate::memories::prompts::build_stage_one_input_message;
use crate::memories::stage_one::RAW_MEMORY_PROMPT;
use crate::memories::stage_one::StageOneOutput;
use crate::memories::stage_one::parse_stage_one_output;
use crate::memories::stage_one::stage_one_output_schema;
use crate::rollout::policy::should_persist_response_item_for_memories;
use codex_protocol::protocol::RolloutItem;
use std::path::Path;

pub(super) async fn extract_stage_one_output(
    session: &Session,
    rollout_path: &Path,
    rollout_cwd: &Path,
    stage_one_context: &StageOneRequestContext,
) -> Result<StageOneOutput, &'static str> {
    let (rollout_items, _thread_id, parse_errors) =
        match RolloutRecorder::load_rollout_items(rollout_path).await {
            Ok(result) => result,
            Err(err) => {
                warn!(
                    "failed to load rollout {} for memories: {err}",
                    rollout_path.display()
                );
                return Err("failed to load rollout");
            }
        };
    if parse_errors > 0 {
        warn!(
            "rollout {} had {parse_errors} parse errors while preparing stage-1 memory input",
            rollout_path.display()
        );
    }

    let rollout_contents = match serialize_filtered_rollout_response_items(&rollout_items) {
        Ok(contents) => contents,
        Err(err) => {
            warn!(
                "failed to prepare filtered rollout payload {} for memories: {err}",
                rollout_path.display()
            );
            return Err("failed to serialize filtered rollout");
        }
    };

    let prompt = Prompt {
        input: vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: build_stage_one_input_message(
                    &stage_one_context.model_info,
                    rollout_path,
                    rollout_cwd,
                    &rollout_contents,
                )
                .map_err(|_e| "error while building the prompt")?,
            }],
            end_turn: None,
            phase: None,
        }],
        tools: Vec::new(),
        parallel_tool_calls: false,
        base_instructions: BaseInstructions {
            text: RAW_MEMORY_PROMPT.to_string(),
        },
        personality: None,
        output_schema: Some(stage_one_output_schema()),
    };

    let mut client_session = session.services.model_client.new_session();
    let mut stream = match client_session
        .stream(
            &prompt,
            &stage_one_context.model_info,
            &stage_one_context.otel_manager,
            stage_one_context.reasoning_effort,
            stage_one_context.reasoning_summary,
            stage_one_context.turn_metadata_header.as_deref(),
        )
        .await
    {
        Ok(stream) => stream,
        Err(err) => {
            warn!(
                "stage-1 memory request failed for rollout {}: {err}",
                rollout_path.display()
            );
            return Err("stage-1 memory request failed");
        }
    };

    let output_text = match collect_response_text_until_completed(&mut stream).await {
        Ok(text) => text,
        Err(err) => {
            warn!(
                "failed while waiting for stage-1 memory response for rollout {}: {err}",
                rollout_path.display()
            );
            return Err("stage-1 memory response stream failed");
        }
    };

    match parse_stage_one_output(&output_text) {
        Ok(output) => Ok(output),
        Err(err) => {
            warn!(
                "invalid stage-1 memory payload for rollout {}: {err}",
                rollout_path.display()
            );
            Err("invalid stage-1 memory payload")
        }
    }
}

async fn collect_response_text_until_completed(stream: &mut ResponseStream) -> CodexResult<String> {
    let mut output_text = String::new();

    loop {
        let Some(event) = stream.next().await else {
            return Err(CodexErr::Stream(
                "stream closed before response.completed".to_string(),
                None,
            ));
        };

        match event? {
            ResponseEvent::OutputTextDelta(delta) => output_text.push_str(&delta),
            ResponseEvent::OutputItemDone(item) => {
                if output_text.is_empty()
                    && let ResponseItem::Message { content, .. } = item
                    && let Some(text) = crate::compact::content_items_to_text(&content)
                {
                    output_text.push_str(&text);
                }
            }
            ResponseEvent::Completed { .. } => return Ok(output_text),
            _ => {}
        }
    }
}

/// Serializes filtered stage-1 memory items for prompt inclusion.
fn serialize_filtered_rollout_response_items(
    items: &[RolloutItem],
) -> crate::error::Result<String> {
    let filtered = items
        .iter()
        .filter_map(|item| {
            if let RolloutItem::ResponseItem(item) = item
                && should_persist_response_item_for_memories(item)
            {
                Some(item.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&filtered).map_err(|err| {
        CodexErr::InvalidRequest(format!("failed to serialize rollout memory: {err}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_filtered_rollout_response_items_keeps_response_items_only() {
        let input = vec![RolloutItem::ResponseItem(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "user input".to_string(),
            }],
            end_turn: None,
            phase: None,
        })];

        let serialized = serialize_filtered_rollout_response_items(&input).expect("serialize");
        let parsed: Vec<ResponseItem> = serde_json::from_str(&serialized).expect("deserialize");

        pretty_assertions::assert_eq!(parsed.len(), 1);
        assert!(matches!(parsed[0], ResponseItem::Message { .. }));
    }
}
