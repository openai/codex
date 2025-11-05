use serde_json::Value as JsonValue;

use crate::pull::PullEvent;

// Convert a single JSON object representing a pull update into one or more events.
pub(crate) fn pull_events_from_value(value: &JsonValue) -> Vec<PullEvent> {
    let mut events = Vec::new();
    if let Some(status) = value.get("status").and_then(|s| s.as_str()) {
        events.push(PullEvent::Status(status.to_string()));
        if status == "success" {
            events.push(PullEvent::Success);
        }
    }
    let digest = value
        .get("digest")
        .and_then(|d| d.as_str())
        .unwrap_or("")
        .to_string();
    let total = value.get("total").and_then(JsonValue::as_u64);
    let completed = value.get("completed").and_then(JsonValue::as_u64);
    if total.is_some() || completed.is_some() {
        events.push(PullEvent::ChunkProgress {
            digest,
            total,
            completed,
        });
    }
    events
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;

    use super::*;

    #[test]
    fn test_pull_events_decoder_status_and_success() {
        let v: JsonValue = serde_json::json!({"status":"verifying"});
        let events = pull_events_from_value(&v);
        assert_matches!(events.as_slice(), [PullEvent::Status(s)] if s == "verifying");

        let v2: JsonValue = serde_json::json!({"status":"success"});
        let events2 = pull_events_from_value(&v2);
        assert_eq!(events2.len(), 2);
        assert_matches!(events2[0], PullEvent::Status(ref s) if s == "success");
        assert_matches!(events2[1], PullEvent::Success);
    }

    #[test]
    fn test_pull_events_decoder_progress() {
        let v: JsonValue = serde_json::json!({"digest":"sha256:abc","total":100});
        let events = pull_events_from_value(&v);
        assert_eq!(events.len(), 1);
        assert_matches!(
            &events[0],
            PullEvent::ChunkProgress {
                digest,
                total,
                completed,
            } if digest == "sha256:abc" && total == &Some(100) && completed.is_none()
        );
        let v2: JsonValue = serde_json::json!({"digest":"sha256:def","completed":42});
        let events2 = pull_events_from_value(&v2);
        assert_eq!(events2.len(), 1);
        assert_matches!(
            &events2[0],
            PullEvent::ChunkProgress {
                digest,
                total,
                completed,
            } if digest == "sha256:def" && total.is_none() && completed == &Some(42)
        );
    }
}
