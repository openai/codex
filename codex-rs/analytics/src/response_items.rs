use crate::byte_len;
use crate::facts::CodexResponseItemType;
use crate::facts::CodexResponsesApiItemMetadata;
use crate::facts::CodexResponsesApiItemPhase;
use crate::nonzero_i64;
use crate::serialized_bytes;
use crate::serialized_string;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;

pub(crate) fn response_items_metadata(
    phase: CodexResponsesApiItemPhase,
    items: &[ResponseItem],
) -> Vec<CodexResponsesApiItemMetadata> {
    items
        .iter()
        .enumerate()
        .map(|(item_index, item)| response_item_metadata(phase, item_index, item))
        .collect()
}

pub(crate) fn response_item_metadata(
    item_phase: CodexResponsesApiItemPhase,
    item_index: usize,
    item: &ResponseItem,
) -> CodexResponsesApiItemMetadata {
    let mut metadata = new_metadata(item_phase, item_index, response_item_type(item));

    match item {
        ResponseItem::Message {
            role,
            content,
            phase,
            ..
        } => {
            metadata.role = Some(role.clone());
            metadata.message_phase = phase.clone();
            metadata.payload_bytes = nonzero_i64(message_content_text_bytes(content));
            let (text_part_count, image_part_count) = message_content_part_counts(content);
            metadata.text_part_count = Some(text_part_count);
            metadata.image_part_count = Some(image_part_count);
        }
        ResponseItem::Reasoning {
            summary,
            content,
            encrypted_content,
            ..
        } => {
            metadata.payload_bytes = encrypted_content
                .as_ref()
                .map(|value| byte_len(value))
                .or_else(|| nonzero_i64(reasoning_content_bytes(summary, content)));
            metadata.text_part_count =
                Some(summary.len() + content.as_ref().map(std::vec::Vec::len).unwrap_or_default());
            metadata.image_part_count = Some(0);
        }
        ResponseItem::LocalShellCall {
            call_id,
            status,
            action,
            ..
        } => {
            metadata.call_id = call_id.clone();
            metadata.tool_name = Some("local_shell".to_string());
            metadata.status = serialized_string(status);
            metadata.payload_bytes = serialized_bytes(action);
        }
        ResponseItem::FunctionCall {
            name,
            arguments,
            call_id,
            ..
        } => {
            metadata.call_id = Some(call_id.clone());
            metadata.tool_name = Some(name.clone());
            metadata.payload_bytes = Some(byte_len(arguments));
        }
        ResponseItem::ToolSearchCall {
            call_id,
            status,
            arguments,
            ..
        } => {
            metadata.call_id = call_id.clone();
            metadata.tool_name = Some("tool_search".to_string());
            metadata.status = status.clone();
            metadata.payload_bytes = serialized_bytes(arguments);
        }
        ResponseItem::FunctionCallOutput { call_id, output } => {
            metadata.call_id = Some(call_id.clone());
            metadata.payload_bytes = function_call_output_bytes(output);
            let (text_part_count, image_part_count) = function_call_output_part_counts(output);
            metadata.text_part_count = text_part_count;
            metadata.image_part_count = image_part_count;
        }
        ResponseItem::CustomToolCall {
            status,
            call_id,
            name,
            input,
            ..
        } => {
            metadata.call_id = Some(call_id.clone());
            metadata.tool_name = Some(name.clone());
            metadata.status = status.clone();
            metadata.payload_bytes = Some(byte_len(input));
        }
        ResponseItem::CustomToolCallOutput {
            call_id,
            name,
            output,
        } => {
            metadata.call_id = Some(call_id.clone());
            metadata.tool_name = name.clone();
            metadata.payload_bytes = function_call_output_bytes(output);
            let (text_part_count, image_part_count) = function_call_output_part_counts(output);
            metadata.text_part_count = text_part_count;
            metadata.image_part_count = image_part_count;
        }
        ResponseItem::ToolSearchOutput {
            call_id,
            status,
            tools,
            ..
        } => {
            metadata.call_id = call_id.clone();
            metadata.tool_name = Some("tool_search".to_string());
            metadata.status = Some(status.clone());
            metadata.payload_bytes = serialized_bytes(tools);
        }
        ResponseItem::WebSearchCall { status, action, .. } => {
            metadata.tool_name = Some("web_search".to_string());
            metadata.status = status.clone();
            metadata.payload_bytes = action.as_ref().and_then(serialized_bytes);
        }
        ResponseItem::ImageGenerationCall {
            id,
            status,
            revised_prompt,
            result,
        } => {
            metadata.call_id = Some(id.clone());
            metadata.tool_name = Some("image_generation".to_string());
            metadata.status = Some(status.clone());
            metadata.payload_bytes = nonzero_i64(byte_len(result))
                .or_else(|| revised_prompt.as_ref().map(|value| byte_len(value)));
        }
        ResponseItem::Compaction { encrypted_content } => {
            metadata.payload_bytes = Some(byte_len(encrypted_content));
        }
        ResponseItem::GhostSnapshot { .. } | ResponseItem::Other => {}
    }

    metadata
}

fn new_metadata(
    item_phase: CodexResponsesApiItemPhase,
    item_index: usize,
    response_item_type: CodexResponseItemType,
) -> CodexResponsesApiItemMetadata {
    CodexResponsesApiItemMetadata {
        item_phase,
        item_index,
        response_item_type,
        role: None,
        status: None,
        message_phase: None,
        call_id: None,
        tool_name: None,
        payload_bytes: None,
        text_part_count: None,
        image_part_count: None,
    }
}

fn response_item_type(item: &ResponseItem) -> CodexResponseItemType {
    match item {
        ResponseItem::Message { .. } => CodexResponseItemType::Message,
        ResponseItem::Reasoning { .. } => CodexResponseItemType::Reasoning,
        ResponseItem::LocalShellCall { .. } => CodexResponseItemType::LocalShellCall,
        ResponseItem::FunctionCall { .. } => CodexResponseItemType::FunctionCall,
        ResponseItem::ToolSearchCall { .. } => CodexResponseItemType::ToolSearchCall,
        ResponseItem::FunctionCallOutput { .. } => CodexResponseItemType::FunctionCallOutput,
        ResponseItem::CustomToolCall { .. } => CodexResponseItemType::CustomToolCall,
        ResponseItem::CustomToolCallOutput { .. } => CodexResponseItemType::CustomToolCallOutput,
        ResponseItem::ToolSearchOutput { .. } => CodexResponseItemType::ToolSearchOutput,
        ResponseItem::WebSearchCall { .. } => CodexResponseItemType::WebSearchCall,
        ResponseItem::ImageGenerationCall { .. } => CodexResponseItemType::ImageGenerationCall,
        ResponseItem::GhostSnapshot { .. } => CodexResponseItemType::GhostSnapshot,
        ResponseItem::Compaction { .. } => CodexResponseItemType::Compaction,
        ResponseItem::Other => CodexResponseItemType::Other,
    }
}

fn message_content_text_bytes(content: &[ContentItem]) -> i64 {
    content
        .iter()
        .map(|item| match item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => byte_len(text),
            ContentItem::InputImage { .. } => 0,
        })
        .sum()
}

fn message_content_part_counts(content: &[ContentItem]) -> (usize, usize) {
    let mut text_part_count = 0;
    let mut image_part_count = 0;
    for item in content {
        match item {
            ContentItem::InputText { .. } | ContentItem::OutputText { .. } => {
                text_part_count += 1;
            }
            ContentItem::InputImage { .. } => {
                image_part_count += 1;
            }
        }
    }
    (text_part_count, image_part_count)
}

fn reasoning_content_bytes(
    summary: &[ReasoningItemReasoningSummary],
    content: &Option<Vec<ReasoningItemContent>>,
) -> i64 {
    let summary_bytes = summary
        .iter()
        .map(|summary| match summary {
            ReasoningItemReasoningSummary::SummaryText { text } => byte_len(text),
        })
        .sum::<i64>();
    let content_bytes = content
        .as_ref()
        .map(|content| {
            content
                .iter()
                .map(|content| match content {
                    ReasoningItemContent::ReasoningText { text }
                    | ReasoningItemContent::Text { text } => byte_len(text),
                })
                .sum::<i64>()
        })
        .unwrap_or_default();
    summary_bytes + content_bytes
}

fn function_call_output_bytes(output: &FunctionCallOutputPayload) -> Option<i64> {
    match &output.body {
        FunctionCallOutputBody::Text(text) => Some(byte_len(text)),
        FunctionCallOutputBody::ContentItems(items) => serialized_bytes(items),
    }
}

fn function_call_output_part_counts(
    output: &FunctionCallOutputPayload,
) -> (Option<usize>, Option<usize>) {
    let Some(content_items) = output.content_items() else {
        return (None, None);
    };
    let mut text_part_count = 0;
    let mut image_part_count = 0;
    for item in content_items {
        match item {
            FunctionCallOutputContentItem::InputText { .. } => {
                text_part_count += 1;
            }
            FunctionCallOutputContentItem::InputImage { .. } => {
                image_part_count += 1;
            }
        }
    }
    (Some(text_part_count), Some(image_part_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ImageDetail;
    use codex_protocol::models::MessagePhase;

    #[test]
    fn maps_message_metadata() {
        let items = vec![ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![
                ContentItem::OutputText {
                    text: "hello".to_string(),
                },
                ContentItem::InputImage {
                    image_url: "data:image/png;base64,abc".to_string(),
                    detail: None,
                },
            ],
            end_turn: None,
            phase: Some(MessagePhase::FinalAnswer),
        }];

        let metadata = response_items_metadata(CodexResponsesApiItemPhase::Output, &items);

        assert_eq!(metadata[0].item_phase, CodexResponsesApiItemPhase::Output);
        assert_eq!(
            metadata[0].response_item_type,
            CodexResponseItemType::Message
        );
        assert_eq!(metadata[0].role.as_deref(), Some("assistant"));
        assert_eq!(metadata[0].message_phase, Some(MessagePhase::FinalAnswer));
        assert_eq!(metadata[0].payload_bytes, Some(5));
        assert_eq!(metadata[0].text_part_count, Some(1));
        assert_eq!(metadata[0].image_part_count, Some(1));
    }

    #[test]
    fn maps_tool_call_output_metadata() {
        let items = vec![ResponseItem::CustomToolCallOutput {
            call_id: "call_1".to_string(),
            name: Some("custom_tool".to_string()),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "result".to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "https://example.test/image.png".to_string(),
                    detail: Some(ImageDetail::High),
                },
            ]),
        }];

        let metadata = response_items_metadata(CodexResponsesApiItemPhase::Output, &items);

        assert_eq!(
            metadata[0].response_item_type,
            CodexResponseItemType::CustomToolCallOutput
        );
        assert_eq!(metadata[0].call_id.as_deref(), Some("call_1"));
        assert_eq!(metadata[0].tool_name.as_deref(), Some("custom_tool"));
        assert!(metadata[0].payload_bytes.unwrap_or_default() > 0);
        assert_eq!(metadata[0].text_part_count, Some(1));
        assert_eq!(metadata[0].image_part_count, Some(1));
    }
}
