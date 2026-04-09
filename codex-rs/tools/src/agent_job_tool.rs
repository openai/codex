use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolOrigin;
use crate::ToolSpec;
use std::collections::BTreeMap;

pub fn create_spawn_agents_on_csv_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "csv_path".to_string(),
            JsonSchema::string(Some("Path to the CSV file containing input rows.".to_string())),
        ),
        (
            "instruction".to_string(),
            JsonSchema::string(Some(
                "Instruction template to apply to each CSV row. Use {column_name} placeholders to inject values from the row."
                    .to_string(),
            )),
        ),
        (
            "id_column".to_string(),
            JsonSchema::string(Some(
                "Optional column name to use as stable item id.".to_string(),
            )),
        ),
        (
            "output_csv_path".to_string(),
            JsonSchema::string(Some("Optional output CSV path for exported results.".to_string())),
        ),
        (
            "max_concurrency".to_string(),
            JsonSchema::number(Some(
                "Maximum concurrent workers for this job. Defaults to 16 and is capped by config."
                    .to_string(),
            )),
        ),
        (
            "max_workers".to_string(),
            JsonSchema::number(Some(
                "Alias for max_concurrency. Set to 1 to run sequentially.".to_string(),
            )),
        ),
        (
            "max_runtime_seconds".to_string(),
            JsonSchema::number(Some(
                "Maximum runtime per worker before it is failed. Defaults to 1800 seconds."
                    .to_string(),
            )),
        ),
        (
            "output_schema".to_string(),
            JsonSchema::object(BTreeMap::new(), /*required*/ None, /*additional_properties*/ None),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "spawn_agents_on_csv".to_string(),
        description: "Process a CSV by spawning one worker sub-agent per row. The instruction string is a template where `{column}` placeholders are replaced with row values. Each worker must call `report_agent_job_result` with a JSON object (matching `output_schema` when provided); missing reports are treated as failures. This call blocks until all rows finish and automatically exports results to `output_csv_path` (or a default path)."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, Some(vec!["csv_path".to_string(), "instruction".to_string()]), Some(false.into())),
        output_schema: None,
        origin: ToolOrigin::Native,
    })
}

pub fn create_report_agent_job_result_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "job_id".to_string(),
            JsonSchema::string(Some("Identifier of the job.".to_string())),
        ),
        (
            "item_id".to_string(),
            JsonSchema::string(Some("Identifier of the job item.".to_string())),
        ),
        (
            "result".to_string(),
            JsonSchema::object(BTreeMap::new(), /*required*/ None, /*additional_properties*/ None),
        ),
        (
            "stop".to_string(),
            JsonSchema::boolean(Some(
                "Optional. When true, cancels the remaining job items after this result is recorded."
                    .to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "report_agent_job_result".to_string(),
        description:
            "Worker-only tool to report a result for an agent job item. Main agents should not call this."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, Some(vec![
                "job_id".to_string(),
                "item_id".to_string(),
                "result".to_string(),
            ]), Some(false.into())),
        output_schema: None,
        origin: ToolOrigin::Native,
    })
}

#[cfg(test)]
#[path = "agent_job_tool_tests.rs"]
mod tests;
