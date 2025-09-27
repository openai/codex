pub use codex_backend_openapi_models::models::CodeTaskDetailsResponse;
pub use codex_backend_openapi_models::models::PaginatedListTaskListItem;
pub use codex_backend_openapi_models::models::TaskListItem;

use serde::Deserialize;
use serde_json::Value;

/// Extension helpers on generated types.
pub trait CodeTaskDetailsResponseExt {
    /// Attempt to extract a unified diff string from `current_diff_task_turn`.
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
        // `current_diff_task_turn` is an object; look for `output_items`.
        // Prefer explicit diff turn; fallback to assistant turn if needed.
        let candidates: [&Option<std::collections::HashMap<String, Value>>; 2] =
            [&self.current_diff_task_turn, &self.current_assistant_turn];

        for map in candidates {
            let items = map
                .as_ref()
                .and_then(|m| m.get("output_items"))
                .and_then(|v| v.as_array());
            if let Some(items) = items {
                for item in items {
                    match item.get("type").and_then(Value::as_str) {
                        Some("output_diff") => {
                            if let Some(s) = item.get("diff").and_then(Value::as_str) {
                                return Some(s.to_string());
                            }
                        }
                        Some("pr") => {
                            if let Some(s) = item
                                .get("output_diff")
                                .and_then(|od| od.get("diff"))
                                .and_then(Value::as_str)
                            {
                                return Some(s.to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        None
    }
    fn assistant_text_messages(&self) -> Vec<String> {
        let mut out = Vec::new();
        let candidates: [&Option<std::collections::HashMap<String, Value>>; 2] =
            [&self.current_diff_task_turn, &self.current_assistant_turn];
        for map in candidates {
            let items = map
                .as_ref()
                .and_then(|m| m.get("output_items"))
                .and_then(|v| v.as_array());
            if let Some(items) = items {
                for item in items {
                    if item.get("type").and_then(Value::as_str) == Some("message")
                        && let Some(content) = item.get("content").and_then(Value::as_array)
                    {
                        for part in content {
                            if part.get("content_type").and_then(Value::as_str) == Some("text")
                                && let Some(txt) = part.get("text").and_then(Value::as_str)
                            {
                                out.push(txt.to_string());
                            }
                        }
                    }
                }
            }
        }
        out
    }

    fn user_text_prompt(&self) -> Option<String> {
        use serde_json::Value;
        let map = self.current_user_turn.as_ref()?;
        let items = map.get("input_items").and_then(Value::as_array)?;
        let mut parts: Vec<String> = Vec::new();
        for item in items {
            if item.get("type").and_then(Value::as_str) == Some("message") {
                // optional role filter (prefer user)
                let is_user = item
                    .get("role")
                    .and_then(Value::as_str)
                    .map(|r| r.eq_ignore_ascii_case("user"))
                    .unwrap_or(true);
                if !is_user {
                    continue;
                }
                if let Some(content) = item.get("content").and_then(Value::as_array) {
                    for c in content {
                        if c.get("content_type").and_then(Value::as_str) == Some("text")
                            && let Some(txt) = c.get("text").and_then(Value::as_str)
                        {
                            parts.push(txt.to_string());
                        }
                    }
                }
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n\n"))
        }
    }

    fn assistant_error_message(&self) -> Option<String> {
        let map = self.current_assistant_turn.as_ref()?;
        let err = map.get("error")?.as_object()?;
        let message = err.get("message").and_then(Value::as_str).unwrap_or("");
        let code = err.get("code").and_then(Value::as_str).unwrap_or("");
        if message.is_empty() && code.is_empty() {
            None
        } else if message.is_empty() {
            Some(code.to_string())
        } else if code.is_empty() {
            Some(message.to_string())
        } else {
            Some(format!("{code}: {message}"))
        }
    }
}

// Removed unused helpers `single_file_paths` and `extract_file_paths_list` to reduce
// surface area; reintroduce as needed near call sites.

#[derive(Clone, Debug, Deserialize)]
pub struct TurnAttemptsSiblingTurnsResponse {
    #[serde(default)]
    pub sibling_turns: Vec<std::collections::HashMap<String, Value>>,
}
