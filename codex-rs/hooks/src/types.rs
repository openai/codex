use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::DateTime;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::models::SandboxPermissions;
use futures::future::BoxFuture;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeMap;
use serde_json::Map;
use serde_json::Value;

pub type HookFn = Arc<dyn for<'a> Fn(&'a HookPayload) -> BoxFuture<'a, HookResult> + Send + Sync>;

#[derive(Debug)]
pub enum HookResult {
    /// Success: hook completed successfully.
    Success,
    /// FailedContinue: hook failed, but other subsequent hooks should still execute and the
    /// operation should continue.
    FailedContinue(Box<dyn std::error::Error + Send + Sync + 'static>),
    /// FailedAbort: hook failed, other subsequent hooks should not execute, and the operation
    /// should be aborted.
    FailedAbort(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl HookResult {
    pub fn should_abort_operation(&self) -> bool {
        matches!(self, Self::FailedAbort(_))
    }
}

#[derive(Debug)]
pub struct HookResponse {
    pub hook_name: String,
    pub result: HookResult,
}

#[derive(Clone)]
pub struct Hook {
    pub name: String,
    pub func: HookFn,
}

impl Default for Hook {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            func: Arc::new(|_| Box::pin(async { HookResult::Success })),
        }
    }
}

impl Hook {
    pub async fn execute(&self, payload: &HookPayload) -> HookResponse {
        HookResponse {
            hook_name: self.name.clone(),
            result: (self.func)(payload).await,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HookPayload {
    pub session_id: ThreadId,
    pub transcript_path: Option<String>,
    pub cwd: PathBuf,
    pub client: Option<String>,
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HookToolKind {
    Function,
    Custom,
    LocalShell,
    Mcp,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct HookToolInputLocalShell {
    pub command: Vec<String>,
    pub workdir: Option<String>,
    pub timeout_ms: Option<u64>,
    pub sandbox_permissions: Option<SandboxPermissions>,
    pub prefix_rule: Option<Vec<String>>,
    pub justification: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "input_type", rename_all = "snake_case")]
pub enum HookToolInput {
    Function {
        arguments: String,
    },
    Custom {
        input: String,
    },
    LocalShell {
        params: HookToolInputLocalShell,
    },
    Mcp {
        server: String,
        tool: String,
        arguments: String,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct HookEventPreToolUse {
    pub turn_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub tool_kind: HookToolKind,
    pub tool_input: HookToolInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mutating: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct HookEventPostToolUse {
    pub turn_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub tool_kind: HookToolKind,
    pub tool_input: HookToolInput,
    pub executed: bool,
    pub success: bool,
    pub duration_ms: u64,
    pub mutating: bool,
    pub sandbox: String,
    pub sandbox_policy: String,
    pub output_preview: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct HookEventLifecycle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_session_id: Option<ThreadId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_assistant_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<HookToolInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_id: Option<ThreadId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub enum HookEvent {
    AfterAgent { event: HookEventAfterAgent },
    PreToolUse { event: HookEventPreToolUse },
    PostToolUse { event: HookEventPostToolUse },
    SessionStart { event: HookEventLifecycle },
    UserPromptSubmit { event: HookEventLifecycle },
    Stop { event: HookEventLifecycle },
    PreCompact { event: HookEventLifecycle },
    SessionEnd { event: HookEventLifecycle },
    SubagentStart { event: HookEventLifecycle },
    SubagentStop { event: HookEventLifecycle },
}

impl HookEvent {
    pub fn name(&self) -> &'static str {
        match self {
            Self::AfterAgent { .. } => "AfterAgent",
            Self::PreToolUse { .. } => "PreToolUse",
            Self::PostToolUse { .. } => "PostToolUse",
            Self::SessionStart { .. } => "SessionStart",
            Self::UserPromptSubmit { .. } => "UserPromptSubmit",
            Self::Stop { .. } => "Stop",
            Self::PreCompact { .. } => "PreCompact",
            Self::SessionEnd { .. } => "SessionEnd",
            Self::SubagentStart { .. } => "SubagentStart",
            Self::SubagentStop { .. } => "SubagentStop",
        }
    }

    pub fn aborts_on_exit_code_two(&self) -> bool {
        matches!(
            self,
            Self::PreToolUse { .. }
                | Self::UserPromptSubmit { .. }
                | Self::Stop { .. }
                | Self::SubagentStop { .. }
        )
    }

    fn fields(&self) -> Result<Map<String, Value>, serde_json::Error> {
        match self {
            Self::AfterAgent { event } => serialize_object(event),
            Self::PreToolUse { event } => serialize_object(event),
            Self::PostToolUse { event } => serialize_object(event),
            Self::SessionStart { event } => serialize_object(event),
            Self::UserPromptSubmit { event } => serialize_object(event),
            Self::Stop { event } => serialize_object(event),
            Self::PreCompact { event } => serialize_object(event),
            Self::SessionEnd { event } => serialize_object(event),
            Self::SubagentStart { event } => serialize_object(event),
            Self::SubagentStop { event } => serialize_object(event),
        }
    }
}

impl Serialize for HookPayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("session_id", &self.session_id)?;
        if let Some(transcript_path) = &self.transcript_path {
            map.serialize_entry("transcript_path", transcript_path)?;
        }
        map.serialize_entry("cwd", &self.cwd)?;
        if let Some(client) = &self.client {
            map.serialize_entry("client", client)?;
        }
        map.serialize_entry(
            "triggered_at",
            &self.triggered_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        )?;
        map.serialize_entry("hook_event_name", self.hook_event.name())?;

        let fields = self
            .hook_event
            .fields()
            .map_err(serde::ser::Error::custom)?;
        for (key, value) in fields {
            map.serialize_entry(&key, &value)?;
        }

        map.end()
    }
}

fn serialize_object<T: Serialize>(value: &T) -> Result<Map<String, Value>, serde_json::Error> {
    match serde_json::to_value(value)? {
        Value::Object(object) => Ok(object),
        _ => Ok(Map::new()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::models::SandboxPermissions;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::HookEvent;
    use super::HookEventAfterAgent;
    use super::HookEventLifecycle;
    use super::HookEventPostToolUse;
    use super::HookEventPreToolUse;
    use super::HookPayload;
    use super::HookToolInput;
    use super::HookToolInputLocalShell;
    use super::HookToolKind;

    fn sample_lifecycle_event(
        previous_session_id: ThreadId,
        subagent_id: ThreadId,
    ) -> HookEventLifecycle {
        let mut metadata = HashMap::new();
        metadata.insert("phase".to_string(), "done".to_string());

        HookEventLifecycle {
            previous_session_id: Some(previous_session_id),
            prompt: Some("hello world".to_string()),
            last_assistant_message: Some("done".to_string()),
            tool_use_id: Some("toolu_123".to_string()),
            tool_input: Some(HookToolInput::Function {
                arguments: "{\"code\":\"cargo test\"}".to_string(),
            }),
            subagent_id: Some(subagent_id),
            metadata: Some(metadata),
        }
    }

    #[test]
    fn hook_payload_serializes_stable_wire_shape() {
        let session_id = ThreadId::new();
        let thread_id = ThreadId::new();
        let payload = HookPayload {
            session_id,
            transcript_path: Some("/tmp/rollout.jsonl".to_string()),
            cwd: PathBuf::from("tmp"),
            client: None,
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
            "transcript_path": "/tmp/rollout.jsonl",
            "cwd": "tmp",
            "triggered_at": "2025-01-01T00:00:00Z",
            "hook_event_name": "AfterAgent",
            "thread_id": thread_id.to_string(),
            "turn_id": "turn-1",
            "input_messages": ["hello"],
            "last_assistant_message": "hi",
        });

        assert_eq!(actual, expected);
    }

    #[test]
    fn post_tool_use_payload_serializes_stable_wire_shape() {
        let session_id = ThreadId::new();
        let payload = HookPayload {
            session_id,
            transcript_path: Some("/tmp/rollout.jsonl".to_string()),
            cwd: PathBuf::from("tmp"),
            client: None,
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::PostToolUse {
                event: HookEventPostToolUse {
                    turn_id: "turn-2".to_string(),
                    call_id: "call-1".to_string(),
                    tool_name: "local_shell".to_string(),
                    tool_kind: HookToolKind::LocalShell,
                    tool_input: HookToolInput::LocalShell {
                        params: HookToolInputLocalShell {
                            command: vec!["cargo".to_string(), "fmt".to_string()],
                            workdir: Some("codex-rs".to_string()),
                            timeout_ms: Some(60_000),
                            sandbox_permissions: Some(SandboxPermissions::UseDefault),
                            justification: None,
                            prefix_rule: None,
                        },
                    },
                    executed: true,
                    success: true,
                    duration_ms: 42,
                    mutating: true,
                    sandbox: "none".to_string(),
                    sandbox_policy: "danger-full-access".to_string(),
                    output_preview: "ok".to_string(),
                },
            },
        };

        let actual = serde_json::to_value(payload).expect("serialize hook payload");
        let expected = json!({
            "session_id": session_id.to_string(),
            "transcript_path": "/tmp/rollout.jsonl",
            "cwd": "tmp",
            "triggered_at": "2025-01-01T00:00:00Z",
            "hook_event_name": "PostToolUse",
            "turn_id": "turn-2",
            "call_id": "call-1",
            "tool_name": "local_shell",
            "tool_kind": "local_shell",
            "tool_input": {
                "input_type": "local_shell",
                "params": {
                    "command": ["cargo", "fmt"],
                    "workdir": "codex-rs",
                    "timeout_ms": 60000,
                    "sandbox_permissions": "use_default",
                    "justification": null,
                    "prefix_rule": null,
                },
            },
            "executed": true,
            "success": true,
            "duration_ms": 42,
            "mutating": true,
            "sandbox": "none",
            "sandbox_policy": "danger-full-access",
            "output_preview": "ok",
        });

        assert_eq!(actual, expected);
    }

    #[test]
    fn pre_tool_use_payload_serializes_stable_wire_shape() {
        let session_id = ThreadId::new();
        let payload = HookPayload {
            session_id,
            transcript_path: Some("/tmp/rollout.jsonl".to_string()),
            cwd: PathBuf::from("tmp"),
            client: Some("codex-tui".to_string()),
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::PreToolUse {
                event: HookEventPreToolUse {
                    turn_id: "turn-2".to_string(),
                    call_id: "call-1".to_string(),
                    tool_name: "local_shell".to_string(),
                    tool_kind: HookToolKind::LocalShell,
                    tool_input: HookToolInput::LocalShell {
                        params: HookToolInputLocalShell {
                            command: vec!["cargo".to_string(), "fmt".to_string()],
                            workdir: Some("codex-rs".to_string()),
                            timeout_ms: Some(60_000),
                            sandbox_permissions: Some(SandboxPermissions::UseDefault),
                            justification: None,
                            prefix_rule: None,
                        },
                    },
                    mutating: Some(true),
                    sandbox: Some("none".to_string()),
                    sandbox_policy: Some("danger-full-access".to_string()),
                },
            },
        };

        let actual = serde_json::to_value(payload).expect("serialize hook payload");
        let expected = json!({
            "session_id": session_id.to_string(),
            "transcript_path": "/tmp/rollout.jsonl",
            "cwd": "tmp",
            "client": "codex-tui",
            "triggered_at": "2025-01-01T00:00:00Z",
            "hook_event_name": "PreToolUse",
            "turn_id": "turn-2",
            "call_id": "call-1",
            "tool_name": "local_shell",
            "tool_kind": "local_shell",
            "tool_input": {
                "input_type": "local_shell",
                "params": {
                    "command": ["cargo", "fmt"],
                    "workdir": "codex-rs",
                    "timeout_ms": 60000,
                    "sandbox_permissions": "use_default",
                    "justification": null,
                    "prefix_rule": null,
                },
            },
            "mutating": true,
            "sandbox": "none",
            "sandbox_policy": "danger-full-access",
        });

        assert_eq!(actual, expected);
    }

    #[test]
    fn all_lifecycle_payloads_serialize_stable_wire_shape() {
        let session_id = ThreadId::new();
        let previous_session_id = ThreadId::new();
        let subagent_id = ThreadId::new();
        let base_payload = |hook_event: HookEvent| HookPayload {
            session_id,
            transcript_path: Some("/tmp/rollout.jsonl".to_string()),
            cwd: PathBuf::from("tmp"),
            client: Some("codex-tui".to_string()),
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event,
        };

        let cases = vec![
            (
                "SessionStart",
                base_payload(HookEvent::SessionStart {
                    event: sample_lifecycle_event(previous_session_id, subagent_id),
                }),
                json!({
                    "session_id": session_id.to_string(),
                    "transcript_path": "/tmp/rollout.jsonl",
                    "cwd": "tmp",
                    "client": "codex-tui",
                    "triggered_at": "2025-01-01T00:00:00Z",
                    "hook_event_name": "SessionStart",
                    "previous_session_id": previous_session_id.to_string(),
                    "prompt": "hello world",
                    "last_assistant_message": "done",
                    "tool_use_id": "toolu_123",
                    "tool_input": {
                        "input_type": "function",
                        "arguments": "{\"code\":\"cargo test\"}",
                    },
                    "subagent_id": subagent_id.to_string(),
                    "metadata": {
                        "phase": "done",
                    },
                }),
            ),
            (
                "UserPromptSubmit",
                base_payload(HookEvent::UserPromptSubmit {
                    event: HookEventLifecycle {
                        previous_session_id: None,
                        subagent_id: None,
                        metadata: None,
                        ..sample_lifecycle_event(previous_session_id, subagent_id)
                    },
                }),
                json!({
                    "session_id": session_id.to_string(),
                    "transcript_path": "/tmp/rollout.jsonl",
                    "cwd": "tmp",
                    "client": "codex-tui",
                    "triggered_at": "2025-01-01T00:00:00Z",
                    "hook_event_name": "UserPromptSubmit",
                    "prompt": "hello world",
                    "last_assistant_message": "done",
                    "tool_use_id": "toolu_123",
                    "tool_input": {
                        "input_type": "function",
                        "arguments": "{\"code\":\"cargo test\"}",
                    },
                }),
            ),
            (
                "Stop",
                base_payload(HookEvent::Stop {
                    event: HookEventLifecycle {
                        previous_session_id: None,
                        subagent_id: None,
                        metadata: None,
                        ..sample_lifecycle_event(previous_session_id, subagent_id)
                    },
                }),
                json!({
                    "session_id": session_id.to_string(),
                    "transcript_path": "/tmp/rollout.jsonl",
                    "cwd": "tmp",
                    "client": "codex-tui",
                    "triggered_at": "2025-01-01T00:00:00Z",
                    "hook_event_name": "Stop",
                    "prompt": "hello world",
                    "last_assistant_message": "done",
                    "tool_use_id": "toolu_123",
                    "tool_input": {
                        "input_type": "function",
                        "arguments": "{\"code\":\"cargo test\"}",
                    },
                }),
            ),
            (
                "PreCompact",
                base_payload(HookEvent::PreCompact {
                    event: HookEventLifecycle {
                        prompt: None,
                        tool_input: None,
                        subagent_id: None,
                        ..sample_lifecycle_event(previous_session_id, subagent_id)
                    },
                }),
                json!({
                    "session_id": session_id.to_string(),
                    "transcript_path": "/tmp/rollout.jsonl",
                    "cwd": "tmp",
                    "client": "codex-tui",
                    "triggered_at": "2025-01-01T00:00:00Z",
                    "hook_event_name": "PreCompact",
                    "previous_session_id": previous_session_id.to_string(),
                    "last_assistant_message": "done",
                    "tool_use_id": "toolu_123",
                    "metadata": {
                        "phase": "done",
                    },
                }),
            ),
            (
                "SessionEnd",
                base_payload(HookEvent::SessionEnd {
                    event: HookEventLifecycle {
                        prompt: None,
                        last_assistant_message: None,
                        tool_use_id: None,
                        tool_input: None,
                        subagent_id: None,
                        metadata: None,
                        ..sample_lifecycle_event(previous_session_id, subagent_id)
                    },
                }),
                json!({
                    "session_id": session_id.to_string(),
                    "transcript_path": "/tmp/rollout.jsonl",
                    "cwd": "tmp",
                    "client": "codex-tui",
                    "triggered_at": "2025-01-01T00:00:00Z",
                    "hook_event_name": "SessionEnd",
                    "previous_session_id": previous_session_id.to_string(),
                }),
            ),
            (
                "SubagentStart",
                base_payload(HookEvent::SubagentStart {
                    event: HookEventLifecycle {
                        previous_session_id: None,
                        last_assistant_message: None,
                        subagent_id: None,
                        metadata: None,
                        ..sample_lifecycle_event(previous_session_id, subagent_id)
                    },
                }),
                json!({
                    "session_id": session_id.to_string(),
                    "transcript_path": "/tmp/rollout.jsonl",
                    "cwd": "tmp",
                    "client": "codex-tui",
                    "triggered_at": "2025-01-01T00:00:00Z",
                    "hook_event_name": "SubagentStart",
                    "prompt": "hello world",
                    "tool_use_id": "toolu_123",
                    "tool_input": {
                        "input_type": "function",
                        "arguments": "{\"code\":\"cargo test\"}",
                    },
                }),
            ),
            (
                "SubagentStop",
                base_payload(HookEvent::SubagentStop {
                    event: HookEventLifecycle {
                        previous_session_id: None,
                        last_assistant_message: None,
                        metadata: None,
                        ..sample_lifecycle_event(previous_session_id, subagent_id)
                    },
                }),
                json!({
                    "session_id": session_id.to_string(),
                    "transcript_path": "/tmp/rollout.jsonl",
                    "cwd": "tmp",
                    "client": "codex-tui",
                    "triggered_at": "2025-01-01T00:00:00Z",
                    "hook_event_name": "SubagentStop",
                    "prompt": "hello world",
                    "tool_use_id": "toolu_123",
                    "tool_input": {
                        "input_type": "function",
                        "arguments": "{\"code\":\"cargo test\"}",
                    },
                    "subagent_id": subagent_id.to_string(),
                }),
            ),
        ];

        for (event_name, payload, expected) in cases {
            assert_eq!(payload.hook_event.name(), event_name);
            let actual = serde_json::to_value(payload).expect("serialize lifecycle payload");
            assert_eq!(actual, expected, "lifecycle event {event_name}");
        }
    }
}
