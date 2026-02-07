//! Tool/function definitions and calls.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

/// Definition of a tool that can be called by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Name of the tool.
    pub name: String,
    /// Description of what the tool does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for the tool's parameters.
    pub parameters: Value,
    /// Custom tool format (OpenAI-only). When set, sent as `type: "custom"` tool.
    /// Non-OpenAI providers ignore this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_format: Option<Value>,
}

impl ToolDefinition {
    /// Create a new tool definition.
    pub fn new(name: impl Into<String>, parameters: Value) -> Self {
        Self {
            name: name.into(),
            description: None,
            parameters,
            custom_format: None,
        }
    }

    /// Set the tool description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Create a tool definition with all fields.
    pub fn full(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: Some(description.into()),
            parameters,
            custom_format: None,
        }
    }

    /// Create a custom tool definition (OpenAI-only).
    ///
    /// The `custom_format` value is sent as the `format` field of an OpenAI
    /// `type: "custom"` tool. Non-OpenAI providers silently skip custom tools.
    pub fn custom(
        name: impl Into<String>,
        description: impl Into<String>,
        custom_format: Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: Some(description.into()),
            parameters: Value::Null,
            custom_format: Some(custom_format),
        }
    }
}

/// How the model should choose which tool to call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    /// Model decides whether to call tools.
    Auto,
    /// Model must call a tool.
    Required,
    /// Model must not call any tools.
    None,
    /// Model must call a specific tool.
    Tool {
        /// Name of the tool to call.
        name: String,
    },
}

impl Default for ToolChoice {
    fn default() -> Self {
        ToolChoice::Auto
    }
}

/// A tool call made by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique ID for this tool call.
    pub id: String,
    /// Name of the tool being called.
    pub name: String,
    /// Arguments as JSON.
    pub arguments: Value,
}

impl ToolCall {
    /// Create a new tool call.
    pub fn new(id: impl Into<String>, name: impl Into<String>, arguments: Value) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }

    /// Get a reference to the arguments as a JSON value.
    pub fn arguments(&self) -> &Value {
        &self.arguments
    }

    /// Parse the arguments as a specific type.
    pub fn parse_arguments<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.arguments.clone())
    }
}

/// Content for a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    /// Plain text result.
    Text(String),
    /// Structured JSON result.
    Json(Value),
    /// Multiple content blocks (for complex results).
    Blocks(Vec<ToolResultBlock>),
}

impl ToolResultContent {
    /// Create a text result.
    pub fn text(text: impl Into<String>) -> Self {
        ToolResultContent::Text(text.into())
    }

    /// Create a JSON result.
    pub fn json(value: Value) -> Self {
        ToolResultContent::Json(value)
    }

    /// Get as text if this is a text result.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ToolResultContent::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Convert to a text string, handling all content types.
    ///
    /// - `Text`: returns the string directly
    /// - `Json`: serializes to JSON string
    /// - `Blocks`: concatenates all text blocks, ignoring images
    pub fn to_text(&self) -> String {
        match self {
            ToolResultContent::Text(s) => s.clone(),
            ToolResultContent::Json(v) => v.to_string(),
            ToolResultContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ToolResultBlock::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(""),
        }
    }
}

/// A block within a tool result (for complex results).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultBlock {
    /// Text content.
    Text {
        /// The text content.
        text: String,
    },
    /// Image content.
    Image {
        /// Base64-encoded image data.
        data: String,
        /// MIME type.
        media_type: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let tool = ToolDefinition::full(
            "get_weather",
            "Get the current weather for a location",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "City name"
                    }
                },
                "required": ["location"]
            }),
        );

        assert_eq!(tool.name, "get_weather");
        assert!(tool.description.is_some());
    }

    #[test]
    fn test_tool_call() {
        let call = ToolCall::new(
            "call_123",
            "get_weather",
            serde_json::json!({"location": "New York"}),
        );

        assert_eq!(call.id, "call_123");
        assert_eq!(call.name, "get_weather");

        #[derive(Deserialize)]
        struct Args {
            location: String,
        }

        let args: Args = call.parse_arguments().unwrap();
        assert_eq!(args.location, "New York");
    }

    #[test]
    fn test_tool_choice_serde() {
        let choice = ToolChoice::Tool {
            name: "get_weather".to_string(),
        };
        let json = serde_json::to_string(&choice).unwrap();
        assert!(json.contains("\"type\":\"tool\""));
        assert!(json.contains("\"name\":\"get_weather\""));
    }

    #[test]
    fn test_tool_result_content() {
        let text = ToolResultContent::text("Success!");
        assert_eq!(text.as_text(), Some("Success!"));

        let json = ToolResultContent::json(serde_json::json!({"status": "ok"}));
        assert!(json.as_text().is_none());
    }

    #[test]
    fn test_tool_result_content_to_text() {
        // Text variant returns the string directly
        let text = ToolResultContent::Text("Hello, world!".to_string());
        assert_eq!(text.to_text(), "Hello, world!");

        // Json variant serializes to JSON string
        let json = ToolResultContent::Json(serde_json::json!({"key": "value"}));
        assert_eq!(json.to_text(), r#"{"key":"value"}"#);

        // Blocks variant concatenates text blocks, ignoring images
        let blocks = ToolResultContent::Blocks(vec![
            ToolResultBlock::Text {
                text: "First ".to_string(),
            },
            ToolResultBlock::Image {
                data: "base64data".to_string(),
                media_type: "image/png".to_string(),
            },
            ToolResultBlock::Text {
                text: "Second".to_string(),
            },
        ]);
        assert_eq!(blocks.to_text(), "First Second");

        // Empty blocks returns empty string
        let empty_blocks = ToolResultContent::Blocks(vec![]);
        assert_eq!(empty_blocks.to_text(), "");
    }
}
