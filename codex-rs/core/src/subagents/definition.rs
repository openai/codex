use crate::openai_tools::JsonSchema;
use codex_protocol::config_types::ReasoningEffort;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubagentSource {
    #[default]
    EmbeddedDefault,
    User,
    Project,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubagentDefinition {
    pub name: String,
    pub description: String,
    /// Base instructions for this subagent.
    pub instructions: String,

    // TODO: add allowed tools. we inherit the parent agent's tools for now.
    // TODO: add cwd and approval policies.
    /// Structured output schema. The subagent must return a single JSON value
    /// that validates against this schema. The schema will be embedded into the
    /// subagent's instructions so the model can adhere to it.
    output_schema: JsonSchema,

    /// Optional model override for this subagent. When not provided, inherits
    /// the parent session's configured model.
    #[serde(default)]
    pub model: Option<String>,

    /// Optional reasoning effort override for this subagent. When not provided,
    /// inherits the parent session's configured reasoning effort.
    #[serde(default)]
    pub reasoning_effort: Option<ReasoningEffort>,

    /// Where this definition was loaded from; used for precedence rules and
    /// behavior differences (e.g., instruction composition).
    /// Not serialized; defaults to EmbeddedDefault.
    #[serde(skip)]
    pub(crate) source: SubagentSource,
}

impl SubagentDefinition {
    pub fn from_json_str(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str::<Self>(s)
    }

    pub fn from_file(path: &Path) -> std::io::Result<Self> {
        let contents = fs::read_to_string(path)?;
        // Surface JSON parsing error with file context
        serde_json::from_str::<Self>(&contents).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid subagent JSON at {}: {e}", path.display()),
            )
        })
    }

    pub(crate) fn output_schema(&self) -> &JsonSchema {
        &self.output_schema
    }
}
