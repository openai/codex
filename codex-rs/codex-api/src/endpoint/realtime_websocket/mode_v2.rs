use crate::endpoint::realtime_websocket::types::ConversationItem;
use crate::endpoint::realtime_websocket::types::ConversationItemContent;
use crate::endpoint::realtime_websocket::types::RealtimeOutboundMessage;
use crate::endpoint::realtime_websocket::types::SessionAudioFormat;
use crate::endpoint::realtime_websocket::types::SessionAudioInputV2;
use crate::endpoint::realtime_websocket::types::SessionAudioOutputFormat;
use crate::endpoint::realtime_websocket::types::SessionAudioOutputV2;
use crate::endpoint::realtime_websocket::types::SessionAudioV2;
use crate::endpoint::realtime_websocket::types::SessionTool;
use crate::endpoint::realtime_websocket::types::SessionToolParameters;
use crate::endpoint::realtime_websocket::types::SessionToolProperties;
use crate::endpoint::realtime_websocket::types::SessionToolProperty;
use crate::endpoint::realtime_websocket::types::SessionTurnDetection;
use crate::endpoint::realtime_websocket::types::SessionUpdateSession;
use crate::endpoint::realtime_websocket::types::SessionUpdateSessionV2;
use serde_json::json;
use std::collections::HashMap;
use url::Url;

pub(super) fn conversation_item_create(text: String) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::ConversationItemCreate {
        item: ConversationItem::Message {
            role: "user".to_string(),
            content: vec![ConversationItemContent {
                kind: "input_text".to_string(),
                text,
            }],
        },
    }
}

pub(super) fn handoff_append(output_text: String) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::ConversationItemCreate {
        item: ConversationItem::Message {
            role: "assistant".to_string(),
            content: vec![ConversationItemContent {
                kind: "output_text".to_string(),
                text: output_text,
            }],
        },
    }
}

pub(super) fn function_call_output(
    call_id: String,
    output_text: String,
) -> RealtimeOutboundMessage {
    let output = json!({
        "content": output_text,
    })
    .to_string();
    RealtimeOutboundMessage::ConversationItemCreate {
        item: ConversationItem::FunctionCallOutput { call_id, output },
    }
}

pub(super) fn session_update(instructions: String) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::SessionUpdate {
        session: SessionUpdateSession::V2(SessionUpdateSessionV2 {
            kind: "realtime".to_string(),
            instructions,
            output_modalities: vec!["audio".to_string()],
            audio: SessionAudioV2 {
                input: SessionAudioInputV2 {
                    format: SessionAudioFormat {
                        kind: "audio/pcm".to_string(),
                        rate: 24_000,
                    },
                    turn_detection: SessionTurnDetection {
                        kind: "semantic_vad".to_string(),
                        interrupt_response: false,
                        create_response: true,
                    },
                },
                output: SessionAudioOutputV2 {
                    format: SessionAudioOutputFormat {
                        kind: "audio/pcm".to_string(),
                        rate: 24_000,
                    },
                    voice: "marin".to_string(),
                },
            },
            tools: vec![SessionTool {
                kind: "function".to_string(),
                name: "codex".to_string(),
                description: "Delegate a request to Codex and return the final result to the user."
                    .to_string(),
                parameters: SessionToolParameters {
                    kind: "object".to_string(),
                    properties: SessionToolProperties {
                        prompt: SessionToolProperty {
                            kind: "string".to_string(),
                            description: "The user request to delegate to Codex.".to_string(),
                        },
                    },
                    required: vec!["prompt".to_string()],
                },
            }],
            tool_choice: "auto".to_string(),
        }),
    }
}

pub(super) fn append_query_params(
    url: &mut Url,
    query_params: Option<&HashMap<String, String>>,
    model: Option<&str>,
) {
    if model.is_none() && query_params.is_none_or(HashMap::is_empty) {
        return;
    }

    let mut query = url.query_pairs_mut();
    if let Some(model) = model {
        query.append_pair("model", model);
    }
    if let Some(query_params) = query_params {
        for (key, value) in query_params {
            if key == "model" && model.is_some() {
                continue;
            }
            query.append_pair(key, value);
        }
    }
}
