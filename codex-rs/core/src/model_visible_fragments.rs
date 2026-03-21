//! Canonical model-visible fragment definitions and registration.
//!
//! This is the single place to add new model-visible prompt context.
//!
//! Turn-state context is always assembled into exactly two envelopes:
//! - one developer message
//! - one contextual-user message
//!
//! Add a new fragment by:
//! 1. Defining a typed fragment struct in this file.
//! 2. Implementing `ModelVisibleContextFragment`, including `type Role`.
//! 3. If the fragment is contextual-user state, defining
//!    `contextual_user_markers()` or overriding
//!    `matches_contextual_user_text()` for custom matching.
//! 4. If the fragment is derived from `TurnContext` and should participate in
//!    initial-context assembly and turn-to-turn diffing, implementing
//!    `build(...)`.
//! 5. Registering the fragment exactly once in
//!    `REGISTERED_MODEL_VISIBLE_FRAGMENTS` in the rough order it should appear
//!    in model-visible context.
//!
//! The registry drives:
//! - contextual-user history detection
//! - turn-state fragment assembly for both envelopes
//!
//! Fragments that are only emitted as runtime/session-prefix messages should
//! leave `build(...)` as `None`; they still belong here so detection and
//! rendering stay standardized.

use crate::apps::render_apps_section;
use crate::codex::TurnContext;
use crate::exec::ExecToolCallOutput;
use crate::model_visible_context::CHILD_AGENTS_INSTRUCTIONS_CLOSE_TAG;
use crate::model_visible_context::CHILD_AGENTS_INSTRUCTIONS_OPEN_TAG;
use crate::model_visible_context::ContextualUserContextRole;
use crate::model_visible_context::ContextualUserFragmentMarkers;
use crate::model_visible_context::ContextualUserTextFragment;
use crate::model_visible_context::DeveloperContextRole;
use crate::model_visible_context::DeveloperTextFragment;
use crate::model_visible_context::JS_REPL_INSTRUCTIONS_CLOSE_TAG;
use crate::model_visible_context::JS_REPL_INSTRUCTIONS_OPEN_TAG;
use crate::model_visible_context::ModelVisibleContextFragment;
use crate::model_visible_context::ModelVisibleContextRole;
use crate::model_visible_context::SKILL_CLOSE_TAG;
use crate::model_visible_context::SKILL_OPEN_TAG;
use crate::model_visible_context::SUBAGENT_NOTIFICATION_CLOSE_TAG;
use crate::model_visible_context::SUBAGENT_NOTIFICATION_OPEN_TAG;
use crate::model_visible_context::SUBAGENTS_CLOSE_TAG;
use crate::model_visible_context::SUBAGENTS_OPEN_TAG;
use crate::model_visible_context::TURN_ABORTED_CLOSE_TAG;
use crate::model_visible_context::TURN_ABORTED_OPEN_TAG;
use crate::model_visible_context::TurnContextDiffParams;
use crate::model_visible_context::USER_SHELL_COMMAND_CLOSE_TAG;
use crate::model_visible_context::USER_SHELL_COMMAND_OPEN_TAG;
use crate::plugins::render_plugin_instructions;
use crate::project_doc::HIERARCHICAL_AGENTS_MESSAGE;
use crate::project_doc::render_js_repl_instructions;
use crate::shell::Shell;
use crate::skills::render_skills_section;
use crate::tools::format_exec_output_str;
use codex_features::Feature;
use codex_protocol::items::HookPromptItem;
use codex_protocol::items::parse_hook_prompt_fragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::MessageRole;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::developer_collaboration_mode_text;
use codex_protocol::models::developer_model_switch_text;
use codex_protocol::models::developer_permissions_text;
use codex_protocol::models::developer_personality_spec_text;
use codex_protocol::models::developer_realtime_end_text;
use codex_protocol::models::developer_realtime_start_text_with_instructions;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::TurnContextNetworkItem;
use codex_protocol::protocol::USER_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::USER_INSTRUCTIONS_OPEN_TAG;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

pub(crate) enum BuiltTurnStateFragment {
    Developer(DeveloperTextFragment),
    ContextualUser(ContextualUserTextFragment),
}

#[derive(Clone, Copy)]
struct ModelVisibleFragmentRegistration {
    detect_contextual_user: fn(&str) -> bool,
    build_turn_state: fn(
        Option<&TurnContextItem>,
        &TurnContext,
        &TurnContextDiffParams<'_>,
    ) -> Option<BuiltTurnStateFragment>,
}

impl ModelVisibleFragmentRegistration {
    const fn of<F: ModelVisibleContextFragment>() -> Self {
        Self {
            detect_contextual_user: detect_registered_contextual_user_fragment::<F>,
            build_turn_state: build_registered_turn_state_fragment::<F>,
        }
    }
}

fn detect_registered_contextual_user_fragment<F: ModelVisibleContextFragment>(text: &str) -> bool {
    if F::Role::MESSAGE_ROLE != MessageRole::User {
        return false;
    }
    F::matches_contextual_user_text(text)
}

fn build_registered_turn_state_fragment<F: ModelVisibleContextFragment>(
    reference_context_item: Option<&TurnContextItem>,
    turn_context: &TurnContext,
    params: &TurnContextDiffParams<'_>,
) -> Option<BuiltTurnStateFragment> {
    let fragment = F::build(turn_context, reference_context_item, params)?;
    match F::Role::MESSAGE_ROLE {
        MessageRole::Developer => Some(BuiltTurnStateFragment::Developer(
            DeveloperTextFragment::new(fragment.render_text()),
        )),
        MessageRole::User => Some(BuiltTurnStateFragment::ContextualUser(
            ContextualUserTextFragment::new(fragment.render_text()),
        )),
        MessageRole::Assistant | MessageRole::System => None,
    }
}

/// Canonical ordered registry for all current model-visible fragments.
const REGISTERED_MODEL_VISIBLE_FRAGMENTS: &[ModelVisibleFragmentRegistration] = &[
    ModelVisibleFragmentRegistration::of::<ModelInstructionsUpdateFragment>(),
    ModelVisibleFragmentRegistration::of::<PermissionsUpdateFragment>(),
    ModelVisibleFragmentRegistration::of::<CustomDeveloperInstructionsUpdateFragment>(),
    ModelVisibleFragmentRegistration::of::<CollaborationModeUpdateFragment>(),
    ModelVisibleFragmentRegistration::of::<RealtimeUpdateFragment>(),
    ModelVisibleFragmentRegistration::of::<PersonalityUpdateFragment>(),
    ModelVisibleFragmentRegistration::of::<AppsInstructionsFragment>(),
    ModelVisibleFragmentRegistration::of::<SkillsSectionFragment>(),
    ModelVisibleFragmentRegistration::of::<PluginInstructions>(),
    ModelVisibleFragmentRegistration::of::<SubagentRosterContext>(),
    ModelVisibleFragmentRegistration::of::<SubagentNotification>(),
    ModelVisibleFragmentRegistration::of::<UserInstructionsFragment>(),
    ModelVisibleFragmentRegistration::of::<AgentsMdInstructions>(),
    ModelVisibleFragmentRegistration::of::<JsReplInstructionsFragment>(),
    ModelVisibleFragmentRegistration::of::<ChildAgentsInstructionsFragment>(),
    ModelVisibleFragmentRegistration::of::<EnvironmentContext>(),
    ModelVisibleFragmentRegistration::of::<SkillInstructions>(),
    ModelVisibleFragmentRegistration::of::<UserShellCommandFragment>(),
    ModelVisibleFragmentRegistration::of::<TurnAbortedMarker>(),
];

// ---------------------------------------------------------------------------
// Developer-envelope turn-state fragments
// ---------------------------------------------------------------------------

pub(crate) struct ModelInstructionsUpdateFragment {
    text: String,
}

impl ModelVisibleContextFragment for ModelInstructionsUpdateFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }

    fn build(
        turn_context: &TurnContext,
        _reference_context_item: Option<&TurnContextItem>,
        params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        let previous_model = params
            .previous_turn_settings
            .map(|settings| settings.model.as_str())?;
        if previous_model == turn_context.model_info.slug.as_str() {
            return None;
        }

        let model_instructions = turn_context
            .model_info
            .get_model_instructions(turn_context.personality);
        if model_instructions.is_empty() {
            return None;
        }

        Some(Self {
            text: developer_model_switch_text(model_instructions),
        })
    }
}

pub(crate) struct PermissionsUpdateFragment {
    text: String,
}

impl ModelVisibleContextFragment for PermissionsUpdateFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }

    fn build(
        turn_context: &TurnContext,
        reference_context_item: Option<&TurnContextItem>,
        params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if reference_context_item.is_some_and(|previous| {
            previous.sandbox_policy == *turn_context.sandbox_policy.get()
                && previous.approval_policy == turn_context.approval_policy.value()
        }) {
            return None;
        }

        Some(Self {
            text: developer_permissions_text(
                turn_context.sandbox_policy.get(),
                turn_context.approval_policy.value(),
                turn_context.config.approvals_reviewer,
                params.exec_policy,
                &turn_context.cwd,
                turn_context
                    .features
                    .enabled(Feature::ExecPermissionApprovals),
                turn_context
                    .features
                    .enabled(Feature::RequestPermissionsTool),
            ),
        })
    }
}

pub(crate) struct CustomDeveloperInstructionsUpdateFragment {
    text: String,
}

impl ModelVisibleContextFragment for CustomDeveloperInstructionsUpdateFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }

    fn build(
        turn_context: &TurnContext,
        reference_context_item: Option<&TurnContextItem>,
        _params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if reference_context_item.is_some_and(|previous| {
            previous.developer_instructions == turn_context.developer_instructions
        }) {
            return None;
        }

        Some(Self {
            text: turn_context.developer_instructions.as_ref()?.clone(),
        })
    }
}

pub(crate) struct CollaborationModeUpdateFragment {
    text: String,
}

impl ModelVisibleContextFragment for CollaborationModeUpdateFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }

    fn build(
        turn_context: &TurnContext,
        reference_context_item: Option<&TurnContextItem>,
        _params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if let Some(previous) = reference_context_item {
            let previous_text = previous
                .collaboration_mode
                .as_ref()
                .and_then(developer_collaboration_mode_text);
            let current_text = developer_collaboration_mode_text(&turn_context.collaboration_mode);
            if previous_text == current_text {
                return None;
            }

            let text = current_text.unwrap_or_else(|| {
                format!(
                    "<collaboration_mode># Collaboration Mode: {}\n\nYou are now in {} mode. Any previous instructions for other modes are no longer active.</collaboration_mode>",
                    turn_context.collaboration_mode.mode.display_name(),
                    turn_context.collaboration_mode.mode.display_name(),
                )
            });
            return Some(Self { text });
        }

        developer_collaboration_mode_text(&turn_context.collaboration_mode)
            .map(|text| Self { text })
    }
}

pub(crate) struct RealtimeUpdateFragment {
    text: String,
}

impl ModelVisibleContextFragment for RealtimeUpdateFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }

    fn build(
        turn_context: &TurnContext,
        reference_context_item: Option<&TurnContextItem>,
        params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        let text = match (
            reference_context_item.and_then(|previous| previous.realtime_active),
            turn_context.realtime_active,
        ) {
            (Some(true), false) => Some(developer_realtime_end_text("inactive")),
            (Some(false), true) | (None, true) => {
                Some(developer_realtime_start_text_with_instructions(
                    turn_context
                        .config
                        .experimental_realtime_start_instructions
                        .as_deref(),
                ))
            }
            (Some(true), true) | (Some(false), false) => None,
            (None, false) => params
                .previous_turn_settings
                .and_then(|settings| settings.realtime_active)
                .filter(|realtime_active| *realtime_active)
                .map(|_| developer_realtime_end_text("inactive")),
        }?;

        Some(Self { text })
    }
}

pub(crate) struct PersonalityUpdateFragment {
    text: String,
}

impl ModelVisibleContextFragment for PersonalityUpdateFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }

    fn build(
        turn_context: &TurnContext,
        reference_context_item: Option<&TurnContextItem>,
        params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if !params.personality_feature_enabled {
            return None;
        }

        let Some(previous) = reference_context_item else {
            let personality = turn_context.personality?;
            let has_baked_personality = params.base_instructions.is_some_and(|base_instructions| {
                turn_context.model_info.supports_personality()
                    && base_instructions
                        == turn_context
                            .model_info
                            .get_model_instructions(Some(personality))
            });
            if has_baked_personality {
                return None;
            }

            let personality_message = turn_context
                .model_info
                .model_messages
                .as_ref()
                .and_then(|spec| spec.get_personality_message(Some(personality)))
                .filter(|message| !message.is_empty())?;
            return Some(Self {
                text: developer_personality_spec_text(personality_message),
            });
        };

        if turn_context.model_info.slug != previous.model {
            return None;
        }
        if let Some(personality) = turn_context.personality
            && turn_context.personality != previous.personality
        {
            let personality_message = turn_context
                .model_info
                .model_messages
                .as_ref()
                .and_then(|spec| spec.get_personality_message(Some(personality)))
                .filter(|message| !message.is_empty())?;
            return Some(Self {
                text: developer_personality_spec_text(personality_message),
            });
        }

        None
    }
}

pub(crate) struct AppsInstructionsFragment {
    text: String,
}

impl ModelVisibleContextFragment for AppsInstructionsFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }

    fn build(
        turn_context: &TurnContext,
        reference_context_item: Option<&TurnContextItem>,
        _params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if reference_context_item.is_some() || !turn_context.apps_enabled() {
            return None;
        }

        Some(Self {
            text: render_apps_section(),
        })
    }
}

// ---------------------------------------------------------------------------
// Developer runtime fragments
// ---------------------------------------------------------------------------

pub(crate) struct SubagentRosterContext {
    subagents: String,
}

impl SubagentRosterContext {
    pub(crate) fn new(subagents: String) -> Option<Self> {
        if subagents.is_empty() {
            None
        } else {
            Some(Self { subagents })
        }
    }
}

impl ModelVisibleContextFragment for SubagentRosterContext {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        let lines = self
            .subagents
            .lines()
            .map(|line| format!("  {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{SUBAGENTS_OPEN_TAG}\n{lines}\n{SUBAGENTS_CLOSE_TAG}")
    }
}

pub(crate) struct SubagentNotification {
    agent_id: String,
    status: AgentStatus,
}

impl SubagentNotification {
    pub(crate) fn new(agent_id: &str, status: &AgentStatus) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            status: status.clone(),
        }
    }
}

impl ModelVisibleContextFragment for SubagentNotification {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        let payload_json = serde_json::json!({
            "agent_id": self.agent_id,
            "status": self.status,
        })
        .to_string();
        format!(
            "{SUBAGENT_NOTIFICATION_OPEN_TAG}\n{payload_json}\n{SUBAGENT_NOTIFICATION_CLOSE_TAG}"
        )
    }
}

pub(crate) fn format_subagent_context_line(agent_id: &str, agent_nickname: Option<&str>) -> String {
    match agent_nickname.filter(|nickname| !nickname.is_empty()) {
        Some(agent_nickname) => format!("- {agent_id}: {agent_nickname}"),
        None => format!("- {agent_id}"),
    }
}

// ---------------------------------------------------------------------------
// Contextual-user turn-state fragments
// ---------------------------------------------------------------------------

pub(crate) struct UserInstructionsFragment {
    text: String,
}

impl ModelVisibleContextFragment for UserInstructionsFragment {
    type Role = ContextualUserContextRole;

    fn render_text(&self) -> String {
        Self::wrap_contextual_user_body(self.text.clone())
    }

    fn build(
        turn_context: &TurnContext,
        reference_context_item: Option<&TurnContextItem>,
        _params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        let current = Self {
            text: turn_context.user_instructions.clone()?,
        };
        if reference_context_item.and_then(|previous| previous.user_instructions.as_deref())
            == Some(current.text.as_str())
        {
            return None;
        }

        Some(current)
    }

    fn contextual_user_markers() -> Option<ContextualUserFragmentMarkers> {
        Some(ContextualUserFragmentMarkers::new(
            USER_INSTRUCTIONS_OPEN_TAG,
            USER_INSTRUCTIONS_CLOSE_TAG,
        ))
    }
}

const AGENTS_MD_START_MARKER: &str = "# AGENTS.md instructions for ";
const AGENTS_MD_END_MARKER: &str = "</INSTRUCTIONS>";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "user_instructions", rename_all = "snake_case")]
pub(crate) struct AgentsMdInstructions {
    pub directory: String,
    pub text: String,
}

impl ModelVisibleContextFragment for AgentsMdInstructions {
    type Role = ContextualUserContextRole;

    fn render_text(&self) -> String {
        // TODO(ccunningham): Switch AGENTS.md rendering/detection to
        // `<AGENTS.md INSTRUCTIONS FOR {dirname}>...` for consistency with the
        // other contextual-user fragments.
        format!(
            "{AGENTS_MD_START_MARKER}{directory}\n\n<INSTRUCTIONS>\n{contents}\n{AGENTS_MD_END_MARKER}",
            directory = self.directory,
            contents = self.text,
        )
    }

    fn build(
        turn_context: &TurnContext,
        reference_context_item: Option<&TurnContextItem>,
        _params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        let current_directory = turn_context.cwd.to_string_lossy().into_owned();
        let current_text = turn_context.project_doc_instructions.as_deref();
        if let Some(previous) = reference_context_item {
            let previous_directory = previous.cwd.to_string_lossy().into_owned();
            if previous_directory == current_directory
                && previous.project_doc_instructions.as_deref() == current_text
            {
                return None;
            }

            if current_text.is_none() && previous.project_doc_instructions.is_some() {
                return Some(Self {
                    directory: current_directory,
                    text: String::new(),
                });
            }
        }

        Some(Self {
            directory: current_directory,
            text: current_text?.to_string(),
        })
    }

    fn matches_contextual_user_text(text: &str) -> bool {
        let trimmed = text.trim_start();
        // TODO(ccunningham): Switch detection to the XML-ish wrapper once we
        // intentionally change the shipped AGENTS.md fragment format.
        trimmed.starts_with(AGENTS_MD_START_MARKER)
            && trimmed.trim_end().ends_with(AGENTS_MD_END_MARKER)
    }
}

pub(crate) struct JsReplInstructionsFragment {
    text: String,
}

impl ModelVisibleContextFragment for JsReplInstructionsFragment {
    type Role = ContextualUserContextRole;

    fn render_text(&self) -> String {
        Self::wrap_contextual_user_body(self.text.clone())
    }

    fn build(
        turn_context: &TurnContext,
        reference_context_item: Option<&TurnContextItem>,
        _params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if reference_context_item.is_some() {
            return None;
        }

        Some(Self {
            text: render_js_repl_instructions(&turn_context.config)?,
        })
    }

    fn contextual_user_markers() -> Option<ContextualUserFragmentMarkers> {
        Some(ContextualUserFragmentMarkers::new(
            JS_REPL_INSTRUCTIONS_OPEN_TAG,
            JS_REPL_INSTRUCTIONS_CLOSE_TAG,
        ))
    }
}

pub(crate) struct SkillsSectionFragment {
    text: String,
}

impl ModelVisibleContextFragment for SkillsSectionFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }

    fn build(
        turn_context: &TurnContext,
        reference_context_item: Option<&TurnContextItem>,
        _params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if reference_context_item.is_some() {
            return None;
        }

        let skills = turn_context
            .turn_skills
            .outcome
            .allowed_skills_for_implicit_invocation();
        Some(Self {
            text: render_skills_section(&skills)?,
        })
    }
}

pub(crate) struct ChildAgentsInstructionsFragment;

impl ModelVisibleContextFragment for ChildAgentsInstructionsFragment {
    type Role = ContextualUserContextRole;

    fn render_text(&self) -> String {
        Self::wrap_contextual_user_body(HIERARCHICAL_AGENTS_MESSAGE.to_string())
    }

    fn build(
        turn_context: &TurnContext,
        reference_context_item: Option<&TurnContextItem>,
        _params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if reference_context_item.is_some()
            || !turn_context.features.enabled(Feature::ChildAgentsMd)
        {
            return None;
        }

        Some(Self)
    }

    fn contextual_user_markers() -> Option<ContextualUserFragmentMarkers> {
        Some(ContextualUserFragmentMarkers::new(
            CHILD_AGENTS_INSTRUCTIONS_OPEN_TAG,
            CHILD_AGENTS_INSTRUCTIONS_CLOSE_TAG,
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "environment_context", rename_all = "snake_case")]
pub(crate) struct EnvironmentContext {
    pub cwd: Option<PathBuf>,
    pub shell: Shell,
    pub current_date: Option<String>,
    pub timezone: Option<String>,
    pub network: Option<NetworkContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub(crate) struct NetworkContext {
    allowed_domains: Vec<String>,
    denied_domains: Vec<String>,
}

impl EnvironmentContext {
    const MARKERS: ContextualUserFragmentMarkers = ContextualUserFragmentMarkers::new(
        ENVIRONMENT_CONTEXT_OPEN_TAG,
        ENVIRONMENT_CONTEXT_CLOSE_TAG,
    );

    pub(crate) fn new(
        cwd: Option<PathBuf>,
        shell: Shell,
        current_date: Option<String>,
        timezone: Option<String>,
        network: Option<NetworkContext>,
    ) -> Self {
        Self {
            cwd,
            shell,
            current_date,
            timezone,
            network,
        }
    }

    pub(crate) fn equals_except_shell(&self, other: &EnvironmentContext) -> bool {
        let EnvironmentContext {
            cwd,
            current_date,
            timezone,
            network,
            shell: _,
        } = other;
        self.cwd == *cwd
            && self.current_date == *current_date
            && self.timezone == *timezone
            && self.network == *network
    }

    fn network_from_turn_context(turn_context: &TurnContext) -> Option<NetworkContext> {
        let network = turn_context
            .config
            .config_layer_stack
            .requirements()
            .network
            .as_ref()?;

        Some(NetworkContext {
            allowed_domains: network.allowed_domains.clone().unwrap_or_default(),
            denied_domains: network.denied_domains.clone().unwrap_or_default(),
        })
    }

    fn network_from_turn_context_item(
        turn_context_item: &TurnContextItem,
    ) -> Option<NetworkContext> {
        let TurnContextNetworkItem {
            allowed_domains,
            denied_domains,
        } = turn_context_item.network.as_ref()?;
        Some(NetworkContext {
            allowed_domains: allowed_domains.clone(),
            denied_domains: denied_domains.clone(),
        })
    }
}

impl ModelVisibleContextFragment for EnvironmentContext {
    type Role = ContextualUserContextRole;

    fn render_text(&self) -> String {
        let mut lines = Vec::new();
        if let Some(cwd) = &self.cwd {
            lines.push(format!("  <cwd>{}</cwd>", cwd.to_string_lossy()));
        }

        let shell_name = self.shell.name();
        lines.push(format!("  <shell>{shell_name}</shell>"));
        if let Some(current_date) = &self.current_date {
            lines.push(format!("  <current_date>{current_date}</current_date>"));
        }
        if let Some(timezone) = &self.timezone {
            lines.push(format!("  <timezone>{timezone}</timezone>"));
        }
        if let Some(network) = &self.network {
            lines.push("  <network enabled=\"true\">".to_string());
            for allowed in &network.allowed_domains {
                lines.push(format!("    <allowed>{allowed}</allowed>"));
            }
            for denied in &network.denied_domains {
                lines.push(format!("    <denied>{denied}</denied>"));
            }
            lines.push("  </network>".to_string());
        }
        Self::MARKERS.wrap_body(lines.join("\n"))
    }

    fn build(
        turn_context: &TurnContext,
        reference_context_item: Option<&TurnContextItem>,
        params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        let current_network = Self::network_from_turn_context(turn_context);
        let current_context = Self::new(
            Some(turn_context.cwd.clone()),
            params.shell.clone(),
            turn_context.current_date.clone(),
            turn_context.timezone.clone(),
            current_network.clone(),
        );

        let Some(previous) = reference_context_item else {
            return Some(current_context);
        };

        let previous_network = Self::network_from_turn_context_item(previous);
        let previous_context = Self::new(
            Some(previous.cwd.clone()),
            params.shell.clone(),
            previous.current_date.clone(),
            previous.timezone.clone(),
            previous_network.clone(),
        );

        if previous_context.equals_except_shell(&current_context) {
            return None;
        }

        let cwd = if previous.cwd != turn_context.cwd {
            Some(turn_context.cwd.clone())
        } else {
            None
        };
        let network = if previous_network != current_network {
            current_network
        } else {
            previous_network
        };

        Some(Self::new(
            cwd,
            params.shell.clone(),
            turn_context.current_date.clone(),
            turn_context.timezone.clone(),
            network,
        ))
    }

    fn contextual_user_markers() -> Option<ContextualUserFragmentMarkers> {
        Some(Self::MARKERS)
    }
}

// ---------------------------------------------------------------------------
// Contextual-user runtime fragments
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "skill_instructions", rename_all = "snake_case")]
pub(crate) struct SkillInstructions {
    pub name: String,
    pub path: String,
    pub contents: String,
}

impl ModelVisibleContextFragment for SkillInstructions {
    type Role = ContextualUserContextRole;

    fn render_text(&self) -> String {
        Self::wrap_contextual_user_body(format!(
            "<name>{}</name>\n<path>{}</path>\n{}",
            self.name, self.path, self.contents
        ))
    }

    fn contextual_user_markers() -> Option<ContextualUserFragmentMarkers> {
        Some(ContextualUserFragmentMarkers::new(
            SKILL_OPEN_TAG,
            SKILL_CLOSE_TAG,
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "plugin_instructions", rename_all = "snake_case")]
pub(crate) struct PluginInstructions {
    pub text: String,
}

impl ModelVisibleContextFragment for PluginInstructions {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }

    fn build(
        _turn_context: &TurnContext,
        reference_context_item: Option<&TurnContextItem>,
        params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if reference_context_item.is_some() {
            return None;
        }

        render_plugin_instructions(params.plugin_capability_summaries?)
    }
}

pub(crate) struct UserShellCommandFragment {
    text: String,
}

impl UserShellCommandFragment {
    pub(crate) fn from_exec_output(
        command: &str,
        exec_output: &ExecToolCallOutput,
        turn_context: &TurnContext,
    ) -> Self {
        let mut sections = Vec::new();
        sections.push("<command>".to_string());
        sections.push(command.to_string());
        sections.push("</command>".to_string());
        sections.push("<result>".to_string());
        sections.push(format!("Exit code: {}", exec_output.exit_code));
        sections.push(format_duration_line(exec_output.duration));
        sections.push("Output:".to_string());
        sections.push(format_exec_output_str(
            exec_output,
            turn_context.truncation_policy,
        ));
        sections.push("</result>".to_string());

        Self {
            text: Self::wrap_contextual_user_body(sections.join("\n")),
        }
    }
}

impl ModelVisibleContextFragment for UserShellCommandFragment {
    type Role = ContextualUserContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }

    fn contextual_user_markers() -> Option<ContextualUserFragmentMarkers> {
        Some(ContextualUserFragmentMarkers::new(
            USER_SHELL_COMMAND_OPEN_TAG,
            USER_SHELL_COMMAND_CLOSE_TAG,
        ))
    }
}

pub(crate) struct TurnAbortedMarker {
    guidance: &'static str,
}

impl TurnAbortedMarker {
    pub(crate) fn interrupted() -> Self {
        Self {
            guidance: "The user interrupted the previous turn on purpose. Any running unified exec processes were terminated. If any tools/commands were aborted, they may have partially executed; verify current state before retrying.",
        }
    }
}

impl ModelVisibleContextFragment for TurnAbortedMarker {
    type Role = ContextualUserContextRole;

    fn render_text(&self) -> String {
        Self::wrap_contextual_user_body(self.guidance.to_string())
    }

    fn contextual_user_markers() -> Option<ContextualUserFragmentMarkers> {
        Some(ContextualUserFragmentMarkers::new(
            TURN_ABORTED_OPEN_TAG,
            TURN_ABORTED_CLOSE_TAG,
        ))
    }
}

fn format_duration_line(duration: Duration) -> String {
    let duration_seconds = duration.as_secs_f64();
    format!("Duration: {duration_seconds:.4} seconds")
}

#[cfg(test)]
pub(crate) fn format_user_shell_command_record(
    command: &str,
    exec_output: &ExecToolCallOutput,
    turn_context: &TurnContext,
) -> String {
    UserShellCommandFragment::from_exec_output(command, exec_output, turn_context).render_text()
}

pub(crate) fn user_shell_command_record_item(
    command: &str,
    exec_output: &ExecToolCallOutput,
    turn_context: &TurnContext,
) -> ResponseItem {
    UserShellCommandFragment::from_exec_output(command, exec_output, turn_context).into_message()
}

// ---------------------------------------------------------------------------
// Shared fragment assembly and detection
// ---------------------------------------------------------------------------

fn is_legacy_contextual_user_fragment(text: &str) -> bool {
    // TODO(ccunningham): Drop this once old user-role subagent notification
    // history no longer needs resume/compaction compatibility.
    ContextualUserFragmentMarkers::new(
        SUBAGENT_NOTIFICATION_OPEN_TAG,
        SUBAGENT_NOTIFICATION_CLOSE_TAG,
    )
    .matches_text(text)
}

/// Returns whether a contextual-user fragment should be omitted from memory
/// stage-1 inputs.
///
/// We exclude injected `AGENTS.md` instructions and skill payloads because
/// they are prompt scaffolding rather than conversation content, so they do
/// not improve the resulting memory. We keep environment context and
/// subagent notifications because they can carry useful execution context or
/// subtask outcomes that should remain visible to memory generation.
pub(crate) fn is_memory_excluded_contextual_user_fragment(content_item: &ContentItem) -> bool {
    let ContentItem::InputText { text } = content_item else {
        return false;
    };

    AgentsMdInstructions::matches_contextual_user_text(text)
        || SkillInstructions::matches_contextual_user_text(text)
}

pub(crate) fn is_contextual_user_fragment(content_item: &ContentItem) -> bool {
    let ContentItem::InputText { text } = content_item else {
        return false;
    };

    parse_hook_prompt_fragment(text).is_some()
        || REGISTERED_MODEL_VISIBLE_FRAGMENTS
            .iter()
            .any(|registration| (registration.detect_contextual_user)(text))
        || is_legacy_contextual_user_fragment(text)
}

pub(crate) fn parse_visible_hook_prompt_message(
    id: Option<&String>,
    content: &[ContentItem],
) -> Option<HookPromptItem> {
    let mut fragments = Vec::new();

    for content_item in content {
        let ContentItem::InputText { text } = content_item else {
            return None;
        };
        if let Some(fragment) = parse_hook_prompt_fragment(text) {
            fragments.push(fragment);
            continue;
        }
        if is_contextual_user_fragment(content_item) {
            continue;
        }
        return None;
    }

    if fragments.is_empty() {
        return None;
    }

    Some(HookPromptItem::from_fragments(id, fragments))
}

pub(crate) fn build_turn_state_fragments(
    reference_context_item: Option<&TurnContextItem>,
    turn_context: &TurnContext,
    params: &TurnContextDiffParams<'_>,
) -> Vec<BuiltTurnStateFragment> {
    REGISTERED_MODEL_VISIBLE_FRAGMENTS
        .iter()
        .filter_map(|registration| {
            (registration.build_turn_state)(reference_context_item, turn_context, params)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "environment_context_tests.rs"]
mod environment_context_tests;

#[cfg(test)]
#[path = "user_shell_command_tests.rs"]
mod user_shell_command_tests;

#[cfg(test)]
#[path = "contextual_user_message_tests.rs"]
mod contextual_user_message_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ContentItem;
    use pretty_assertions::assert_eq;

    #[test]
    fn detects_environment_context_fragment() {
        assert!(is_contextual_user_fragment(&ContentItem::InputText {
            text: "<environment_context>\n<cwd>/tmp</cwd>\n</environment_context>".to_string(),
        }));
    }

    #[test]
    fn detects_agents_instructions_fragment() {
        assert!(is_contextual_user_fragment(&ContentItem::InputText {
            text: "# AGENTS.md instructions for /tmp\n\n<INSTRUCTIONS>\nbody\n</INSTRUCTIONS>"
                .to_string(),
        }));
    }

    #[test]
    fn detects_user_instructions_fragment() {
        assert!(is_contextual_user_fragment(&ContentItem::InputText {
            text: "<user_instructions>\ncustom guidance\n</user_instructions>".to_string(),
        }));
    }

    #[test]
    fn detects_legacy_subagent_notification_fragment() {
        assert!(is_contextual_user_fragment(&ContentItem::InputText {
            text: "<subagent_notification>\n{\"agent_id\":\"a\",\"status\":\"completed\"}\n</subagent_notification>"
                .to_string(),
        }));
    }

    #[test]
    fn ignores_regular_user_text() {
        assert!(!is_contextual_user_fragment(&ContentItem::InputText {
            text: "hello".to_string(),
        }));
    }

    #[test]
    fn marker_matching_ignores_plain_text() {
        assert!(!SkillInstructions::matches_contextual_user_text(
            "plain text"
        ));
    }

    #[test]
    fn serializes_subagent_roster_context() {
        let context =
            SubagentRosterContext::new("- agent-1: Atlas\n- agent-2: Juniper".to_string())
                .expect("context expected");

        assert_eq!(
            context.render_text(),
            "<subagents>\n  - agent-1: Atlas\n  - agent-2: Juniper\n</subagents>"
        );
    }

    #[test]
    fn skips_empty_subagent_roster_context() {
        assert!(SubagentRosterContext::new(String::new()).is_none());
    }
}
