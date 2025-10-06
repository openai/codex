pub use codex_backend_openapi_models::models::PaginatedListTaskListItem;
pub use codex_backend_openapi_models::models::TaskListItem;

use serde::Deserialize;
use serde::de::Deserializer;
use serde_json::Value;
use std::collections::HashMap;

/// Hand-rolled models for the Cloud Tasks task-details response.
/// The generated OpenAPI models are pretty bad. This is a half-step
/// towards hand-rolling them.
#[derive(Clone, Debug, Deserialize)]
pub struct CodeTaskDetailsResponse {
    #[serde(default)]
    pub current_user_turn: Option<Turn>,
    #[serde(default)]
    pub current_assistant_turn: Option<Turn>,
    #[serde(default)]
    pub current_diff_task_turn: Option<Turn>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Turn {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub attempt_placement: Option<i64>,
    #[serde(default, rename = "turn_status")]
    pub turn_status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_vec")]
    pub sibling_turn_ids: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_vec")]
    pub input_items: Vec<TurnItem>,
    #[serde(default, deserialize_with = "deserialize_vec")]
    pub output_items: Vec<TurnItem>,
    #[serde(default)]
    pub worklog: Option<Worklog>,
    #[serde(default)]
    pub error: Option<TurnError>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TurnItem {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default, deserialize_with = "deserialize_vec")]
    pub content: Vec<ContentFragment>,
    #[serde(default)]
    pub diff: Option<String>,
    #[serde(default)]
    pub output_diff: Option<DiffPayload>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ContentFragment {
    Structured(StructuredContent),
    Text(String),
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct StructuredContent {
    #[serde(rename = "content_type", default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DiffPayload {
    #[serde(default)]
    pub diff: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Worklog {
    #[serde(default, deserialize_with = "deserialize_vec")]
    pub messages: Vec<WorklogMessage>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct WorklogMessage {
    #[serde(default)]
    pub author: Option<Author>,
    #[serde(default)]
    pub content: Option<WorklogContent>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Author {
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct WorklogContent {
    #[serde(default)]
    pub parts: Vec<ContentFragment>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TurnError {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl ContentFragment {
    fn text(&self) -> Option<&str> {
        match self {
            ContentFragment::Structured(inner) => {
                if inner
                    .content_type
                    .as_deref()
                    .map(|ct| ct.eq_ignore_ascii_case("text"))
                    .unwrap_or(false)
                {
                    inner.text.as_deref().filter(|s| !s.is_empty())
                } else {
                    None
                }
            }
            ContentFragment::Text(raw) => {
                if raw.trim().is_empty() {
                    None
                } else {
                    Some(raw.as_str())
                }
            }
        }
    }
}

impl TurnItem {
    fn text_values(&self) -> Vec<String> {
        self.content
            .iter()
            .filter_map(|fragment| fragment.text().map(str::to_string))
            .collect()
    }

    fn diff_text(&self) -> Option<String> {
        if self.kind == "output_diff" {
            if let Some(diff) = &self.diff
                && !diff.is_empty()
            {
                return Some(diff.clone());
            }
        } else if self.kind == "pr"
            && let Some(payload) = &self.output_diff
            && let Some(diff) = &payload.diff
            && !diff.is_empty()
        {
            return Some(diff.clone());
        }
        None
    }
}

impl Turn {
    fn unified_diff(&self) -> Option<String> {
        self.output_items.iter().find_map(TurnItem::diff_text)
    }

    fn message_texts(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .output_items
            .iter()
            .filter(|item| item.kind == "message")
            .flat_map(TurnItem::text_values)
            .collect();

        if let Some(log) = &self.worklog {
            for message in &log.messages {
                if message.is_assistant() {
                    out.extend(message.text_values());
                }
            }
        }

        out
    }

    fn user_prompt(&self) -> Option<String> {
        let parts: Vec<String> = self
            .input_items
            .iter()
            .filter(|item| item.kind == "message")
            .filter(|item| {
                item.role
                    .as_deref()
                    .map(|r| r.eq_ignore_ascii_case("user"))
                    .unwrap_or(true)
            })
            .flat_map(TurnItem::text_values)
            .collect();

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(
                "

",
            ))
        }
    }

    fn error_summary(&self) -> Option<String> {
        self.error.as_ref().and_then(TurnError::summary)
    }
}

impl WorklogMessage {
    fn is_assistant(&self) -> bool {
        self.author
            .as_ref()
            .and_then(|a| a.role.as_deref())
            .map(|role| role.eq_ignore_ascii_case("assistant"))
            .unwrap_or(false)
    }

    fn text_values(&self) -> Vec<String> {
        self.content
            .as_ref()
            .map(|content| {
                content
                    .parts
                    .iter()
                    .filter_map(|fragment| fragment.text().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl TurnError {
    fn summary(&self) -> Option<String> {
        let code = self.code.as_deref().unwrap_or("");
        let message = self.message.as_deref().unwrap_or("");
        match (code.is_empty(), message.is_empty()) {
            (true, true) => None,
            (false, true) => Some(code.to_string()),
            (true, false) => Some(message.to_string()),
            (false, false) => Some(format!("{code}: {message}")),
        }
    }
}

pub trait CodeTaskDetailsResponseExt {
    /// Attempt to extract a unified diff string from the assistant or diff turn.
    fn unified_diff(&self) -> Option<String>;
    /// Extract assistant text output messages (no diff) from current turns.
    fn assistant_text_messages(&self) -> Vec<String>;
    /// Extract the user's prompt text from the current user turn, when present.
    fn user_text_prompt(&self) -> Option<String>;
    /// Extract an assistant error message (if the turn failed and provided one).
    fn assistant_error_message(&self) -> Option<String>;
}

impl CodeTaskDetailsResponseExt for CodeTaskDetailsResponse {
    fn unified_diff(&self) -> Option<String> {
        [
            self.current_diff_task_turn.as_ref(),
            self.current_assistant_turn.as_ref(),
        ]
        .into_iter()
        .flatten()
        .find_map(Turn::unified_diff)
    }

    fn assistant_text_messages(&self) -> Vec<String> {
        let mut out = Vec::new();
        for turn in [
            self.current_diff_task_turn.as_ref(),
            self.current_assistant_turn.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            out.extend(turn.message_texts());
        }
        out
    }

    fn user_text_prompt(&self) -> Option<String> {
        self.current_user_turn.as_ref().and_then(Turn::user_prompt)
    }

    fn assistant_error_message(&self) -> Option<String> {
        self.current_assistant_turn
            .as_ref()
            .and_then(Turn::error_summary)
    }
}

fn deserialize_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<Vec<T>>::deserialize(deserializer).map(|opt| opt.unwrap_or_default())
}

#[derive(Clone, Debug, Deserialize)]
pub struct TurnAttemptsSiblingTurnsResponse {
    #[serde(default)]
    pub sibling_turns: Vec<HashMap<String, Value>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn fixture(name: &str) -> CodeTaskDetailsResponse {
        let json = match name {
            "diff" => include_str!("../tests/fixtures/task_details_with_diff.json"),
            "error" => include_str!("../tests/fixtures/task_details_with_error.json"),
            other => panic!("unknown fixture {other}"),
        };
        serde_json::from_str(json).expect("fixture should deserialize")
    }

    #[test]
    fn unified_diff_prefers_current_diff_task_turn() {
        let details = fixture("diff");
        let diff = details.unified_diff().expect("diff present");
        assert!(diff.contains("diff --git"));
    }

    #[test]
    fn unified_diff_falls_back_to_pr_output_diff() {
        let details = fixture("error");
        let diff = details.unified_diff().expect("diff from pr output");
        assert!(diff.contains("lib.rs"));
    }

    #[test]
    fn assistant_text_messages_extracts_text_content() {
        let details = fixture("diff");
        let messages = details.assistant_text_messages();
        assert_eq!(messages, vec!["Assistant response".to_string()]);
    }

    #[test]
    fn user_text_prompt_joins_parts_with_spacing() {
        let details = fixture("diff");
        let prompt = details.user_text_prompt().expect("prompt present");
        assert_eq!(
            prompt,
            "First line

Second line"
        );
    }

    #[test]
    fn assistant_error_message_combines_code_and_message() {
        let details = fixture("error");
        let msg = details
            .assistant_error_message()
            .expect("error should be present");
        assert_eq!(msg, "APPLY_FAILED: Patch could not be applied");
    }
}
