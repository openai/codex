//! Defines reviewer tools, read-only permissions and per-turn input.
//! Guardian supplies concrete session configuration to the temporary host context adapter.

use std::collections::HashMap;

use codex_extension_api::ToolName;
use codex_extension_api::ToolPolicy;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::PermissionProfileSnapshot;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnStartOptions;
use codex_protocol::user_input::UserInput;
use serde_json::Value;

/// Guardian's configuration function for the host's concrete config type.
/// The host applies it to each captured parent config before building context and reuse keys.
/// Keeping it in thread data avoids caching settings that can change between reviews.
pub struct ReviewerConfig<C>(pub fn(&C) -> anyhow::Result<C>);

/// Reviewer tool selection and construction restrictions, enforced by core.
pub fn reviewer_tool_policy() -> ToolPolicy {
    ToolPolicy {
        allowed_tools: Some(
            ["exec_command", "write_stdin", "view_image", "exec", "wait"]
                .into_iter()
                .map(ToolName::plain)
                .collect(),
        ),
        require_managed_sandbox: true,
        require_unified_exec: true,
        expose_additional_permissions: false,
    }
}

/// Applies the same read-only ceiling to the reviewer and each inherited environment.
pub fn reviewer_permission_profile(profile: &PermissionProfile) -> PermissionProfile {
    profile
        .intersect_with_read_only()
        .unwrap_or(PermissionProfile::External {
            network: codex_protocol::permissions::NetworkSandboxPolicy::Restricted,
        })
}

/// Context and parent settings captured for a single reviewer turn.
pub struct ReviewerTurn {
    pub items: Vec<UserInput>,
    pub environments: TurnEnvironmentSelections,
    pub permission_profile: PermissionProfile,
    pub reasoning_summary: ReasoningSummary,
    pub personality: Option<Personality>,
    pub model: String,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub parent_response_id: Option<String>,
    pub schema: Value,
    pub parent_turn_id: String,
    pub root_turn_id: Option<String>,
}

impl ReviewerTurn {
    pub fn into_request(mut self) -> TurnInputRequest {
        // Apply the same read-only ceiling to every inherited environment.
        for environment in &mut self.environments.environments {
            if let EnvironmentConfigState::Ready(config) = &mut environment.config {
                config.permission_profile = PermissionProfileSnapshot::legacy(
                    reviewer_permission_profile(config.permission_profile.permission_profile()),
                );
            }
        }
        TurnInputRequest::user_input(self.items)
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(self.environments),
                approval_policy: Some(AskForApproval::Never),
                permission_profile: Some(self.permission_profile),
                summary: Some(self.reasoning_summary),
                personality: self.personality,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: self.model,
                        reasoning_effort: self.reasoning_effort,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            })
            .with_responses_metadata(
                self.parent_response_id
                    .map(|id| HashMap::from([("parent_response_id".to_owned(), id)])),
            )
            .on_start(TurnStartOptions {
                turn_trigger: Some("guardian_review".to_owned()),
                final_output_json_schema: Some(self.schema),
                parent_turn_id: Some(self.parent_turn_id),
                root_turn_id: self.root_turn_id,
                ..Default::default()
            })
    }
}
