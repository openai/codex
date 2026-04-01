//! Responses API tool specs for thread-local runtime job management.
//!
//! These specs expose the `JobCreate`, `JobDelete`, and `JobList` built-in
//! tools so models can create, inspect, and delete jobs on the current thread.

use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use std::collections::BTreeMap;

pub fn create_job_create_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "cron_expression".to_string(),
            JsonSchema::String {
                description: Some(
                    "Scheduler expression for the job. Supported values are scheduler-specific."
                        .to_string(),
                ),
            },
        ),
        (
            "prompt".to_string(),
            JsonSchema::String {
                description: Some("Prompt to execute when the job fires.".to_string()),
            },
        ),
        (
            "run_once".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "Optional. When true, delete the job after its next execution is claimed."
                        .to_string(),
                ),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "JobCreate".to_string(),
        description:
            "Create a runtime-only thread job using a structured scheduler expression and prompt."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["cron_expression".to_string(), "prompt".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

pub fn create_job_delete_tool() -> ToolSpec {
    let properties = BTreeMap::from([(
        "id".to_string(),
        JsonSchema::String {
            description: Some("Identifier of the job to delete.".to_string()),
        },
    )]);

    ToolSpec::Function(ResponsesApiTool {
        name: "JobDelete".to_string(),
        description: "Delete a runtime-only thread job by id.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["id".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

pub fn create_job_list_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: "JobList".to_string(),
        description: "List runtime-only thread jobs for the current thread.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties: BTreeMap::new(),
            required: None,
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

#[cfg(test)]
#[path = "job_tool_tests.rs"]
mod tests;
