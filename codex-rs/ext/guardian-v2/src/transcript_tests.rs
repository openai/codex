use codex_extension_api::ResponseItem;
use codex_protocol::models::AgentMessageInputContent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use pretty_assertions::assert_eq;

use super::MANUAL_APPROVAL_DEVELOPER_PREFIX;
use super::MAX_TRANSCRIPT_BYTES;
use super::TranscriptConfig;
use super::TranscriptSource;

#[test]
fn transcript_keeps_conversation_and_configured_sources() {
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Inspect the workspace.".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "exec_command".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            encrypted_function_args: None,
            call_id: "call-1".to_string(),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text("Workspace inspected.".to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Reasoning {
            id: None,
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "Review the current files.".to_string(),
            }],
            content: Some(vec![ReasoningItemContent::ReasoningText {
                text: "Plaintext reasoning.".to_string(),
            }]),
            encrypted_content: Some("encrypted-reasoning-blob".to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: Vec::new(),
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let transcript = TranscriptConfig::default().build(&items);
    assert_eq!(
        transcript,
        concat!(
            "[1] user: Inspect the workspace.\n",
            "[2] tool exec_command call: {}\n",
            "[3] tool exec_command result: Workspace inspected.\n",
            "[4] reasoning: Review the current files.\n",
            "Plaintext reasoning.\n",
        )
    );

    let output_and_reasoning = TranscriptConfig {
        sources: vec![TranscriptSource::ToolOutputs, TranscriptSource::Reasoning],
    };

    let transcript = output_and_reasoning.build(&items);
    assert_eq!(
        transcript,
        concat!(
            "[1] user: Inspect the workspace.\n",
            "[2] tool exec_command result: Workspace inspected.\n",
            "[3] reasoning: Review the current files.\n",
            "Plaintext reasoning.\n",
        )
    );

    let calls_only = TranscriptConfig {
        sources: vec![TranscriptSource::ToolCalls],
    };

    let transcript = calls_only.build(&items);
    assert_eq!(
        transcript,
        concat!(
            "[1] user: Inspect the workspace.\n",
            "[2] tool exec_command call: {}\n",
        )
    );
}

#[test]
fn transcript_retains_the_most_recent_bounded_content() {
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "é".repeat(MAX_TRANSCRIPT_BYTES),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "latest response".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let transcript = TranscriptConfig::default().build(&items);

    assert!(transcript.len() <= MAX_TRANSCRIPT_BYTES);
    assert!(transcript.contains("latest response"));
}

#[test]
fn transcript_keeps_only_manual_approval_developer_messages() {
    let approval_text = format!("{MANUAL_APPROVAL_DEVELOPER_PREFIX}\n\nApproved action:\n{{}}");
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: "ordinary developer context".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: approval_text.clone(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let transcript = TranscriptConfig::default().build(&items);
    assert_eq!(transcript, format!("[1] developer: {approval_text}\n"));
}

#[test]
fn transcript_omits_media_payloads_and_keeps_readable_content() {
    let oversized_image = "A".repeat(MAX_TRANSCRIPT_BYTES + 1);
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText {
                    text: "Review this screenshot.".to_string(),
                },
                ContentItem::InputImage {
                    image_url: format!("data:image/png;base64,{oversized_image}"),
                    detail: None,
                },
                ContentItem::InputAudio {
                    audio_url: "data:audio/wav;base64,audio-payload".to_string(),
                },
            ],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "Screenshot captured.".to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,tool-image".to_string(),
                    detail: None,
                },
                FunctionCallOutputContentItem::InputAudio {
                    audio_url: "data:audio/wav;base64,tool-audio".to_string(),
                },
            ]),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::ImageGenerationCall {
            id: None,
            status: "completed".to_string(),
            revised_prompt: Some("A screenshot.".to_string()),
            result: oversized_image,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::ToolSearchCall {
            id: None,
            call_id: Some("search-1".to_string()),
            status: Some("completed".to_string()),
            execution: "client".to_string(),
            arguments: serde_json::json!({ "query": "Find screenshot tools" }),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::ToolSearchOutput {
            id: None,
            call_id: Some("search-1".to_string()),
            status: "completed".to_string(),
            execution: "client".to_string(),
            tools: vec![serde_json::json!({ "name": "capture_screenshot" })],
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let transcript = TranscriptConfig::default().build(&items);
    assert_eq!(
        transcript,
        concat!(
            "[1] user: Review this screenshot.\n",
            "[2] tool result: Screenshot captured.\n",
        )
    );
}

#[test]
fn transcript_omits_encrypted_messages_arguments_and_tool_outputs() {
    let items = vec![
        ResponseItem::AgentMessage {
            id: None,
            author: "worker".to_string(),
            recipient: "parent".to_string(),
            content: vec![AgentMessageInputContent::EncryptedContent {
                encrypted_content: "encrypted-agent-message".to_string(),
            }],
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::AgentMessage {
            id: None,
            author: "worker".to_string(),
            recipient: "parent".to_string(),
            content: vec![AgentMessageInputContent::InputText {
                text: "The workspace is ready.".to_string(),
            }],
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "exec_command".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            encrypted_function_args: Some(vec!["encrypted-function-arguments".to_string()]),
            call_id: "call-1".to_string(),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::CustomToolCallOutput {
            id: None,
            call_id: "call-1".to_string(),
            name: None,
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "Command completed.".to_string(),
                },
                FunctionCallOutputContentItem::EncryptedContent {
                    encrypted_content: "encrypted-tool-output".to_string(),
                },
            ]),
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let transcript = TranscriptConfig::default().build(&items);
    assert_eq!(
        transcript,
        concat!(
            "[1] assistant: Agent message from worker:\n",
            "The workspace is ready.\n",
            "[2] tool exec_command call: {}\n",
            "[3] tool exec_command result: Command completed.\n",
        )
    );
}
