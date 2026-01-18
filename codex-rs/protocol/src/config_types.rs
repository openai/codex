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
    Plan(Settings),
    PairProgramming(Settings),
    Execute(Settings),
    Custom(Settings),
}

impl CollaborationMode {
    /// Returns a reference to the settings, regardless of variant.
    fn settings(&self) -> &Settings {
        match self {
            CollaborationMode::Plan(settings)
            | CollaborationMode::PairProgramming(settings)
            | CollaborationMode::Execute(settings)
            | CollaborationMode::Custom(settings) => settings,
        }
    }

    pub fn model(&self) -> &str {
        self.settings().model.as_str()
    }

    pub fn reasoning_effort(&self) -> Option<ReasoningEffort> {
        self.settings().reasoning_effort
    }

    /// Updates the collaboration mode with new model and/or effort values.
    ///
    /// - `model`: `Some(s)` to update the model, `None` to keep the current model
    /// - `effort`: `Some(Some(e))` to set effort to `e`, `Some(None)` to clear effort, `None` to keep current effort
    /// - `developer_instructions`: `Some(s)` to update developer instructions, `None` to keep current
    ///
    /// Returns a new `CollaborationMode` with updated values, preserving the variant.
    pub fn with_updates(
        &self,
        model: Option<String>,
        effort: Option<Option<ReasoningEffort>>,
        developer_instructions: Option<String>,
    ) -> Self {
        let settings = self.settings();
        let updated_settings = Settings {
            model: model.unwrap_or_else(|| settings.model.clone()),
            reasoning_effort: effort.unwrap_or(settings.reasoning_effort),
            developer_instructions: developer_instructions
                .or_else(|| settings.developer_instructions.clone()),
        };

        match self {
            CollaborationMode::Plan(_) => CollaborationMode::Plan(updated_settings),
            CollaborationMode::PairProgramming(_) => {
                CollaborationMode::PairProgramming(updated_settings)
            }
            CollaborationMode::Execute(_) => CollaborationMode::Execute(updated_settings),
            CollaborationMode::Custom(_) => CollaborationMode::Custom(updated_settings),
        }
    }
}

/// Settings for a collaboration mode.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct Settings {
    pub model: String,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub developer_instructions: Option<String>,
}
