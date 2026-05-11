use codex_protocol::ToolName;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::JsonSchema;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FunctionToolSpec {
    pub name: String,
    pub description: String,
    pub strict: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
    pub parameters: JsonSchema,
    #[serde(skip)]
    pub output_schema: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreeformToolSpec {
    pub name: String,
    pub description: String,
    pub format: FreeformToolFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreeformToolFormat {
    pub r#type: String,
    pub syntax: String,
    pub definition: String,
}

/// Leaf tool specs that can be owned and executed by one contributed bundle.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum ExecutableToolSpec {
    #[serde(rename = "function")]
    Function(FunctionToolSpec),
    #[serde(rename = "custom")]
    Freeform(FreeformToolSpec),
}

impl ExecutableToolSpec {
    pub fn name(&self) -> &str {
        match self {
            Self::Function(tool) => tool.name.as_str(),
            Self::Freeform(tool) => tool.name.as_str(),
        }
    }

    pub fn tool_name(&self) -> ToolName {
        ToolName::plain(self.name())
    }
}

impl From<FunctionToolSpec> for ExecutableToolSpec {
    fn from(value: FunctionToolSpec) -> Self {
        Self::Function(value)
    }
}

impl From<FreeformToolSpec> for ExecutableToolSpec {
    fn from(value: FreeformToolSpec) -> Self {
        Self::Freeform(value)
    }
}
