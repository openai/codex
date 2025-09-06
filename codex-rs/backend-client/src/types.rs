pub use codex_backend_openapi_models::models::CodeTaskDetailsResponse;
pub use codex_backend_openapi_models::models::PaginatedListTaskListItem;
pub use codex_backend_openapi_models::models::TaskListItem;

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
    /// Best-effort: extract a single file old/new path for header synthesis when only hunk bodies are provided.
    fn single_file_paths(&self) -> Option<(String, String)>;
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
        if parts.is_empty() { None } else { Some(parts.join("\n\n")) }
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
    fn single_file_paths(&self) -> Option<(String, String)> {
        fn try_from_items(items: &Vec<serde_json::Value>) -> Option<(String, String)> {
            use serde_json::Value;
            for item in items {
                if let Some(obj) = item.as_object() {
                    if let Some(p) = obj.get("path").and_then(Value::as_str) {
                        let p = p.to_string();
                        return Some((p.clone(), p));
                    }
                    let old = obj.get("old_path").and_then(Value::as_str);
                    let newp = obj.get("new_path").and_then(Value::as_str);
                    if let (Some(o), Some(n)) = (old, newp) {
                        return Some((o.to_string(), n.to_string()));
                    }
                    if let Some(od) = obj.get("output_diff").and_then(Value::as_object) {
                        if let Some(fm) = od.get("files_modified") {
                            if let Some(map) = fm.as_object()
                                && map.len() == 1
                                && let Some((k, _)) = map.iter().next()
                            {
                                let p = k.to_string();
                                return Some((p.clone(), p));
                            } else if let Some(arr) = fm.as_array()
                                && arr.len() == 1
                            {
                                let el = &arr[0];
                                if let Some(p) = el.as_str() {
                                    let p = p.to_string();
                                    return Some((p.clone(), p));
                                }
                                if let Some(o) = el.as_object() {
                                    let path = o.get("path").and_then(Value::as_str);
                                    let oldp = o.get("old_path").and_then(Value::as_str);
                                    let newp = o.get("new_path").and_then(Value::as_str);
                                    if let Some(p) = path {
                                        let p = p.to_string();
                                        return Some((p.clone(), p));
                                    }
                                    if let (Some(o1), Some(n1)) = (oldp, newp) {
                                        return Some((o1.to_string(), n1.to_string()));
                                    }
                                }
                            }
                        }
                        if let Some(p) = od.get("path").and_then(Value::as_str) {
                            let p = p.to_string();
                            return Some((p.clone(), p));
                        }
                    }
                }
            }
            None
        }
        let candidates: [&Option<std::collections::HashMap<String, serde_json::Value>>; 2] =
            [&self.current_diff_task_turn, &self.current_assistant_turn];
        for map in candidates {
            if let Some(m) = map.as_ref()
                && let Some(items) = m.get("output_items").and_then(serde_json::Value::as_array)
                && let Some(p) = try_from_items(items)
            {
                return Some(p);
            }
        }
        None
    }
}

/// Best-effort extraction of a list of (old_path, new_path) pairs for files involved
/// in the current task's diff output. For entries where only a single `path` is present,
/// the pair will be (path, path).
pub fn extract_file_paths_list(details: &CodeTaskDetailsResponse) -> Vec<(String, String)> {
    use serde_json::Value;
    fn push_from_items(out: &mut Vec<(String, String)>, items: &Vec<Value>) {
        for item in items {
            if let Some(obj) = item.as_object() {
                if let Some(p) = obj.get("path").and_then(Value::as_str) {
                    let p = p.to_string();
                    out.push((p.clone(), p));
                    continue;
                }
                let old = obj.get("old_path").and_then(Value::as_str);
                let newp = obj.get("new_path").and_then(Value::as_str);
                if let (Some(o), Some(n)) = (old, newp) {
                    out.push((o.to_string(), n.to_string()));
                    continue;
                }
                if let Some(od) = obj.get("output_diff").and_then(Value::as_object) {
                    if let Some(fm) = od.get("files_modified") {
                        if let Some(map) = fm.as_object() {
                            for (k, _v) in map {
                                let p = k.to_string();
                                out.push((p.clone(), p));
                            }
                        } else if let Some(arr) = fm.as_array() {
                            for el in arr {
                                if let Some(p) = el.as_str() {
                                    let p = p.to_string();
                                    out.push((p.clone(), p));
                                } else if let Some(o) = el.as_object() {
                                    let path = o.get("path").and_then(Value::as_str);
                                    let oldp = o.get("old_path").and_then(Value::as_str);
                                    let newp = o.get("new_path").and_then(Value::as_str);
                                    if let Some(p) = path {
                                        let p = p.to_string();
                                        out.push((p.clone(), p));
                                    } else if let (Some(o1), Some(n1)) = (oldp, newp) {
                                        out.push((o1.to_string(), n1.to_string()));
                                    }
                                }
                            }
                        }
                    }
                    if let Some(p) = od.get("path").and_then(Value::as_str) {
                        let p = p.to_string();
                        out.push((p.clone(), p));
                    }
                }
            }
        }
    }
    let mut out: Vec<(String, String)> = Vec::new();
    let candidates: [&Option<std::collections::HashMap<String, Value>>; 2] = [
        &details.current_diff_task_turn,
        &details.current_assistant_turn,
    ];
    for map in candidates {
        if let Some(m) = map.as_ref()
            && let Some(items) = m.get("output_items").and_then(Value::as_array)
        {
            push_from_items(&mut out, items);
        }
    }
    out
}
