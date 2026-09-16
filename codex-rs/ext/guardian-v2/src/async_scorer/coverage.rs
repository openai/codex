//! Owns Guardian's category policy and translates legacy config at the boundary.
//! Scoring and approval consume the same policy; host-required reviews still take precedence.
//! Model policies review nested actions; legacy tool coverage remains unchanged.

use codex_extension_api::ToolPayload;
use codex_features::GuardianV2ReviewScopeConfigToml;
use codex_protocol::ToolName;
use codex_protocol::openai_models::GuardianModelPolicy;
use codex_protocol::openai_models::GuardianReviewMode;
use codex_protocol::openai_models::GuardianScope;
use codex_protocol::openai_models::ModelInfo;

use GuardianReviewMode::Adaptive;
use GuardianReviewMode::Disabled;
use GuardianReviewMode::Synchronous;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum UnscoredAction {
    Ignore,
    AgeScore,
    InvalidateScore,
}

/// Canonical runtime policy, including the existing scorer's compatibility behavior.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct GuardianPolicy {
    pub(super) categories: GuardianModelPolicy,
    pub(super) other_tools: GuardianReviewMode,
    pub(super) unscored_action: UnscoredAction,
    pub(super) initial_cua_call: bool,
    sandboxed_exec_commands: bool,
}

impl GuardianPolicy {
    pub(super) fn from_legacy(scope: Option<&GuardianV2ReviewScopeConfigToml>) -> Self {
        let computer_use_only = scope
            .and_then(|scope| scope.computer_use_only)
            .unwrap_or(/*default*/ true);
        let other = if computer_use_only {
            Synchronous
        } else {
            Adaptive
        };
        Self {
            categories: GuardianModelPolicy {
                computer_use: Some(Adaptive),
                shell: Some(other),
                file_changes: Some(other),
                mcp: Some(other),
                network: Some(other),
                permissions: Some(other),
            },
            other_tools: other,
            unscored_action: if computer_use_only {
                UnscoredAction::Ignore
            } else {
                UnscoredAction::AgeScore
            },
            initial_cua_call: computer_use_only,
            sandboxed_exec_commands: !computer_use_only
                && scope
                    .and_then(|scope| scope.sandboxed_exec_commands)
                    .unwrap_or(/*default*/ false),
        }
    }

    pub(super) fn for_model(&self, model: Option<&ModelInfo>) -> Self {
        match model.and_then(|model| model.guardian.as_ref()) {
            Some(categories) => Self {
                categories: categories.clone(),
                other_tools: Disabled,
                unscored_action: UnscoredAction::InvalidateScore,
                initial_cua_call: categories.computer_use == Some(Adaptive),
                sandboxed_exec_commands: true,
            },
            None => {
                let mut policy = self.clone();
                if policy.initial_cua_call
                    && model.is_some_and(|model| !model.node_repl_auto_review_required)
                {
                    policy.categories.computer_use = Some(Synchronous);
                    policy.initial_cua_call = false;
                    policy.unscored_action = UnscoredAction::InvalidateScore;
                }
                policy
            }
        }
    }

    pub(super) fn mode(&self, scope: GuardianScope) -> GuardianReviewMode {
        self.categories.review_mode(scope)
    }

    pub(super) fn scoring_enabled(&self) -> bool {
        [
            self.categories.computer_use,
            self.categories.shell,
            self.categories.file_changes,
            self.categories.mcp,
            self.categories.network,
            self.categories.permissions,
        ]
        .contains(&Some(Adaptive))
    }

    pub(super) fn disable_scoring(&mut self) {
        for mode in [
            &mut self.categories.computer_use,
            &mut self.categories.shell,
            &mut self.categories.file_changes,
            &mut self.categories.mcp,
            &mut self.categories.network,
            &mut self.categories.permissions,
        ] {
            if *mode == Some(Adaptive) {
                *mode = Some(Synchronous);
            }
        }
        self.other_tools = Synchronous;
    }

    pub(super) fn enforce_required_model(&mut self) {
        let computer_use = self.categories.computer_use;
        self.disable_scoring();
        if self.initial_cua_call {
            self.categories.computer_use = computer_use;
        }
    }

    pub(super) fn scores_tool(
        &self,
        tool: &ToolName,
        payload: &ToolPayload,
        scope: Option<GuardianScope>,
    ) -> bool {
        if scope.map_or(self.other_tools, |scope| self.mode(scope)) != Adaptive {
            return false;
        }
        if self.sandboxed_exec_commands
            || !tool.is_default_namespace()
            || tool.name != "exec_command"
        {
            return true;
        }
        matches!(payload, ToolPayload::Function { arguments }
        if serde_json::from_str::<serde_json::Value>(arguments).ok().is_some_and(|arguments| {
            arguments.get("sandbox_permissions").and_then(serde_json::Value::as_str)
                == Some("require_escalated")
        }))
    }
}
