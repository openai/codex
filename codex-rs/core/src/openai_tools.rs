use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::client_common::Prompt;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResponsesApiTool {
    name: &'static str,
    description: &'static str,
    strict: bool,
    parameters: JsonSchema,
}

/// When serialized as JSON, this produces a valid "Tool" in the OpenAI
/// Responses API.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub(crate) enum OpenAiTool {
    #[serde(rename = "function")]
    Function(ResponsesApiTool),
    #[serde(rename = "local_shell")]
    LocalShell {},
}

/// Generic JSON‑Schema subset needed for our tool definitions
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(crate) enum JsonSchema {
    String,
    Number,
    Array {
        items: Box<JsonSchema>,
    },
    Object {
        properties: BTreeMap<String, JsonSchema>,
        required: &'static [&'static str],
        #[serde(rename = "additionalProperties")]
        additional_properties: bool,
    },
}

/// Tool usage specification
static DEFAULT_TOOLS: LazyLock<Vec<OpenAiTool>> = LazyLock::new(|| {
    let mut properties = BTreeMap::new();
    properties.insert(
        "command".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::String),
        },
    );
    properties.insert("workdir".to_string(), JsonSchema::String);
    properties.insert("timeout".to_string(), JsonSchema::Number);

    vec![OpenAiTool::Function(ResponsesApiTool {
        name: "shell",
        description: "Runs a shell command, and returns its output.",
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: &["command"],
            additional_properties: false,
        },
    })]
});

static DEFAULT_CODEX_MODEL_TOOLS: LazyLock<Vec<OpenAiTool>> =
    LazyLock::new(|| vec![OpenAiTool::LocalShell {}]);

/// Returns JSON values that are compatible with Function Calling in the
/// Responses API:
/// https://platform.openai.com/docs/guides/function-calling?api-mode=responses
pub(crate) fn create_tools_json_for_responses_api(
    prompt: &Prompt,
    model: &str,
) -> crate::error::Result<Vec<serde_json::Value>> {
    // Assemble tool list: built-in tools + any extra tools from the prompt.
    let default_tools = if model.starts_with("codex") {
        &DEFAULT_CODEX_MODEL_TOOLS
    } else {
        &DEFAULT_TOOLS
    };
    let mut tools_json = Vec::with_capacity(default_tools.len() + prompt.extra_tools.len());
    for t in default_tools.iter() {
        tools_json.push(serde_json::to_value(t)?);
    }
    tools_json.extend(
        prompt
            .extra_tools
            .clone()
            .into_iter()
            .map(|(name, tool)| mcp_tool_to_openai_tool(name, tool)),
    );

    Ok(tools_json)
}

/// Returns JSON values that are compatible with Function Calling in the
/// Chat Completions API:
/// https://platform.openai.com/docs/guides/function-calling?api-mode=chat
pub(crate) fn create_tools_json_for_chat_completions_api(
    prompt: &Prompt,
    _model: &str,
) -> crate::error::Result<Vec<serde_json::Value>> {
    // For Chat Completions API, always use DEFAULT_TOOLS (shell function) regardless of model name
    // This ensures non-OpenAI providers get tools even with default "codex-mini-latest" model
    let mut tools_json = Vec::with_capacity(DEFAULT_TOOLS.len() + prompt.extra_tools.len());
    for t in DEFAULT_TOOLS.iter() {
        if let Ok(tool_value) = serde_json::to_value(t) {
            if tool_value.get("type") == Some(&serde_json::Value::String("function".to_string())) {
                if let Some(map) = tool_value.as_object() {
                    let mut function_map = map.clone();
                    function_map.remove("type");
                    tools_json.push(json!({
                        "type": "function",
                        "function": function_map,
                    }));
                }
            }
        }
    }
    
    // Add MCP tools
    tools_json.extend(
        prompt
            .extra_tools
            .clone()
            .into_iter()
            .map(|(name, tool)| mcp_tool_to_openai_tool(name, tool)),
    );

    Ok(tools_json)
}

fn mcp_tool_to_openai_tool(
    fully_qualified_name: String,
    tool: mcp_types::Tool,
) -> serde_json::Value {
    let mcp_types::Tool {
        description,
        mut input_schema,
        ..
    } = tool;

    // OpenAI models mandate the "properties" field in the schema. The Agents
    // SDK fixed this by inserting an empty object for "properties" if it is not
    // already present https://github.com/openai/openai-agents-python/issues/449
    // so here we do the same.
    if input_schema.properties.is_none() {
        input_schema.properties = Some(serde_json::Value::Object(serde_json::Map::new()));
    }

    // TODO(mbolin): Change the contract of this function to return
    // ResponsesApiTool.
    json!({
        "name": fully_qualified_name,
        "description": description,
        "parameters": input_schema,
        "type": "function",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client_common::Prompt;
    use std::collections::HashMap;

    #[test]
    fn test_chat_completions_api_always_gets_shell_tools() {
        let prompt = Prompt {
            input: vec![],
            prev_id: None,
            user_instructions: None,
            store: false,
            extra_tools: HashMap::new(),
        };

        // Test with default "codex-mini-latest" model (the problematic case)
        let tools = create_tools_json_for_chat_completions_api(&prompt, "codex-mini-latest")
            .expect("Should create tools successfully");
        
        assert!(!tools.is_empty(), "Chat Completions API should always get tools");
        
        // Verify we have the shell tool
        let has_shell_tool = tools.iter().any(|tool| {
            tool.get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                == Some("shell")
        });
        assert!(has_shell_tool, "Should have shell tool for Chat Completions API");
    }

    #[test]
    fn test_chat_completions_api_works_with_non_codex_models() {
        let prompt = Prompt {
            input: vec![],
            prev_id: None,
            user_instructions: None,
            store: false,
            extra_tools: HashMap::new(),
        };

        // Test with non-codex model names
        for model in ["qwen2", "llama3", "mistral-7b", "gpt-4"] {
            let tools = create_tools_json_for_chat_completions_api(&prompt, model)
                .expect(&format!("Should create tools for model {}", model));
            
            assert!(!tools.is_empty(), "Model {} should get tools", model);
            
            // Verify we have the shell tool
            let has_shell_tool = tools.iter().any(|tool| {
                tool.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    == Some("shell")
            });
            assert!(has_shell_tool, "Model {} should have shell tool", model);
        }
    }

    #[test]
    fn test_responses_api_still_respects_model_name() {
        let prompt = Prompt {
            input: vec![],
            prev_id: None,
            user_instructions: None,
            store: false,
            extra_tools: HashMap::new(),
        };

        // Test that Responses API still uses model-based logic
        let codex_tools = create_tools_json_for_responses_api(&prompt, "codex-mini-latest")
            .expect("Should create tools for codex model");
        let non_codex_tools = create_tools_json_for_responses_api(&prompt, "qwen2")
            .expect("Should create tools for non-codex model");
        
        // Both should have tools, but potentially different ones
        assert!(!codex_tools.is_empty(), "Codex model should get tools");
        assert!(!non_codex_tools.is_empty(), "Non-codex model should get tools");
    }
}
