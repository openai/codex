use std::path::PathBuf;
use std::sync::Arc;

use chrono::DateTime;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_protocol::ThreadId;
use futures::future::BoxFuture;
use serde::Serialize;
use serde::Serializer;

pub type HookFn = Arc<dyn for<'a> Fn(&'a HookPayload) -> BoxFuture<'a, HookOutcome> + Send + Sync>;

#[derive(Clone)]
pub struct Hook {
    pub func: HookFn,
}

impl Default for Hook {
    fn default() -> Self {
        Self {
            func: Arc::new(|_| Box::pin(async { HookOutcome::Continue })),
        }
    }
}

impl Hook {
    pub async fn execute(&self, payload: &HookPayload) -> HookOutcome {
        (self.func)(payload).await
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct HookPayload {
    pub session_id: ThreadId,
    pub cwd: PathBuf,
    #[serde(serialize_with = "serialize_triggered_at")]
    pub triggered_at: DateTime<Utc>,
    pub hook_event: HookEvent,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct HookEventAfterAgent {
    pub thread_id: ThreadId,
    pub turn_id: String,
    pub input_messages: Vec<String>,
    pub last_assistant_message: Option<String>,
}

fn serialize_triggered_at<S>(value: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_rfc3339_opts(SecondsFormat::Secs, true))
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum HookEvent {
    AfterAgent {
        #[serde(flatten)]
        event: HookEventAfterAgent,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookOutcome {
    Continue,
    #[allow(dead_code)]
    Stop,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::HookEvent;
    use super::HookEventAfterAgent;
    use super::HookPayload;

    #[test]
    fn hook_payload_serializes_stable_wire_shape() {
        let session_id = ThreadId::new();
        let thread_id = ThreadId::new();
        let payload = HookPayload {
            session_id,
            cwd: PathBuf::from("tmp"),
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::AfterAgent {
                event: HookEventAfterAgent {
                    thread_id,
                    turn_id: "turn-1".to_string(),
                    input_messages: vec!["hello".to_string()],
                    last_assistant_message: Some("hi".to_string()),
                },
            },
        };

        let actual = serde_json::to_value(payload).expect("serialize hook payload");
        let expected = json!({
            "session_id": session_id.to_string(),
            "cwd": "tmp",
            "triggered_at": "2025-01-01T00:00:00Z",
            "hook_event": {
                "event_type": "after_agent",
                "thread_id": thread_id.to_string(),
                "turn_id": "turn-1",
                "input_messages": ["hello"],
                "last_assistant_message": "hi",
            },
        });

        assert_eq!(actual, expected);
    }
}
