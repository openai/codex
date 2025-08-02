use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::client_common::Prompt;
use crate::plan_tool::PLAN_TOOL;
use crate::protocol::SandboxPolicy;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResponsesApiTool {
    pub(crate) name: &'static str,
    pub(crate) description: String,
    pub(crate) strict: bool,
    pub(crate) parameters: JsonSchema,
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
    Boolean {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    String {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    Number {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
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
            items: Box::new(JsonSchema::String { description: None }),
        },
    );
    properties.insert(
        "workdir".to_string(),
        JsonSchema::String { description: None },
    );
    properties.insert(
        "timeout".to_string(),
        JsonSchema::Number { description: None },
    );

    vec![OpenAiTool::Function(ResponsesApiTool {
        name: "shell",
        description: "Runs a shell command, and returns its output.".to_string(),
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
/// TODO: we're starting to get more complex with our tool config, let's
pub(crate) fn create_tools_json_for_responses_api(
    prompt: &Prompt,
    model: &str,
    include_plan_tool: bool,
    sandbox_policy: Option<SandboxPolicy>,
) -> crate::error::Result<Vec<serde_json::Value>> {
    // Assemble tool list: built-in tools + any extra tools from the prompt.
    let default_tools = if let Some(sandbox_policy) = sandbox_policy {
        // if sandbox_policy is provided, create the shell tool
        &vec![create_shell_tool_for_sandbox(sandbox_policy)]
    } else if model.contains("codex") {
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

    if include_plan_tool {
        tools_json.push(serde_json::to_value(PLAN_TOOL.clone())?);
    }

    Ok(tools_json)
}

fn create_shell_tool_for_sandbox(sandbox_policy: SandboxPolicy) -> OpenAiTool {
    let mut properties = BTreeMap::new();
    properties.insert(
        "command".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::String {
                description: Some("The command to execute".to_string()),
            }),
        },
    );
    properties.insert(
        "workdir".to_string(),
        JsonSchema::String {
            description: Some("The working directory to execute the command in".to_string()),
        },
    );
    properties.insert(
        "timeout".to_string(),
        JsonSchema::Number {
            description: Some("The timeout for the command in milliseconds".to_string()),
        },
    );

    if sandbox_policy != SandboxPolicy::DangerFullAccess {
        properties.insert(
        "with_escalated_permissions".to_string(),
        JsonSchema::Boolean {
            description: Some("Whether to request escalated permissions. Set to true if command needs to be run without sandbox restrictions".to_string()),
        },
    );
        properties.insert(
        "justification".to_string(),
        JsonSchema::String {
            description: Some("Only set if ask_for_escalated_permissions is true. 1-sentence explanation of why we want to run this command.".to_string()),
        },
    );
    }

    let description = match sandbox_policy {
        SandboxPolicy::WorkspaceWrite {
            writable_roots: _,
            network_access,
        } => {
            format!(
                r#"
The shell tool is used to execute shell commands.

- When invoking the shell tool, your call will be running in a landlock sandbox, and some shell commands will require escalated privileges:
  - Types of actions that require escalated privileges:
    - Reading files outside the current directory
    - Writing files outside the current directory, and protected folders like .git or .env{}
  - Examples of commands that require escalated privileges:
    - git commit
    - npm install or pnpm install
    - cargo build
    - cargo test
- When invoking a command that will require escalated privileges:
  - Provide the with_escalated_permissions parameter with the boolean value true
  - Include a short, 1 sentence explanation for why we need to run with_escalated_permissions in the justification parameter."#,
                if !network_access {
                    "\n  - Commands that require network access\n"
                } else {
                    ""
                }
            )
        }
        SandboxPolicy::DangerFullAccess => {
            "Runs a shell command, and returns its output.".to_string()
        }
        SandboxPolicy::ReadOnly => {
            r#"
The shell tool is used to execute shell commands.
IMPORTANT: If you are running the apply_patch command, you will need to provide the with_escalated_permissions parameter with the boolean value true.

- When invoking the shell tool, your call will be running in a landlock sandbox, and some shell commands (including apply_patch) will require escalated permissions:
  - Types of actions that require escalated privileges:
    - Reading files outside the current directory
    - Writing files
    - Applying patches
  - Examples of commands that require escalated privileges:
    - apply_patch
    - git commit
    - npm install or pnpm install
    - cargo build
    - cargo test
- When invoking a command that will require escalated privileges:
  - Provide the with_escalated_permissions parameter with the boolean value true
  - Include a short, 1 sentence explanation for why we need to run with_escalated_permissions in the justification parameter"#.to_string()
        }
    };

    OpenAiTool::Function(ResponsesApiTool {
        name: "shell",
        description,
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: &["command"],
            additional_properties: false,
        },
    })
}

/// Returns JSON values that are compatible with Function Calling in the
/// Chat Completions API:
/// https://platform.openai.com/docs/guides/function-calling?api-mode=chat
pub(crate) fn create_tools_json_for_chat_completions_api(
    prompt: &Prompt,
    model: &str,
    include_plan_tool: bool,
    sandbox_policy: Option<SandboxPolicy>,
) -> crate::error::Result<Vec<serde_json::Value>> {
    // We start with the JSON for the Responses API and than rewrite it to match
    // the chat completions tool call format.
    let responses_api_tools_json =
        create_tools_json_for_responses_api(prompt, model, include_plan_tool, sandbox_policy)?;
    let tools_json = responses_api_tools_json
        .into_iter()
        .filter_map(|mut tool| {
            if tool.get("type") != Some(&serde_json::Value::String("function".to_string())) {
                return None;
            }

            if let Some(map) = tool.as_object_mut() {
                // Remove "type" field as it is not needed in chat completions.
                map.remove("type");
                Some(json!({
                    "type": "function",
                    "function": map,
                }))
            } else {
                None
            }
        })
        .collect::<Vec<serde_json::Value>>();
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
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_create_tools_json_for_responses_api() {
        let prompt = Prompt {
            ..Default::default()
        };
        let model = "gpt-4o-mini";
        let include_plan_tool = true;
        let sandbox_policy = None;

        let tools_json =
            create_tools_json_for_responses_api(&prompt, model, include_plan_tool, sandbox_policy)
                .unwrap();
        assert_eq!(tools_json[0]["name"], "shell");
        let properties = tools_json[0]["parameters"]["properties"]
            .as_object()
            .unwrap();
        assert!(!properties.contains_key("with_escalated_permissions"));
        assert!(!properties.contains_key("justification"));
    }

    #[test]
    fn test_create_tools_json_for_responses_api_with_sandbox() {
        let prompt = Prompt {
            ..Default::default()
        };
        let model = "codex-mini-latest";
        let include_plan_tool = true;
        let sandbox_policy = Some(SandboxPolicy::ReadOnly);

        let tools_json =
            create_tools_json_for_responses_api(&prompt, model, include_plan_tool, sandbox_policy)
                .unwrap();
        assert_eq!(tools_json[0]["name"], "shell");
        let properties = tools_json[0]["parameters"]["properties"]
            .as_object()
            .unwrap();
        assert!(properties.contains_key("with_escalated_permissions"));
        assert!(properties.contains_key("justification"));
    }
}
