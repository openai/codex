use crate::endpoint::realtime_websocket::types::ConversationItem;
use crate::endpoint::realtime_websocket::types::ConversationItemContent;
use crate::endpoint::realtime_websocket::types::RealtimeOutboundMessage;
use crate::endpoint::realtime_websocket::types::SessionAudioFormat;
use crate::endpoint::realtime_websocket::types::SessionAudioInputV1;
use crate::endpoint::realtime_websocket::types::SessionAudioOutputV1;
use crate::endpoint::realtime_websocket::types::SessionAudioV1;
use crate::endpoint::realtime_websocket::types::SessionUpdateSession;
use crate::endpoint::realtime_websocket::types::SessionUpdateSessionV1;
use std::collections::HashMap;
use url::Url;

pub(super) fn conversation_item_create(text: String) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::ConversationItemCreate {
        item: ConversationItem::Message {
            role: "user".to_string(),
            content: vec![ConversationItemContent {
                kind: "text".to_string(),
                text,
            }],
        },
    }
}

pub(super) fn handoff_append(handoff_id: String, output_text: String) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::ConversationHandoffAppend {
        handoff_id,
        output_text,
    }
}

pub(super) fn session_update(instructions: String) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::SessionUpdate {
        session: SessionUpdateSession::V1(SessionUpdateSessionV1 {
            kind: "quicksilver".to_string(),
            instructions,
            audio: SessionAudioV1 {
                input: SessionAudioInputV1 {
                    format: SessionAudioFormat {
                        kind: "audio/pcm".to_string(),
                        rate: 24_000,
                    },
                },
                output: SessionAudioOutputV1 {
                    voice: "mundo".to_string(),
                },
            },
        }),
    }
}

pub(super) fn append_query_params(
    url: &mut Url,
    query_params: Option<&HashMap<String, String>>,
    model: Option<&str>,
) {
    let mut query = url.query_pairs_mut();
    query.append_pair("intent", "quicksilver");
    if let Some(model) = model {
        query.append_pair("model", model);
    }
    if let Some(query_params) = query_params {
        for (key, value) in query_params {
            if (key == "model" && model.is_some()) || key == "intent" {
                continue;
            }
            query.append_pair(key, value);
        }
    }
}
