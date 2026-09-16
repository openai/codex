//! Assembles multi-agent role instructions from selected text and runtime capabilities.
//! The segment owns rendering and attribution; consumers select and capture its inputs.

use crate::without_update_plan_instructions;
use codex_context_fragments::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

const DEFAULT_MULTI_AGENT_V2_MODEL_OVERRIDE_USAGE_HINT_TEXT: &str = "Full-history forks (`fork_turns` omitted or `\"all\"`) inherit the parent model and reasoning effort and do not accept overrides. Only set `model` or `reasoning_effort` when explicitly requested by the user, applicable `AGENTS.md` instructions, or skill instructions; when doing so, set `fork_turns` to `\"none\"` or a positive integer string.";
const DEFAULT_MULTI_AGENT_V2_WAIT_AGENT_USAGE_HINT_TEXT: &str =
    "When calling `wait_agent`, prefer longer waits (minutes) to avoid busy polling.";
const DEFAULT_MULTI_AGENT_V2_SHARED_USAGE_HINT_TEXT: &str = r#"Note that collaboration tools cannot be called from inside `functions.exec`. Call `spawn_agent`, `send_message`, `followup_task`, `wait_agent`, `interrupt_agent`, and `list_agents` only as direct tool calls using the recipient shown in their tool definitions, such as `to=functions.collaboration.spawn_agent`, since they are intentionally absent from the `functions.exec` `tools.*` namespace. Available tools in `functions.exec` are explicitly described with a `tools` namespace in the developer message.

All agents share the same directory. In detail:
- All agents have access to the same container and filesystem as you.
- All agents use the same current working directory.
- As a result, edits made by one agent are immediately visible to all other agents.
"#;

/// Multi-agent role text and the captured capabilities used to render its context segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultiAgentRoleInstructions {
    /// Complete configured instructions, emitted verbatim without catalog markers.
    Configured(String),
    /// Selected catalog or bundled text, composed with captured runtime guidance.
    Composed {
        base: String,
        marked: bool,
        omit_update_plan_instructions: bool,
        max_concurrency: usize,
        wait_agent_enabled: bool,
        expose_model_overrides: bool,
    },
}

impl ContextualUserFragment for MultiAgentRoleInstructions {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("multi_agent.role_instructions".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn requires_separate_message(&self) -> bool {
        true
    }

    fn markers(&self) -> (&'static str, &'static str) {
        match self {
            Self::Composed { marked: true, .. } => Self::type_markers(),
            Self::Configured(_) | Self::Composed { marked: false, .. } => ("", ""),
        }
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<multi_agent_role>", "</multi_agent_role>")
    }

    fn body(&self) -> String {
        match self {
            Self::Configured(text) => text.clone(),
            Self::Composed {
                base,
                omit_update_plan_instructions,
                max_concurrency,
                wait_agent_enabled,
                expose_model_overrides,
                ..
            } => {
                let base = if *omit_update_plan_instructions {
                    without_update_plan_instructions(base)
                } else {
                    base.clone()
                };
                let wait_agent_guidance = if *wait_agent_enabled {
                    format!("{DEFAULT_MULTI_AGENT_V2_WAIT_AGENT_USAGE_HINT_TEXT}\n\n")
                } else {
                    String::new()
                };
                let shared = DEFAULT_MULTI_AGENT_V2_SHARED_USAGE_HINT_TEXT;
                let mut text = format!(
                    "{base}\n{shared}\n{wait_agent_guidance}There are {max_concurrency} available concurrency slots, meaning that up to {max_concurrency} agents can be active at once, including you."
                );
                if *expose_model_overrides {
                    text.push_str("\n\n");
                    text.push_str(DEFAULT_MULTI_AGENT_V2_MODEL_OVERRIDE_USAGE_HINT_TEXT);
                }
                text
            }
        }
    }
}

#[cfg(test)]
#[path = "multi_agent_instructions_tests.rs"]
mod tests;
