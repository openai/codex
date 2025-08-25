use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::openai_tools::JsonSchema;
use crate::openai_tools::OpenAiTool;
use crate::openai_tools::ResponsesApiTool;

pub(crate) static SUBAGENT_TOOL: LazyLock<OpenAiTool> = LazyLock::new(|| {
    let mut properties = BTreeMap::new();
    properties.insert(
        "name".to_string(),
        JsonSchema::String {
            description: Some("Registered subagent name".to_string()),
        },
    );
    properties.insert(
        "input".to_string(),
        JsonSchema::String {
            description: Some("Task or instruction for the subagent".to_string()),
        },
    );
    properties.insert(
        "context".to_string(),
        JsonSchema::String {
            description: Some("Optional extra context to aid the task".to_string()),
        },
    );

    OpenAiTool::Function(ResponsesApiTool {
        name: "subagent_run".to_string(),
        description: "Invoke a named subagent with isolated context and return its result"
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["name".to_string(), "input".to_string()]),
            additional_properties: Some(false),
        },
    })
});

pub(crate) static SUBAGENT_LIST_TOOL: LazyLock<OpenAiTool> = LazyLock::new(|| {
    let properties = BTreeMap::new();
    OpenAiTool::Function(ResponsesApiTool {
        name: "subagent_list".to_string(),
        description:
            "List available subagents (name and description). Call before subagent_run if unsure."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: None,
            additional_properties: Some(false),
        },
    })
});
