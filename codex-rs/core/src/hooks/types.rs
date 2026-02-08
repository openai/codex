use std::path::PathBuf;
use std::sync::Arc;

use chrono::DateTime;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_protocol::ThreadId;
use futures::future::BoxFuture;
use serde::Serialize;
use serde::Serializer;

pub(crate) type HookFn =
    Arc<dyn for<'a> Fn(&'a HookPayload) -> BoxFuture<'a, HookOutcome> + Send + Sync>;

#[derive(Clone)]
pub(crate) struct Hook {
    pub(crate) func: HookFn,
}

impl Default for Hook {
    fn default() -> Self {
        Self {
            func: Arc::new(|_| Box::pin(async { HookOutcome::Proceed })),
        }
    }
}

impl Hook {
    pub(super) async fn execute(&self, payload: &HookPayload) -> HookOutcome {
        (self.func)(payload).await
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub(crate) struct HookPayload {
    pub(crate) session_id: ThreadId,
    pub(crate) cwd: PathBuf,
    #[serde(serialize_with = "serialize_triggered_at")]
    pub(crate) triggered_at: DateTime<Utc>,
    pub(crate) hook_event: HookEvent,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct HookEventAfterAgent {
    pub thread_id: ThreadId,
    pub turn_id: String,
    pub input_messages: Vec<String>,
    pub last_assistant_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct HookEventPreToolUse {
    pub tool_name: String,
    pub tool_input: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct HookEventPostToolUse {
    pub tool_name: String,
    pub tool_output: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct HookEventStop {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct HookEventUserPromptSubmit {
    pub user_message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct HookEventNotification {
    pub message: String,
    pub level: String,
}

fn serialize_triggered_at<S>(value: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_rfc3339_opts(SecondsFormat::Secs, true))
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub(crate) enum HookEvent {
    AfterAgent {
        #[serde(flatten)]
        event: HookEventAfterAgent,
    },
    PreToolUse {
        #[serde(flatten)]
        event: HookEventPreToolUse,
    },
    PostToolUse {
        #[serde(flatten)]
        event: HookEventPostToolUse,
    },
    #[allow(dead_code)] // Integration point in codex.rs agent loop requires separate PR.
    Stop {
        #[serde(flatten)]
        event: HookEventStop,
    },
    #[allow(dead_code)] // Integration point requires architectural changes.
    UserPromptSubmit {
        #[serde(flatten)]
        event: HookEventUserPromptSubmit,
    },
    #[allow(dead_code)] // Integration point requires architectural changes.
    Notification {
        #[serde(flatten)]
        event: HookEventNotification,
    },
}

/// Outcome of a hook execution that determines how the agent should proceed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HookOutcome {
    /// Hook completed; proceed with the operation normally.
    Proceed,
    /// Hook requests blocking the operation (e.g. deny a tool call).
    Block { message: Option<String> },
    /// Hook requests modifying the input or output content.
    Modify { content: String },
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

    #[test]
    fn hook_event_pre_tool_use_serializes_with_flattened_fields() {
        use super::HookEventPreToolUse;

        let hook_event = HookEvent::PreToolUse {
            event: HookEventPreToolUse {
                tool_name: "bash".to_string(),
                tool_input: r#"{"command": "ls"}"#.to_string(),
            },
        };

        let actual = serde_json::to_value(&hook_event).expect("serialize pre_tool_use event");
        let expected = json!({
            "event_type": "pre_tool_use",
            "tool_name": "bash",
            "tool_input": r#"{"command": "ls"}"#,
        });

        assert_eq!(actual, expected);
    }

    #[test]
    fn hook_event_post_tool_use_serializes_correctly() {
        use super::HookEventPostToolUse;

        let hook_event = HookEvent::PostToolUse {
            event: HookEventPostToolUse {
                tool_name: "bash".to_string(),
                tool_output: "file1.txt\nfile2.txt".to_string(),
            },
        };

        let actual = serde_json::to_value(&hook_event).expect("serialize post_tool_use event");
        let expected = json!({
            "event_type": "post_tool_use",
            "tool_name": "bash",
            "tool_output": "file1.txt\nfile2.txt",
        });

        assert_eq!(actual, expected);
    }

    #[test]
    fn hook_event_stop_serializes_correctly() {
        use super::HookEventStop;

        let hook_event = HookEvent::Stop {
            event: HookEventStop {
                reason: "max_tokens_reached".to_string(),
            },
        };

        let actual = serde_json::to_value(&hook_event).expect("serialize stop event");
        let expected = json!({
            "event_type": "stop",
            "reason": "max_tokens_reached",
        });

        assert_eq!(actual, expected);
    }

    #[test]
    fn hook_event_user_prompt_submit_serializes_correctly() {
        use super::HookEventUserPromptSubmit;

        let hook_event = HookEvent::UserPromptSubmit {
            event: HookEventUserPromptSubmit {
                user_message: "Help me debug this code".to_string(),
            },
        };

        let actual = serde_json::to_value(&hook_event).expect("serialize user_prompt_submit event");
        let expected = json!({
            "event_type": "user_prompt_submit",
            "user_message": "Help me debug this code",
        });

        assert_eq!(actual, expected);
    }

    #[test]
    fn hook_event_notification_serializes_correctly() {
        use super::HookEventNotification;

        let hook_event = HookEvent::Notification {
            event: HookEventNotification {
                message: "Build completed successfully".to_string(),
                level: "info".to_string(),
            },
        };

        let actual = serde_json::to_value(&hook_event).expect("serialize notification event");
        let expected = json!({
            "event_type": "notification",
            "message": "Build completed successfully",
            "level": "info",
        });

        assert_eq!(actual, expected);
    }
}
