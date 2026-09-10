//! Defines the reviewer's runtime settings. Hosts apply these settings to their
//! concrete configuration; context construction and managed constraints stay with the host.

use std::collections::HashMap;

use codex_features::Feature;
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

/// Configuration policy for a reviewer. The host retains managed constraints and
/// live network rules while applying these values to its concrete runtime config.
pub struct ReviewerConfigOverrides {
    pub model: String,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub request_max_retries: u64,
    pub stream_max_retries: u64,
    pub include_skill_instructions: bool,
    pub use_memories: bool,
    pub dedicated_memory_tools: bool,
    pub inherit_token_budget: bool,
    pub notify: Option<Vec<String>>,
    pub developer_instructions: Option<String>,
    pub approval_policy: AskForApproval,
    pub permission_profile: PermissionProfile,
    pub include_apps_instructions: bool,
    pub inherit_mcp_servers: bool,
    pub disabled_features: Vec<Feature>,
}

pub fn reviewer_config_overrides(
    parent_permissions: &PermissionProfile,
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
) -> ReviewerConfigOverrides {
    ReviewerConfigOverrides {
        model: model.to_owned(),
        reasoning_effort,
        request_max_retries: 1,
        stream_max_retries: 1,
        include_skill_instructions: false,
        use_memories: false,
        dedicated_memory_tools: false,
        inherit_token_budget: false,
        notify: None,
        developer_instructions: None,
        approval_policy: AskForApproval::Never,
        permission_profile: read_only_guardian_permission_profile(parent_permissions),
        include_apps_instructions: false,
        inherit_mcp_servers: false,
        disabled_features: vec![
            Feature::Collab,
            Feature::MultiAgentV2,
            Feature::GuardianV2,
            Feature::TokenBudget,
            Feature::ContextManagement,
            Feature::CodexHooks,
            Feature::Apps,
            Feature::Plugins,
            Feature::WebSearchRequest,
            Feature::WebSearchCached,
        ],
    }
}

fn read_only_guardian_permission_profile(profile: &PermissionProfile) -> PermissionProfile {
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
                config.permission_profile =
                    PermissionProfileSnapshot::legacy(read_only_guardian_permission_profile(
                        config.permission_profile.permission_profile(),
                    ));
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
