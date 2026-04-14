use crate::HarnessError;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

const BASE_INSTRUCTIONS: &str = include_str!("../../core/prompt.md");
const BROWSER_TOOLING_NOTE: &str = concat!(
    "Browser prototype note:\n",
    "- Only one host tool is available: `exec_js`.\n",
    "- `exec_js` runs JavaScript inside a browser-managed sandbox and returns stdout-like text.\n",
    "- Native shell, filesystem, MCP, and plugin tools are unavailable in this prototype."
);

pub const EXEC_JS_TOOL_NAME: &str = "exec_js";

#[derive(Clone, Debug, Serialize)]
pub struct ResponsesRequest {
    pub model: String,
    pub instructions: String,
    pub input: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ResponsesTool>>,
    pub parallel_tool_calls: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ResponsesTool {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl ResponsesTool {
    #[must_use]
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
    ) -> Self {
        Self {
            kind: "function".to_string(),
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }

    #[must_use]
    pub fn exec_js() -> Self {
        Self::function(
            EXEC_JS_TOOL_NAME,
            "Execute JavaScript inside the browser sandbox and return the textual result.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "JavaScript source code to execute in the browser sandbox."
                    }
                },
                "required": ["code"],
                "additionalProperties": false
            }),
        )
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HarnessEvent {
    TurnStarted {
        turn_id: String,
        model_context_window: Option<i64>,
        collaboration_mode_kind: String,
    },
    UserMessage {
        turn_id: String,
        message: String,
    },
    AgentMessageDelta {
        turn_id: String,
        delta: String,
    },
    AgentMessage {
        turn_id: String,
        message: String,
    },
    ToolCallStarted {
        turn_id: String,
        response_id: String,
        call_id: String,
        name: String,
        arguments: String,
    },
    ToolCallCompleted {
        turn_id: String,
        response_id: String,
        call_id: String,
        name: String,
        output: String,
    },
    TurnError {
        turn_id: String,
        message: String,
    },
    TurnComplete {
        turn_id: String,
        last_agent_message: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize)]
pub struct ResponsesResponse {
    pub id: Option<String>,
    pub output_text: Option<String>,
    pub output: Option<Vec<ResponsesOutputItem>>,
    pub error: Option<ResponsesError>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ResponsesError {
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ResponsesOutputItem {
    #[serde(rename = "type")]
    pub kind: String,
    pub call_id: Option<String>,
    pub name: Option<String>,
    pub arguments: Option<String>,
    pub content: Option<Vec<ResponsesContentItem>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ResponsesContentItem {
    pub text: Option<String>,
    pub output_text: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResponsesFunctionCall {
    pub call_id: String,
    pub name: String,
    pub arguments: String,
}

impl ResponsesResponse {
    #[must_use]
    pub fn response_text(&self) -> String {
        if let Some(output_text) = &self.output_text
            && !output_text.is_empty()
        {
            return output_text.clone();
        }

        let mut chunks = Vec::new();
        for item in self.output.as_ref().into_iter().flatten() {
            for content in item.content.as_ref().into_iter().flatten() {
                if let Some(text) = &content.text {
                    chunks.push(text.clone());
                } else if let Some(output_text) = &content.output_text {
                    chunks.push(output_text.clone());
                }
            }
        }

        chunks.join("\n")
    }

    pub fn function_calls(&self) -> Result<Vec<ResponsesFunctionCall>, HarnessError> {
        let mut function_calls = Vec::new();
        for item in self.output.as_ref().into_iter().flatten() {
            if item.kind != "function_call" {
                continue;
            }
            let call_id = item.call_id.clone().ok_or_else(|| {
                HarnessError::new("Responses API function_call item omitted call_id")
            })?;
            let name = item.name.clone().ok_or_else(|| {
                HarnessError::new("Responses API function_call item omitted name")
            })?;
            let arguments = item.arguments.clone().ok_or_else(|| {
                HarnessError::new("Responses API function_call item omitted arguments")
            })?;
            function_calls.push(ResponsesFunctionCall {
                call_id,
                name,
                arguments,
            });
        }
        Ok(function_calls)
    }
}

#[must_use]
pub fn tool_output_item(call_id: &str, output: String) -> Value {
    serde_json::json!({
        "type": "function_call_output",
        "call_id": call_id,
        "output": output,
    })
}

#[must_use]
pub fn build_browser_instructions() -> String {
    format!("{BASE_INSTRUCTIONS}\n\n{BROWSER_TOOLING_NOTE}")
}

#[cfg(test)]
mod tests {
    use super::ResponsesResponse;
    use super::ResponsesTool;
    use super::build_browser_instructions;
    use pretty_assertions::assert_eq;

    #[test]
    fn browser_instructions_append_browser_note() {
        let instructions = build_browser_instructions();
        assert!(instructions.contains("Only one host tool is available: `exec_js`"));
        assert!(instructions.contains("You are a coding agent running in the Codex CLI"));
    }

    #[test]
    fn extracts_output_text_when_present() {
        let response: ResponsesResponse = serde_json::from_str(
            r#"{
                "id": "resp_123",
                "output_text": "final answer",
                "output": []
            }"#,
        )
        .expect("response should deserialize");
        assert_eq!(response.response_text(), "final answer");
    }

    #[test]
    fn extracts_function_calls_from_output_items() {
        let response: ResponsesResponse = serde_json::from_str(
            r#"{
                "id": "resp_123",
                "output": [
                    {
                        "type": "function_call",
                        "call_id": "call_123",
                        "name": "exec_js",
                        "arguments": "{\"code\":\"console.log('hi')\"}"
                    }
                ]
            }"#,
        )
        .expect("response should deserialize");

        let function_calls = response
            .function_calls()
            .expect("function calls should parse");
        assert_eq!(function_calls.len(), 1);
        assert_eq!(function_calls[0].call_id, "call_123");
        assert_eq!(function_calls[0].name, "exec_js");
        assert_eq!(
            function_calls[0].arguments,
            r#"{"code":"console.log('hi')"}"#
        );
    }

    #[test]
    fn exec_tool_schema_is_function_tool() {
        let tool = ResponsesTool::exec_js();
        assert_eq!(tool.kind, "function");
        assert_eq!(tool.name, "exec_js");
    }
}
