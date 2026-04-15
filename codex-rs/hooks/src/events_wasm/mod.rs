pub mod post_tool_use {
    use std::path::PathBuf;

    use codex_protocol::ThreadId;
    use codex_protocol::protocol::HookCompletedEvent;
    use serde_json::Value;

    #[derive(Debug, Clone)]
    pub struct PostToolUseRequest {
        pub session_id: ThreadId,
        pub turn_id: String,
        pub cwd: PathBuf,
        pub transcript_path: Option<PathBuf>,
        pub model: String,
        pub permission_mode: String,
        pub tool_name: String,
        pub tool_use_id: String,
        pub command: String,
        pub tool_response: Value,
    }

    #[derive(Debug)]
    pub struct PostToolUseOutcome {
        pub hook_events: Vec<HookCompletedEvent>,
        pub should_stop: bool,
        pub stop_reason: Option<String>,
        pub additional_contexts: Vec<String>,
        pub feedback_message: Option<String>,
    }
}

pub mod pre_tool_use {
    use std::path::PathBuf;

    use codex_protocol::ThreadId;
    use codex_protocol::protocol::HookCompletedEvent;

    #[derive(Debug, Clone)]
    pub struct PreToolUseRequest {
        pub session_id: ThreadId,
        pub turn_id: String,
        pub cwd: PathBuf,
        pub transcript_path: Option<PathBuf>,
        pub model: String,
        pub permission_mode: String,
        pub tool_name: String,
        pub tool_use_id: String,
        pub command: String,
    }

    #[derive(Debug)]
    pub struct PreToolUseOutcome {
        pub hook_events: Vec<HookCompletedEvent>,
        pub should_block: bool,
        pub block_reason: Option<String>,
    }
}

pub mod session_start {
    use std::path::PathBuf;

    use codex_protocol::ThreadId;
    use codex_protocol::protocol::HookCompletedEvent;

    #[derive(Debug, Clone, Copy)]
    pub enum SessionStartSource {
        Startup,
        Resume,
    }

    impl SessionStartSource {
        pub fn as_str(self) -> &'static str {
            match self {
                Self::Startup => "startup",
                Self::Resume => "resume",
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct SessionStartRequest {
        pub session_id: ThreadId,
        pub cwd: PathBuf,
        pub transcript_path: Option<PathBuf>,
        pub model: String,
        pub permission_mode: String,
        pub source: SessionStartSource,
    }

    #[derive(Debug)]
    pub struct SessionStartOutcome {
        pub hook_events: Vec<HookCompletedEvent>,
        pub should_stop: bool,
        pub stop_reason: Option<String>,
        pub additional_contexts: Vec<String>,
    }
}

pub mod stop {
    use std::path::PathBuf;

    use codex_protocol::ThreadId;
    use codex_protocol::items::HookPromptFragment;
    use codex_protocol::protocol::HookCompletedEvent;

    #[derive(Debug, Clone)]
    pub struct StopRequest {
        pub session_id: ThreadId,
        pub turn_id: String,
        pub cwd: PathBuf,
        pub transcript_path: Option<PathBuf>,
        pub model: String,
        pub permission_mode: String,
        pub stop_hook_active: bool,
        pub last_assistant_message: Option<String>,
    }

    #[derive(Debug)]
    pub struct StopOutcome {
        pub hook_events: Vec<HookCompletedEvent>,
        pub should_stop: bool,
        pub stop_reason: Option<String>,
        pub should_block: bool,
        pub block_reason: Option<String>,
        pub continuation_fragments: Vec<HookPromptFragment>,
    }
}

pub mod user_prompt_submit {
    use std::path::PathBuf;

    use codex_protocol::ThreadId;
    use codex_protocol::protocol::HookCompletedEvent;

    #[derive(Debug, Clone)]
    pub struct UserPromptSubmitRequest {
        pub session_id: ThreadId,
        pub turn_id: String,
        pub cwd: PathBuf,
        pub transcript_path: Option<PathBuf>,
        pub model: String,
        pub permission_mode: String,
        pub prompt: String,
    }

    #[derive(Debug)]
    pub struct UserPromptSubmitOutcome {
        pub hook_events: Vec<HookCompletedEvent>,
        pub should_stop: bool,
        pub stop_reason: Option<String>,
        pub additional_contexts: Vec<String>,
    }
}
