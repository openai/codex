//! Bounds task results without cutting JSON or sacrificing answers to supporting history.

use serde_json::Value;
use serde_json::json;

// Leave room below the harness's default 10,000-byte fallback budget, including
// tool wrappers. Explicitly configured smaller harness budgets may still truncate.
pub(super) const MAX_RESPONSE_BYTES: usize = 8_000;

pub(super) fn serialize(mut value: Value) -> Result<String, String> {
    let text = value.to_string();
    if text.len() <= MAX_RESPONSE_BYTES {
        return Ok(text);
    }

    value["truncated"] = Value::Bool(true);
    if let Some(turns) = value.get_mut("turns").and_then(Value::as_array_mut) {
        for turn in turns {
            let answer = turn["items"]
                .as_array()
                .and_then(|items| items.iter().rfind(|item| item["type"] == "agentMessage"));
            *turn = json!({
                "id": turn["id"],
                "status": turn["status"],
                "error": turn["error"].as_object().map(|error| json!({"message": error["message"]})),
                "items": answer.cloned().into_iter().collect::<Vec<_>>(),
                "truncated": true
            });
        }
        value["thread"] = json!({
            "id": value["thread"]["id"],
            "status": value["thread"]["status"]
        });
    } else if let Some(polls) = value.get_mut("polls").and_then(Value::as_array_mut) {
        for poll in polls {
            *poll = json!({
                "schemaVersion": poll["schemaVersion"],
                "thread": poll["thread"],
                "cursor": poll["cursor"],
                "changed": poll["changed"],
                "latestTurn": poll["latestTurn"].as_object().map(|turn| json!({
                    "id": turn["id"], "status": turn["status"],
                    "error": turn.get("error").and_then(Value::as_object).map(|error| json!({"message": error["message"]}))
                })),
                "latestAssistantMessage": poll["latestAssistantMessage"],
                "truncated": true
            });
        }
    } else if let Some(threads) = value.get_mut("threads").and_then(Value::as_array_mut) {
        // Keep every returned entry so the server's pagination cursor remains valid.
        for thread in threads {
            *thread = json!({
                "id": thread["id"],
                "kind": thread["kind"],
                "title": thread["title"],
                "summary": thread["summary"],
                "status": thread["status"],
                "truncated": true
            });
        }
    }

    let text = value.to_string();
    if text.len() <= MAX_RESPONSE_BYTES {
        return Ok(text);
    }

    // Search from the original compact result each time: a failed fit must not
    // permanently erase text that fits once a smaller per-field limit is chosen.
    let mut low = 0;
    let mut high = MAX_RESPONSE_BYTES;
    let mut best = None;
    while low <= high {
        let limit = low + (high - low) / 2;
        let mut candidate = value.clone();
        truncate_content(&mut candidate, limit);
        let text = candidate.to_string();
        if text.len() <= MAX_RESPONSE_BYTES {
            best = Some(text);
            low = limit + 1;
        } else if limit == 0 {
            break;
        } else {
            high = limit - 1;
        }
    }
    best.ok_or_else(|| "Task response metadata exceeds the output budget".to_string())
}

fn truncate_content(value: &mut Value, limit: usize) {
    match value {
        Value::Object(object) => {
            let mut truncated = false;
            for (key, value) in object.iter_mut() {
                // IDs, cursors, phases, status, and other protocol fields are never shortened.
                if matches!(key.as_str(), "text" | "title" | "summary" | "message")
                    && let Some(text) = value.as_str()
                    && text.chars().count() > limit
                {
                    *value = Value::String(text.chars().take(limit).collect());
                    truncated = true;
                } else {
                    truncate_content(value, limit);
                }
            }
            if truncated {
                object.insert("truncated".to_string(), Value::Bool(true));
            }
        }
        Value::Array(values) => {
            for value in values {
                truncate_content(value, limit);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

#[cfg(test)]
#[path = "dynamic_tools_response_tests.rs"]
mod tests;
