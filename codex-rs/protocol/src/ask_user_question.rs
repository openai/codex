use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AskUserQuestionKind {
    SingleChoice,
    MultipleChoice,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct AskUserQuestionOption {
    /// Placeholder label for a custom option when `custom` is true.
    pub label: String,
    /// One short sentence explaining impact/tradeoff if selected.
    pub description: String,
    /// When true, the user provides the option label text.
    #[serde(default)]
    pub custom: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct AskUserQuestionQuestion {
    pub id: String,
    /// Short header label shown in the UI tab (ideally <= ~12 chars).
    pub header: String,
    /// Prompt shown to the user.
    pub question: String,
    pub kind: AskUserQuestionKind,
    pub options: Vec<AskUserQuestionOption>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct AskUserQuestionArgs {
    pub questions: Vec<AskUserQuestionQuestion>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct AskUserQuestionEvent {
    /// Responses API call id for the associated tool call, if available.
    pub call_id: String,
    /// Turn ID that this request belongs to.
    /// Uses `#[serde(default)]` for backwards compatibility.
    #[serde(default)]
    pub turn_id: String,
    pub questions: Vec<AskUserQuestionQuestion>,
}
