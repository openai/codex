use codex_config::ConfigLayerStack;

use crate::events::post_tool_use::PostToolUseOutcome;
use crate::events::post_tool_use::PostToolUseRequest;
use crate::events::pre_tool_use::PreToolUseOutcome;
use crate::events::pre_tool_use::PreToolUseRequest;
use crate::events::session_start::SessionStartOutcome;
use crate::events::session_start::SessionStartRequest;
use crate::events::stop::StopOutcome;
use crate::events::stop::StopRequest;
use crate::events::user_prompt_submit::UserPromptSubmitOutcome;
use crate::events::user_prompt_submit::UserPromptSubmitRequest;
use crate::types::Hook;
use crate::types::HookEvent;
use crate::types::HookPayload;
use crate::types::HookResponse;

#[derive(Default, Clone)]
pub struct HooksConfig {
    pub legacy_notify_argv: Option<Vec<String>>,
    pub feature_enabled: bool,
    pub config_layer_stack: Option<ConfigLayerStack>,
    pub shell_program: Option<String>,
    pub shell_args: Vec<String>,
}

#[derive(Clone, Default)]
pub struct Hooks {
    after_agent: Vec<Hook>,
    after_tool_use: Vec<Hook>,
}

impl Hooks {
    pub fn new(config: HooksConfig) -> Self {
        let after_agent = config
            .legacy_notify_argv
            .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
            .map(crate::notify_hook)
            .into_iter()
            .collect();
        Self {
            after_agent,
            after_tool_use: Vec::new(),
        }
    }

    pub fn startup_warnings(&self) -> &[String] {
        &[]
    }

    fn hooks_for_event(&self, hook_event: &HookEvent) -> &[Hook] {
        match hook_event {
            HookEvent::AfterAgent { .. } => &self.after_agent,
            HookEvent::AfterToolUse { .. } => &self.after_tool_use,
        }
    }

    pub async fn dispatch(&self, hook_payload: HookPayload) -> Vec<HookResponse> {
        let hooks = self.hooks_for_event(&hook_payload.hook_event);
        let mut outcomes = Vec::with_capacity(hooks.len());
        for hook in hooks {
            let outcome = hook.execute(&hook_payload).await;
            let should_abort_operation = outcome.result.should_abort_operation();
            outcomes.push(outcome);
            if should_abort_operation {
                break;
            }
        }
        outcomes
    }

    pub fn preview_session_start(
        &self,
        _request: &SessionStartRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        Vec::new()
    }

    pub fn preview_pre_tool_use(
        &self,
        _request: &PreToolUseRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        Vec::new()
    }

    pub fn preview_post_tool_use(
        &self,
        _request: &PostToolUseRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        Vec::new()
    }

    pub async fn run_session_start(
        &self,
        _request: SessionStartRequest,
        _turn_id: Option<String>,
    ) -> SessionStartOutcome {
        SessionStartOutcome {
            hook_events: Vec::new(),
            should_stop: false,
            stop_reason: None,
            additional_contexts: Vec::new(),
        }
    }

    pub async fn run_pre_tool_use(&self, _request: PreToolUseRequest) -> PreToolUseOutcome {
        PreToolUseOutcome {
            hook_events: Vec::new(),
            should_block: false,
            block_reason: None,
        }
    }

    pub async fn run_post_tool_use(&self, _request: PostToolUseRequest) -> PostToolUseOutcome {
        PostToolUseOutcome {
            hook_events: Vec::new(),
            should_stop: false,
            stop_reason: None,
            additional_contexts: Vec::new(),
            feedback_message: None,
        }
    }

    pub fn preview_user_prompt_submit(
        &self,
        _request: &UserPromptSubmitRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        Vec::new()
    }

    pub async fn run_user_prompt_submit(
        &self,
        _request: UserPromptSubmitRequest,
    ) -> UserPromptSubmitOutcome {
        UserPromptSubmitOutcome {
            hook_events: Vec::new(),
            should_stop: false,
            stop_reason: None,
            additional_contexts: Vec::new(),
        }
    }

    pub fn preview_stop(
        &self,
        _request: &StopRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        Vec::new()
    }

    pub async fn run_stop(&self, _request: StopRequest) -> StopOutcome {
        StopOutcome {
            hook_events: Vec::new(),
            should_stop: false,
            stop_reason: None,
            should_block: false,
            block_reason: None,
            continuation_fragments: Vec::new(),
        }
    }
}

pub fn command_from_argv(argv: &[String]) -> Option<std::process::Command> {
    let (program, args) = argv.split_first()?;
    if program.is_empty() {
        return None;
    }
    let mut command = std::process::Command::new(program);
    command.args(args);
    Some(command)
}
