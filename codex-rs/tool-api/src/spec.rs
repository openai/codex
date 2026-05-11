use serde_json::Value;

/// Model-visible definition for one contributed function tool.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionToolSpec {
    pub name: String,
    pub description: String,
    pub strict: bool,
    pub parameters: Value,
}
