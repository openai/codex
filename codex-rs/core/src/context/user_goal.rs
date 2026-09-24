//! User-authored goal changes, separate from agent-created goals and runtime steering.
//! Host annotations establish provenance and objective completeness; text markers control visibility.
//! Oversized objectives are omitted whole so truncation cannot turn a restriction into a grant.

use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ThreadGoalStatus;

const MAX_OBJECTIVE_BYTES: usize = 700;

/// An explicit goal mutation accepted from the user-facing goal API.
/// Never construct this from tool output or an automatic continuation.
pub enum UserGoalUpdate {
    Set {
        objective: Option<String>,
        status: Option<ThreadGoalStatus>,
    },
    Clear,
}

impl UserGoalUpdate {
    pub(crate) const OMITTED_OBJECTIVE_KIND: &str = "user.goal.omitted";

    /// Reads only a host-annotated goal instruction, never a matching text wrapper alone.
    pub(crate) fn message_text(item: &ResponseItem) -> Option<&str> {
        let ResponseItem::Message {
            role,
            content,
            internal_chat_message_metadata_passthrough: Some(metadata),
            ..
        } = item
        else {
            return None;
        };
        let [kind] = metadata.content_item_kinds.as_deref()? else {
            return None;
        };
        let [ContentItem::InputText { text }] = content.as_slice() else {
            return None;
        };
        (role == "user" && matches!(kind.0.as_str(), "user.goal" | Self::OMITTED_OBJECTIVE_KIND))
            .then_some(text)
    }
}

impl ContextualUserFragment for UserGoalUpdate {
    fn role(&self) -> &'static str {
        "user"
    }

    fn content_kind(&self) -> ContentItemKind {
        let kind = match self {
            Self::Set {
                objective: Some(objective),
                ..
            } if serde_json::json!(objective).to_string().len() > MAX_OBJECTIVE_BYTES => {
                Self::OMITTED_OBJECTIVE_KIND
            }
            Self::Set { .. } | Self::Clear => "user.goal",
        };
        ContentItemKind(kind.to_owned())
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            "<codex_internal_context source=\"user_goal\">",
            "</codex_internal_context>",
        )
    }

    fn body(&self) -> String {
        match self {
            Self::Set { objective, status } => {
                let mut text = "\n".to_owned();
                if let Some(objective) = objective {
                    // Preserve the objective as data, including quotes and wrapper-like text.
                    let objective = serde_json::json!(objective).to_string();
                    // Even one token per byte leaves room for the wrapper and status below 1K.
                    if objective.len() <= MAX_OBJECTIVE_BYTES {
                        text.push_str(&format!("User set the goal: {objective}"));
                    } else {
                        text.push_str(
                            "User set the goal: [objective omitted; exceeds the evidence limit].",
                        );
                    }
                    text.push('\n');
                }
                if let Some(status) = status {
                    text.push_str(&format!(
                        "User set goal status: {}.\n",
                        serde_json::json!(status)
                    ));
                }
                text
            }
            Self::Clear => "\nUser cleared the goal.\n".to_owned(),
        }
    }
}
