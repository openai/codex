use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub fn create_spawn_agents_on_csv_tool() -> ToolSpec {
    let mut output_schema = JsonSchema::object(
        BTreeMap::new(),
        /*required*/ None,
        /*additional_properties*/ None,
    );
    output_schema.description =
        Some("JSON Schema for each worker result. Omit to accept any result object.".to_string());

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
                "CSV column to use as stable item id. Omit to use row numbers.".to_string(),
            )),
        ),
        (
            "output_csv_path".to_string(),
            JsonSchema::string(Some(
                "Output CSV path for exported results. Omit to create one next to the input CSV."
                    .to_string(),
            )),
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
                "Alias for max_concurrency. Defaults to 16 and is capped by config.".to_string(),
            )),
        ),
        (
            "max_runtime_seconds".to_string(),
            JsonSchema::number(Some(
                "Maximum runtime per worker before failure. Defaults to 1800 seconds; config may set a different default."
                    .to_string(),
            )),
        ),
        ("output_schema".to_string(), output_schema),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "spawn_agents_on_csv".to_string(),
        description: "Process a CSV by spawning one worker sub-agent per row. The instruction string is a template where `{column}` placeholders are replaced with row values. Each worker must call `report_agent_job_result` with a JSON object (matching `output_schema` when provided); missing reports are treated as failures. This call blocks until all rows finish and automatically exports results to `output_csv_path` (or a default path)."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, Some(vec!["csv_path".to_string(), "instruction".to_string()]), Some(false.into())),
        output_schema: None,
    })
}

pub fn create_report_agent_job_result_tool() -> ToolSpec {
    let mut result_schema = JsonSchema::object(
        BTreeMap::new(),
        /*required*/ None,
        /*additional_properties*/ None,
    );
    result_schema.description = Some("Result object for this job item.".to_string());

    let properties = BTreeMap::from([
        (
            "job_id".to_string(),
            JsonSchema::string(Some("Identifier of the job.".to_string())),
        ),
        (
            "item_id".to_string(),
            JsonSchema::string(Some("Identifier of the job item.".to_string())),
        ),
        ("result".to_string(), result_schema),
        (
            "stop".to_string(),
            JsonSchema::boolean(Some(
                "True cancels remaining job items after this result is recorded; false or omitted continues the job."
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
    })
}

#[cfg(test)]
#[path = "agent_jobs_spec_tests.rs"]
mod tests;
