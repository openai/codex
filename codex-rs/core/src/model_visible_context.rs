//! Shared model-visible context abstractions.
//!
//! Use this path for any injected prompt context, whether it renders in the
//! developer envelope or the contextual-user envelope.
//!
//! Fragment registration and concrete fragment definitions live in
//! `model_visible_fragments.rs`. This module keeps only the shared rendering,
//! role, and turn-context parameter helpers that every fragment uses.
//!
//! Contributor guide:
//!
//! - If the model should not see the data, do not add a fragment.
//! - If it should, define a typed fragment in `model_visible_fragments.rs`,
//!   implement `ModelVisibleContextFragment`, and register it exactly once in
//!   the central registry there. Registration is what enables shared
//!   contextual-user detection and registry-driven turn-state assembly.
//! - Choose the role intentionally:
//!   - `DeveloperContextRole` for developer guidance/policy
//!   - `ContextualUserContextRole` for contextual user-role state that must be
//!     parsed as context rather than literal user intent
//! - If the fragment is durable turn/session state that should rebuild across
//!   resume, compaction, backtracking, or fork, implement `build(...)`.
//!   `reference_context_item` is the baseline already represented in
//!   model-visible history; compare against it to avoid duplicates, and use
//!   `TurnContextDiffParams` for other runtime/session inputs such as
//!   `previous_turn_settings`.
//! - If the fragment is a runtime/session-prefix marker rather than turn-state
//!   context, leave `build(...)` as `None`.
//! - Contextual-user fragments must have stable detection. Prefer
//!   `contextual_user_markers()`; override `matches_contextual_user_text()`
//!   only when matching is genuinely custom.
//! - Keep the turn-state two-envelope invariant intact: turn-state developer
//!   fragments are grouped into one developer message, and turn-state
//!   contextual-user fragments are grouped into one contextual-user message.
//!   Runtime/session-prefix fragments may still be emitted as standalone
//!   messages.
//! - Keep logic fragment-local. The fragment type should own rendering,
//!   state/diff inspection, and contextual-user detection when applicable.
//! - Keep legacy compatibility bounded: if old shipped history needs special
//!   detection for a wrapper we no longer emit, add a small shim in the
//!   detection path rather than inventing a fake current fragment type.

use crate::codex::PreviousTurnSettings;
use crate::codex::TurnContext;
use crate::plugins::PluginCapabilitySummary;
use crate::shell::Shell;
use codex_execpolicy::Policy;
use codex_protocol::models::ContentItem;
use codex_protocol::models::CustomDeveloperInstructions;
use codex_protocol::models::MessageRole;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TurnContextItem;

pub(crate) const SKILL_OPEN_TAG: &str = "<skill>";
pub(crate) const SKILL_CLOSE_TAG: &str = "</skill>";
pub(crate) const JS_REPL_INSTRUCTIONS_OPEN_TAG: &str = "<js_repl_instructions>";
pub(crate) const JS_REPL_INSTRUCTIONS_CLOSE_TAG: &str = "</js_repl_instructions>";
pub(crate) const CHILD_AGENTS_INSTRUCTIONS_OPEN_TAG: &str = "<child_agents_instructions>";
pub(crate) const CHILD_AGENTS_INSTRUCTIONS_CLOSE_TAG: &str = "</child_agents_instructions>";
pub(crate) const USER_SHELL_COMMAND_OPEN_TAG: &str = "<user_shell_command>";
pub(crate) const USER_SHELL_COMMAND_CLOSE_TAG: &str = "</user_shell_command>";
pub(crate) const TURN_ABORTED_OPEN_TAG: &str = "<turn_aborted>";
pub(crate) const TURN_ABORTED_CLOSE_TAG: &str = "</turn_aborted>";
pub(crate) const SUBAGENTS_OPEN_TAG: &str = "<subagents>";
pub(crate) const SUBAGENTS_CLOSE_TAG: &str = "</subagents>";
pub(crate) const SUBAGENT_NOTIFICATION_OPEN_TAG: &str = "<subagent_notification>";
pub(crate) const SUBAGENT_NOTIFICATION_CLOSE_TAG: &str = "</subagent_notification>";

pub(crate) trait ModelVisibleContextRole {
    const MESSAGE_ROLE: MessageRole;
}

pub(crate) struct DeveloperContextRole;

impl ModelVisibleContextRole for DeveloperContextRole {
    const MESSAGE_ROLE: MessageRole = MessageRole::Developer;
}

pub(crate) struct ContextualUserContextRole;

impl ModelVisibleContextRole for ContextualUserContextRole {
    const MESSAGE_ROLE: MessageRole = MessageRole::User;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContextualUserFragmentMarkers {
    start_marker: &'static str,
    end_marker: &'static str,
}

impl ContextualUserFragmentMarkers {
    pub(crate) const fn new(start_marker: &'static str, end_marker: &'static str) -> Self {
        Self {
            start_marker,
            end_marker,
        }
    }

    pub(crate) fn matches_text(self, text: &str) -> bool {
        let trimmed = text.trim_start();
        let starts_with_marker = trimmed
            .get(..self.start_marker.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(self.start_marker));
        let trimmed = trimmed.trim_end();
        let ends_with_marker = trimmed
            .get(trimmed.len().saturating_sub(self.end_marker.len())..)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(self.end_marker));
        starts_with_marker && ends_with_marker
    }

    pub(crate) fn wrap_body(self, body: String) -> String {
        format!("{}\n{}\n{}", self.start_marker, body, self.end_marker)
    }
}

pub(crate) fn model_visible_content_item(text: String) -> ContentItem {
    ContentItem::InputText { text }
}

pub(crate) fn model_visible_message<R: ModelVisibleContextRole>(text: String) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: R::MESSAGE_ROLE.to_string(),
        content: vec![model_visible_content_item(text)],
        end_turn: None,
        phase: None,
    }
}

pub(crate) fn model_visible_response_input_item<R: ModelVisibleContextRole>(
    text: String,
) -> ResponseInputItem {
    ResponseInputItem::Message {
        role: R::MESSAGE_ROLE.to_string(),
        content: vec![model_visible_content_item(text)],
    }
}

pub(crate) struct TurnContextDiffParams<'a> {
    pub(crate) shell: &'a Shell,
    pub(crate) previous_turn_settings: Option<&'a PreviousTurnSettings>,
    pub(crate) exec_policy: &'a Policy,
    pub(crate) personality_feature_enabled: bool,
    pub(crate) base_instructions: Option<&'a str>,
    pub(crate) plugin_capability_summaries: Option<&'a [PluginCapabilitySummary]>,
}

impl<'a> TurnContextDiffParams<'a> {
    pub(crate) fn new(
        shell: &'a Shell,
        previous_turn_settings: Option<&'a PreviousTurnSettings>,
        exec_policy: &'a Policy,
        personality_feature_enabled: bool,
        base_instructions: Option<&'a str>,
        plugin_capability_summaries: Option<&'a [PluginCapabilitySummary]>,
    ) -> Self {
        Self {
            shell,
            previous_turn_settings,
            exec_policy,
            personality_feature_enabled,
            base_instructions,
            plugin_capability_summaries,
        }
    }
}

/// Implement this for any model-visible prompt fragment, regardless of which
/// envelope it renders into.
pub(crate) trait ModelVisibleContextFragment: Sized {
    type Role: ModelVisibleContextRole;

    fn render_text(&self) -> String;

    /// Build the fragment from the current turn state and an optional baseline
    /// context item that represents the turn state already reflected in
    /// model-visible history.
    ///
    /// Implementations that are not turn-state fragments should leave the
    /// default `None`.
    fn build(
        _turn_context: &TurnContext,
        _reference_context_item: Option<&TurnContextItem>,
        _params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        None
    }

    /// Stable markers used to recognize contextual-user fragments in persisted
    /// history. Developer fragments should keep the default `None`.
    fn contextual_user_markers() -> Option<ContextualUserFragmentMarkers> {
        None
    }

    fn matches_contextual_user_text(text: &str) -> bool {
        Self::contextual_user_markers().is_some_and(|markers| markers.matches_text(text))
    }

    fn wrap_contextual_user_body(body: String) -> String {
        let Some(markers) = Self::contextual_user_markers() else {
            panic!("contextual-user fragments using wrap_contextual_user_body must define markers");
        };
        markers.wrap_body(body)
    }

    fn into_content_item(self) -> ContentItem {
        model_visible_content_item(self.render_text())
    }

    fn into_message(self) -> ResponseItem {
        model_visible_message::<Self::Role>(self.render_text())
    }

    fn into_response_input_item(self) -> ResponseInputItem {
        model_visible_response_input_item::<Self::Role>(self.render_text())
    }
}

pub(crate) struct DeveloperTextFragment {
    text: String,
}

impl DeveloperTextFragment {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

pub(crate) struct ContextualUserTextFragment {
    text: String,
}

impl ContextualUserTextFragment {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl ModelVisibleContextFragment for CustomDeveloperInstructions {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.clone().into_text()
    }
}

impl ModelVisibleContextFragment for DeveloperTextFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }
}

impl ModelVisibleContextFragment for ContextualUserTextFragment {
    type Role = ContextualUserContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }
}
