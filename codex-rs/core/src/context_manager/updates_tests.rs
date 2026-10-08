//! Verifies message grouping around explicit world-state response items.

use super::*;
use crate::context::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use pretty_assertions::assert_eq;
use serde_json::json;

struct TestFragment {
    role: &'static str,
    text: &'static str,
    markers: (&'static str, &'static str),
}

impl ContextualUserFragment for TestFragment {
    fn role(&self) -> &'static str {
        self.role
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("test.context".to_string())
    }

    fn markers(&self) -> (&'static str, &'static str) {
        self.markers
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        self.text.to_string()
    }
}

fn fragment(role: &'static str, text: &'static str) -> TestFragment {
    TestFragment {
        role,
        text,
        markers: ("", ""),
    }
}

fn message(role: &str, texts: &[&str]) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: role.to_string(),
        content: texts
            .iter()
            .map(|text| ContentItem::InputText {
                text: text.to_string(),
            })
            .collect(),
        phase: None,
        internal_chat_message_metadata_passthrough: Some(InternalChatMessageMetadataPassthrough {
            content_item_kinds: Some(
                texts
                    .iter()
                    .map(|_| ContentItemKind("test.context".to_string()))
                    .collect(),
            ),
            ..Default::default()
        }),
    }
}

fn additional_tools() -> ResponseItem {
    ResponseItem::AdditionalTools {
        id: None,
        role: "developer".to_string(),
        tools: vec![json!({"type": "function", "name": "lookup", "parameters": {}})],
    }
}

#[test]
fn updates_preserve_item_boundaries_roles_and_standalone_fragments() {
    let tools = additional_tools();
    let updates = vec![
        WorldStateUpdate::fragment(fragment("developer", "before one")),
        WorldStateUpdate::fragment(fragment("developer", "before two")),
        WorldStateUpdate {
            placement: Placement::Prefix,
            ..WorldStateUpdate::fragment(fragment("developer", "prefix"))
        },
        WorldStateUpdate::fragment(fragment("developer", "after prefix")),
        WorldStateUpdate {
            placement: Placement::Prefix,
            content: WorldStateUpdateContent::Item(Box::new(tools.clone())),
        },
        WorldStateUpdate::fragment(fragment("developer", "after")),
        WorldStateUpdate::fragment(fragment("developer", "standalone")).standalone(),
        WorldStateUpdate::fragment(fragment("developer", "last developer")),
        WorldStateUpdate::fragment(fragment("user", "user")),
        WorldStateUpdate {
            placement: Placement::Standalone,
            content: WorldStateUpdateContent::Item(Box::new(tools.clone())),
        },
    ];

    assert_eq!(
        merge_world_state_updates(updates),
        vec![
            message("developer", &["before one", "before two"]),
            message("developer", &["prefix"]),
            message("developer", &["after prefix"]),
            tools.clone(),
            message("developer", &["after"]),
            message("developer", &["standalone"]),
            message("developer", &["last developer"]),
            message("user", &["user"]),
            tools,
        ]
    );
}
