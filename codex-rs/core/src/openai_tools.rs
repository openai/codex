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

static PLAN_TOOL: LazyLock<OpenAiTool> = LazyLock::new(|| {
    let mut plan_item_props = BTreeMap::new();
    plan_item_props.insert("step".to_string(), JsonSchema::String);
    plan_item_props.insert("status".to_string(), JsonSchema::String);

    let plan_items_schema = JsonSchema::Array {
        items: Box::new(JsonSchema::Object {
            properties: plan_item_props,
            required: &["step", "status"],
            additional_properties: false,
        }),
    };

    let mut properties = BTreeMap::new();
    properties.insert("explanation".to_string(), JsonSchema::String);
    properties.insert("plan".to_string(), plan_items_schema);

    OpenAiTool::Function(ResponsesApiTool {
        name: "update_plan",
        description: r#"Use the update_plan tool to keep the user updated on the current plan for the task.
After understanding the user's task, call the update_plan tool with an initial plan. An example of a plan:
1. Explore the codebase to find relevant files (status: in_progress)
2. Implement the feature in the XYZ component (status: pending)
3. Commit changes and make a pull request (status: pending)
Each step should be a short, 1-sentence description.
Until all the steps are finished, there should always be exactly one in_progress step in the plan.
Call the update_plan tool whenever you finish a step, marking the completed step as `completed` and marking the next step as `in_progress`.
Before running a command, consider whether or not you have completed the previous step, and make sure to mark it as completed before moving on to the next step.
Sometimes, you may need to change plans in the middle of a task: call `update_plan` with the updated plan and make sure to provide an `explanation` of the rationale when doing so.
When all steps are completed, call update_plan one last time with all steps marked as `completed`."#,
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: &["plan"],
            additional_properties: false,
        },
    })
});

static DEFAULT_CODEX_MODEL_TOOLS: LazyLock<Vec<OpenAiTool>> =
    LazyLock::new(|| vec![OpenAiTool::LocalShell {}]);

/// Returns JSON values that are compatible with Function Calling in the
/// Responses API:
/// https://platform.openai.com/docs/guides/function-calling?api-mode=responses
pub(crate) fn create_tools_json_for_responses_api(
    prompt: &Prompt,
    model: &str,
    include_plan_tool: bool,
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

    if include_plan_tool {
        tools_json.push(serde_json::to_value(PLAN_TOOL.clone())?);
    }

    Ok(tools_json)
}

/// Returns JSON values that are compatible with Function Calling in the
/// Chat Completions API:
/// https://platform.openai.com/docs/guides/function-calling?api-mode=chat
pub(crate) fn create_tools_json_for_chat_completions_api(
    prompt: &Prompt,
    model: &str,
    include_plan_tool: bool,
) -> crate::error::Result<Vec<serde_json::Value>> {
    // We start with the JSON for the Responses API and than rewrite it to match
    // the chat completions tool call format.
    let responses_api_tools_json =
        create_tools_json_for_responses_api(prompt, model, include_plan_tool)?;
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
