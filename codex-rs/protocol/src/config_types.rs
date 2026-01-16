use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display;
use ts_rs::TS;

use crate::openai_models::ReasoningEffort;

/// A summary of the reasoning performed by the model. This can be useful for
/// debugging and understanding the model's reasoning process.
/// See https://platform.openai.com/docs/guides/reasoning?api-mode=responses#reasoning-summaries
#[derive(
    Debug, Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, Display, JsonSchema, TS,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ReasoningSummary {
    #[default]
    Auto,
    Concise,
    Detailed,
    /// Option to disable reasoning summaries.
    None,
}

/// Controls output length/detail on GPT-5 models via the Responses API.
/// Serialized with lowercase values to match the OpenAI API.
#[derive(
    Hash,
    Debug,
    Serialize,
    Deserialize,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Display,
    JsonSchema,
    TS,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum Verbosity {
    Low,
    #[default]
    Medium,
    High,
}

#[derive(
    Deserialize, Debug, Clone, Copy, PartialEq, Default, Serialize, Display, JsonSchema, TS,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SandboxMode {
    #[serde(rename = "read-only")]
    #[default]
    ReadOnly,

    #[serde(rename = "workspace-write")]
    WorkspaceWrite,

    #[serde(rename = "danger-full-access")]
    DangerFullAccess,
}

#[derive(
    Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, JsonSchema, TS, Default,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum WebSearchMode {
    #[default]
    Disabled,
    Cached,
    Live,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ForcedLoginMethod {
    Chatgpt,
    Api,
}

/// Represents the trust level for a project directory.
/// This determines the approval policy and sandbox mode applied.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum TrustLevel {
    Trusted,
    Untrusted,
}

/// Controls whether the TUI uses the terminal's alternate screen buffer.
///
/// **Background:** The alternate screen buffer provides a cleaner fullscreen experience
/// without polluting the terminal's scrollback history. However, it conflicts with terminal
/// multiplexers like Zellij that strictly follow the xterm specification, which defines
/// that alternate screen buffers should not have scrollback.
///
/// **Zellij's behavior:** Zellij intentionally disables scrollback in alternate screen mode
/// (see https://github.com/zellij-org/zellij/pull/1032) to comply with the xterm spec. This
/// is by design and not configurable in Zellij—there is no option to enable scrollback in
/// alternate screen mode.
///
/// **Solution:** This setting provides a pragmatic workaround:
/// - `auto` (default): Automatically detect the terminal multiplexer. If running in Zellij,
///   disable alternate screen to preserve scrollback. Enable it everywhere else.
/// - `always`: Always use alternate screen mode (original behavior before this fix).
/// - `never`: Never use alternate screen mode. Runs in inline mode, preserving scrollback
///   in all multiplexers.
///
/// The CLI flag `--no-alt-screen` can override this setting at runtime.
#[derive(
    Debug, Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, Display, JsonSchema, TS,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum AltScreenMode {
    /// Auto-detect: disable alternate screen in Zellij, enable elsewhere.
    #[default]
    Auto,
    /// Always use alternate screen (original behavior).
    Always,
    /// Never use alternate screen (inline mode only).
    Never,
}

/// Collaboration mode for a Codex session.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum CollaborationMode {
    Plan(CollaborationModeSettings),
    Collaborate(CollaborationModeSettings),
    Execute(CollaborationModeSettings),
}

impl CollaborationMode {
    pub fn model(&self) -> &str {
        match self {
            CollaborationMode::Plan(settings) => &settings.model,
            CollaborationMode::Collaborate(settings) => &settings.model,
            CollaborationMode::Execute(settings) => &settings.model,
        }
    }

    pub fn reasoning_effort(&self) -> Option<ReasoningEffort> {
        match self {
            CollaborationMode::Plan(settings) => settings.reasoning_effort,
            CollaborationMode::Collaborate(settings) => settings.reasoning_effort,
            CollaborationMode::Execute(settings) => settings.reasoning_effort,
        }
    }

    /// Updates the collaboration mode with new model and/or effort values.
    ///
    /// - `model`: `Some(s)` to update the model, `None` to keep the current model
    /// - `effort`: `Some(Some(e))` to set effort to `e`, `Some(None)` to clear effort, `None` to keep current effort
    ///
    /// Returns `None` if neither model nor effort is provided, or if the provided values
    /// are the same as the current values (no change needed).
    /// Otherwise returns a new `CollaborationMode` with updated values, preserving the variant.
    pub fn with_updates(
        &self,
        model: Option<String>,
        effort: Option<Option<ReasoningEffort>>,
    ) -> Option<Self> {
        let current_settings = match self {
            CollaborationMode::Plan(settings) => settings,
            CollaborationMode::Collaborate(settings) => settings,
            CollaborationMode::Execute(settings) => settings,
        };

        let updated_model = model.as_ref().unwrap_or(&current_settings.model);
        let updated_effort = effort.unwrap_or(current_settings.reasoning_effort);

        // Check if there's actually a change
        let model_changed = model.is_some() && updated_model != &current_settings.model;
        let effort_changed =
            effort.is_some() && updated_effort != current_settings.reasoning_effort;

        if !model_changed && !effort_changed {
            return None;
        }

        let updated_settings = CollaborationModeSettings {
            model: updated_model.clone(),
            reasoning_effort: updated_effort,
        };

        Some(match self {
            CollaborationMode::Plan(_) => CollaborationMode::Plan(updated_settings),
            CollaborationMode::Collaborate(_) => CollaborationMode::Collaborate(updated_settings),
            CollaborationMode::Execute(_) => CollaborationMode::Execute(updated_settings),
        })
    }
}

/// Settings for a collaboration mode.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct CollaborationModeSettings {
    pub model: String,
    pub reasoning_effort: Option<ReasoningEffort>,
}
